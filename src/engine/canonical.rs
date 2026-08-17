use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use url::Url;

use crate::domain::Source;

use super::google::{GoogleResolver, is_google_news_article_url};
use super::http::HttpClient;
use super::model::{FetchedItem, ResolveError, UnresolvedItem};

const MAX_ATTEMPTS: usize = 5;
const ITEM_TIMEOUT: Duration = Duration::from_secs(3);
const SKIPPED_REASON: &str = "url canonicalization skipped after 5-item source limit";

#[derive(Clone, Copy)]
enum Strategy {
    None,
    FeedBurner,
    GoogleNews,
}

struct FeedItemResult {
    item: Option<FetchedItem>,
    unresolved: Option<UnresolvedItem>,
}

pub fn process_feed_items(
    client: &HttpClient,
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
                let result = process_one(client, google, source, item, item_strategy, skipped);
                let _previous = results.insert(position, result);
            } else {
                let handle = scope
                    .spawn(move || process_one(client, google, source, item, item_strategy, false));
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
    client: &HttpClient,
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
    match resolve(client, google, strategy, &original.url, deadline) {
        Ok(canonical_url) => resolved(source, original, canonical_url),
        Err(error) => failed(source, original, &error),
    }
}

fn resolve(
    client: &HttpClient,
    google: &GoogleResolver,
    strategy: Strategy,
    url: &str,
    deadline: Instant,
) -> Result<String, ResolveError> {
    match strategy {
        Strategy::FeedBurner => client.follow_redirect_until(url, deadline),
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
        title: item.title.clone(),
        url: item.url.clone(),
        reason: error.reason(),
    };
    if source.outlet_extraction == "url_host" {
        return FeedItemResult {
            item: None,
            unresolved: Some(unresolved),
        };
    }
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
        "url_host" => Url::parse(&item.url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_lowercase))
            .map_or_else(String::new, |host| {
                host.strip_prefix("www.").unwrap_or(&host).to_owned()
            }),
        "rss_source" => clean_text(&item.rss_source),
        _ => String::new(),
    }
}

fn strategy(value: &str) -> Result<Strategy> {
    match value {
        "" | "none" => Ok(Strategy::None),
        "feedburner_redirect" => Ok(Strategy::FeedBurner),
        "google_news_article_url" => Ok(Strategy::GoogleNews),
        unsupported => bail!("unsupported url canonicalization strategy {unsupported:?}"),
    }
}

fn should_attempt(strategy: Strategy, value: &str) -> bool {
    match strategy {
        Strategy::FeedBurner => !value.trim().is_empty(),
        Strategy::GoogleNews => is_google_news_article_url(value),
        Strategy::None => false,
    }
}

fn clean_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
