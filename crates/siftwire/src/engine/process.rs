use std::collections::BTreeMap;

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::contract::{FetchStatus, SportsUpdate, SuppressedPolicyItem, SuppressedUnresolvedItem};
use crate::domain::{OutletPolicy, SOURCE_KIND_SCHEDULE, Source};
use crate::storage::SourceState;

use super::dedup::CollectedItem;
use super::health::{add_fetch_failure_warning, add_news_date_warning};
use super::model::{FetchOutput, FetchedItem};
use super::policy::apply_outlet_policies;
use super::selection::{item_to_brief_item, select_current_news, select_new_items};

#[derive(Debug, Default)]
pub struct SourceOutcome {
    pub collected: Vec<CollectedItem>,
    /// Absent for current news, failed fetches, or an empty post-policy feed.
    pub next_state: Option<SourceState>,
    pub status: FetchStatus,
    pub sports_updates: Vec<SportsUpdate>,
    pub suppressed_policy: Vec<SuppressedPolicyItem>,
    pub suppressed_unresolved: Vec<SuppressedUnresolvedItem>,
    pub warnings: BTreeMap<String, String>,
}

#[must_use]
pub fn process_source(
    source: &Source,
    output: Result<FetchOutput>,
    policies: &[OutletPolicy],
    state: Option<&SourceState>,
    now: DateTime<Utc>,
) -> SourceOutcome {
    match output {
        Ok(output) => process_items(source, output, policies, state, now),
        Err(error) => {
            let error = error.to_string();
            let mut warnings = BTreeMap::new();
            add_fetch_failure_warning(&source.key, &error, &mut warnings);
            SourceOutcome {
                status: FetchStatus {
                    source_key: source.key.clone(),
                    source_label: source.label.clone(),
                    status: "error".to_owned(),
                    error,
                    ..FetchStatus::default()
                },
                warnings,
                ..SourceOutcome::default()
            }
        }
    }
}

fn process_items(
    source: &Source,
    output: FetchOutput,
    policies: &[OutletPolicy],
    state: Option<&SourceState>,
    now: DateTime<Utc>,
) -> SourceOutcome {
    let item_count = if source.kind == SOURCE_KIND_SCHEDULE {
        output.sports_updates.len()
    } else {
        output.items.len()
    };
    let sports_updates = output.sports_updates;
    let suppressed_unresolved = output
        .unresolved
        .into_iter()
        .map(|item| SuppressedUnresolvedItem {
            disposition: item.disposition,
            source_key: source.key.clone(),
            title: item.title,
            url: item.url,
            reason: item.reason,
        })
        .collect::<Vec<_>>();
    let (items, current_news) = if source.is_current_news() {
        let (eligible, status) = select_current_news(output.items, now);
        (eligible, Some(status))
    } else {
        (output.items, None)
    };
    let policy_result = apply_outlet_policies(source, items, policies);
    let next_state = if current_news.is_some() {
        None
    } else {
        policy_result.items.first().map(|top| SourceState {
            source_key: source.key.clone(),
            latest_identity: top.identity.clone(),
            latest_feed_identity: top.feed_identity().to_owned(),
            latest_title: top.title.clone(),
            latest_url: top.url.clone(),
            latest_published_at: top.published_at.clone(),
            checked_at: now,
        })
    };
    let eligible_items = if current_news.is_some() {
        policy_result.items
    } else {
        select_new_items(&policy_result.items, state, output.truncated)
    };
    let new_count = if source.kind == SOURCE_KIND_SCHEDULE {
        sports_updates.len()
    } else {
        eligible_items.len()
    };
    let mut warnings = BTreeMap::new();
    add_news_date_warning(&source.key, current_news.as_ref(), &mut warnings);
    SourceOutcome {
        collected: collect_items(source, &eligible_items),
        next_state,
        status: FetchStatus {
            source_key: source.key.clone(),
            source_label: source.label.clone(),
            status: "ok".to_owned(),
            items: item_count,
            new_items: current_news.is_none().then_some(new_count),
            current_news,
            suppressed_policy: policy_result.audit.len(),
            suppressed_unresolved: suppressed_unresolved.len(),
            ..FetchStatus::default()
        },
        sports_updates,
        suppressed_policy: policy_result.audit,
        suppressed_unresolved,
        warnings,
    }
}

pub(super) fn collect_items(source: &Source, items: &[FetchedItem]) -> Vec<CollectedItem> {
    items
        .iter()
        .map(|item| CollectedItem {
            source: source.clone(),
            brief_item: item_to_brief_item(source, item),
        })
        .collect()
}
