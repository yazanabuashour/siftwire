use std::cmp::Ordering;
use std::collections::BTreeSet;

use chrono::{DateTime, FixedOffset};
use url::Url;

use crate::contract::{BriefItem, SuppressedItem};
use crate::domain::{Reporting, SOURCE_KIND_SCHEDULE, Source};
use crate::storage::normalize_title_key;

#[derive(Clone, Debug)]
pub struct CollectedItem {
    pub source: Source,
    pub brief_item: BriefItem,
    pub fixture_identity: String,
}

#[derive(Clone, Debug, Default)]
pub struct ClassifiedItems {
    pub must_include: Vec<BriefItem>,
    pub candidates: Vec<CollectedItem>,
    pub suppressed: Vec<SuppressedItem>,
}

struct TopicIdentity {
    title_key: String,
    topic_key: String,
}

#[must_use]
pub fn classify_and_dedupe(items: Vec<CollectedItem>) -> ClassifiedItems {
    let mut must_include = Vec::new();
    let mut candidates = Vec::new();
    let mut fixture_duplicates = Vec::new();
    let mut fixture_keys = BTreeSet::new();
    for item in items {
        let reporting = item.source.reporting();
        if matches!(reporting, Reporting::Required | Reporting::Sports) {
            if item.source.kind == SOURCE_KIND_SCHEDULE
                && !fixture_keys.insert(item.fixture_identity.clone())
            {
                fixture_duplicates.push(suppressed_item(&item.brief_item, "duplicate_fixture"));
                continue;
            }
            must_include.push(item.brief_item);
        } else if reporting != Reporting::Observe {
            candidates.push(item);
        }
    }
    let (mut candidates, duplicate_suppressed) = collapse_duplicates(candidates);
    fixture_duplicates.extend(duplicate_suppressed);
    sort_brief_items(&mut must_include);
    sort_collected_items(&mut candidates);
    ClassifiedItems {
        must_include,
        candidates,
        suppressed: fixture_duplicates,
    }
}

pub fn sort_brief_items(items: &mut [BriefItem]) {
    items.sort_by(|left, right| {
        left.priority_rank
            .cmp(&right.priority_rank)
            .then_with(|| left.source_key.cmp(&right.source_key))
            .then_with(|| left.title.cmp(&right.title))
    });
}

pub fn sort_collected_items(items: &mut [CollectedItem]) {
    items.sort_by(compare_preference);
}

fn collapse_duplicates(items: Vec<CollectedItem>) -> (Vec<CollectedItem>, Vec<SuppressedItem>) {
    let mut representatives: Vec<CollectedItem> = Vec::new();
    let mut suppressed = Vec::new();
    for item in items {
        let duplicate = representatives.iter_mut().find(|existing| {
            duplicate_urls(&item, existing)
                || (effective_group(&item.source) == effective_group(&existing.source)
                    && duplicate_titles(&item.brief_item.title, &existing.brief_item.title))
        });
        let Some(existing) = duplicate else {
            representatives.push(item);
            continue;
        };
        if compare_preference(&item, existing).is_lt() {
            suppressed.push(suppressed_item(&existing.brief_item, "same_run_duplicate"));
            *existing = item;
        } else {
            suppressed.push(suppressed_item(&item.brief_item, "same_run_duplicate"));
        }
    }
    (representatives, suppressed)
}

fn duplicate_urls(left: &CollectedItem, right: &CollectedItem) -> bool {
    !left.brief_item.url.is_empty()
        && left.brief_item.url == right.brief_item.url
        && (left.source.key == right.source.key || Url::parse(&left.brief_item.url).is_ok())
}

fn effective_group(source: &Source) -> &str {
    if source.dedup_group.is_empty() {
        &source.key
    } else {
        &source.dedup_group
    }
}

fn duplicate_titles(left: &str, right: &str) -> bool {
    let left = topic_identity(left);
    let right = topic_identity(right);
    if left.title_key.is_empty() || right.title_key.is_empty() {
        return false;
    }
    if left.title_key == right.title_key {
        return true;
    }
    let left_tokens = left.topic_key.split_whitespace().collect::<Vec<_>>();
    let right_tokens = right.topic_key.split_whitespace().collect::<Vec<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }
    let right_set = right_tokens.iter().copied().collect::<BTreeSet<_>>();
    let overlap = left_tokens
        .iter()
        .filter(|token| right_set.contains(**token))
        .count();
    overlap >= 4
        && overlap
            .checked_mul(10)
            .zip(right_tokens.len().checked_mul(7))
            .is_some_and(|(numerator, denominator)| numerator >= denominator)
}

fn topic_identity(title: &str) -> TopicIdentity {
    let title_key = normalize_title_key(title);
    TopicIdentity {
        topic_key: build_topic_key(&title_key),
        title_key,
    }
}

fn build_topic_key(title_key: &str) -> String {
    let mut normalized = title_key.to_owned();
    for prefix in [
        "live updates ",
        "first thing ",
        "photos ",
        "analysis ",
        "report ",
        "watch ",
    ] {
        if let Some(without_prefix) = normalized.strip_prefix(prefix) {
            normalized = without_prefix.to_owned();
        }
    }
    for marker in [
        " as ",
        " after ",
        " amid ",
        " over ",
        " following ",
        " because ",
    ] {
        if let Some(position) = normalized.find(marker) {
            normalized.truncate(position);
            normalized = normalized.trim().to_owned();
            break;
        }
    }
    topic_tokens(&normalized).join(" ")
}

fn topic_tokens(value: &str) -> Vec<String> {
    let mut raw = value.split_whitespace().peekable();
    let mut tokens = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(token) = raw.next() {
        let token = if token == "playstation" && raw.peek() == Some(&"5") {
            let _five = raw.next();
            "ps5"
        } else {
            normalize_topic_token(token)
        };
        if token.is_empty() || is_stop_word(token) || !seen.insert(token) {
            continue;
        }
        tokens.push(token.to_owned());
        if tokens.len() == 6 {
            break;
        }
    }
    tokens
}

fn normalize_topic_token(token: &str) -> &str {
    match token {
        "prices" | "price" | "raise" | "raises" | "raising" | "raised" | "hike" | "hikes"
        | "hiked" => "price",
        _ => token,
    }
}

fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "as"
            | "at"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "of"
            | "on"
            | "or"
            | "the"
            | "to"
            | "with"
            | "says"
            | "will"
            | "live"
            | "update"
            | "updates"
            | "report"
            | "reportedly"
            | "analysis"
            | "watch"
            | "first"
            | "photos"
    )
}

fn compare_preference(left: &CollectedItem, right: &CollectedItem) -> Ordering {
    left.source
        .priority_rank
        .cmp(&right.source.priority_rank)
        .then_with(|| {
            compare_dates(
                &left.brief_item.published_at,
                &right.brief_item.published_at,
            )
        })
        .then_with(|| left.source.key.cmp(&right.source.key))
        .then_with(|| left.brief_item.title.cmp(&right.brief_item.title))
}

fn compare_dates(left: &str, right: &str) -> Ordering {
    match (parse_item_time(left), parse_item_time(right)) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn parse_item_time(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value)
        .or_else(|_error| DateTime::parse_from_rfc2822(value))
        .ok()
}

pub fn suppressed_item(item: &BriefItem, reason: &str) -> SuppressedItem {
    SuppressedItem {
        source_key: item.source_key.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
        reason: reason.to_owned(),
    }
}

pub fn titles_match(left: &str, right: &str) -> bool {
    duplicate_titles(left, right)
}
