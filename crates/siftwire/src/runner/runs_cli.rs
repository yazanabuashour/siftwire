use std::process::ExitCode;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::contract::{FetchStatus, SentItem};
use crate::storage::{
    RUN_ITEM_ANNOTATION, RUN_ITEM_CANDIDATE, RUN_ITEM_DROPPED, RUN_ITEM_MUST_INCLUDE, RunDetail,
    RunItemRow, RunListOptions, RunPage, RunSummary, StoredSentItem,
};

use super::delivery::sent_item;
use super::evidence::{DeliveryStatus, delivery_status};
use super::format::{print_table, truncate};
use super::{EXIT_FAILURE, EXIT_OK, EXIT_USAGE_CODE, emit_json, open_store};
use crate::contract::ItemDisposition;

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
    let mut options = RunListOptions::default();
    if let Err(error) = parse_flags(&rest, &mut json, &mut limit, &mut options) {
        return usage_failure(&error);
    }
    match open_store(database.as_deref()) {
        Ok((_paths, store)) => {
            match store.list_runs(limit.unwrap_or(DEFAULT_RUN_LIMIT), &options) {
                Ok(runs) => render_runs(&runs, json),
                Err(error) => failure(&format!("list runs: {error:#}")),
            }
        }
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

fn parse_flags(
    arguments: &[String],
    json: &mut bool,
    limit: &mut Option<i64>,
    options: &mut RunListOptions,
) -> Result<()> {
    let mut values = arguments.iter();
    while let Some(argument) = values.next() {
        if argument == "--json" {
            *json = true;
        } else if argument == "--delivered" {
            options.delivered = true;
        } else if matches!(argument.as_str(), "--before" | "--search") {
            let value = values
                .next()
                .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?;
            if argument == "--before" {
                if value.trim().is_empty() {
                    bail!("--before requires a run id");
                }
                options.before = Some(value.clone());
            } else {
                options.search = Some(value.clone());
            }
        } else if argument == "--limit" {
            let raw = values
                .next()
                .ok_or_else(|| anyhow::anyhow!("flag needs an argument: {argument}"))?;
            *limit = match raw.parse::<i64>() {
                Ok(value) if value > 0 && value < i64::MAX => Some(value),
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

fn render_runs(page: &RunPage, json: bool) -> ExitCode {
    if json {
        return emit_json(page);
    }
    let runs = &page.runs;
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
    if let Some(before) = &page.next_before {
        println!("next_before: {before}");
    }
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
            detail,
        );
        println!();
        render_items(
            "candidates",
            detail
                .items
                .iter()
                .filter(|item| item.category == RUN_ITEM_CANDIDATE),
            detail,
        );
    }
    if parsed.sections.dropped {
        println!();
        render_dropped(detail.items.iter().filter(|item| is_drop(item)));
        println!("annotations:");
        for item in detail.items.iter().filter(|item| is_annotation(item)) {
            println!(
                "  [{}] {} {:?}: {}",
                item.source_key,
                item.title,
                disposition(item),
                item.detail
            );
        }
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
        let label = if log.source_label.is_empty() {
            log.source_key.clone()
        } else {
            format!("{} [{}]", log.source_label, log.source_key)
        };
        if log.status == "ok" {
            if let Some(news) = &log.current_news {
                println!(
                    "  {label} ok items={} eligible={} stale={} undated={} future={} window={}..={}",
                    log.item_count,
                    news.eligible_items,
                    news.stale_items,
                    news.undated_items,
                    news.future_items,
                    news.since,
                    news.until
                );
            } else {
                println!(
                    "  {} ok items={} new={}",
                    label,
                    log.item_count,
                    log.new_item_count
                        .map_or_else(|| "unknown".to_owned(), |count| count.to_string())
                );
            }
        } else {
            println!(
                "  {} {} {}",
                label,
                log.status,
                truncate(&log.error, SUMMARY_WIDTH)
            );
        }
    }
}

fn render_items<'a>(label: &str, items: impl Iterator<Item = &'a RunItemRow>, detail: &RunDetail) {
    println!("{label}:");
    for item in items {
        let status = delivery_status(item, detail);
        println!("  {status:?} [{}] {}", item.source_key, item.title);
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

pub(super) fn detail_json(detail: &RunDetail) -> serde_json::Value {
    serde_json::json!({
        "run": detail.summary,
        "delivery_html": detail.delivery_html,
        "must_include": items_json(
            detail.items.iter().filter(|item| item.category == RUN_ITEM_MUST_INCLUDE),
            detail,
        ),
        "candidates": items_json(
            detail.items.iter().filter(|item| item.category == RUN_ITEM_CANDIDATE),
            detail,
        ),
        "dropped": drops_json(
            detail.items.iter().filter(|item| is_drop(item)),
        ),
        "annotations": drops_json(detail.items.iter().filter(|item| is_annotation(item))),
        "fetch": fetch_json(detail),
        "sent_items": sent_json(&detail.sent_items),
    })
}

fn disposition(item: &RunItemRow) -> ItemDisposition {
    if item.reason == "unresolved" {
        return match serde_json::from_str::<serde_json::Value>(&item.detail)
            .ok()
            .and_then(|detail| {
                detail
                    .get("disposition")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .as_deref()
        {
            Some("retained") => ItemDisposition::Retained,
            Some("dropped") => ItemDisposition::Dropped,
            _ => ItemDisposition::Unknown,
        };
    }
    if item.category == RUN_ITEM_DROPPED {
        ItemDisposition::Dropped
    } else {
        ItemDisposition::Retained
    }
}

fn is_drop(item: &RunItemRow) -> bool {
    item.category == RUN_ITEM_DROPPED && disposition(item) == ItemDisposition::Dropped
}

fn is_annotation(item: &RunItemRow) -> bool {
    item.category == RUN_ITEM_ANNOTATION
        || (item.category == RUN_ITEM_DROPPED && disposition(item) == ItemDisposition::Unknown)
}

fn items_json<'a>(
    items: impl Iterator<Item = &'a RunItemRow>,
    detail: &RunDetail,
) -> Vec<ItemJson> {
    items
        .map(|item| {
            let status = delivery_status(item, detail);
            ItemJson {
                id: item.id.clone(),
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
                selected: status.selected(),
                delivery_status: status,
                reporting: crate::domain::Reporting::from_recorded(
                    &item.kind,
                    &item.threshold,
                    item.always_report,
                ),
            }
        })
        .collect()
}

fn drops_json<'a>(items: impl Iterator<Item = &'a RunItemRow>) -> Vec<DroppedJson> {
    items
        .map(|item| DroppedJson {
            id: item.id.clone(),
            source_label: item.source_label.clone(),
            disposition: disposition(item),
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
            source_label: log.source_label.clone(),
            status: log.status.clone(),
            error: log.error.clone(),
            items: log.item_count,
            new_items: log.new_item_count,
            current_news: log.current_news.clone(),
            ..FetchStatus::default()
        })
        .collect()
}

fn sent_json(items: &[StoredSentItem]) -> Vec<SentItem> {
    items.iter().map(|entry| sent_item(entry.clone())).collect()
}

#[derive(Serialize)]
struct ItemJson {
    id: String,
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
    delivery_status: DeliveryStatus,
    reporting: Option<crate::domain::Reporting>,
}

#[derive(Serialize)]
struct DroppedJson {
    id: String,
    source_label: String,
    disposition: ItemDisposition,
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
        detail.items.iter().filter(|item| is_drop(item)).count(),
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
