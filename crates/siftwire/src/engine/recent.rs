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

#[must_use]
pub fn suppress_recent_candidates(
    items: Vec<CollectedItem>,
    recent: &[StoredSentItem],
) -> RecentSuppression {
    let mut candidates = Vec::new();
    let mut suppressed_recent = Vec::new();
    let mut suppressed = Vec::new();
    for item in items {
        // A changed headline at a reused URL remains for the agent's recent-context judgment.
        let Some(prior) = recent
            .iter()
            .find(|prior| titles_match(&item.brief_item.title, &prior.title))
        else {
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
