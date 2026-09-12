use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

use anyhow::{Result, ensure};
use rusqlite::Connection;

use crate::types::{JobResult, RunResult};
use crate::verify_checks::Checker;
use crate::{adapter, output, report, run};

#[test]
fn executable_boundary_passes_scenario_and_reports_opaque_runtime() -> Result<()> {
    let root = tempfile::tempdir()?;
    let run_dir = root.path().join("scenario");
    let workspace = run_dir.join("workspace");
    fs::create_dir_all(&workspace)?;
    let response = serde_json::json!({
        "protocol": "siftwire-agent-eval/v1",
        "runtime": {"adapter":"shell-stub", "model":"opaque deployment", "reasoning_effort":null},
        "turns": [
            {"final_message":"first", "assistant_calls":0, "actions":[]},
            {"final_message":"last", "assistant_calls":0, "actions":[]}
        ]
    });
    let vendor = root.path().join("vendor");
    fs::create_dir_all(vendor.join("current"))?;
    symlink(vendor.join("current"), root.path().join("link"))?;
    let executable = vendor.join("unrelated-executable");
    let requested = root.path().join("link/../unrelated-executable");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\nset -eu\ncat > observed-request.json\nprintf '%s' \"$TMPDIR\" > observed-tmp\nprintf '%s\\n' '{response}'\n"
        ),
    )?;
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
    let resolved = adapter::resolve_executable(&requested.to_string_lossy())?;
    let request = adapter::request(
        root.path(),
        &run_dir,
        vec!["first".to_owned(), "last".to_owned()],
    );
    let bytes = adapter::run(&resolved, &run_dir, &request)?;
    let observed: serde_json::Value =
        serde_json::from_slice(&fs::read(workspace.join("observed-request.json"))?)?;
    ensure!(
        observed == serde_json::to_value(&request)?,
        "request changed across stdin boundary"
    );
    ensure!(
        fs::read_to_string(workspace.join("observed-tmp"))?
            == run_dir.join("tmp").to_string_lossy(),
        "adapter host TMPDIR not private"
    );
    let parsed = output::parse(&bytes, &request)?;
    ensure!(parsed.final_message == "last", "final turn not retained");
    let receipt = RunResult {
        run_root: "<run-root>".to_owned(),
        scenario_count: 1,
        elapsed_seconds: 0.0,
        results: vec![JobResult {
            runtime: Some(parsed.runtime),
            passed: true,
            ..JobResult::default()
        }],
    };
    report::write_reduced(root.path(), "receipt", &receipt)?;
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("receipt.json"))?)?;
    ensure!(
        json.pointer("/scenario_results/0/runtime") == response.get("runtime"),
        "runtime receipt changed"
    );
    ensure!(
        fs::read_to_string(root.path().join("receipt.md"))?.contains("opaque deployment"),
        "opaque runtime missing from Markdown"
    );
    fs::write(
        &executable,
        "#!/bin/sh\nprintf 'adapter diagnosis' >&2\nexit 9\n",
    )?;
    ensure!(
        adapter::run(&resolved, &run_dir, &request).is_err(),
        "failed adapter accepted"
    );
    ensure!(
        fs::read_to_string(run_dir.join("adapter.stderr"))? == "adapter diagnosis",
        "failure diagnostics lost"
    );
    fs::remove_file(&executable)?;
    ensure!(
        adapter::resolve_executable(&requested.to_string_lossy()).is_err(),
        "missing adapter accepted"
    );
    Ok(())
}

#[test]
fn run_options_preserve_report_contract() -> Result<()> {
    let arguments = [
        "--adapter",
        "custom-agent",
        "--run-root",
        "run-root",
        "--scenario",
        "routine-agent-hygiene",
        "--report-dir",
        "docs/agent-eval-results",
        "--report-name",
        "siftwire-v0.2.0-candidate",
    ]
    .map(str::to_owned);
    let options = run::parse_options(&arguments)?;
    ensure!(
        options.report_dir == "docs/agent-eval-results",
        "report directory differs"
    );
    ensure!(
        options.report_name == "siftwire-v0.2.0-candidate",
        "report name differs"
    );
    ensure!(
        options.adapter == "custom-agent",
        "adapter selection changed"
    );
    ensure!(
        run::parse_options(&[]).is_err(),
        "implicit adapter selected"
    );
    let invalid = [
        "--adapter",
        "custom-agent",
        "--report-dir",
        "docs/agent-eval-results",
    ]
    .map(str::to_owned);
    ensure!(
        run::parse_options(&invalid).is_err(),
        "report directory without a name was accepted"
    );
    Ok(())
}

#[test]
fn report_name_rejects_path_traversal() {
    for name in ["../escape", "/tmp/escape", ".", "nested/report"] {
        let arguments = [
            "--adapter".to_owned(),
            "custom-agent".to_owned(),
            "--report-dir".to_owned(),
            "reports".to_owned(),
            "--report-name".to_owned(),
            name.to_owned(),
        ];
        assert!(
            run::parse_options(&arguments).is_err(),
            "unsafe report name was accepted: {name}"
        );
    }
}

#[test]
fn delivery_verification_matches_current_brief() -> Result<()> {
    let database = Connection::open_in_memory()?;
    database.execute("CREATE TABLE delivery (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL, message TEXT NOT NULL, delivered_at TEXT NOT NULL)", [])?;
    database.execute("CREATE TABLE delivery_plan (id TEXT PRIMARY KEY, run_id TEXT NOT NULL, message TEXT NOT NULL)", [])?;
    database.execute(
        "INSERT INTO delivery (run_id, message, delivered_at) VALUES ('run-1', ?1, ?2)",
        ["- [One](<https://example.com/1>)", "2026-04-23T01:00:00Z"],
    )?;
    let mut missing = Checker::new(&database);
    missing.prepared_deliveries();
    ensure!(
        !missing.finish().database_pass,
        "unprepared delivery passed verification"
    );
    database.execute(
        "INSERT INTO delivery_plan VALUES ('plan-1', 'run-1', 'Different body')",
        [],
    )?;
    let mut changed = Checker::new(&database);
    changed.prepared_deliveries();
    ensure!(
        !changed.finish().database_pass,
        "delivery differing from immutable plan passed verification"
    );
    database.execute(
        "UPDATE delivery_plan SET message = (SELECT message FROM delivery WHERE run_id = 'run-1')",
        [],
    )?;
    let mut checker = Checker::new(&database);
    checker.prepared_deliveries();
    checker.recorded_delivery("Current brief\n\n- [One](<https://example.com/1>)");
    let result = checker.finish();
    ensure!(
        result.passed,
        "delivery verification failed: {}",
        result.details
    );

    database.execute(
        "INSERT INTO delivery (run_id, message, delivered_at) VALUES ('old-run', 'NO_REPLY', '2026-04-22T01:00:00.1234Z')",
        [],
    )?;
    let answer = "Current brief\n\n- [One](<https://example.com/1>)\n\nPrevious brief (2026-04-22T01:00:00.123400Z)\n\nNO_REPLY";
    let mut history = Checker::new(&database);
    history.recorded_delivery(answer);
    ensure!(history.finish().passed, "unchanged history rejected");
    for changed in [
        answer.replace("One", "Rewritten"),
        answer.replace("NO_REPLY", "No news"),
    ] {
        let mut checker = Checker::new(&database);
        checker.recorded_delivery(&changed);
        ensure!(
            !checker.finish().assistant_pass,
            "rewritten body/history accepted"
        );
    }
    Ok(())
}

#[test]
fn delivery_verification_matches_selected_evidence() -> Result<()> {
    let database = Connection::open_in_memory()?;
    database.execute_batch(
        r#"CREATE TABLE brief_run_item (id INTEGER, run_id TEXT, category TEXT, title TEXT, url TEXT, kind TEXT);
        CREATE TABLE delivery_plan (run_id TEXT, candidate_indexes_json TEXT, items_json TEXT);
        CREATE TABLE delivery (id INTEGER, run_id TEXT);
        CREATE TABLE sent_item (delivery_id INTEGER, run_id TEXT, title TEXT, url TEXT, kind TEXT);
        INSERT INTO brief_run_item VALUES
            (1, 'run', 'must_include', 'Required', 'https://example.com/required', 'rss'),
            (2, 'run', 'candidate', 'Skipped', 'https://example.com/skipped', 'rss'),
            (3, 'run', 'candidate', 'Selected', 'https://example.com/selected', 'rss');
        INSERT INTO delivery_plan VALUES ('run', '[1]', '[
            {"run_item_ids":["1"],"title":"Required","url":"https://example.com/required","kind":"rss"},
            {"run_item_ids":["3"],"title":"Selected","url":"https://example.com/selected","kind":"rss"}
        ]');
        INSERT INTO delivery_plan VALUES ('empty-run', '[]', '[]');
        INSERT INTO delivery VALUES (1, 'run'), (2, 'empty-run');
        INSERT INTO sent_item VALUES
            (1, 'run', 'Required', 'https://example.com/required', 'rss'),
            (1, 'run', 'Selected', 'https://example.com/selected', 'rss');"#,
    )?;
    let mut checker = Checker::new(&database);
    checker.selected_evidence();
    let result = checker.finish();
    ensure!(
        result.passed,
        "selected evidence rejected: {}",
        result.details
    );
    for mutation in [
        "UPDATE delivery_plan SET candidate_indexes_json = '[0]'",
        "UPDATE delivery_plan SET items_json = json_remove(items_json, '$[0]')",
        "UPDATE delivery_plan SET items_json = json_set(items_json, '$[1].run_item_ids[0]', '2')",
        "UPDATE delivery_plan SET items_json = json_set(items_json, '$[1].title', 'Changed')",
        "UPDATE sent_item SET url = 'https://example.com/wrong'",
        "DELETE FROM sent_item",
        "INSERT INTO sent_item VALUES (1, 'run', 'Skipped', 'https://example.com/skipped', 'rss')",
    ] {
        database.execute_batch("SAVEPOINT corruption")?;
        database.execute(mutation, [])?;
        let mut checker = Checker::new(&database);
        checker.selected_evidence();
        ensure!(
            !checker.finish().database_pass,
            "invalid evidence accepted: {mutation}"
        );
        database.execute_batch("ROLLBACK TO corruption; RELEASE corruption")?;
    }
    Ok(())
}
