#![expect(
    clippy::panic_in_result_fn,
    reason = "behavior tests return Result for fixture decoding while assertions report contract failures"
)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};

use crate::contract::HealthDelta;
use crate::domain::{OutletPolicy, Source};
use crate::storage::{FetchLog, SourceState, StoredSentItem};

use super::canonical::process_feed_items;
use super::dedup::classify_and_dedupe;
use super::feed::parse_feed;
use super::google::GoogleResolver;
use super::health::{add_recurring_failure_warnings, build_health_footnote};
use super::http::HttpClient;
use super::model::FetchedItem;
use super::policy::apply_outlet_policies;
use super::selection::select_new_items;
use super::{CollectedItem, collect_new_items, suppress_recent_candidates};

fn fetched(title: &str, url: &str, identity: &str) -> FetchedItem {
    FetchedItem {
        title: title.to_owned(),
        url: url.to_owned(),
        identity: identity.to_owned(),
        feed_identity: identity.to_owned(),
        ..FetchedItem::default()
    }
}

fn source(key: &str) -> Source {
    Source {
        key: key.to_owned(),
        label: key.to_owned(),
        kind: "rss".to_owned(),
        section: "technology".to_owned(),
        threshold: "medium".to_owned(),
        enabled: true,
        ..Source::default()
    }
}

fn collected(source: &Source, title: &str, url: &str) -> Result<CollectedItem> {
    collect_new_items(source, &[fetched(title, url, url)])
        .into_iter()
        .next()
        .context("collected fixture item")
}

fn process_local(source: &Source, item: FetchedItem) -> Result<FetchedItem> {
    let google = GoogleResolver::new(HttpClient::new());
    let (items, unresolved, truncated) = process_feed_items(&google, source, vec![item])?;
    assert!(
        unresolved.is_empty(),
        "local processing produced unresolved items"
    );
    assert!(!truncated, "one local item was marked truncated");
    items.into_iter().next().context("processed local item")
}

#[test]
fn rss_and_atom_parsing_preserve_identity_and_raw_dates() -> Result<()> {
    let rss = parse_feed(
        br#"<?xml version="1.0"?><rss version="2.0"><channel>
        <item><title><![CDATA[ RSS  Story ]]></title><link>https://example.test/rss</link>
        <guid>rss-guid</guid><pubDate>Thu, 23 Apr 2026 01:00:00 G&#77;&#x54;</pubDate>
        <source>Outlet &#38; Co &#x1F680; &amp; Partners</source></item></channel></rss>"#,
    )?;
    let rss_item = rss.first().context("RSS item")?;
    assert_eq!(rss_item.title, "RSS Story", "RSS title cleaning changed");
    assert_eq!(rss_item.identity, "rss-guid", "RSS identity changed");
    assert_eq!(
        rss_item.published_at, "Thu, 23 Apr 2026 01:00:00 GMT",
        "RSS raw publication date changed"
    );
    assert!(
        rss_item.outlet.is_empty(),
        "RSS source metadata became an outlet"
    );

    let atom = parse_feed(
        br#"<?xml version="1.0"?><feed xmlns="http://www.w3.org/2005/Atom">
        <title>Fixture</title><id>tag:example.test,2026:feed</id><updated>2026-04-23T00:00:00Z</updated>
        <entry><title>Atom Story</title><id>tag:example.test,2026:atom</id>
        <updated>2026-04-23T01:02:03Z</updated><link rel="alternate" href="https://example.test/atom" />
        </entry></feed>"#,
    )?;
    let atom_item = atom.first().context("Atom item")?;
    assert_eq!(
        atom_item.url, "https://example.test/atom",
        "Atom link selection changed"
    );
    assert_eq!(
        atom_item.identity, "tag:example.test,2026:atom",
        "Atom identity changed"
    );
    assert_eq!(
        atom_item.published_at, "2026-04-23T01:02:03Z",
        "Atom update changed"
    );

    let invalid = parse_feed(br#"<?xml version="1.0"?><error><message>bad</message></error>"#);
    let Err(error) = invalid else {
        bail!("non-feed XML was accepted");
    };
    assert!(
        error.to_string().contains("parse feed XML"),
        "feed diagnostic changed: {error}"
    );
    Ok(())
}

#[test]
fn latest_seen_selection_matches_canonical_and_feed_identities() {
    let mut prior = fetched("Prior", "https://publisher.test/prior", "canonical-prior");
    prior.feed_identity = "feed-prior".to_owned();
    let items = vec![
        fetched("Newest", "https://publisher.test/new", "canonical-new"),
        prior,
        fetched("Older", "https://publisher.test/old", "canonical-old"),
    ];

    let feed_state = SourceState {
        latest_identity: "feed-prior".to_owned(),
        ..SourceState::default()
    };
    let selected = select_new_items(&items, Some(&feed_state), false);
    assert_eq!(selected.len(), 1, "feed identity did not stop selection");
    assert_eq!(
        selected.first().map(|item| item.title.as_str()),
        Some("Newest"),
        "wrong new item"
    );

    let canonical_state = SourceState {
        latest_identity: "stale-canonical".to_owned(),
        latest_feed_identity: "canonical-prior".to_owned(),
        ..SourceState::default()
    };
    assert_eq!(
        select_new_items(&items, Some(&canonical_state), false).len(),
        1,
        "canonical identity did not match latest feed state"
    );

    let missing_state = SourceState {
        latest_identity: "outside-window".to_owned(),
        ..SourceState::default()
    };
    assert_eq!(
        select_new_items(&items, Some(&missing_state), false).len(),
        1,
        "unbounded missing state should keep only the latest item"
    );
    assert_eq!(
        select_new_items(&items, Some(&missing_state), true).len(),
        items.len(),
        "bounded canonicalization should keep every processed item before unseen state"
    );
}

#[test]
fn outlet_extraction_and_policy_matching_work_together() -> Result<()> {
    let mut title_source = source("title");
    title_source.outlet_extraction = "title_suffix".to_owned();
    let title_item = process_local(
        &title_source,
        fetched(
            "Story - Blocked Outlet",
            "https://story.test/title",
            "title",
        ),
    )?;

    let alias_item = process_local(
        &title_source,
        fetched("Story - example.com", "https://story.test/alias", "alias"),
    )?;
    let watched_item = process_local(
        &title_source,
        fetched("Story - Watch Outlet", "https://story.test/watch", "watch"),
    )?;

    let policies = vec![
        OutletPolicy {
            name: "Blocked Outlet.com".to_owned(),
            policy: "block".to_owned(),
            enabled: true,
            ..OutletPolicy::default()
        },
        OutletPolicy {
            name: "Publisher".to_owned(),
            aliases: vec!["example.com".to_owned()],
            policy: "block".to_owned(),
            enabled: true,
            ..OutletPolicy::default()
        },
        OutletPolicy {
            name: "Watch Outlet".to_owned(),
            policy: "watch".to_owned(),
            enabled: true,
            ..OutletPolicy::default()
        },
    ];
    let result = apply_outlet_policies(
        &source("combined"),
        vec![title_item, alias_item, watched_item],
        &policies,
    );
    assert_eq!(
        result.audit.len(),
        3,
        "not every extracted outlet was audited"
    );
    assert_eq!(result.items.len(), 1, "block/watch policy behavior changed");
    assert_eq!(
        result.items.first().map(|item| item.outlet.as_str()),
        Some("Watch Outlet"),
        "watch policy removed its item"
    );
    Ok(())
}

#[test]
fn same_run_exact_url_dedupes_different_titles() -> Result<()> {
    let feed = source("hnrss");
    let url = "https://www.githubstatus.com/incidents/zkxwbgr0cnmx";
    let mut current = collected(&feed, "Incident with Github.com", url)?;
    current.brief_item.published_at = "Mon, 17 Aug 2026 13:40:55 +0000".to_owned();
    let mut resolved = collected(&feed, "Incident with Github.com [resolved]", url)?;
    resolved.brief_item.published_at = "Mon, 17 Aug 2026 13:35:06 +0000".to_owned();

    let classified = classify_and_dedupe(vec![resolved, current]);

    assert_eq!(
        classified
            .candidates
            .iter()
            .map(|item| item.brief_item.title.as_str())
            .collect::<Vec<_>>(),
        ["Incident with Github.com"],
        "exact-URL dedup did not keep only the preferred item"
    );
    Ok(())
}

#[test]
fn same_run_relative_urls_do_not_cross_source_boundaries() -> Result<()> {
    let first = source("first");
    let second = source("second");

    let classified = classify_and_dedupe(vec![
        collected(&first, "First status", "/status")?,
        collected(&second, "Second status", "/status")?,
    ]);

    assert_eq!(
        classified.candidates.len(),
        2,
        "relative URLs crossed source boundaries"
    );
    Ok(())
}

#[test]
fn same_run_preference_flows_into_recent_topic_suppression() -> Result<()> {
    let title = "Sony raises PlayStation 5 prices in US";
    let mut generic = source("generic");
    generic.dedup_group = "news".to_owned();
    generic.priority_rank = 5;
    let mut preferred = source("preferred");
    preferred.dedup_group = "news".to_owned();
    preferred.priority_rank = 1;
    let independent = source("independent");

    let classified = classify_and_dedupe(vec![
        collected(&generic, title, "https://generic.test/story")?,
        collected(&preferred, title, "https://preferred.test/story")?,
        collected(&independent, title, "https://independent.test/story")?,
    ]);
    assert_eq!(
        classified.candidates.len(),
        2,
        "dedup groups collapsed incorrectly"
    );
    assert!(
        classified
            .candidates
            .iter()
            .any(|item| item.source.key == "preferred"),
        "same-run dedup did not keep the preferred source"
    );
    assert!(
        classified
            .candidates
            .iter()
            .all(|item| item.source.key != "generic"),
        "same-run dedup retained the lower-priority source"
    );
    assert_eq!(
        classified.suppressed.len(),
        1,
        "same-run suppression audit changed"
    );

    let sent_at = DateTime::parse_from_rfc3339("2026-04-23T00:00:00Z")?.with_timezone(&Utc);
    let recent = [StoredSentItem {
        title: title.to_owned(),
        url: "https://previous.test/story".to_owned(),
        kind: String::new(),
        sent_at,
    }];
    let suppressed = suppress_recent_candidates(classified.candidates, &recent);
    assert!(
        suppressed.candidates.is_empty(),
        "recent title dedup left candidates"
    );
    assert_eq!(
        suppressed.suppressed_recent.len(),
        2,
        "recent suppression audit changed"
    );
    assert!(
        suppressed
            .suppressed
            .iter()
            .all(|item| item.reason == "recently_sent"),
        "recent compatibility reason changed"
    );
    Ok(())
}

#[test]
fn health_footnotes_and_recurring_failures_report_only_durable_changes() {
    let footnote = build_health_footnote(&HealthDelta {
        new_warnings: vec![
            "Feed `new` failed this run (new error)".to_owned(),
            "Feed `flap` failed this run (again)".to_owned(),
        ],
        resolved_warnings: vec![
            "Feed `old` failed this run (old error)".to_owned(),
            "Feed `flap` failed this run (before)".to_owned(),
        ],
    });
    assert_eq!(
        footnote,
        "Feed health changes - NEW: new | RESOLVED: old | FLAPPED (new+resolved this run): flap.",
        "health footnote categories changed"
    );

    let logs = vec![
        fetch_log("broken", "ok", ""),
        fetch_log("broken", "error", "first"),
        fetch_log("broken", "error", "second"),
        fetch_log("broken", "error", "latest"),
        fetch_log("retired", "error", "one"),
        fetch_log("retired", "error", "two"),
        fetch_log("retired", "error", "three"),
    ];
    let mut warnings = BTreeMap::new();
    add_recurring_failure_warnings(&logs, &BTreeSet::from(["broken".to_owned()]), &mut warnings);
    assert_eq!(warnings.len(), 1, "recurring warning scope changed");
    assert_eq!(
        warnings.get("feed-recurring:broken").map(String::as_str),
        Some("Feed `broken` has failed 3+ consecutive runs (last error: latest)"),
        "recurring warning content changed"
    );
}

fn fetch_log(source_key: &str, status: &str, error: &str) -> FetchLog {
    FetchLog {
        source_key: source_key.to_owned(),
        status: status.to_owned(),
        error: error.to_owned(),
        ..FetchLog::default()
    }
}
