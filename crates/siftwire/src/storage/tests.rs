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

use crate::domain::{
    SCHEDULE_FILTER_STANDINGS_TOP_TWO, SCHEDULE_FORMAT_RIOT, SOURCE_KIND_SCHEDULE, Source,
};

use super::{DeliveryPlan, RUNTIME_CONFIG_MAX_DELIVERY_ITEMS, Store, StoredSentItem};

fn test_store() -> Result<(TempDir, Store)> {
    let temp = TempDir::new()?;
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
    Ok((temp, store))
}

#[test]
fn incompatible_databases_are_rejected_without_mutation() -> Result<()> {
    let temp = TempDir::new()?;
    for (name, sql) in [
        (
            "old",
            "CREATE TABLE brief_source (key TEXT PRIMARY KEY); INSERT INTO brief_source VALUES ('preserve');",
        ),
        (
            "partial",
            "CREATE TABLE brief_source (key TEXT PRIMARY KEY); PRAGMA user_version = 1;",
        ),
        ("future", "PRAGMA user_version = 2;"),
        ("foreign", "PRAGMA application_id = 42;"),
    ] {
        let path = temp.path().join(format!("{name}.sqlite"));
        Connection::open(&path)?.execute_batch(sql)?;
        let before = fs::read(&path)?;
        let error = Store::open(&path)
            .err()
            .context("incompatible database accepted")?;
        assert!(error.to_string().contains("incompatible"), "{error:#}");
        assert_eq!(fs::read(&path)?, before, "rejection mutated {name}");
        assert!(!sidecar(&path, "-wal").exists());
    }
    let path = temp.path().join("modified.sqlite");
    let store = Store::open(&path)?;
    store
        .connection
        .execute_batch("DROP INDEX idx_fetch_log_run_id")?;
    drop(store);
    let before = fs::read(&path)?;
    assert!(
        Store::open(&path).is_err(),
        "modified current schema accepted"
    );
    assert_eq!(fs::read(&path)?, before);
    Ok(())
}

#[test]
fn incompatible_uncheckpointed_wal_is_rejected_without_mutation() -> Result<()> {
    let temp = TempDir::new()?;
    let source = temp.path().join("source.sqlite");
    drop(Store::open(&source)?);
    let writer = Connection::open(&source)?;
    writer.execute_batch(
        "PRAGMA wal_autocheckpoint = 0; \
         PRAGMA user_version = 2; \
         CREATE TABLE preserved (value TEXT); \
         INSERT INTO preserved VALUES ('committed only in WAL');",
    )?;
    // Copy while the writer is open to reproduce an interrupted process without
    // SQLite's last-connection checkpoint. The main database alone is compatible.
    let path = temp.path().join("interrupted.sqlite");
    let wal = sidecar(&path, "-wal");
    fs::copy(&source, &path)?;
    fs::copy(sidecar(&source, "-wal"), &wal)?;
    let before = fs::read(&path)?;
    let wal_before = fs::read(&wal)?;
    assert!(!wal_before.is_empty(), "fixture has no committed WAL");
    let error = Store::open(&path)
        .err()
        .context("incompatible WAL database accepted")?;
    assert!(error.to_string().contains("incompatible"), "{error:#}");
    assert!(fs::read(&path)? == before, "rejection checkpointed WAL");
    assert!(fs::read(&wal)? == wal_before, "rejection modified WAL");
    Ok(())
}

#[test]
fn reopening_preserves_configuration_state_and_delivery_evidence() -> Result<()> {
    let (temp, store) = test_store()?;
    let runtime = store.runtime_config()?;
    assert_eq!(
        runtime
            .get(RUNTIME_CONFIG_MAX_DELIVERY_ITEMS)
            .map(String::as_str),
        Some("7")
    );
    store.upsert_source(Source {
        key: "feed".to_owned(),
        label: "Feed".to_owned(),
        kind: "rss".to_owned(),
        url: "https://example.test/feed".to_owned(),
        section: "news".to_owned(),
        threshold: "high".to_owned(),
        enabled: true,
        ..Source::default()
    })?;
    store.upsert_source_state(&super::SourceState {
        source_key: "feed".to_owned(),
        latest_identity: "guid".to_owned(),
        ..super::SourceState::default()
    })?;
    let run_id = store.start_run(false)?;
    store.insert_run_items(
        &run_id,
        &[super::RunItemRow {
            category: super::RUN_ITEM_CANDIDATE.to_owned(),
            source_key: "feed".to_owned(),
            kind: "rss".to_owned(),
            threshold: "high".to_owned(),
            title: "Recorded story".to_owned(),
            ..super::RunItemRow::default()
        }],
    )?;
    let message = "Exact email\r\n  Preserve spacing & punctuation.\n";
    let html = "<p>Exact email &amp; spacing</p>\r\n";
    store.insert_delivery_plan(&DeliveryPlan {
        id: format!("plan-{run_id}"),
        run_id: run_id.clone(),
        candidate_indexes: Vec::new(),
        message: message.to_owned(),
        text: message.to_owned(),
        html: html.to_owned(),
        items: Vec::new(),
    })?;
    store.insert_delivery(&run_id, message, Vec::new())?;
    let state = store.source_state("feed")?.context("original state")?;
    let items = serde_json::to_value(store.run_detail(&run_id)?.context("original run")?.items)?;
    let config_rows: Vec<(String, String, String)> = store
        .connection
        .prepare("SELECT key_name, value_text, updated_at FROM runtime_config ORDER BY key_name")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(store);
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
    let source = store.list_sources(true)?.remove(0);
    store.upsert_source(Source {
        threshold: "medium".to_owned(),
        ..source
    })?;
    assert_eq!(
        store
            .source_state("feed")?
            .context("preserved state")?
            .checked_at,
        state.checked_at
    );
    let detail = store.run_detail(&run_id)?.context("preserved run")?;
    assert_eq!(serde_json::to_value(detail.items)?, items);
    assert_eq!(detail.summary.message.as_deref(), Some(message));
    assert_eq!(detail.delivery_html.as_deref(), Some(html));
    let after: Vec<(String, String, String)> = store
        .connection
        .prepare("SELECT key_name, value_text, updated_at FROM runtime_config ORDER BY key_name")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    assert_eq!(after, config_rows, "reopening rewrote configuration");
    Ok(())
}

#[test]
fn malformed_stored_values_are_rejected_instead_of_adapted() -> Result<()> {
    let (_temp, store) = test_store()?;
    store
        .recent_deliveries(0)
        .err()
        .context("zero delivery limit accepted")?;
    store
        .recent_fetch_logs(0)
        .err()
        .context("zero fetch log limit accepted")?;
    store.connection.execute_batch(
        "INSERT INTO brief_source (key, label, kind, url, section, threshold, enabled, created_at, updated_at) \
         VALUES ('feed', 'Feed', 'atom', 'https://example.test/feed', 'news', 'high', 0, '', ''); \
         INSERT INTO outlet_policy VALUES ('Outlet', 'invalid json', 'allow', '', 1, '', ''); \
         INSERT INTO delivery (run_id, message, delivered_at) VALUES ('run', 'message', 'invalid timestamp');"
    )?;
    assert!(
        store.list_sources(false).is_err(),
        "stored atom was adapted"
    );
    assert!(
        store.list_outlet_policies().is_err(),
        "malformed aliases were discarded"
    );
    assert!(
        store.recent_deliveries(1).is_err(),
        "malformed timestamp became zero time"
    );
    store.insert_fetch_log(&super::FetchLog::default())?;
    store
        .connection
        .execute("UPDATE fetch_log SET selection_json = ''", [])?;
    assert!(
        store.recent_fetch_logs(1).is_err(),
        "missing selection evidence was fabricated"
    );
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
fn concurrent_fresh_database_initializations_are_serialized() -> Result<()> {
    let temp = TempDir::new()?;
    let path = temp.path().join("fresh.sqlite");
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
            .map_err(|_panic| anyhow::anyhow!("initialization worker panicked"))??;
        assert_eq!(
            runtime
                .get(RUNTIME_CONFIG_MAX_DELIVERY_ITEMS)
                .map(String::as_str),
            Some("7")
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
    assert_eq!(mode(&explicit_parent)?, 0o755);
    assert_eq!(mode(&database)?, 0o600);
    for sidecar in [
        sidecar(&database, "-wal"),
        sidecar(&database, "-shm"),
        sidecar(&database, ".siftwire-initialization.lock"),
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
        0o700
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
    let (temp, store) = test_store()?;
    let original = StoredSentItem {
        title: "Original Story".to_owned(),
        url: "https://example.test/original".to_owned(),
        ..StoredSentItem::default()
    };
    let inserted = store.insert_delivery("run-1", "same message", vec![original])?;
    let inserted_item = inserted.first().context("inserted sent item")?;
    drop(store);
    let store = Store::open(temp.path().join("siftwire.sqlite"))?;
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
    assert_eq!(replay.len(), 1);
    assert_eq!(replayed_item.title, "Original Story");
    assert_eq!(replayed_item.url, "https://example.test/original");
    assert_eq!(replayed_item.sent_at, inserted_item.sent_at);
    let Err(conflict) = store.insert_delivery("run-1", "changed message", Vec::new()) else {
        bail!("changed delivery message was accepted");
    };
    assert!(conflict.is_conflict(), "{conflict}");
    assert_eq!(store.recent_deliveries(10)?.len(), 1);
    Ok(())
}

#[test]
fn delivered_run_with_empty_plan_html_has_no_delivery_html() -> Result<()> {
    let (temp, store) = test_store()?;
    let run_id = store.start_run(false)?;
    store.finish_run(&run_id, "ok", "no items")?;
    store.insert_delivery_plan(&DeliveryPlan {
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
    assert!(detail.summary.delivered_at.is_some());
    assert_eq!(detail.delivery_html, None);
    Ok(())
}

#[test]
fn sports_delivery_evidence_does_not_enter_normal_recent_suppression() -> Result<()> {
    let (temp, store) = test_store()?;
    let run_id = store.start_run(false)?;
    let sports_title = "Team Alpha defeated Team Beta";
    let sports_url = "https://example.test/result";
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
                kind: "sports_schedule".to_owned(),
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
    assert_eq!(preview.new_warnings.len(), 1);
    assert!(preview.resolved_warnings.is_empty());
    assert_eq!(
        store.health_delta(&first, true)?.new_warnings,
        preview.new_warnings
    );
    let unchanged = store.health_delta(&first, true)?;
    assert!(unchanged.new_warnings.is_empty());
    assert!(unchanged.resolved_warnings.is_empty());
    let second = BTreeMap::from([(
        "feed:b".to_owned(),
        "Feed `b` failed this run (HTTP 502)".to_owned(),
    )]);
    let changed = store.health_delta(&second, true)?;
    assert_eq!(
        changed.new_warnings,
        ["Feed `b` failed this run (HTTP 502)"]
    );
    assert_eq!(
        changed.resolved_warnings,
        ["Feed `a` failed this run (timeout)"]
    );
    Ok(())
}
