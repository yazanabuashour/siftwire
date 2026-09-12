use chrono::SecondsFormat;

use crate::contract::{BriefItem, SuppressedRecentItem};
use crate::storage::StoredSentItem;

use super::dedup::{CollectedItem, sort_brief_items, titles_match};

#[derive(Clone, Debug, Default)]
pub struct RecentSuppression {
    pub candidates: Vec<BriefItem>,
    pub suppressed_recent: Vec<SuppressedRecentItem>,
}

#[must_use]
pub fn suppress_recent_candidates(
    items: Vec<CollectedItem>,
    recent: &[StoredSentItem],
) -> RecentSuppression {
    let mut candidates = Vec::new();
    let mut suppressed_recent = Vec::new();
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
            title: item.brief_item.title,
            url: item.brief_item.url,
            matched_prior_title: prior.title.clone(),
            prior_sent_at: prior.sent_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        });
    }
    sort_brief_items(&mut candidates);
    RecentSuppression {
        candidates,
        suppressed_recent,
    }
}
