use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};

use crate::contract::{BriefItem, HealthDelta};
use crate::domain::Source;
use crate::storage::FetchLog;

#[must_use]
pub fn build_health_footnote(delta: &HealthDelta) -> String {
    let new_sources = warning_sources(&delta.new_warnings);
    let resolved_sources = warning_sources(&delta.resolved_warnings);
    let new_only = new_sources
        .difference(&resolved_sources)
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
        format!("Feed health changes - {}.", parts.join(" | "))
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

pub fn add_stale_heartbeat_warning(
    runtime_config: &BTreeMap<String, String>,
    current_warnings: &mut BTreeMap<String, String>,
    now: DateTime<Utc>,
) {
    let Some(last_check) = runtime_config
        .get("last_check")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let Ok(checked_at) = DateTime::parse_from_rfc3339(last_check) else {
        return;
    };
    let elapsed = now.signed_duration_since(checked_at.with_timezone(&Utc));
    if elapsed.num_hours() <= 4 {
        return;
    }
    let hours = elapsed.num_hours();
    let minutes = elapsed.num_minutes().rem_euclid(60);
    let _previous = current_warnings.insert(
        "runtime:last_check".to_owned(),
        format!("`last_check` was {hours}h {minutes}m ago - heartbeat may be stalled"),
    );
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
