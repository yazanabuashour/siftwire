use chrono::{DateTime, FixedOffset, SecondsFormat, TimeDelta, Utc};

use crate::contract::{BriefItem, CurrentNewsStatus};
use crate::domain::Source;
use crate::storage::SourceState;

use super::model::FetchedItem;

// Operator-selected window matches the existing 24-hour repeat context.
// See docs/architecture/current-news.md for the decision receipt.
pub const CURRENT_NEWS_HOURS: i64 = 24;

pub fn select_current_news(
    items: Vec<FetchedItem>,
    now: DateTime<Utc>,
) -> (Vec<FetchedItem>, CurrentNewsStatus) {
    let since = now
        .checked_sub_signed(TimeDelta::hours(CURRENT_NEWS_HOURS))
        .unwrap_or(DateTime::<Utc>::MIN_UTC);
    let mut status = CurrentNewsStatus {
        since: since.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        until: now.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        ..CurrentNewsStatus::default()
    };
    let mut eligible = Vec::new();
    for item in items {
        let Some(published) = parse_item_time(&item.published_at) else {
            status.undated_items = status.undated_items.saturating_add(1);
            continue;
        };
        if published < since {
            status.stale_items = status.stale_items.saturating_add(1);
        } else if published > now {
            status.future_items = status.future_items.saturating_add(1);
        } else {
            eligible.push(item);
        }
    }
    status.eligible_items = eligible.len();
    (eligible, status)
}

pub fn parse_item_time(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .or_else(|_error| DateTime::parse_from_rfc2822(value))
        .ok()
}

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
