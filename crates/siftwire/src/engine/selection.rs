use crate::contract::BriefItem;
use crate::domain::Source;
use crate::storage::SourceState;

use super::model::FetchedItem;

#[must_use]
pub fn select_new_items(
    items: &[FetchedItem],
    state: Option<&SourceState>,
    truncated: bool,
) -> Vec<FetchedItem> {
    if items.is_empty() {
        return Vec::new();
    }
    let Some(state) = state.filter(|state| !state.latest_identity.is_empty()) else {
        return items.iter().take(1).cloned().collect();
    };
    if let Some(position) = items
        .iter()
        .position(|item| item_matches_state(item, state))
    {
        return items.iter().take(position).cloned().collect();
    }
    if truncated {
        items.to_vec()
    } else {
        items.iter().take(1).cloned().collect()
    }
}

#[must_use]
pub fn item_to_brief_item(source: &Source, item: &FetchedItem) -> BriefItem {
    BriefItem {
        source_key: source.key.clone(),
        source_label: source.label.clone(),
        kind: source.kind.clone(),
        section: source.section.clone(),
        threshold: source.threshold.clone(),
        priority_rank: source.priority_rank,
        always_report: source.always_report,
        title: item.title.clone(),
        url: item.url.clone(),
        published_at: item.published_at.clone(),
        outlet: item.outlet.clone(),
    }
}

fn item_matches_state(item: &FetchedItem, state: &SourceState) -> bool {
    [item.identity.as_str(), item.feed_identity()]
        .into_iter()
        .filter(|identity| !identity.trim().is_empty())
        .any(|identity| identity == state.latest_identity || identity == state.latest_feed_identity)
}
