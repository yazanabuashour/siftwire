#![expect(
    clippy::panic_in_result_fn,
    reason = "storage behavior tests return Result for SQLite setup while assertions report contract failures"
)]

use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;

use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::contract::SportsUpdate;
use crate::domain::{
    SCHEDULE_FILTER_STANDINGS_TOP_TWO, SCHEDULE_FORMAT_RIOT, SOURCE_KIND_SCHEDULE, Source,
};

use super::{
    CONFIGURATION_VERSION_V2, DeliveryPlan, RUNTIME_CONFIG_CONFIGURATION_VERSION,
    RUNTIME_CONFIG_MAX_DELIVERY_ITEMS, RunDeliveryContext, Store, StoredSentItem,
};

fn test_store() -> Result<(TempDir, Store)> {
    let temp = TempDir::new()?;
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
    Ok((temp, store))
}

fn create_legacy_database(path: &Path) -> Result<()> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "CREATE TABLE brief_source (\
            key TEXT PRIMARY KEY, label TEXT NOT NULL, kind TEXT NOT NULL,\
            url TEXT NOT NULL DEFAULT '', repo TEXT NOT NULL DEFAULT '',\
            section TEXT NOT NULL, threshold TEXT NOT NULL, enabled INTEGER NOT NULL,\
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL\
        );\
        CREATE TABLE source_state (\
            source_key TEXT PRIMARY KEY REFERENCES brief_source(key) ON DELETE CASCADE,\
            latest_identity TEXT NOT NULL, latest_title TEXT NOT NULL, latest_url TEXT NOT NULL,\
            latest_published_at TEXT NOT NULL, checked_at TEXT NOT NULL\
        );\
        CREATE TABLE delivery (\
            id INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL,\
            message TEXT NOT NULL, delivered_at TEXT NOT NULL\
        );\
        INSERT INTO brief_source VALUES (\
            'legacy', 'Legacy', 'rss', 'https://example.test/feed.xml', '',\
            'technology', 'medium', 1, '2026-04-23T00:00:00Z', '2026-04-23T00:00:00Z'\
        );\
        INSERT INTO source_state VALUES (\
            'legacy', 'legacy-guid', 'Legacy story', 'https://example.test/story',\
            'Thu, 23 Apr 2026 00:00:00 GMT', '2026-04-23T00:00:00Z'\
        );\
        INSERT INTO delivery (run_id, message, delivered_at) VALUES\
            ('legacy-run', 'stale', '2026-04-23T00:00:00Z'),\
            ('legacy-run', 'latest', '2026-04-23T01:00:00Z');",
    )?;
    Ok(())
}

#[test]
fn opening_an_old_schema_migrates_fields_and_latest_delivery_claims() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join("legacy.sqlite");
    create_legacy_database(&path)?;

    let store = Store::open(&path)?;
    let sources = store.list_sources(false)?;
    let source = sources.first().context("migrated source")?;
    assert_eq!(source.key, "legacy", "legacy source disappeared");
    assert_eq!(
        source.url_canonicalization, "none",
        "canonicalization default changed"
    );
    assert_eq!(
        source.outlet_extraction, "none",
        "outlet extraction default changed"
    );
    assert_eq!(source.dedup_group, "", "dedup group default changed");
    assert_eq!(source.priority_rank, 0, "priority default changed");
    assert_eq!(source.threshold, "medium", "legacy threshold changed");
    assert_eq!(
        source.schedule_filter, "all",
        "schedule filter migration default changed"
    );

    let state = store
        .source_state("legacy")?
        .context("migrated source state")?;
    assert_eq!(
        state.latest_identity, "legacy-guid",
        "legacy state disappeared"
    );
    assert_eq!(
        state.latest_feed_identity, "",
        "feed identity migration default changed"
    );

    let runtime = store.runtime_config()?;
    assert_eq!(
        runtime
            .get(RUNTIME_CONFIG_CONFIGURATION_VERSION)
            .map(String::as_str),
        Some(CONFIGURATION_VERSION_V2),
        "configuration version migration changed"
    );
    assert_eq!(
        runtime
            .get(RUNTIME_CONFIG_MAX_DELIVERY_ITEMS)
            .map(String::as_str),
        Some("7"),
        "delivery limit migration changed"
    );

    let replay = store.insert_delivery("legacy-run", "latest", Vec::new())?;
    assert!(
        replay.is_empty(),
        "legacy delivery replay invented sent items"
    );
    let Err(conflict) = store.insert_delivery("legacy-run", "stale", Vec::new()) else {
        bail!("migration accepted the stale delivery message");
    };
    assert!(
        conflict.is_conflict(),
        "stale delivery returned the wrong error: {conflict}"
    );
    Ok(())
}

const LEGACY_EMAIL: &str = "Exact email\r\n  Preserve spacing & punctuation.\n";
const LEGACY_HTML: &str = "<p>Exact email &amp; spacing</p>\r\n";

fn insert_legacy_source_evidence(store: &Store) -> Result<String> {
    let run_id = store.start_run(false)?;
    store.insert_run_items(
        &run_id,
        &[super::RunItemRow {
            category: super::RUN_ITEM_MUST_INCLUDE.to_owned(),
            source_key: "legacy".to_owned(),
            kind: "atom".to_owned(),
            threshold: "audit".to_owned(),
            always_report: true,
            title: "Historical evidence".to_owned(),
            ..super::RunItemRow::default()
        }],
    )?;
    let _plan = store.insert_delivery_plan(&DeliveryPlan {
        id: format!("plan-{run_id}"),
        run_id: run_id.clone(),
        candidate_indexes: Vec::new(),
        message: LEGACY_EMAIL.to_owned(),
        text: LEGACY_EMAIL.to_owned(),
        html: LEGACY_HTML.to_owned(),
        items: Vec::new(),
    })?;
    store.insert_delivery(&run_id, LEGACY_EMAIL, Vec::new())?;
    Ok(run_id)
}

fn assert_legacy_source_evidence(
    store: &Store,
    run_id: &str,
    recorded: &serde_json::Value,
) -> Result<()> {
    let detail = store.run_detail(run_id)?.context("historical run")?;
    assert_eq!(
        &serde_json::to_value(detail.items)?,
        recorded,
        "historical items changed"
    );
    assert_eq!(
        detail.summary.message.as_deref(),
        Some(LEGACY_EMAIL),
        "email text changed"
    );
    assert_eq!(
        detail.delivery_html.as_deref(),
        Some(LEGACY_HTML),
        "email HTML changed"
    );
    Ok(())
}

#[test]
fn legacy_source_reads_preserve_raw_configuration_state_and_delivery_evidence() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join("legacy.sqlite");
    create_legacy_database(&path)?;
    let store = Store::open(&path)?;
    store.connection.execute(
        "UPDATE brief_source SET kind = 'atom', threshold = 'audit', always_report = 1",
        [],
    )?;
    let run_id = insert_legacy_source_evidence(&store)?;
    let before = store.run_detail(&run_id)?.context("original run")?;
    let recorded = serde_json::to_value(before.items)?;
    let state = store.source_state("legacy")?.context("original state")?;
    drop(store);

    let store = Store::open(&path)?;
    let sources = store.list_sources(true)?;
    let source = sources.first().context("effective source")?;
    assert_eq!(source.kind, "rss", "stored atom was not adapted");
    assert_eq!(
        source.threshold, "always",
        "legacy required precedence changed"
    );
    assert_eq!(
        source.reporting(),
        crate::domain::Reporting::Required,
        "legacy flag lost"
    );
    let raw: (String, String, i64, String) = store.connection.query_row(
        "SELECT kind, threshold, always_report, updated_at FROM brief_source WHERE key = 'legacy'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    assert_eq!(
        raw,
        (
            "atom".to_owned(),
            "audit".to_owned(),
            1,
            "2026-04-23T00:00:00Z".to_owned()
        ),
        "read rewrote raw source"
    );
    assert_legacy_source_evidence(&store, &run_id, &recorded)?;

    store.upsert_source(Source {
        threshold: "high".to_owned(),
        ..source.clone()
    })?;
    let reopened = store.list_sources(true)?;
    assert_eq!(
        reopened.first().map(|source| source.threshold.as_str()),
        Some("high"),
        "legacy flag overrode explicit edit"
    );
    let raw_flag: i64 = store.connection.query_row(
        "SELECT always_report FROM brief_source WHERE key = 'legacy'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(raw_flag, 0, "upsert retained legacy flag");
    let after = store.source_state("legacy")?.context("preserved state")?;
    assert_eq!(
        after.latest_identity, state.latest_identity,
        "upsert lost latest identity"
    );
    assert_eq!(
        after.latest_feed_identity, state.latest_feed_identity,
        "upsert changed feed identity"
    );
    assert_eq!(
        after.checked_at, state.checked_at,
        "upsert changed checked time"
    );
    assert_legacy_source_evidence(&store, &run_id, &recorded)?;
    Ok(())
}

fn insert_unsupported_legacy_sources(store: &Store) -> Result<()> {
    for (key, kind, threshold, canonicalization, extraction, schedule, repo) in [
        ("audit", "rss", "audit", "none", "none", "", ""),
        (
            "redirect",
            "rss",
            "medium",
            "feedburner_redirect",
            "none",
            "",
            "",
        ),
        ("host", "rss", "medium", "none", "url_host", "", ""),
        ("rss_source", "rss", "medium", "none", "rss_source", "", ""),
        (
            "core",
            "sports_schedule",
            "always",
            "none",
            "none",
            "espn_core",
            "",
        ),
        (
            "github",
            "github_release",
            "always",
            "none",
            "none",
            "",
            "owner/repo",
        ),
    ] {
        store.connection.execute(
            "INSERT INTO brief_source (key, label, kind, url, repo, section, threshold, enabled, \
             url_canonicalization, outlet_extraction, schedule_format, created_at, updated_at) \
             VALUES (?1, ?1, ?2, 'https://example.test/feed', ?7, 'news', ?3, 0, ?4, ?5, ?6, 'original', 'original')",
            rusqlite::params![key, kind, threshold, canonicalization, extraction, schedule, repo],
        )?;
    }
    Ok(())
}

#[test]
fn unsupported_stored_sources_are_readable_but_never_enabled_for_fetching() -> Result<()> {
    let (temp, store) = test_store()?;
    insert_unsupported_legacy_sources(&store)?;
    drop(store);
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
    let sources = store.list_sources(false)?;
    assert_eq!(
        sources.len(),
        6,
        "disabled legacy configuration disappeared"
    );
    assert!(
        store.list_sources(true)?.is_empty(),
        "disabled sources became active"
    );
    for source in sources {
        let _error = store
            .upsert_source(source.clone())
            .err()
            .context("unsupported source was writable")?;
        let _error = store
            .replace_sources(vec![source.clone()])
            .err()
            .context("replacement accepted unsupported source")?;
        assert_eq!(
            store.list_sources(false)?.len(),
            6,
            "failed replacement deleted sources"
        );
        if source.key == "audit" {
            assert_eq!(source.threshold, "audit", "disabled audit policy changed");
            assert_eq!(
                source.reporting(),
                crate::domain::Reporting::Observe,
                "disabled audit became optional delivery"
            );
        }
        store.connection.execute(
            "UPDATE brief_source SET enabled = 1 WHERE key = ?1",
            [&source.key],
        )?;
        let error = store
            .list_sources(true)
            .err()
            .context("unsupported enabled source reached fetching")?;
        assert!(
            error
                .to_string()
                .contains(&format!("enabled source {:?} is unsupported", source.key)),
            "source rejection omitted the key: {error}"
        );
        store.connection.execute(
            "UPDATE brief_source SET enabled = 0 WHERE key = ?1",
            [&source.key],
        )?;
        let updated: String = store.connection.query_row(
            "SELECT updated_at FROM brief_source WHERE key = ?1",
            [&source.key],
            |row| row.get(0),
        )?;
        assert_eq!(updated, "original", "rejected write changed source");
        if source.key == "audit" {
            store.upsert_source(Source {
                threshold: "high".to_owned(),
                ..source
            })?;
        }
    }
    Ok(())
}

#[test]
fn riot_schedule_filter_round_trips_through_storage() -> Result<()> {
    let (_temp, store) = test_store()?;
    store.upsert_source(Source {
        key: "lol_example".to_owned(),
        label: "Example League".to_owned(),
        kind: SOURCE_KIND_SCHEDULE.to_owned(),
        url: "https://esports.example/persisted/gw/getSchedule?leagueId=fixture".to_owned(),
        section: "sports".to_owned(),
        enabled: true,
        schedule_format: SCHEDULE_FORMAT_RIOT.to_owned(),
        schedule_filter: SCHEDULE_FILTER_STANDINGS_TOP_TWO.to_owned(),
        api_key: "public-key".to_owned(),
        ..Source::default()
    })?;
    let sources = store.list_sources(false)?;
    assert_eq!(
        sources
            .first()
            .map(|source| source.schedule_filter.as_str()),
        Some(SCHEDULE_FILTER_STANDINGS_TOP_TWO)
    );
    Ok(())
}

#[test]
fn concurrent_legacy_schema_migrations_are_serialized() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join("legacy.sqlite");
    create_legacy_database(&path)?;
    let barrier = Arc::new(Barrier::new(2));
    let workers = (0..2)
        .map(|_worker| {
            let barrier = Arc::clone(&barrier);
            let path = path.clone();
            thread::spawn(move || -> Result<BTreeMap<String, String>> {
                barrier.wait();
                Store::open(path)?.runtime_config()
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        let runtime = worker
            .join()
            .map_err(|_panic| anyhow::anyhow!("migration worker panicked"))??;
        assert_eq!(
            runtime
                .get(RUNTIME_CONFIG_CONFIGURATION_VERSION)
                .map(String::as_str),
            Some(CONFIGURATION_VERSION_V2),
            "concurrent migration did not finish"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn new_database_paths_are_owner_only_without_changing_existing_parents() -> Result<()> {
    let temp = TempDir::new()?;
    let explicit_parent = temp.path().join("explicit");
    fs::create_dir(&explicit_parent)?;
    fs::set_permissions(&explicit_parent, fs::Permissions::from_mode(0o755))?;
    let database = explicit_parent.join("siftwire.sqlite");
    let _store = Store::open(&database)?;

    assert_eq!(
        mode(&explicit_parent)?,
        0o755,
        "opening a database changed its existing explicit parent"
    );
    assert_eq!(mode(&database)?, 0o600, "new database was not owner-only");
    for sidecar in [
        sidecar(&database, "-wal"),
        sidecar(&database, "-shm"),
        sidecar(&database, ".siftwire-migration.lock"),
    ] {
        assert_eq!(
            mode(&sidecar)?,
            0o600,
            "new SQLite sidecar was not owner-only: {}",
            sidecar.display()
        );
    }

    let nested_database = temp.path().join("new/nested/siftwire.sqlite");
    let _nested_store = Store::open(&nested_database)?;
    assert_eq!(
        mode(nested_database.parent().context("nested database parent")?)?,
        0o700,
        "new database directory was not owner-only"
    );
    Ok(())
}

#[cfg(unix)]
fn mode(path: &Path) -> Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o777)
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[test]
fn delivery_replay_returns_original_items_and_changed_messages_conflict() -> Result<()> {
    let (_temp, store) = test_store()?;
    let original = StoredSentItem {
        title: "Original Story".to_owned(),
        url: "https://example.test/original".to_owned(),
        ..StoredSentItem::default()
    };
    let inserted = store.insert_delivery("run-1", "same message", vec![original])?;
    let inserted_item = inserted.first().context("inserted sent item")?;

    let replay = store.insert_delivery(
        "run-1",
        "same message",
        vec![StoredSentItem {
            title: "Replacement".to_owned(),
            url: "https://example.test/replacement".to_owned(),
            ..StoredSentItem::default()
        }],
    )?;
    let replayed_item = replay.first().context("replayed sent item")?;
    assert_eq!(replay.len(), 1, "delivery replay item count changed");
    assert_eq!(
        replayed_item.title, "Original Story",
        "replay used request items"
    );
    assert_eq!(
        replayed_item.url, "https://example.test/original",
        "replay URL changed"
    );
    assert_eq!(
        replayed_item.sent_at, inserted_item.sent_at,
        "replay timestamp changed"
    );

    let Err(conflict) = store.insert_delivery("run-1", "changed message", Vec::new()) else {
        bail!("changed delivery message was accepted");
    };
    assert!(
        conflict.is_conflict(),
        "changed delivery returned the wrong error: {conflict}"
    );
    let deliveries = store.recent_deliveries(10)?;
    assert_eq!(
        deliveries.len(),
        1,
        "idempotent replay inserted another delivery"
    );
    Ok(())
}

#[test]
fn delivered_run_with_empty_plan_html_has_no_delivery_html() -> Result<()> {
    let (temp, store) = test_store()?;
    let run_id = store.start_run(false)?;
    store.finish_run(&run_id, "ok", "no items")?;
    let _plan = store.insert_delivery_plan(&DeliveryPlan {
        id: format!("plan-{run_id}"),
        run_id: run_id.clone(),
        candidate_indexes: Vec::new(),
        message: "NO_REPLY".to_owned(),
        text: "NO_REPLY".to_owned(),
        html: String::new(),
        items: Vec::new(),
    })?;
    store.insert_delivery(&run_id, "NO_REPLY", Vec::new())?;
    drop(store);
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
    let detail = store.run_detail(&run_id)?.context("delivered run")?;
    assert!(
        detail.summary.delivered_at.is_some(),
        "run was not delivered"
    );
    assert_eq!(detail.delivery_html, None, "empty HTML must be absent");
    Ok(())
}

#[test]
fn sports_delivery_evidence_does_not_enter_normal_recent_suppression() -> Result<()> {
    let (temp, store) = test_store()?;
    let run_id = store.start_run(false)?;
    let sports_title = "Team Alpha defeated Team Beta";
    let sports_url = "https://example.test/result";
    store.insert_run_delivery_context(
        &run_id,
        &RunDeliveryContext {
            max_delivery_items: 7,
            sports_updates: vec![SportsUpdate {
                title: sports_title.to_owned(),
                url: sports_url.to_owned(),
                ..SportsUpdate::default()
            }],
            sports_timezone: "America/Chicago".to_owned(),
            health_footnote: String::new(),
        },
    )?;
    store.insert_delivery(
        &run_id,
        "message",
        vec![
            StoredSentItem {
                title: "Normal story".to_owned(),
                url: "https://example.test/story".to_owned(),
                kind: "rss".to_owned(),
                ..StoredSentItem::default()
            },
            StoredSentItem {
                title: sports_title.to_owned(),
                url: sports_url.to_owned(),
                ..StoredSentItem::default()
            },
        ],
    )?;
    drop(store);
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
    let recent = store.recent_sent_items(chrono::DateTime::<Utc>::default())?;
    assert_eq!(recent.len(), 1);
    assert_eq!(
        recent.first().map(|item| item.title.as_str()),
        Some("Normal story")
    );
    Ok(())
}

#[test]
fn health_delta_previews_persists_and_resolves_by_warning_key() -> Result<()> {
    let (_temp, store) = test_store()?;
    let first = BTreeMap::from([(
        "feed:a".to_owned(),
        "Feed `a` failed this run (timeout)".to_owned(),
    )]);
    let preview = store.health_delta(&first, false)?;
    assert_eq!(
        preview.new_warnings.len(),
        1,
        "health preview omitted a new warning"
    );
    assert!(
        preview.resolved_warnings.is_empty(),
        "health preview resolved a warning"
    );

    let persisted = store.health_delta(&first, true)?;
    assert_eq!(
        persisted.new_warnings, preview.new_warnings,
        "persisted delta changed"
    );
    let unchanged = store.health_delta(&first, true)?;
    assert!(
        unchanged.new_warnings.is_empty(),
        "unchanged warning was reported as new"
    );
    assert!(
        unchanged.resolved_warnings.is_empty(),
        "unchanged warning was resolved"
    );

    let second = BTreeMap::from([(
        "feed:b".to_owned(),
        "Feed `b` failed this run (HTTP 502)".to_owned(),
    )]);
    let changed = store.health_delta(&second, true)?;
    assert_eq!(
        changed.new_warnings,
        ["Feed `b` failed this run (HTTP 502)"],
        "new warning delta changed"
    );
    assert_eq!(
        changed.resolved_warnings,
        ["Feed `a` failed this run (timeout)"],
        "resolved warning delta changed"
    );
    Ok(())
}
