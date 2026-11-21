use jiff::Timestamp;
mod automation;
mod brief;
mod delivery;
mod email;
mod evidence;
mod format;
mod notes;
mod options;
mod runs_cli;
mod sections;

use std::env;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use anyhow::{Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::contract::{ConfigRequest, ConfigResult, Paths};
use crate::paths;
use crate::storage::{ConfigMutationError, Store};

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
        "config" => run_json_task(&rest, "config", run_config_action),
        "brief" => run_json_task(&rest, "brief", |paths, store, request| {
            brief::run_action(paths, store, &request)
        }),
        "runs" => runs_cli::run(&rest),
        "automation" => automation::run(&rest),
        "notes" => notes::run(&rest),
        _ => {
            eprintln!("unknown siftwire command {command:?}");
            usage(false);
            ExitCode::from(EXIT_USAGE_CODE)
        }
    }
}

fn emit_json<T: Serialize>(value: &T) -> ExitCode {
    match format::write_json(value) {
        Ok(()) => EXIT_OK,
        Err(error) => {
            eprintln!("{error:#}");
            EXIT_FAILURE
        }
    }
}

fn run_json_task<Q: DeserializeOwned, R: Serialize>(
    arguments: &[String],
    label: &str,
    action: impl FnOnce(Paths, &Store, Q) -> Result<R>,
) -> ExitCode {
    let database = match parse_database_argument(arguments) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_USAGE_CODE);
        }
    };
    let request = match decode_request() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("decode {label} request: {error}");
            return EXIT_FAILURE;
        }
    };
    match open_store(database.as_deref()).and_then(|(paths, store)| action(paths, &store, request))
    {
        Ok(result) => write_result(&result, label),
        Err(error) => {
            eprintln!("run {label} task: {error:#}");
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
        if argument == "--db" {
            database = Some(
                values
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?
                    .clone(),
            );
        } else if let Some(value) = argument.strip_prefix("--db=") {
            database = Some(value.to_owned());
        } else {
            rest.push(argument.clone());
            if matches!(argument.as_str(), "--search" | "--before" | "--limit") {
                rest.push(
                    values
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?
                        .clone(),
                );
            }
        }
    }
    Ok((database, rest))
}

fn parse_database_argument(arguments: &[String]) -> Result<Option<String>> {
    let mut database = None;
    let mut values = arguments.iter();
    while let Some(argument) = values.next() {
        if argument == "--db" {
            database = Some(
                values
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?
                    .clone(),
            );
        } else if let Some(value) = argument.strip_prefix("--db=") {
            database = Some(value.to_owned());
        } else if argument.starts_with('-') {
            bail!("flag provided but not defined: {argument}");
        } else {
            bail!("unexpected positional arguments: {arguments:?}");
        }
    }
    Ok(database)
}

fn decode_request<T: DeserializeOwned>() -> Result<T> {
    let mut input = Vec::new();
    let _bytes_read = io::stdin().read_to_end(&mut input)?;
    if !input.trim_ascii_start().starts_with(b"{") {
        bail!("request must be a JSON object");
    }
    let mut decoder = serde_json::Deserializer::from_slice(&input);
    let request = T::deserialize(&mut decoder)?;
    decoder.end()?;
    Ok(request)
}

fn open_store(explicit: Option<&str>) -> Result<(Paths, Store)> {
    let paths = paths::resolve(explicit)?;
    let store = Store::open(&paths.database_path)?;
    Ok((paths, store))
}

fn run_config_action(paths: Paths, store: &Store, request: ConfigRequest) -> Result<ConfigResult> {
    let mut result = config_action(paths, store, request)?;
    result.source_reporting = result
        .sources
        .iter()
        .map(|source| (source.key.clone(), source.reporting()))
        .collect();
    Ok(result)
}

fn config_action(paths: Paths, store: &Store, request: ConfigRequest) -> Result<ConfigResult> {
    match request.action.as_str() {
        "init" => Ok(ConfigResult {
            paths,
            runtime_config: store.runtime_config()?,
            summary: "initialized SiftWire database".to_owned(),
            ..ConfigResult::default()
        }),
        "inspect_config" => inspect_config(paths, store),
        "replace_sources" => {
            let sources = match store.replace_sources(request.sources) {
                Ok(value) => value,
                Err(ConfigMutationError::Validation(error)) => {
                    return Ok(rejected_config(paths, error.to_string()));
                }
                Err(ConfigMutationError::Storage(error)) => return Err(error),
            };
            Ok(ConfigResult {
                paths,
                summary: format!("stored {} sources", sources.len()),
                sources,
                ..ConfigResult::default()
            })
        }
        "upsert_source" => upsert_source(paths, store, request),
        "delete_source" => delete_source(paths, store, &request.key),
        "replace_outlet_policies" => replace_outlets(paths, store, request),
        "set_brief_options" => set_brief_options(paths, store, &request),
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

fn upsert_source(paths: Paths, store: &Store, request: ConfigRequest) -> Result<ConfigResult> {
    let source = match store.upsert_source(request.source) {
        Ok(value) => value,
        Err(ConfigMutationError::Validation(error)) => {
            return Ok(rejected_config(paths, error.to_string()));
        }
        Err(ConfigMutationError::Storage(error)) => return Err(error),
    };
    Ok(ConfigResult {
        paths,
        summary: format!("stored source {}", source.key),
        sources: vec![source],
        ..ConfigResult::default()
    })
}

fn delete_source(paths: Paths, store: &Store, key: &str) -> Result<ConfigResult> {
    match store.delete_source(key) {
        Ok(()) => {}
        Err(ConfigMutationError::Validation(error)) => {
            return Ok(rejected_config(paths, error.to_string()));
        }
        Err(ConfigMutationError::Storage(error)) => return Err(error),
    }
    let sources = store.list_sources(false)?;
    Ok(ConfigResult {
        paths,
        summary: format!("deleted source {key}"),
        sources,
        ..ConfigResult::default()
    })
}

fn replace_outlets(paths: Paths, store: &Store, request: ConfigRequest) -> Result<ConfigResult> {
    let outlets = match store.replace_outlet_policies(request.outlets) {
        Ok(value) => value,
        Err(ConfigMutationError::Validation(error)) => {
            return Ok(rejected_config(paths, error.to_string()));
        }
        Err(ConfigMutationError::Storage(error)) => return Err(error),
    };
    Ok(ConfigResult {
        paths,
        summary: format!("stored {} outlet policies", outlets.len()),
        outlets,
        ..ConfigResult::default()
    })
}

fn set_brief_options(paths: Paths, store: &Store, request: &ConfigRequest) -> Result<ConfigResult> {
    let values = match options::updates(request, Timestamp::now()) {
        Ok(values) => values,
        Err(error) => return Ok(rejected_config(paths, error.to_string())),
    };
    store.set_runtime_config_values(&values)?;
    Ok(ConfigResult {
        paths,
        runtime_config: store.runtime_config()?,
        summary: format!("stored {} brief options", values.len()),
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
    let text = "usage: siftwire <version|config|brief|runs|automation|notes>\n       siftwire config [--db path] < request.json\n       siftwire brief [--db path] < request.json\n       siftwire runs list [--delivered] [--before run_id] [--search text] [--limit N] [--json] [--db path]\n       siftwire runs show <run_id> [--candidates --dropped --selected] [--json] [--db path]\n       siftwire automation [--replay brief.json new_output_directory]\n       siftwire notes publish [--db path] < request.json";
    if stdout {
        println!("{text}");
    } else {
        eprintln!("{text}");
    }
}
