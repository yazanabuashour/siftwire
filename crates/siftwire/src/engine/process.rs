use crate::contract::{SuppressedPolicyItem, SuppressedUnresolvedItem};
use crate::domain::{OutletPolicy, SOURCE_KIND_SCHEDULE, Source};
use crate::storage::SourceState;

use super::dedup::CollectedItem;
use super::model::{FetchOutput, FetchedItem};
use super::policy::apply_outlet_policies;
use super::selection::{item_to_brief_item, select_new_items};

#[derive(Clone, Debug, Default)]
pub struct ProcessedSource {
    pub items: Vec<FetchedItem>,
    pub new_items: Vec<FetchedItem>,
    pub suppressed_policy: Vec<SuppressedPolicyItem>,
    pub suppressed_unresolved: Vec<SuppressedUnresolvedItem>,
}

#[must_use]
pub fn process_source_items(
    source: &Source,
    output: FetchOutput,
    policies: &[OutletPolicy],
    state: Option<&SourceState>,
) -> ProcessedSource {
    let suppressed_unresolved = output
        .unresolved
        .into_iter()
        .map(|item| SuppressedUnresolvedItem {
            source_key: source.key.clone(),
            title: item.title,
            url: item.url,
            reason: item.reason,
        })
        .collect();
    let policy_result = apply_outlet_policies(source, output.items, policies);
    // Schedule items repeat by window arithmetic (the two runs before kickoff),
    // so latest-seen suppression would drop their second appearance.
    let new_items = if source.kind == SOURCE_KIND_SCHEDULE {
        policy_result.items.clone()
    } else {
        select_new_items(&policy_result.items, state, output.truncated)
    };
    ProcessedSource {
        items: policy_result.items,
        new_items,
        suppressed_policy: policy_result.audit,
        suppressed_unresolved,
    }
}

#[must_use]
pub fn collect_new_items(source: &Source, items: &[FetchedItem]) -> Vec<CollectedItem> {
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
