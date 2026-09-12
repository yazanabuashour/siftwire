#![expect(
    clippy::panic_in_result_fn,
    reason = "assertions report storage contract failures"
)]

use super::{DeliveryPlan, FetchLog, OutletPolicy, RunListOptions, Store};
use anyhow::{Context, Result};
use rusqlite::params;

fn archive_fixture() -> Result<(tempfile::TempDir, Store)> {
    let temp = tempfile::TempDir::new()?;
    let store = Store::open(temp.path().join("archive.sqlite"))?;
    for (id, started, summary) in [
        ("old", "2026-01-01T00:00:00Z", "100% literal _ [news]"),
        ("new", "2026-01-02T00:00:00Z", "second"),
        ("tie", "2026-01-02T00:00:00Z", "third"),
        ("unsent", "2026-01-03T00:00:00Z", "latest activity"),
    ] {
        store.connection.execute("INSERT INTO brief_run(id, started_at, dry_run, status, summary) VALUES (?1, ?2, 0, 'ok', ?3)", params![id, started, summary])?;
    }
    for (id, delivered) in [
        ("old", "2026-02-03T00:00:00Z"),
        ("new", "2026-02-02T00:00:00Z"),
        ("tie", "2026-02-02T00:00:00Z"),
    ] {
        store.insert_delivery(&DeliveryPlan {
            id: format!("plan-{id}"),
            run_id: id.to_owned(),
            candidate_indexes: Vec::new(),
            message: "NO_REPLY".to_owned(),
            text: "NO_REPLY".to_owned(),
            html: String::new(),
            items: Vec::new(),
        })?;
        store.connection.execute(
            "UPDATE delivery SET delivered_at = ?1 WHERE run_id = ?2",
            params![delivered, id],
        )?;
    }
    Ok((temp, store))
}

#[test]
fn archive_filters_before_paging_and_uses_delivery_chronology_and_literal_search() -> Result<()> {
    let (_temp, store) = archive_fixture()?;
    let first = store.list_runs(
        1,
        &RunListOptions {
            delivered: true,
            ..RunListOptions::default()
        },
    )?;
    assert_eq!(first.runs.first().context("first delivery")?.run_id, "old");
    assert_eq!(first.next_before.as_deref(), Some("old"));
    let second = store.list_runs(
        1,
        &RunListOptions {
            delivered: true,
            before: first.next_before,
            search: None,
        },
    )?;
    assert_eq!(
        second.runs.first().context("second delivery")?.run_id,
        "tie"
    );
    let last = store.list_runs(
        1,
        &RunListOptions {
            delivered: true,
            before: second.next_before,
            search: None,
        },
    )?;
    assert_eq!(last.runs.first().context("last delivery")?.run_id, "new");
    assert!(last.next_before.is_none());
    let activity = store.list_runs(2, &RunListOptions::default())?;
    assert_eq!(
        activity
            .runs
            .iter()
            .map(|run| run.run_id.as_str())
            .collect::<Vec<_>>(),
        ["unsent", "tie"]
    );
    assert!(
        store.run_detail("old")?.is_some(),
        "direct detail must not depend on the current page"
    );
    for search in ["%", "_", "[news]", "OLD", "2026-01-01", "2026-02-03"] {
        let page = store.list_runs(
            1,
            &RunListOptions {
                delivered: true,
                search: Some(search.to_owned()),
                before: None,
            },
        )?;
        assert_eq!(page.runs.first().context("literal match")?.run_id, "old");
        assert!(page.next_before.is_none());
    }
    for search in [".*", "--db", "not stored"] {
        assert!(
            store
                .list_runs(
                    1,
                    &RunListOptions {
                        search: Some(search.to_owned()),
                        ..RunListOptions::default()
                    }
                )?
                .runs
                .is_empty()
        );
    }
    for (before, delivered) in [("missing", false), ("unsent", true)] {
        let error = store
            .list_runs(
                1,
                &RunListOptions {
                    before: Some(before.to_owned()),
                    delivered,
                    search: None,
                },
            )
            .err()
            .context("unknown cursor accepted")?;
        assert!(error.to_string().contains("unknown cursor"));
    }
    Ok(())
}

#[test]
fn archive_cursors_order_fractional_seconds_chronologically() -> Result<()> {
    let (_temp, store) = archive_fixture()?;
    for (id, timestamp) in [
        ("old", "2026-01-01T00:00:00Z"),
        ("new", "2026-01-01T00:00:00.1Z"),
        ("tie", "2026-01-01T00:00:00.12Z"),
        ("unsent", "2026-01-01T00:00:00.123Z"),
    ] {
        store.connection.execute(
            "UPDATE brief_run SET started_at = ?1 WHERE id = ?2",
            params![timestamp, id],
        )?;
        store.connection.execute(
            "UPDATE delivery SET delivered_at = ?1 WHERE run_id = ?2",
            params![timestamp, id],
        )?;
    }
    for delivered in [false, true] {
        let mut options = RunListOptions {
            delivered,
            ..RunListOptions::default()
        };
        let mut ids = Vec::new();
        loop {
            let page = store.list_runs(1, &options)?;
            ids.extend(page.runs.into_iter().map(|run| run.run_id));
            options.before = page.next_before;
            if options.before.is_none() {
                break;
            }
        }
        let expected: &[&str] = if delivered {
            &["tie", "new", "old"]
        } else {
            &["unsent", "tie", "new", "old"]
        };
        assert_eq!(ids, expected);
    }
    Ok(())
}

#[test]
fn outlet_conflicts_are_rejected_and_collection_writes_are_atomic() -> Result<()> {
    let temp = tempfile::TempDir::new()?;
    let store = Store::open(temp.path().join("outlets.sqlite"))?;
    let policy = |name: &str, aliases: &[&str], enabled| OutletPolicy {
        name: name.to_owned(),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
        policy: "watch".to_owned(),
        note: "Keep this rationale".to_owned(),
        enabled,
    };
    store.replace_outlet_policies(vec![policy("First", &["Example.com"], true)])?;
    let policies = vec![
        policy("First", &["Example.com", "example"], true),
        policy("Second", &["EXAMPLE.news"], true),
    ];
    let conflicts = crate::domain::outlet_conflicts(&policies);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts.first().context("conflict")?.matcher, "example");
    assert_eq!(
        conflicts.first().context("conflict")?.names,
        ["First", "Second"]
    );
    store
        .replace_outlet_policies(policies.clone())
        .err()
        .context("conflicting write accepted")?;
    let stored = store.list_outlet_policies()?;
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored.first().context("retained policy")?.note,
        "Keep this rationale"
    );
    let mut disabled = policies;
    disabled.get_mut(1).context("second")?.enabled = false;
    store.replace_outlet_policies(disabled)?;
    // Simulate corruption that bypassed the write boundary.
    store.connection.execute(
        "UPDATE outlet_policy SET enabled = 1 WHERE name = 'Second'",
        [],
    )?;
    store
        .list_outlet_policies()
        .err()
        .context("conflicting stored policies accepted")?;
    store
        .replace_outlet_policies(vec![
            policy("Example", &[], true),
            policy("Other", &["example.org"], true),
        ])
        .err()
        .context("name/alias collision accepted")?;
    Ok(())
}

#[test]
fn fetch_evidence_preserves_new_snapshots_on_reopen() -> Result<()> {
    let temp = tempfile::TempDir::new()?;
    let database = temp.path().join("fetch.sqlite");
    let store = Store::open(&database)?;
    store.insert_fetch_log(&FetchLog {
        run_id: "new".to_owned(),
        source_key: "feed".to_owned(),
        source_label: "Recorded label".to_owned(),
        status: "error".to_owned(),
        ..FetchLog::default()
    })?;
    drop(store);
    let store = Store::open(&database)?;
    let logs = store.recent_fetch_logs(10)?;
    assert_eq!(logs.len(), 1);
    assert_eq!(logs.first().context("fetch log")?.new_item_count, None);
    assert_eq!(
        logs.last().context("new log")?.source_label,
        "Recorded label"
    );
    assert_eq!(logs.last().context("new log")?.status, "error");
    Ok(())
}
