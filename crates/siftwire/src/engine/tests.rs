#![expect(
    clippy::panic_in_result_fn,
    reason = "behavior tests return Result for fixture decoding while assertions report contract failures"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};

use crate::contract::{CurrentNewsStatus, HealthDelta};
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
use super::process::collect_items;
use super::selection::select_new_items;
use super::{CollectedItem, FetchOutput, process_source, suppress_recent_candidates};

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
    collect_items(source, &[fetched(title, url, url)])
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
    let required = Source {
        threshold: "always".to_owned(),
        ..source("required")
    };
    let rss = parse_feed(
        &required,
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
        &required,
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

    let rdf = parse_feed(
        &source("rdf"),
        br#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:r="http://purl.org/rss/1.0/" xmlns:d="http://purl.org/dc/elements/1.1/">
        <r:item><r:title>Linkless</r:title><d:date>2026-04-20T00:00:00Z</d:date></r:item>
        <r:item><r:title>Empty link</r:title><r:link/><d:date>2026-04-20T00:00:00Z</d:date></r:item>
        <r:item><r:title>Nested link</r:title><r:link><r:link>https://example.test/fake</r:link></r:link><d:date>2026-04-20T00:00:00Z</d:date></r:item>
        <r:item><r:link>https://example.test/no-title</r:link><d:date>2026-04-20T00:00:00Z</d:date></r:item>
        <r:item><r:title>RDF Story</r:title><r:link><!-- comment -->https://example.test/rdf</r:link><d:date>2026-04-23T01:00:00Z</d:date></r:item>
        <r:item><r:title>RDF undated</r:title><r:link>https://example.test/undated<r:ignored/></r:link><r:description><d:date>2026-04-23T01:00:00Z</d:date></r:description></r:item>
        </rdf:RDF>"#,
    )?;
    assert_eq!(
        rdf.iter()
            .map(|item| (item.identity.as_str(), item.published_at.as_str()))
            .collect::<Vec<_>>(),
        [
            ("https://example.test/rdf", "2026-04-23T01:00:00Z"),
            ("https://example.test/undated", "")
        ],
        "RSS1 linkless or untitled entries shifted publication onto another identity"
    );

    let invalid = parse_feed(
        &required,
        br#"<?xml version="1.0"?><error><message>bad</message></error>"#,
    );
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

    let mut same_title = current.clone();
    same_title.brief_item.url = "https://example.test/other-url".to_owned();
    same_title.brief_item.published_at = "Mon, 17 Aug 2026 13:36:00 +0000".to_owned();
    let other_feed = Source {
        priority_rank: i64::MIN,
        ..source("other-default-group")
    };
    let mut cross_source = collected(&other_feed, "Syndicated incident", url)?;
    cross_source.brief_item.published_at = "Mon, 17 Aug 2026 13:39:00 +0000".to_owned();
    let classified = classify_and_dedupe(vec![resolved, same_title, cross_source, current]);

    assert_eq!(
        classified
            .candidates
            .iter()
            .map(|item| item.brief_item.title.as_str())
            .collect::<Vec<_>>(),
        ["Incident with Github.com"],
        "exact-URL dedup across default source groups did not keep the newest item"
    );
    assert_eq!(
        classified
            .candidates
            .first()
            .map(|item| item.source.key.as_str()),
        Some("hnrss"),
        "cross-source URL dedup changed the representative's source identity"
    );
    assert_eq!(
        classified.suppressed.len(),
        3,
        "URL duplicates were not audited"
    );
    Ok(())
}

#[test]
fn same_run_relative_urls_do_not_cross_source_boundaries() -> Result<()> {
    let mut first = source("first");
    first.dedup_group = "shared".to_owned();
    let second = Source {
        dedup_group: "shared".to_owned(),
        ..source("second")
    };

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
fn same_run_preference_flows_into_recent_exact_headline_suppression() -> Result<()> {
    let title = "Sony raises PlayStation 5 prices in US";
    let mut generic = source("generic");
    generic.dedup_group = "news".to_owned();
    generic.priority_rank = i64::MAX;
    let mut preferred = source("preferred");
    preferred.dedup_group = "news".to_owned();
    preferred.priority_rank = i64::MIN;
    let independent = source("independent");
    let mut older = collected(&preferred, title, "https://preferred.test/older")?;
    older.brief_item.published_at = "2026-04-22T23:00:00Z".to_owned();
    let mut newer = collected(&generic, title, "https://generic.test/newer")?;
    newer.brief_item.published_at = "2026-04-23T00:00:00Z".to_owned();
    let mut tie = newer.clone();
    tie.source = preferred.clone();
    tie.brief_item.source_key = preferred.key.clone();
    tie.brief_item.priority_rank = preferred.priority_rank;
    tie.brief_item.url = "https://preferred.test/newer".to_owned();

    let date_winner = classify_and_dedupe(vec![older.clone(), newer.clone()]);
    assert_eq!(
        date_winner
            .candidates
            .first()
            .map(|item| item.source.key.as_str()),
        Some("generic"),
        "priority overrode newer publication"
    );
    let classified = classify_and_dedupe(vec![
        older,
        newer,
        tie,
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
    assert_eq!(
        classified.suppressed.len(),
        2,
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
        suppressed.suppressed_recent.iter().all(|item| {
            item.matched_prior_title == title && item.prior_sent_at == "2026-04-23T00:00:00Z"
        }),
        "recent suppression lost prior delivery evidence"
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

#[test]
fn exact_dedup_keeps_distinct_developments_and_required_items() -> Result<()> {
    let feed = source("news");
    let required = Source {
        threshold: "always".to_owned(),
        ..feed.clone()
    };
    let sports = Source {
        kind: crate::domain::SOURCE_KIND_SCHEDULE.to_owned(),
        ..feed.clone()
    };
    let title = "Harbor bridge closes after inspectors find cracks";
    let changed = "Harbor bridge closes after inspectors find explosives";
    let reused_url = "https://example.test/live";
    let classified = classify_and_dedupe(vec![
        collected(&feed, title, "https://example.test/first")?,
        collected(&feed, changed, reused_url)?,
        collected(
            &feed,
            "Harbor bridge closes amid evacuation",
            "https://example.test/evacuation",
        )?,
        collected(&feed, "!!!", "https://example.test/empty-one")?,
        collected(&feed, "???", "https://example.test/empty-two")?,
        collected(&required, title, "https://example.test/first")?,
        collected(&required, title, "https://example.test/first")?,
        collected(&sports, "Upcoming fixture", "https://example.test/fixture")?,
    ]);
    assert_eq!(
        classified.candidates.len(),
        5,
        "fuzzy or empty-headline matching collapsed distinct items"
    );
    assert_eq!(
        classified.must_include.len(),
        2,
        "Required items were deduplicated or Sports entered ordinary items"
    );
    let recent = [StoredSentItem {
        title: title.to_uppercase(),
        url: reused_url.to_owned(),
        kind: String::new(),
        sent_at: DateTime::parse_from_rfc3339("2026-04-23T00:00:00Z")?.with_timezone(&Utc),
    }];
    let result = suppress_recent_candidates(classified.candidates, &recent);
    assert_eq!(
        result.suppressed_recent.len(),
        1,
        "only the exact normalized headline should repeat"
    );
    assert!(
        result.candidates.iter().any(|item| item.title == changed),
        "reused URL hid a changed headline"
    );
    assert_eq!(
        result.candidates.len(),
        4,
        "recent suppression used fuzzy matching"
    );
    Ok(())
}

fn current_news_feed() -> Result<(Source, FetchOutput)> {
    let feed = Source {
        url_canonicalization: "google_news_article_url".to_owned(),
        outlet_extraction: "title_suffix".to_owned(),
        threshold: "high".to_owned(),
        ..source("news")
    };
    let dates = [
        "2026-04-23T01:00:00Z",
        "2026-04-22T11:59:59Z",
        "",
        "invalid",
        "2026-04-23T12:00:01Z",
        "2026-04-22T12:00:00Z",
        "2026-04-23T12:00:00Z",
        "2026-04-23T10:00:00Z",
        "Thu, 23 Apr 2026 14:00:00 +0200",
    ];
    let mut xml_items = String::new();
    for (index, date) in dates.iter().enumerate() {
        write!(
            xml_items,
            "<item><title>Story {index} - Publisher</title><guid>id-{index}</guid><link>https://news.google.com/rss/articles/synthetic-{index}</link><pubDate>{date}</pubDate></item>"
        )?;
    }
    let xml = format!(
        r#"<rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:fake="urn:fake"><channel>
        <!-- <item><pubDate>2026-04-23T10:00:00Z</pubDate></item> -->
        <![CDATA[<item><pubDate>2026-04-23T10:00:00Z</pubDate></item>]]>
        <fake:item><pubDate>2026-04-23T10:00:00Z</pubDate></fake:item>
        <description><item><pubDate>2026-04-23T10:00:00Z</pubDate></item></description>
        <item><pubDate>2026-04-20T00:00:00Z</pubDate></item>
        <item><title/><pubDate>2026-04-20T00:00:00Z</pubDate></item>
        {xml_items}
        <item><title>Parsed publication - Publisher</title><guid>id-dc</guid><link>https://news.google.com/rss/articles/synthetic-dc</link><!-- <pubDate>invalid</pubDate> --><dc:date>2026-04-23T11:00:00Z</dc:date></item>
        <item><title>Comment date - Publisher</title><!-- <pubDate>2026-04-23T11:00:00Z</pubDate> --></item>
        <item><title>CDATA date - Publisher</title><description><![CDATA[<pubDate>2026-04-23T11:00:00Z</pubDate>]]></description></item>
        <item><title>Nested date - Publisher</title><description><pubDate>2026-04-23T11:00:00Z</pubDate></description></item>
        <item><title>Foreign dates - Publisher</title><fake:pubDate>2026-04-23T11:00:00Z</fake:pubDate><fake:date>2026-04-23T11:00:00Z</fake:date></item>
        <item><title>Invalid primary - Publisher</title><pubDate>invalid</pubDate><dc:date>2026-04-23T11:00:00Z</dc:date></item>
        <item><title>Empty primary - Publisher</title><pubDate/><dc:date>2026-04-23T11:00:00Z</dc:date></item>
        <item><title>Nested primary - Publisher</title><pubDate><fake:date>2026-04-23T11:00:00Z</fake:date></pubDate><dc:date>2026-04-23T11:00:00Z</dc:date></item>
        <item><title>Nested fallback - Publisher</title><description><dc:date>2026-04-23T11:00:00Z</dc:date></description></item>
        </channel></rss>"#
    );
    let parsed = parse_feed(&feed, xml.as_bytes())?;
    let google = GoogleResolver::new(HttpClient::new());
    let (items, unresolved, truncated) = process_feed_items(&google, &feed, parsed.clone())?;
    assert_eq!(
        items.len(),
        18,
        "optional feed hit the resolver's five-item limit"
    );
    assert!(unresolved.is_empty(), "optional feed attempted resolution");
    assert!(!truncated, "optional feed was truncated");
    for (actual, original) in items.iter().zip(&parsed) {
        assert_eq!(actual.url, original.url, "optional URL changed");
        assert_eq!(
            actual.identity, original.identity,
            "optional identity changed"
        );
        assert_eq!(
            actual.outlet, "Publisher",
            "title-suffix extraction was lost"
        );
    }
    Ok((
        feed,
        FetchOutput {
            items,
            ..FetchOutput::default()
        },
    ))
}

#[test]
fn current_news_uses_the_whole_feed_and_dates_not_markers_or_order() -> Result<()> {
    let (mut feed, output) = current_news_feed()?;
    let now = DateTime::parse_from_rfc3339("2026-04-23T12:00:00Z")?.with_timezone(&Utc);
    let expected = CurrentNewsStatus {
        since: "2026-04-22T12:00:00Z".to_owned(),
        until: "2026-04-23T12:00:00Z".to_owned(),
        eligible_items: 6,
        stale_items: 1,
        undated_items: 10,
        future_items: 1,
    };
    let state = SourceState {
        latest_identity: "id-0".to_owned(),
        ..SourceState::default()
    };
    let fresh = process_source(&feed, Ok(output.clone()), &[], None, now);
    let mut reordered = output.clone();
    reordered.items.reverse();
    feed.threshold = "medium".to_owned();
    let repeated = process_source(&feed, Ok(reordered), &[], Some(&state), now);
    assert!(
        fresh.next_state.is_none(),
        "optional source proposed a marker"
    );
    assert!(
        repeated.next_state.is_none(),
        "optional source advanced a marker"
    );
    assert_eq!(fresh.status.items, 18, "fetch count must precede selection");
    assert_eq!(
        fresh.status.new_items, None,
        "current news is not marker-new"
    );
    assert!(fresh.warnings.contains_key("news_dates:news"));
    assert_eq!(
        serde_json::to_value(&fresh.status.current_news)?,
        serde_json::to_value(&expected)?,
        "publication classifications changed"
    );
    assert_eq!(
        serde_json::to_value(&repeated.status.current_news)?,
        serde_json::to_value(&fresh.status.current_news)?,
        "marker or order changed counts"
    );
    let urls = |items: &[CollectedItem]| {
        items
            .iter()
            .map(|item| item.brief_item.url.clone())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        urls(&fresh.collected),
        BTreeSet::from(
            ["0", "5", "6", "7", "8", "dc"]
                .map(|id| { format!("https://news.google.com/rss/articles/synthetic-{id}") })
        ),
        "boundary or beyond-five item lost"
    );
    assert_eq!(
        urls(&fresh.collected),
        urls(&repeated.collected),
        "marker or order changed candidates"
    );
    let blocked = process_source(
        &feed,
        Ok(output),
        &[OutletPolicy {
            name: "Publisher".to_owned(),
            policy: "block".to_owned(),
            enabled: true,
            ..OutletPolicy::default()
        }],
        Some(&state),
        now,
    );
    assert_eq!(
        serde_json::to_value(&blocked.status.current_news)?,
        serde_json::to_value(&expected)?,
        "outlet policy changed date accounting"
    );
    assert!(
        blocked.collected.is_empty(),
        "blocked outlet survived date eligibility"
    );
    assert_eq!(blocked.status.suppressed_policy, 6);
    assert_eq!(blocked.status.new_items, None);
    assert!(blocked.next_state.is_none());
    Ok(())
}

#[test]
fn current_news_atom_requires_publication_while_required_keeps_updated() -> Result<()> {
    let xml = br#"<feed xmlns="http://www.w3.org/2005/Atom">
        <title>Fixture</title><id>fixture</id><updated>2026-04-23T12:00:00Z</updated>
        <entry><title>Old publication</title><id>old</id><published>2026-04-20T00:00:00Z</published><updated>2026-04-23T12:00:00Z</updated></entry>
        <entry><title>Update only</title><id>updated</id><updated>2026-04-23T12:00:00Z</updated></entry>
        <entry><title>Invalid publication</title><id>invalid</id><published>invalid</published><updated>2026-04-23T12:00:00Z</updated></entry>
        <!-- <entry><published>2026-04-23T11:00:00Z</published></entry> -->
        <![CDATA[<entry><published>2026-04-23T11:00:00Z</published></entry>]]>
        <entry xmlns="urn:fake"><published>2026-04-23T11:00:00Z</published></entry>
        <content><entry><published>2026-04-23T11:00:00Z</published></entry></content>
        <entry><published>2026-04-20T00:00:00Z</published></entry>
        <entry><title/><published>2026-04-20T00:00:00Z</published></entry>
        <entry><title>Publication only</title><id>published</id><!-- <published>invalid</published> --><p:published xmlns:p="http://www.w3.org/2005/Atom"><![CDATA[2026-04-23T11:00:00Z]]></p:published></entry>
        <entry><title>No dates</title><id>missing</id></entry>
        <entry><title>Comment date</title><!-- <published>2026-04-23T11:00:00Z</published> --></entry>
        <entry><title>CDATA date</title><content><![CDATA[<published>2026-04-23T11:00:00Z</published>]]></content></entry>
        <entry><title>Nested date</title><content><published>2026-04-23T11:00:00Z</published></content></entry>
        <entry><title>Foreign date</title><published xmlns="urn:fake">2026-04-23T11:00:00Z</published><unknown:published>2026-04-23T11:00:00Z</unknown:published></entry>
        <entry><title>Unbound date</title><published xmlns="">2026-04-23T11:00:00Z</published></entry>
        <entry><title>Empty primary</title><published/><pubDate>2026-04-23T11:00:00Z</pubDate><updated>2026-04-23T12:00:00Z</updated></entry>
        <entry><title>Invalid primary</title><published>invalid</published><pubDate>2026-04-23T11:00:00Z</pubDate></entry>
        <unknown:entry><title>Undeclared entry</title><published>2026-04-23T11:00:00Z</published></unknown:entry>
        <entry><title>Nested primary</title><published><date>2026-04-23T11:00:00Z</date></published></entry>
        </feed>"#;
    let feed = source("atom");
    let now = DateTime::parse_from_rfc3339("2026-04-23T12:00:00Z")?.with_timezone(&Utc);
    let processed = process_source(
        &feed,
        Ok(FetchOutput {
            items: parse_feed(&feed, xml)?,
            ..FetchOutput::default()
        }),
        &[],
        None,
        now,
    );
    let status = processed
        .status
        .current_news
        .context("current-news stats")?;
    assert_eq!(
        (
            status.eligible_items,
            status.stale_items,
            status.undated_items,
            status.future_items
        ),
        (1, 1, 12, 0),
        "Atom updated or fetch time became publication"
    );
    assert_eq!(
        processed
            .collected
            .first()
            .map(|item| item.brief_item.title.as_str()),
        Some("Publication only"),
        "wrong Atom publication selected"
    );
    let required = Source {
        threshold: "always".to_owned(),
        ..feed
    };
    let required_items = parse_feed(&required, xml)?;
    assert_eq!(
        required_items
            .first()
            .map(|item| item.published_at.as_str()),
        Some("2026-04-23T12:00:00Z"),
        "Required lost Atom updated date"
    );
    let state = SourceState {
        latest_identity: "invalid".to_owned(),
        ..SourceState::default()
    };
    let processed = process_source(
        &required,
        Ok(FetchOutput {
            items: required_items,
            ..FetchOutput::default()
        }),
        &[],
        Some(&state),
        now,
    );
    assert!(
        processed.status.current_news.is_none(),
        "Required entered current-news selection"
    );
    assert_eq!(
        processed.collected.len(),
        2,
        "Required marker selection changed"
    );
    assert_eq!(processed.status.new_items, Some(2));
    let next = processed.next_state.context("Required next marker")?;
    assert_eq!(next.latest_identity, "old");
    assert_eq!(next.latest_feed_identity, "old");
    assert_eq!(next.checked_at, now);
    Ok(())
}

fn fetch_log(source_key: &str, status: &str, error: &str) -> FetchLog {
    FetchLog {
        source_key: source_key.to_owned(),
        status: status.to_owned(),
        error: error.to_owned(),
        ..FetchLog::default()
    }
}
