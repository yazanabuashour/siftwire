use anyhow::{Result, ensure};
use rusqlite::Connection;

use crate::run;
use crate::verify_checks::Checker;

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
