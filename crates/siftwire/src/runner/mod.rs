mod brief;
mod delivery;
mod format;
mod runs_cli;
mod source_cli;

use std::env;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use anyhow::{Result, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::contract::{BriefRequest, BriefResult, ConfigRequest, ConfigResult, Paths};
use crate::paths;
use crate::storage::{RUNTIME_CONFIG_MAX_DELIVERY_ITEMS, Store};

const EXIT_OK: ExitCode = ExitCode::SUCCESS;
const EXIT_FAILURE: ExitCode = ExitCode::FAILURE;
const EXIT_USAGE_CODE: u8 = 2;

pub fn run_process() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        usage(false);
        return ExitCode::from(EXIT_USAGE_CODE);
    };
    let rest: Vec<String> = arguments.collect();
    match command.as_str() {
        "help" | "-h" | "--help" => {
            usage(true);
            EXIT_OK
        }
        "version" | "--version" => {
            println!("siftwire v{}", env!("CARGO_PKG_VERSION"));
            EXIT_OK
        }
        "config" => run_config_process(&rest),
        "brief" => run_brief_process(&rest),
        "source" => source_cli::run(&rest),
        "runs" => runs_cli::run(&rest),
        _ => {
            eprintln!("unknown siftwire command {command:?}");
            usage(false);
            ExitCode::from(EXIT_USAGE_CODE)
        }
    }
}

fn emit_json<T: serde::Serialize>(value: &T) -> ExitCode {
    match crate::runner::format::write_json(value) {
        Ok(()) => EXIT_OK,
        Err(error) => {
            eprintln!("{error:#}");
            EXIT_FAILURE
        }
    }
}

fn run_config_process(arguments: &[String]) -> ExitCode {
    let database = match parse_database_argument(arguments) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_USAGE_CODE);
        }
    };
    let request = match decode_request::<ConfigRequest>() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("decode config request: {error}");
            return EXIT_FAILURE;
        }
    };
    match open_store(database.as_deref())
        .and_then(|(paths, store)| run_config_action(paths, &store, request))
    {
        Ok(result) => write_result(&result, "config"),
        Err(error) => {
            eprintln!("run config task: {error:#}");
            EXIT_FAILURE
        }
    }
}

fn run_brief_process(arguments: &[String]) -> ExitCode {
    let database = match parse_database_argument(arguments) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_USAGE_CODE);
        }
    };
    let request = match decode_request::<BriefRequest>() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("decode brief request: {error}");
            return EXIT_FAILURE;
        }
    };
    match open_store(database.as_deref())
        .and_then(|(paths, store)| run_brief_action(paths, &store, &request))
    {
        Ok(result) => write_result(&result, "brief"),
        Err(error) => {
            eprintln!("run brief task: {error:#}");
            EXIT_FAILURE
        }
    }
}

/// Removes the database flag so each operator subcommand parses only its own
/// flags.
fn split_database_flag(arguments: &[String]) -> Result<(Option<String>, Vec<String>)> {
    let mut database = None;
    let mut rest = Vec::with_capacity(arguments.len());
    let mut values = arguments.iter();
    while let Some(argument) = values.next() {
        if matches!(argument.as_str(), "--db" | "-db") {
            database = Some(
                values
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?
                    .clone(),
            );
        } else if let Some(value) = argument
            .strip_prefix("--db=")
            .or_else(|| argument.strip_prefix("-db="))
        {
            database = Some(value.to_owned());
        } else {
            rest.push(argument.clone());
        }
    }
    Ok((database, rest))
}

fn parse_database_argument(arguments: &[String]) -> Result<Option<String>> {
    let mut database = None;
    let mut values = arguments.iter();
    while let Some(argument) = values.next() {
        if matches!(argument.as_str(), "--db" | "-db") {
            database = Some(
                values
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?
                    .clone(),
            );
        } else if let Some(value) = argument
            .strip_prefix("--db=")
            .or_else(|| argument.strip_prefix("-db="))
        {
            database = Some(value.to_owned());
        } else if argument.starts_with('-') {
            bail!("flag provided but not defined: {argument}");
        } else {
            bail!("unexpected positional arguments: {arguments:?}");
        }
    }
    Ok(database)
}

fn decode_request<T: Default + DeserializeOwned>() -> Result<T> {
    let mut input = Vec::new();
    let _bytes_read = io::stdin().read_to_end(&mut input)?;
    let mut decoder = serde_json::Deserializer::from_slice(&input);
    let request = Option::<T>::deserialize(&mut decoder)?.unwrap_or_default();
    decoder.end()?;
    Ok(request)
}

fn open_store(explicit: Option<&str>) -> Result<(Paths, Store)> {
    let paths = paths::resolve(explicit)?;
    let store = Store::open(&paths.database_path)?;
    Ok((paths, store))
}

fn run_config_action(paths: Paths, store: &Store, request: ConfigRequest) -> Result<ConfigResult> {
    match request.action.as_str() {
        "init" => Ok(ConfigResult {
            paths,
            runtime_config: store.runtime_config()?,
            summary: "initialized Siftwire database".to_owned(),
            ..ConfigResult::default()
        }),
        "inspect_config" => inspect_config(paths, store),
        "replace_sources" => {
            let sources = match store.replace_sources(request.sources) {
                Ok(value) => value,
                Err(error) => return Ok(rejected_config(paths, error.to_string())),
            };
            Ok(ConfigResult {
                paths,
                summary: format!("stored {} sources", sources.len()),
                sources,
                ..ConfigResult::default()
            })
        }
        "upsert_source" => Ok(upsert_source(paths, store, request)),
        "delete_source" => delete_source(paths, store, &request.key),
        "replace_outlet_policies" => Ok(replace_outlets(paths, store, request)),
        "set_brief_options" => set_brief_options(paths, store, request.max_delivery_items),
        _ => Ok(rejected_config(
            paths,
            format!("unsupported config action {:?}", request.action),
        )),
    }
}

fn inspect_config(paths: Paths, store: &Store) -> Result<ConfigResult> {
    let runtime_config = store.runtime_config()?;
    let sources = store.list_sources(false)?;
    let outlets = store.list_outlet_policies()?;
    Ok(ConfigResult {
        paths,
        summary: format!(
            "configured sources={} outlet_policies={}",
            sources.len(),
            outlets.len()
        ),
        runtime_config,
        sources,
        outlets,
        ..ConfigResult::default()
    })
}

fn upsert_source(paths: Paths, store: &Store, request: ConfigRequest) -> ConfigResult {
    let source = match store.upsert_source(request.source) {
        Ok(value) => value,
        Err(error) => return rejected_config(paths, error.to_string()),
    };
    ConfigResult {
        paths,
        summary: format!("stored source {}", source.key),
        sources: vec![source],
        ..ConfigResult::default()
    }
}

fn delete_source(paths: Paths, store: &Store, key: &str) -> Result<ConfigResult> {
    if let Err(error) = store.delete_source(key) {
        return Ok(rejected_config(paths, error.to_string()));
    }
    let sources = store.list_sources(false)?;
    Ok(ConfigResult {
        paths,
        summary: format!("deleted source {key}"),
        sources,
        ..ConfigResult::default()
    })
}

fn replace_outlets(paths: Paths, store: &Store, request: ConfigRequest) -> ConfigResult {
    let outlets = match store.replace_outlet_policies(request.outlets) {
        Ok(value) => value,
        Err(error) => return rejected_config(paths, error.to_string()),
    };
    ConfigResult {
        paths,
        summary: format!("stored {} outlet policies", outlets.len()),
        outlets,
        ..ConfigResult::default()
    }
}

fn set_brief_options(paths: Paths, store: &Store, value: i64) -> Result<ConfigResult> {
    if let Err(error) = validate_max_delivery_items(value) {
        return Ok(rejected_config(paths, error.to_string()));
    }
    store.set_runtime_config(RUNTIME_CONFIG_MAX_DELIVERY_ITEMS, &value.to_string())?;
    Ok(ConfigResult {
        paths,
        runtime_config: store.runtime_config()?,
        summary: format!("stored max_delivery_items={value}"),
        ..ConfigResult::default()
    })
}

fn rejected_config(paths: Paths, reason: String) -> ConfigResult {
    ConfigResult {
        rejected: true,
        rejection_reason: reason.clone(),
        paths,
        summary: reason,
        ..ConfigResult::default()
    }
}

fn write_result<T: Serialize>(result: &T, label: &str) -> ExitCode {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    if let Err(error) = serde_json::to_writer(&mut writer, result)
        .and_then(|()| writer.write_all(b"\n").map_err(serde_json::Error::io))
    {
        eprintln!("encode {label} result: {error}");
        return EXIT_FAILURE;
    }
    EXIT_OK
}

fn usage(stdout: bool) {
    let text = "usage: siftwire <version|config|brief|source|runs>\n       siftwire config [--db path] < request.json\n       siftwire brief [--db path] < request.json\n       siftwire source add [--json] [--db path] < source.json\n       siftwire source list [--enabled] [--json] [--db path]\n       siftwire runs list [--limit N] [--json] [--db path]\n       siftwire runs show <run_id> [--candidates --dropped --selected] [--json] [--db path]";
    if stdout {
        println!("{text}");
    } else {
        eprintln!("{text}");
    }
}

fn validate_max_delivery_items(value: i64) -> Result<()> {
    if !(1..=crate::storage::MAX_DELIVERY_ITEMS_UPPER_BOUND).contains(&value) {
        bail!(
            "{} must be between 1 and {}",
            RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
            crate::storage::MAX_DELIVERY_ITEMS_UPPER_BOUND
        );
    }
    Ok(())
}

fn run_brief_action(paths: Paths, store: &Store, request: &BriefRequest) -> Result<BriefResult> {
    brief::run_action(paths, store, request)
}
