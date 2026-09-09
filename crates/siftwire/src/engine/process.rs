use chrono::{DateTime, Utc};

use crate::contract::{
    CurrentNewsStatus, SportsUpdate, SuppressedPolicyItem, SuppressedUnresolvedItem,
};
use crate::domain::{OutletPolicy, SOURCE_KIND_SCHEDULE, Source};
use crate::storage::SourceState;

use super::dedup::CollectedItem;
use super::model::{FetchOutput, FetchedItem};
use super::policy::apply_outlet_policies;
use super::selection::{item_to_brief_item, select_current_news, select_new_items};

#[derive(Clone, Debug, Default)]
pub struct ProcessedSource {
    pub items: Vec<FetchedItem>,
    pub eligible_items: Vec<FetchedItem>,
    pub current_news: Option<CurrentNewsStatus>,
    pub sports_updates: Vec<SportsUpdate>,
    pub suppressed_policy: Vec<SuppressedPolicyItem>,
    pub suppressed_unresolved: Vec<SuppressedUnresolvedItem>,
}

#[must_use]
pub fn process_source_items(
    source: &Source,
    output: FetchOutput,
    policies: &[OutletPolicy],
    state: Option<&SourceState>,
    now: DateTime<Utc>,
) -> ProcessedSource {
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
        .collect();
    let (items, current_news) = if source.is_current_news() {
        let (eligible, status) = select_current_news(output.items, now);
        (eligible, Some(status))
    } else {
        (output.items, None)
    };
    let policy_result = apply_outlet_policies(source, items, policies);
    // Legacy consumers still receive upcoming fixtures through `must_include`.
    // The dedicated sports section governs repetition for updated consumers.
    let eligible_items = if current_news.is_some() || source.kind == SOURCE_KIND_SCHEDULE {
        policy_result.items.clone()
    } else {
        select_new_items(&policy_result.items, state, output.truncated)
    };
    ProcessedSource {
        items: policy_result.items,
        eligible_items,
        current_news,
        sports_updates,
        suppressed_policy: policy_result.audit,
        suppressed_unresolved,
    }
}

#[must_use]
pub fn collect_items(source: &Source, items: &[FetchedItem]) -> Vec<CollectedItem> {
    items
        .iter()
        .map(|item| CollectedItem {
            source: source.clone(),
            brief_item: item_to_brief_item(source, item),
            fixture_identity: if source.kind == SOURCE_KIND_SCHEDULE {
                item.identity.clone()
            } else {
                String::new()
            },
        })
        .collect()
}
