use std::collections::BTreeMap;

use chrono::SecondsFormat;

use crate::contract::{BriefItem, SuppressedItem, SuppressedRecentItem};
use crate::storage::StoredSentItem;

use super::dedup::{CollectedItem, sort_brief_items, suppressed_item, titles_match};

#[derive(Clone, Debug, Default)]
pub struct RecentSuppression {
    pub candidates: Vec<BriefItem>,
    pub suppressed_recent: Vec<SuppressedRecentItem>,
    pub suppressed: Vec<SuppressedItem>,
}

struct RecentLookup<'a> {
    items: &'a [StoredSentItem],
    urls: BTreeMap<&'a str, &'a StoredSentItem>,
}

#[must_use]
pub fn suppress_recent_candidates(
    items: Vec<CollectedItem>,
    recent: &[StoredSentItem],
) -> RecentSuppression {
    let lookup = recent_lookup(recent);
    let mut candidates = Vec::new();
    let mut suppressed_recent = Vec::new();
    let mut suppressed = Vec::new();
    for item in items {
        let Some(prior) = recently_sent_match(&item.brief_item, &lookup) else {
            candidates.push(item.brief_item);
            continue;
        };
        suppressed_recent.push(SuppressedRecentItem {
            source_key: item.source.key,
            title: item.brief_item.title.clone(),
            url: item.brief_item.url.clone(),
            matched_prior_title: prior.title.clone(),
            prior_sent_at: prior.sent_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        });
        suppressed.push(suppressed_item(&item.brief_item, "recently_sent"));
    }
    sort_brief_items(&mut candidates);
    RecentSuppression {
        candidates,
        suppressed_recent,
        suppressed,
    }
}

fn recent_lookup(items: &[StoredSentItem]) -> RecentLookup<'_> {
    let mut urls = BTreeMap::new();
    for item in items {
        if !item.url.is_empty() {
            let _previous = urls.insert(item.url.as_str(), item);
        }
    }
    RecentLookup { items, urls }
}

fn recently_sent_match<'a>(
    item: &BriefItem,
    lookup: &'a RecentLookup<'_>,
) -> Option<&'a StoredSentItem> {
    if !item.url.is_empty()
        && let Some(prior) = lookup.urls.get(item.url.as_str())
    {
        return Some(prior);
    }
    lookup
        .items
        .iter()
        .find(|prior| titles_match(&item.title, &prior.title))
}
