use std::cmp::Ordering;

use url::Url;

use crate::contract::{BriefItem, SuppressedItem};
use crate::domain::{Reporting, Source};
use crate::storage::normalize_title_key;

use super::selection::parse_item_time;

#[derive(Clone, Debug)]
pub struct CollectedItem {
    pub source: Source,
    pub brief_item: BriefItem,
}

#[derive(Clone, Debug, Default)]
pub struct ClassifiedItems {
    pub must_include: Vec<BriefItem>,
    pub candidates: Vec<CollectedItem>,
    pub suppressed: Vec<SuppressedItem>,
}

#[must_use]
pub fn classify_and_dedupe(items: Vec<CollectedItem>) -> ClassifiedItems {
    let mut must_include = Vec::new();
    let mut candidates = Vec::new();
    for item in items {
        match item.source.reporting() {
            Reporting::Required => must_include.push(item.brief_item),
            Reporting::Sports => {}
            Reporting::Major | Reporting::Highlights => candidates.push(item),
        }
    }
    let (mut candidates, suppressed) = collapse_duplicates(candidates);
    sort_brief_items(&mut must_include);
    sort_collected_items(&mut candidates);
    ClassifiedItems {
        must_include,
        candidates,
        suppressed,
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

fn collapse_duplicates(mut items: Vec<CollectedItem>) -> (Vec<CollectedItem>, Vec<SuppressedItem>) {
    items.sort_by(|left, right| {
        compare_dates(
            &left.brief_item.published_at,
            &right.brief_item.published_at,
        )
        .then_with(|| compare_preference(left, right))
    });
    let mut representatives: Vec<CollectedItem> = Vec::new();
    let mut suppressed = Vec::new();
    for item in items {
        let duplicate = representatives.iter().any(|existing| {
            duplicate_urls(&item, existing)
                || (effective_group(&item.source) == effective_group(&existing.source)
                    && titles_match(&item.brief_item.title, &existing.brief_item.title))
        });
        if duplicate {
            suppressed.push(suppressed_item(&item.brief_item, "same_run_duplicate"));
        } else {
            representatives.push(item);
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

pub fn suppressed_item(item: &BriefItem, reason: &str) -> SuppressedItem {
    SuppressedItem {
        source_key: item.source_key.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
        reason: reason.to_owned(),
    }
}

pub fn titles_match(left: &str, right: &str) -> bool {
    let left = normalize_title_key(left);
    !left.is_empty() && left == normalize_title_key(right)
}
