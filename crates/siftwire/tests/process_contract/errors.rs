use super::*;

#[test]
fn config_write_failures_are_not_validation_refusals() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("errors.sqlite")
        .to_string_lossy()
        .into_owned();
    let args = ["config", "--db", &database];
    let environment = BTreeMap::new();
    let source = json!({"key":"feed","label":"Fixture","kind":"rss", "url":"https://example.test/feed","section":"technology","threshold":"always","enabled":true});
    invoke_json(
        &args,
        &json!({"action":"upsert_source","source":source}),
        &environment,
    )?;
    let before = invoke_json(&args, &json!({"action":"inspect_config"}), &environment)?;
    let db = rusqlite::Connection::open(&database)?;
    db.execute_batch("BEGIN IMMEDIATE")?;
    for request in [
        json!({"action":"upsert_source","source":source}),
        json!({"action":"replace_sources","sources":[source]}),
        json!({"action":"delete_source","key":"feed"}),
        json!({"action":"replace_outlet_policies","outlets":[{"name":"Fixture","policy":"allow","enabled":true}]}),
    ] {
        let output = invoke(&args, Some(&request.to_string()), &environment)?;
        assert_eq!(
            output.status.code(),
            Some(1),
            "runtime error became refusal for {request}: {output:?}"
        );
        assert!(
            output.stdout.is_empty(),
            "runtime error emitted success JSON: {output:?}"
        );
        assert!(
            text(&output.stderr).contains("run config task:"),
            "missing task diagnostic: {output:?}"
        );
        assert!(
            text(&output.stderr).contains("database is locked"),
            "missing SQL error: {output:?}"
        );
    }
    db.execute_batch("ROLLBACK")?;
    let after = invoke_json(&args, &json!({"action":"inspect_config"}), &environment)?;
    assert_eq!(before, after, "failed writes changed durable configuration");
    Ok(())
}

#[test]
fn preparation_corruption_fails_but_saved_plan_replays() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("prepare.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::new();
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"init"}),
        &environment,
    )?;
    let db = rusqlite::Connection::open(&database)?;
    db.execute_batch("INSERT INTO brief_run (id,started_at,dry_run,status,summary) VALUES ('run','2026-01-01T00:00:00Z',0,'ok','fixture')")?;
    let request = json!({"action":"prepare_delivery","run_id":"run","candidate_indexes":[]});
    for (started_at, timezone, diagnostic) in [
        (
            "2026-01-01T00:00:00Z",
            "invalid",
            "stored sports timezone is invalid",
        ),
        ("invalid", "UTC", "stored brief start time is invalid"),
    ] {
        db.execute("UPDATE brief_run SET started_at = ?1", [started_at])?;
        let context = json!({"max_delivery_items":7,"sports_updates":[],"sports_timezone":timezone,"health_footnote":""});
        db.execute("INSERT INTO brief_run_delivery_context VALUES ('run',?1) ON CONFLICT(run_id) DO UPDATE SET context_json = excluded.context_json", [context.to_string()])?;
        let output = invoke(
            &["brief", "--db", &database],
            Some(&request.to_string()),
            &environment,
        )?;
        assert_eq!(
            output.status.code(),
            Some(1),
            "stored corruption became refusal: {output:?}"
        );
        assert!(
            output.stdout.is_empty(),
            "failed preparation emitted result: {output:?}"
        );
        assert!(
            text(&output.stderr).contains(diagnostic),
            "missing {diagnostic}: {output:?}"
        );
        let plans: i64 =
            db.query_row("SELECT count(*) FROM delivery_plan", [], |row| row.get(0))?;
        assert_eq!(plans, 0, "failed rendering persisted a plan");
    }
    db.execute_batch("UPDATE brief_run SET started_at = '2026-01-01T00:00:00Z'")?;
    let prepared = invoke_json(&["brief", "--db", &database], &request, &environment)?;
    db.execute_batch("UPDATE brief_run SET started_at = 'invalid'; UPDATE brief_run_delivery_context SET context_json = 'invalid'")?;
    let replay = invoke_json(&["brief", "--db", &database], &request, &environment)?;
    assert_eq!(
        prepared, replay,
        "saved plan replay read mutable rendering inputs"
    );
    Ok(())
}

#[test]
fn confirmation_history_failure_is_visible_and_retry_keeps_one_delivery() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("confirm.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::new();
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"init"}),
        &environment,
    )?;
    let db = rusqlite::Connection::open(&database)?;
    db.execute_batch("INSERT INTO brief_run (id,started_at,dry_run,status,summary) VALUES ('run','2026-01-01T00:00:00Z',0,'ok','fixture'); INSERT INTO delivery_plan VALUES ('plan-run','run','[]','NO_REPLY','NO_REPLY','','[]'); INSERT INTO delivery (run_id,message,delivered_at) VALUES ('older','history','invalid')")?;
    let request = json!({"action":"confirm_delivery","delivery_plan_id":"plan-run"});
    let output = invoke(
        &["brief", "--db", &database],
        Some(&request.to_string()),
        &environment,
    )?;
    assert_eq!(
        output.status.code(),
        Some(1),
        "unreadable history silently disappeared: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "history failure emitted partial success: {output:?}"
    );
    let diagnostic = text(&output.stderr);
    assert!(
        diagnostic.contains("delivery is recorded"),
        "missing committed-write context: {output:?}"
    );
    assert!(
        diagnostic.contains("invalid stored timestamp"),
        "missing underlying error: {output:?}"
    );
    let count: i64 = db.query_row(
        "SELECT count(*) FROM delivery WHERE run_id = 'run'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        count, 1,
        "confirmation was not recorded before history read"
    );
    db.execute_batch(
        "UPDATE delivery SET delivered_at = '2025-01-01T00:00:00Z' WHERE run_id = 'older'",
    )?;
    let retried = invoke_json(&["brief", "--db", &database], &request, &environment)?;
    assert_eq!(retried.get("rejected"), Some(&json!(false)));
    assert!(
        retried
            .get("final_answer")
            .and_then(serde_json::Value::as_str)
            .context("final answer")?
            .contains("Previous brief (2025-01-01T00:00:00Z)\n\nhistory")
    );
    let count: i64 = db.query_row(
        "SELECT count(*) FROM delivery WHERE run_id = 'run'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(count, 1, "same-plan retry duplicated delivery");
    Ok(())
}
