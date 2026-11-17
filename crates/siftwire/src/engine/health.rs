use std::collections::{BTreeMap, BTreeSet};

use crate::contract::{BriefItem, CurrentNewsStatus, FetchStatus, HealthDelta};
use crate::domain::Source;
use crate::storage::FetchLog;

#[must_use]
pub fn build_health_footnote(delta: &HealthDelta, statuses: &[FetchStatus]) -> String {
    let unavailable = statuses
        .iter()
        .filter(|status| status.status == "error")
        .map(|status| status.source_key.clone())
        .collect::<BTreeSet<_>>();
    let empty = statuses
        .iter()
        .filter(|status| {
            status.status == "ok" && status.items == 0 && status.current_news.is_some()
        })
        .map(|status| status.source_key.clone())
        .collect::<BTreeSet<_>>();
    let new_sources = warning_sources(&delta.new_warnings);
    let resolved_sources = warning_sources(&delta.resolved_warnings);
    let new_only = new_sources
        .difference(&resolved_sources)
        .filter(|source| !unavailable.contains(*source))
        .cloned()
        .collect::<Vec<_>>();
    let resolved_only = resolved_sources
        .difference(&new_sources)
        .cloned()
        .collect::<Vec<_>>();
    let flapped = new_sources
        .intersection(&resolved_sources)
        .cloned()
        .collect::<Vec<_>>();
    let mut parts = Vec::new();
    if !unavailable.is_empty() {
        parts.push(format!(
            "UNAVAILABLE: {}",
            join_warning_sources(&unavailable.into_iter().collect::<Vec<_>>())
        ));
    }
    if !empty.is_empty() {
        parts.push(format!(
            "EMPTY NEWS FEEDS: {}",
            join_warning_sources(&empty.into_iter().collect::<Vec<_>>())
        ));
    }
    if !new_only.is_empty() {
        parts.push(format!("NEW: {}", join_warning_sources(&new_only)));
    }
    if !resolved_only.is_empty() {
        parts.push(format!(
            "RESOLVED: {}",
            join_warning_sources(&resolved_only)
        ));
    }
    if !flapped.is_empty() {
        parts.push(format!(
            "FLAPPED (new+resolved this run): {}",
            join_warning_sources(&flapped)
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("Source coverage - {}.", parts.join(" | "))
    }
}

pub fn add_fetch_failure_warning(
    source_key: &str,
    error: &str,
    current_warnings: &mut BTreeMap<String, String>,
) {
    let _previous = current_warnings.insert(
        format!("feed:{source_key}"),
        format!("Feed `{source_key}` failed this run ({error})"),
    );
}

pub fn add_news_date_warning(
    source_key: &str,
    news: Option<&CurrentNewsStatus>,
    current_warnings: &mut BTreeMap<String, String>,
) {
    if let Some(news) = news
        && (news.undated_items > 0 || news.future_items > 0)
    {
        current_warnings.insert(
            format!("news_dates:{source_key}"),
            format!(
                "Feed `{source_key}` excluded {} undated/invalid and {} future-dated news entries",
                news.undated_items, news.future_items
            ),
        );
    }
}

#[must_use]
pub fn enabled_source_keys(sources: &[Source]) -> BTreeSet<String> {
    sources.iter().map(|source| source.key.clone()).collect()
}

pub fn add_recurring_failure_warnings(
    logs: &[FetchLog],
    active_sources: &BTreeSet<String>,
    current_warnings: &mut BTreeMap<String, String>,
) {
    let mut by_source: BTreeMap<&str, Vec<&FetchLog>> = BTreeMap::new();
    for log in logs {
        if active_sources.contains(&log.source_key) {
            by_source
                .entry(log.source_key.as_str())
                .or_default()
                .push(log);
        }
    }
    for (source, source_logs) in by_source {
        let recent = source_logs
            .iter()
            .rev()
            .take(3)
            .copied()
            .collect::<Vec<_>>();
        if recent.len() < 3 || recent.iter().any(|log| log.status != "error") {
            continue;
        }
        let last_error = recent
            .first()
            .map(|log| log.error.as_str())
            .filter(|error| !error.is_empty())
            .unwrap_or("unknown error");
        let _previous = current_warnings.insert(
            format!("feed-recurring:{source}"),
            format!("Feed `{source}` has failed 3+ consecutive runs (last error: {last_error})"),
        );
    }
}

#[must_use]
pub fn brief_summary(
    must_include: &[BriefItem],
    candidates: &[BriefItem],
    sports_updates: usize,
    health_footnote: &str,
) -> String {
    if must_include.is_empty()
        && candidates.is_empty()
        && sports_updates == 0
        && health_footnote.is_empty()
    {
        return "NO_REPLY".to_owned();
    }
    format!(
        "must_include={} candidates={} sports_updates={} health_footnote={}",
        must_include.len(),
        candidates.len(),
        sports_updates,
        !health_footnote.is_empty()
    )
}

fn warning_sources(warnings: &[String]) -> BTreeSet<String> {
    warnings
        .iter()
        .filter_map(|warning| {
            warning
                .strip_prefix("Feed `")
                .and_then(|rest| rest.split_once('`'))
                .map(|(source, _message)| source.to_owned())
        })
        .collect()
}

fn join_warning_sources(names: &[String]) -> String {
    names.join(", ")
}
