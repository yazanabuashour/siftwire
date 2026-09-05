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
    database.execute("CREATE TABLE delivery (id INTEGER PRIMARY KEY AUTOINCREMENT, message TEXT NOT NULL, delivered_at TEXT NOT NULL)", [])?;
    database.execute(
        "INSERT INTO delivery (message, delivered_at) VALUES (?1, ?2)",
        ["- [One](<https://example.com/1>)", "2026-04-23T01:00:00Z"],
    )?;
    let mut checker = Checker::new(&database);
    checker.bullet_count(
        "- [One](<https://example.com/1>)\n- [Two](<https://example.com/2>)",
        2,
    );
    checker.recorded_delivery("Current brief\n\n- [One](<https://example.com/1>)\n\nPrevious brief (2026-04-22T01:00:00Z)\n\n- [Old](<https://example.com/old>)");
    let result = checker.finish();
    ensure!(
        result.database_pass && result.assistant_pass,
        "delivery verification failed: {}",
        result.details
    );
    Ok(())
}
