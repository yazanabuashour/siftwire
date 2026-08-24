use std::process::ExitCode;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::contract::{FetchStatus, SentItem};
use crate::storage::{
    RUN_ITEM_CANDIDATE, RUN_ITEM_DROPPED, RUN_ITEM_MUST_INCLUDE, RunDetail, RunItemRow, RunSummary,
    StoredSentItem,
};

use super::delivery::sent_item;
use super::format::{print_table, truncate};
use super::{EXIT_FAILURE, EXIT_OK, EXIT_USAGE_CODE, emit_json, open_store};

const DEFAULT_RUN_LIMIT: i64 = 20;
const SUMMARY_WIDTH: usize = 64;

pub(super) fn run(arguments: &[String]) -> ExitCode {
    let Some((action, rest)) = arguments.split_first() else {
        super::usage(false);
        return ExitCode::from(EXIT_USAGE_CODE);
    };
    match action.as_str() {
        "list" => list(rest),
        "show" => show(rest),
        _ => {
            eprintln!("unknown siftwire runs action {action:?}");
            ExitCode::from(EXIT_USAGE_CODE)
        }
    }
}

fn list(arguments: &[String]) -> ExitCode {
    let (database, rest) = match super::split_database_flag(arguments) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    let mut json = false;
    let mut limit: Option<i64> = None;
    if let Err(error) = parse_flags(&rest, &mut json, &mut limit) {
        return usage_failure(&error);
    }
    match open_store(database.as_deref()) {
        Ok((_paths, store)) => match store.list_runs(limit.unwrap_or(DEFAULT_RUN_LIMIT)) {
            Ok(runs) => render_runs(&runs, json),
            Err(error) => failure(&format!("list runs: {error:#}")),
        },
        Err(error) => failure(&format!("open database: {error:#}")),
    }
}

#[derive(Default)]
struct ShowArguments {
    json: bool,
    sections: Sections,
    run_id: String,
}

#[derive(Default)]
struct Sections {
    candidates: bool,
    dropped: bool,
    selected: bool,
}

/// Parses show flags; the database flag was already removed by the caller.
fn parse_show_arguments(arguments: &[String]) -> Result<ShowArguments> {
    let mut json = false;
    let mut sections = Sections::default();
    let mut run_id = None;
    for argument in arguments {
        match argument.as_str() {
            "--json" => json = true,
            "--candidates" => sections.candidates = true,
            "--dropped" => sections.dropped = true,
            "--selected" => sections.selected = true,
            _ if argument.starts_with('-') => {
                bail!("flag provided but not defined: {argument}");
            }
            _ if run_id.is_none() => run_id = Some(argument.clone()),
            _ => bail!("unexpected extra positional arguments: {arguments:?}"),
        }
    }
    Ok(ShowArguments {
        json,
        sections,
        run_id: run_id.ok_or_else(|| anyhow::anyhow!("runs show requires a run id"))?,
    })
}

fn show(arguments: &[String]) -> ExitCode {
    let (database, rest) = match super::split_database_flag(arguments) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    let parsed = match parse_show_arguments(&rest) {
        Ok(value) => value,
        Err(error) => return usage_failure(&error),
    };
    match open_store(database.as_deref()) {
        Ok((_paths, store)) => match store.run_detail(&parsed.run_id) {
            Ok(Some(detail)) => render_detail(&detail, &parsed),
            Ok(None) => failure(&format!("run not found: {}", parsed.run_id)),
            Err(error) => failure(&format!("show run: {error:#}")),
        },
        Err(error) => failure(&format!("open database: {error:#}")),
    }
}

fn parse_flags(arguments: &[String], json: &mut bool, limit: &mut Option<i64>) -> Result<()> {
    let mut values = arguments.iter();
    while let Some(argument) = values.next() {
        if argument == "--json" {
            *json = true;
        } else if argument == "--limit" {
            let raw = values
                .next()
                .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?;
            *limit = match raw.parse::<i64>() {
                Ok(value) if value > 0 => Some(value),
                Ok(_) | Err(_) => {
                    return Err(anyhow::anyhow!(
                        "--limit expects a positive number: {raw:?}"
                    ));
                }
            };
        } else if argument.starts_with('-') {
            bail!("flag provided but not defined: {argument}");
        } else {
            bail!("unexpected positional arguments: {arguments:?}");
        }
    }
    Ok(())
}

fn render_runs(runs: &[RunSummary], json: bool) -> ExitCode {
    if json {
        return emit_json(&serde_json::json!({ "runs": runs }));
    }
    let header = ["RUN_ID", "STARTED", "STATUS", "DELIVERED", "SUMMARY"];
    let rows: Vec<Vec<String>> = runs
        .iter()
        .map(|run| {
            vec![
                run.run_id.clone(),
                truncate(&run.started_at, 25),
                run.status.clone(),
                delivered_text(run).to_owned(),
                truncate(&run.summary, SUMMARY_WIDTH),
            ]
        })
        .collect();
    print_table(&header, &rows);
    EXIT_OK
}

fn render_detail(detail: &RunDetail, parsed: &ShowArguments) -> ExitCode {
    if parsed.json {
        return emit_json(&detail_json(detail));
    }
    render_summary_block(detail);
    println!();
    render_fetch_block(detail);
    if parsed.sections.candidates {
        println!();
        render_items(
            "must include",
            detail
                .items
                .iter()
                .filter(|item| item.category == RUN_ITEM_MUST_INCLUDE),
            &detail.sent_items,
        );
        println!();
        render_items(
            "candidates",
            detail
                .items
                .iter()
                .filter(|item| item.category == RUN_ITEM_CANDIDATE),
            &detail.sent_items,
        );
    }
    if parsed.sections.dropped {
        println!();
        render_dropped(
            detail
                .items
                .iter()
                .filter(|item| item.category == RUN_ITEM_DROPPED),
        );
    }
    if parsed.sections.selected {
        println!();
        render_selected(detail);
    }
    EXIT_OK
}

fn render_summary_block(detail: &RunDetail) {
    let run = &detail.summary;
    println!("run: {}", run.run_id);
    println!("started: {}", run.started_at);
    if let Some(finished) = &run.finished_at {
        println!("finished: {finished}");
    }
    println!("status: {} dry_run: {}", run.status, yes_no(run.dry_run));
    match &run.delivered_at {
        Some(delivered_at) => println!("delivered: {delivered_at}"),
        None => println!("delivered: no"),
    }
    println!("summary: {}", run.summary);
    let counts = category_counts(detail);
    println!(
        "items: must_include={} candidates={} dropped={}",
        counts.0, counts.1, counts.2
    );
}

fn render_fetch_block(detail: &RunDetail) {
    if detail.fetch_logs.is_empty() {
        return;
    }
    println!("fetch:");
    for log in &detail.fetch_logs {
        if log.status == "ok" {
            println!(
                "  {} ok items={} new={}",
                log.source_key, log.item_count, log.new_item_count
            );
        } else {
            println!(
                "  {} {} {}",
                log.source_key,
                log.status,
                truncate(&log.error, SUMMARY_WIDTH)
            );
        }
    }
}

fn render_items<'a>(
    label: &str,
    items: impl Iterator<Item = &'a RunItemRow>,
    sent: &[StoredSentItem],
) {
    println!("{label}:");
    for item in items {
        let mark = if is_selected(item, sent) { "*" } else { "-" };
        println!("  {mark} [{}] {}", item.source_key, item.title);
        println!("    {}", item.url);
    }
}

fn render_dropped<'a>(items: impl Iterator<Item = &'a RunItemRow>) {
    println!("dropped:");
    for item in items {
        println!("  - [{}] {}", item.source_key, item.title);
        println!("    reason: {}", item.reason);
        println!("    {}", item.url);
    }
}

fn render_selected(detail: &RunDetail) {
    println!("selected:");
    for entry in &detail.sent_items {
        println!("  - {}", entry.title);
        println!("    {}", entry.url);
    }
    if let Some(message) = &detail.summary.message {
        println!();
        println!("delivery message:");
        println!("{message}");
    }
}

fn detail_json(detail: &RunDetail) -> serde_json::Value {
    serde_json::json!({
        "run": detail.summary,
        "must_include": items_json(
            detail.items.iter().filter(|item| item.category == RUN_ITEM_MUST_INCLUDE),
            &detail.sent_items,
        ),
        "candidates": items_json(
            detail.items.iter().filter(|item| item.category == RUN_ITEM_CANDIDATE),
            &detail.sent_items,
        ),
        "dropped": drops_json(
            detail.items.iter().filter(|item| item.category == RUN_ITEM_DROPPED),
        ),
        "fetch": fetch_json(detail),
        "sent_items": sent_json(&detail.sent_items),
    })
}

/// Selection matches the persisted title-and-URL pair so a candidate sharing a
/// URL with a distinct must-include item is not falsely marked.
fn is_selected(item: &RunItemRow, sent: &[StoredSentItem]) -> bool {
    sent.iter()
        .any(|entry| entry.url == item.url && entry.title == item.title)
}

fn items_json<'a>(
    items: impl Iterator<Item = &'a RunItemRow>,
    sent: &[StoredSentItem],
) -> Vec<ItemJson> {
    items
        .map(|item| ItemJson {
            source_key: item.source_key.clone(),
            source_label: item.source_label.clone(),
            kind: item.kind.clone(),
            section: item.section.clone(),
            threshold: item.threshold.clone(),
            priority_rank: item.priority_rank,
            always_report: item.always_report,
            published_at: item.published_at.clone(),
            outlet: item.outlet.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            selected: is_selected(item, sent),
        })
        .collect()
}

fn drops_json<'a>(items: impl Iterator<Item = &'a RunItemRow>) -> Vec<DroppedJson> {
    items
        .map(|item| DroppedJson {
            source_key: item.source_key.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            reason: item.reason.clone(),
            detail: serde_json::from_str(&item.detail)
                .unwrap_or_else(|_| serde_json::Value::String(item.detail.clone())),
        })
        .collect()
}

fn fetch_json(detail: &RunDetail) -> Vec<FetchStatus> {
    detail
        .fetch_logs
        .iter()
        .map(|log| FetchStatus {
            source_key: log.source_key.clone(),
            status: log.status.clone(),
            error: log.error.clone(),
            items: log.item_count,
            new_items: log.new_item_count,
            ..FetchStatus::default()
        })
        .collect()
}

fn sent_json(items: &[StoredSentItem]) -> Vec<SentItem> {
    items.iter().map(|entry| sent_item(entry.clone())).collect()
}

#[derive(Serialize)]
struct ItemJson {
    source_key: String,
    source_label: String,
    kind: String,
    section: String,
    threshold: String,
    priority_rank: i64,
    always_report: bool,
    published_at: String,
    outlet: String,
    title: String,
    url: String,
    selected: bool,
}

#[derive(Serialize)]
struct DroppedJson {
    source_key: String,
    title: String,
    url: String,
    reason: String,
    detail: serde_json::Value,
}

fn category_counts(detail: &RunDetail) -> (usize, usize, usize) {
    let count = |category: &str| {
        detail
            .items
            .iter()
            .filter(|item| item.category == category)
            .count()
    };
    (
        count(RUN_ITEM_MUST_INCLUDE),
        count(RUN_ITEM_CANDIDATE),
        count(RUN_ITEM_DROPPED),
    )
}

const fn delivered_text(run: &RunSummary) -> &'static str {
    if run.delivered_at.is_some() {
        "yes"
    } else {
        "no"
    }
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
