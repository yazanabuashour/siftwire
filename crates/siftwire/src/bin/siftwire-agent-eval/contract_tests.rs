use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use rusqlite::Connection;

use crate::types::{RunResult, Scenario, Turn};
use crate::verify_checks::Checker;
use crate::{codex, report, run};

#[test]
fn fast_model_snapshot_reaches_every_turn_and_report() -> Result<()> {
    let root = tempfile::tempdir()?;
    let helper = root.path().join("model-role");
    fs::write(
        &helper,
        "#!/bin/sh\n[ \"$*\" = 'fast --codex' ] || exit 1\nprintf '%s\\n' synthetic-fast\n",
    )?;
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700))?;
    let model = codex::resolve_fast_model(&helper)?;

    for prompts in [vec!["single"], vec!["first", "second"]] {
        let scenario = Scenario {
            id: "synthetic",
            turns: prompts
                .into_iter()
                .map(|prompt| Turn {
                    prompt: prompt.to_owned(),
                })
                .collect(),
        };
        for (index, turn) in scenario.turns.iter().enumerate() {
            let arguments = codex::args_for_turn(
                Path::new("run-root/workspace"),
                Path::new("run-root"),
                &scenario,
                turn,
                index.saturating_add(1),
                "session-123",
                &model,
            );
            ensure!(
                arguments
                    .windows(2)
                    .any(|pair| pair == ["-m", "synthetic-fast"]),
                "model snapshot missing: {arguments:?}"
            );
            ensure!(
                arguments
                    .windows(2)
                    .any(|pair| pair == ["-c", "model_reasoning_effort=\"medium\""]),
                "effort changed: {arguments:?}"
            );
        }
    }

    let receipt = RunResult {
        model,
        reasoning_effort: codex::REASONING_EFFORT.to_owned(),
        run_root: "<run-root>".to_owned(),
        codex_home: "<run-root>/codex-home".to_owned(),
        scenario_count: 0,
        results: Vec::new(),
        elapsed_seconds: 0.0,
    };
    report::write_reduced(root.path(), "receipt", &receipt)?;
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.path().join("receipt.json"))?)?;
    ensure!(
        json.get("model").and_then(serde_json::Value::as_str) == Some("synthetic-fast")
            && json
                .get("reasoning_effort")
                .and_then(serde_json::Value::as_str)
                == Some("medium"),
        "JSON model receipt differs"
    );
    let markdown = fs::read_to_string(root.path().join("receipt.md"))?;
    ensure!(
        markdown.contains("`synthetic-fast` via `model-role fast --codex`")
            && markdown.contains("Reasoning effort: `medium`"),
        "Markdown model receipt differs"
    );
    Ok(())
}

#[test]
fn fast_model_resolution_fails_without_fallback() -> Result<()> {
    let root = tempfile::tempdir()?;
    let helper = root.path().join("model-role");
    let missing = codex::resolve_fast_model(&helper)
        .err()
        .context("missing helper was accepted")?;
    ensure!(
        missing.to_string().contains("install model-role on PATH"),
        "missing-helper guidance absent"
    );
    fs::write(
        &helper,
        "#!/bin/sh\nprintf '%s\\n' synthetic-fast\nprintf '%s\\n' 'fast provider must be openai-codex' >&2\nexit 1\n",
    )?;
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700))?;
    let rejected = codex::resolve_fast_model(&helper)
        .err()
        .context("failed helper was accepted")?;
    ensure!(
        rejected
            .to_string()
            .contains("fast provider must be openai-codex")
            && rejected.to_string().contains("AI_MODEL_ROLES_FILE"),
        "helper diagnostic absent: {rejected}"
    );
    for output in [
        "",
        "\\n",
        "synthetic-fast",
        "synthetic-fast\\nextra\\n",
        " synthetic-fast\\n",
        "-option\\n",
        "\\0377\\n",
    ] {
        fs::write(&helper, format!("#!/bin/sh\nprintf '%b' '{output}'\n"))?;
        ensure!(
            codex::resolve_fast_model(&helper).is_err(),
            "malformed helper output accepted: {output}"
        );
    }
    Ok(())
}

#[test]
fn run_options_preserve_report_contract() -> Result<()> {
    let arguments = [
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
    let invalid = ["--report-dir", "docs/agent-eval-results"].map(str::to_owned);
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
