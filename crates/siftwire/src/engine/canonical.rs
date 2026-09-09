use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use crate::domain::Source;

use super::google::{GoogleResolver, is_google_news_article_url};
use super::model::{FetchedItem, ResolveError, UnresolvedItem};

const MAX_ATTEMPTS: usize = 5;
const ITEM_TIMEOUT: Duration = Duration::from_secs(3);
const SKIPPED_REASON: &str = "url canonicalization skipped after 5-item source limit";

#[derive(Clone, Copy)]
enum Strategy {
    None,
    GoogleNews,
}

struct FeedItemResult {
    item: Option<FetchedItem>,
    unresolved: Option<UnresolvedItem>,
}

pub fn process_feed_items(
    google: &GoogleResolver,
    source: &Source,
    items: Vec<FetchedItem>,
) -> Result<(Vec<FetchedItem>, Vec<UnresolvedItem>, bool)> {
    let strategy = strategy(&source.url_canonicalization)?;
    if matches!(strategy, Strategy::None) {
        return Ok((process_local(source, items), Vec::new(), false));
    }
    let mut attempts = 0_usize;
    let mut truncated = false;
    let work = items
        .into_iter()
        .map(|item| {
            if !should_attempt(strategy, &item.url) {
                return (item, Strategy::None, false);
            }
            attempts = attempts.saturating_add(1);
            if attempts > MAX_ATTEMPTS {
                truncated = true;
                (item, Strategy::None, true)
            } else {
                (item, strategy, false)
            }
        })
        .collect::<Vec<_>>();
    let results = thread::scope(|scope| {
        let mut results = BTreeMap::new();
        let mut pending = Vec::new();
        for (position, (item, item_strategy, skipped)) in work.into_iter().enumerate() {
            if skipped || matches!(item_strategy, Strategy::None) {
                let result = process_one(google, source, item, item_strategy, skipped);
                let _previous = results.insert(position, result);
            } else {
                let handle =
                    scope.spawn(move || process_one(google, source, item, item_strategy, false));
                pending.push((position, handle));
            }
        }
        for (position, handle) in pending {
            let result = match handle.join() {
                Ok(result) => result,
                Err(_panic) => {
                    return Err(anyhow::anyhow!("url canonicalization worker panicked"));
                }
            };
            let _previous = results.insert(position, result);
        }
        Ok(results.into_values().collect::<Vec<_>>())
    })?;
    let mut processed = Vec::new();
    let mut unresolved = Vec::new();
    for result in results {
        if let Some(item) = result.item {
            processed.push(item);
        }
        if let Some(item) = result.unresolved {
            unresolved.push(item);
        }
    }
    Ok((processed, unresolved, truncated))
}

fn process_one(
    google: &GoogleResolver,
    source: &Source,
    original: FetchedItem,
    strategy: Strategy,
    skipped: bool,
) -> FeedItemResult {
    if skipped {
        return unresolved_only(&original, SKIPPED_REASON.to_owned());
    }
    if matches!(strategy, Strategy::None) {
        return FeedItemResult {
            item: Some(with_outlet(source, original)),
            unresolved: None,
        };
    }
    let now = Instant::now();
    let deadline = now.checked_add(ITEM_TIMEOUT).unwrap_or(now);
    match resolve(google, strategy, &original.url, deadline) {
        Ok(canonical_url) => resolved(source, original, canonical_url),
        Err(error) => failed(source, original, &error),
    }
}

fn resolve(
    google: &GoogleResolver,
    strategy: Strategy,
    url: &str,
    deadline: Instant,
) -> Result<String, ResolveError> {
    match strategy {
        Strategy::GoogleNews => google.resolve(url, deadline),
        Strategy::None => Ok(url.to_owned()),
    }
}

fn resolved(source: &Source, mut item: FetchedItem, canonical_url: String) -> FeedItemResult {
    let feed_identity = item.feed_identity().to_owned();
    item.feed_identity = feed_identity;
    if !canonical_url.is_empty() {
        if item.identity == item.url {
            item.identity.clone_from(&canonical_url);
        }
        item.url = canonical_url;
    }
    FeedItemResult {
        item: Some(with_outlet(source, item)),
        unresolved: None,
    }
}

fn failed(source: &Source, mut item: FetchedItem, error: &ResolveError) -> FeedItemResult {
    let unresolved = UnresolvedItem {
        disposition: crate::contract::ItemDisposition::Retained,
        title: item.title.clone(),
        url: item.url.clone(),
        reason: error.reason(),
    };
    let feed_identity = item.feed_identity().to_owned();
    item.feed_identity = feed_identity;
    FeedItemResult {
        item: Some(with_outlet(source, item)),
        unresolved: Some(unresolved),
    }
}

fn unresolved_only(item: &FetchedItem, reason: String) -> FeedItemResult {
    FeedItemResult {
        item: None,
        unresolved: Some(UnresolvedItem {
            disposition: crate::contract::ItemDisposition::Dropped,
            title: item.title.clone(),
            url: item.url.clone(),
            reason,
        }),
    }
}

fn process_local(source: &Source, items: Vec<FetchedItem>) -> Vec<FetchedItem> {
    items
        .into_iter()
        .map(|item| with_outlet(source, item))
        .collect()
}

fn with_outlet(source: &Source, mut item: FetchedItem) -> FetchedItem {
    let outlet = extract_outlet(source, &item);
    if !outlet.is_empty() {
        item.outlet = outlet;
    }
    item
}

fn extract_outlet(source: &Source, item: &FetchedItem) -> String {
    match source.outlet_extraction.as_str() {
        "title_suffix" => item
            .title
            .rsplit_once(" - ")
            .map_or_else(String::new, |(_title, outlet)| outlet.trim().to_owned()),
        _ => String::new(),
    }
}

fn strategy(value: &str) -> Result<Strategy> {
    match value {
        "" | "none" => Ok(Strategy::None),
        "google_news_article_url" => Ok(Strategy::GoogleNews),
        unsupported => bail!("unsupported url canonicalization strategy {unsupported:?}"),
    }
}

fn should_attempt(strategy: Strategy, value: &str) -> bool {
    match strategy {
        Strategy::GoogleNews => is_google_news_article_url(value),
        Strategy::None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{SKIPPED_REASON, failed, unresolved_only};
    use crate::contract::ItemDisposition;
    use crate::domain::Source;
    use crate::engine::model::{FetchedItem, ResolveError};

    #[test]
    fn failed_resolution_retains_the_item_but_skipped_resolution_drops_it() {
        let source = Source {
            outlet_extraction: "title_suffix".to_owned(),
            ..Source::default()
        };
        let item = FetchedItem {
            title: "Story - Publisher".to_owned(),
            url: "https://news.google.com/rss/articles/fixture".to_owned(),
            identity: "feed-id".to_owned(),
            ..FetchedItem::default()
        };
        let failure = failed(&source, item.clone(), &ResolveError::Timeout);
        let mut expected = item.clone();
        expected.feed_identity = "feed-id".to_owned();
        expected.outlet = "Publisher".to_owned();
        assert_eq!(
            failure.item,
            Some(expected),
            "failure changed the item identity or outlet"
        );
        assert_eq!(
            failure
                .unresolved
                .as_ref()
                .map(|item| (item.disposition, item.reason.as_str())),
            Some((ItemDisposition::Retained, "url canonicalization timed out")),
            "resolution failure must report the retained item"
        );
        let skipped = unresolved_only(&item, SKIPPED_REASON.to_owned());
        assert!(
            skipped.item.is_none(),
            "skipped item survived the source limit"
        );
        assert_eq!(
            skipped
                .unresolved
                .as_ref()
                .map(|item| (item.disposition, item.reason.as_str())),
            Some((
                ItemDisposition::Dropped,
                "url canonicalization skipped after 5-item source limit"
            )),
            "skipped item must report the source limit"
        );
    }
}
