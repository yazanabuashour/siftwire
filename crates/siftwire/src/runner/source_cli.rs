use std::io::{self, Read};
use std::process::ExitCode;

use anyhow::{Result, bail};

use crate::domain::Source;

use super::format::{print_table, truncate};
use super::{EXIT_FAILURE, EXIT_OK, EXIT_USAGE_CODE, emit_json, open_store};

pub(super) fn run(arguments: &[String]) -> ExitCode {
    let Some((action, rest)) = arguments.split_first() else {
        super::usage(false);
        return ExitCode::from(EXIT_USAGE_CODE);
    };
    match action.as_str() {
        "list" => list(rest),
        "add" => add(rest),
        _ => {
            eprintln!("unknown siftwire source action {action:?}");
            ExitCode::from(EXIT_USAGE_CODE)
        }
    }
}

/// Parses source flags; the database flag was already removed by the caller.
fn parse_flags(arguments: &[String], accept_enabled: bool) -> Result<(bool, bool)> {
    let mut json = false;
    let mut enabled_only = false;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            "--enabled" if accept_enabled => enabled_only = true,
            _ if argument.starts_with('-') => {
                bail!("flag provided but not defined: {argument}");
            }
            _ => bail!("unexpected positional arguments: {arguments:?}"),
        }
    }
    Ok((json, enabled_only))
}

fn list(arguments: &[String]) -> ExitCode {
    let (database, rest) = match super::split_database_flag(arguments) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    let (json, enabled_only) = match parse_flags(&rest, true) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    match open_store(database.as_deref()) {
        Ok((_paths, store)) => match store.list_sources(enabled_only) {
            Ok(sources) => render_sources(&sources, json),
            Err(error) => failure(&format!("list sources: {error:#}")),
        },
        Err(error) => failure(&format!("open database: {error:#}")),
    }
}

/// Reads exactly one JSON source object from stdin and stores it.
fn add(arguments: &[String]) -> ExitCode {
    let (database, rest) = match super::split_database_flag(arguments) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    let (json, _) = match parse_flags(&rest, false) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    let mut input = Vec::new();
    if io::stdin().read_to_end(&mut input).is_err() {
        eprintln!("read source request: stdin is unreadable");
        return EXIT_FAILURE;
    }
    let source = match serde_json::from_slice::<Source>(&input) {
        Ok(value) => value,
        Err(error) => return failure(&format!("decode source request: {error}")),
    };
    match open_store(database.as_deref()) {
        Ok((_paths, store)) => match store.upsert_source(source) {
            Ok(stored) => {
                if json {
                    return emit_json(&stored);
                }
                println!("stored source {}", stored.key);
                EXIT_OK
            }
            Err(error) => failure(&format!("store source: {error:#}")),
        },
        Err(error) => failure(&format!("open database: {error:#}")),
    }
}

fn render_sources(sources: &[Source], json: bool) -> ExitCode {
    if json {
        return emit_json(&serde_json::json!({ "sources": sources }));
    }
    let header = ["KEY", "ENABLED", "KIND", "SECTION", "THRESHOLD", "LABEL"];
    let rows: Vec<Vec<String>> = sources
        .iter()
        .map(|source| {
            vec![
                source.key.clone(),
                yes_no(source.enabled).to_owned(),
                source.kind.clone(),
                source.section.clone(),
                source.threshold.clone(),
                truncate(&source.label, 48),
            ]
        })
        .collect();
    print_table(&header, &rows);
    EXIT_OK
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn usage_failure(error: &anyhow::Error) -> ExitCode {
    eprintln!("{error}");
    ExitCode::from(EXIT_USAGE_CODE)
}

fn failure(message: &str) -> ExitCode {
    eprintln!("{message}");
    EXIT_FAILURE
}
