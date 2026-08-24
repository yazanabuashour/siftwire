#![expect(
    clippy::panic_in_result_fn,
    reason = "process tests return Result for setup while assertions report contract failures"
)]

use std::collections::BTreeMap;
use std::fs;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;

const BINARY: &str = env!("CARGO_BIN_EXE_siftwire");

#[derive(Deserialize)]
struct ConfigStatus {
    rejected: bool,
}

#[derive(Deserialize)]
struct RunResult {
    rejected: bool,
    run_id: String,
    #[serde(default)]
    delivery_message_scope: String,
    #[serde(default)]
    candidates: Vec<Item>,
    #[serde(default)]
    suppressed_recent: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Item {
    title: String,
    url: String,
}

#[derive(Deserialize)]
struct DeliveryResult {
    rejected: bool,
    #[serde(default)]
    rejection_reason: String,
    #[serde(default)]
    final_answer: String,
}

#[test]
fn process_framing_and_exits() -> Result<()> {
    let unknown = invoke(&["unknown"], None, &BTreeMap::new())?;
    assert_eq!(
        unknown.status.code(),
        Some(2),
        "unknown command: {unknown:?}"
    );
    assert!(
        text(&unknown.stderr).contains("unknown siftwire command"),
        "unknown diagnostic: {unknown:?}"
    );

    let temp = TempDir::new()?;
    let database = temp.path().join("siftwire.sqlite");
    let database_text = database.to_string_lossy().into_owned();
    let equals_database = format!("-db={database_text}");
    for arguments in [
        vec!["config", "-db", database_text.as_str()],
        vec!["config", equals_database.as_str()],
    ] {
        let output = invoke(
            &arguments,
            Some(r#"{"action":"inspect_config"}"#),
            &BTreeMap::new(),
        )?;
        assert!(
            output.status.success(),
            "Go-compatible database flag failed: {output:?}"
        );
    }

    for input in [
        "{\"action\":",
        "{\"action\":\"inspect_config\",\"unknown\":true}",
        "{\"action\":\"inspect_config\"}\n{\"action\":\"inspect_config\"}",
    ] {
        let output = invoke(
            &["config", "--db", &database_text],
            Some(input),
            &BTreeMap::new(),
        )?;
        assert_eq!(output.status.code(), Some(1), "input {input:?}: {output:?}");
        assert!(output.stdout.is_empty(), "input {input:?}: {output:?}");
        assert!(
            text(&output.stderr).contains("decode config request:"),
            "input {input:?}: {output:?}"
        );
    }
    Ok(())
}

#[test]
fn config_and_delivery_through_process_contract() -> Result<()> {
    let temp = TempDir::new()?;
    let feed = temp.path().join("feed.xml");
    fs::write(
        &feed,
        r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Fixture</title><item><title>Contract story</title><link>https://example.test/story</link><guid>story-1</guid><pubDate>Thu, 23 Apr 2026 01:00:00 GMT</pubDate></item></channel></rss>"#,
    )?;
    let feed_url = url::Url::from_file_path(&feed)
        .map_err(|()| anyhow::anyhow!("fixture path is not an absolute file URL"))?;
    let database = temp.path().join("contract.sqlite");
    let database_text = database.to_string_lossy().into_owned();
    let mut environment = BTreeMap::new();
    environment.insert("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1");
    let upsert = json!({
        "action": "upsert_source",
        "source": {
            "key": "fixture", "label": "Fixture", "kind": "rss",
            "url": feed_url, "section": "technology", "threshold": "medium",
            "enabled": true
        }
    });
    let configured: ConfigStatus = decode(invoke_json(
        &["config", "--db", &database_text],
        &upsert,
        &environment,
    )?)?;
    assert!(!configured.rejected, "source configuration was rejected");

    let run: RunResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &json!({"action": "run_brief", "dry_run": false}),
        &environment,
    )?)?;
    assert!(!run.rejected, "run was rejected");
    assert_eq!(run.candidates.len(), 1, "candidate count differs");
    assert_eq!(
        run.delivery_message_scope, "current_brief_only",
        "delivery message scope differs"
    );
    let item = run
        .candidates
        .first()
        .context("run did not return a candidate")?;
    let message = format!("- [{}](<{}>)", item.title, item.url);
    assert_history_wrapper_rejected(&database_text, &run.run_id, &message, &environment)?;
    let request =
        json!({"action": "record_delivery", "run_id": run.run_id.clone(), "message": message});
    let delivery: DeliveryResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &request,
        &environment,
    )?)?;
    assert!(!delivery.rejected, "delivery was rejected");
    assert_eq!(
        delivery.final_answer,
        format!("Current brief\n\n{message}"),
        "final answer differs"
    );
    let retry: DeliveryResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &request,
        &environment,
    )?)?;
    assert!(!retry.rejected, "identical retry was rejected");

    let conflict_request =
        json!({"action": "record_delivery", "run_id": run.run_id, "message": "different"});
    let conflict: DeliveryResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &conflict_request,
        &environment,
    )?)?;
    assert!(conflict.rejected, "changed delivery was accepted");
    assert!(
        conflict.rejection_reason.contains("different message"),
        "conflict reason differs: {}",
        conflict.rejection_reason
    );

    let repeat: RunResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &json!({"action": "run_brief", "dry_run": false}),
        &environment,
    )?)?;
    assert!(
        repeat.candidates.is_empty(),
        "repeat candidate was returned"
    );
    assert!(
        repeat.suppressed_recent.is_empty(),
        "latest-seen repeat reached recent suppression"
    );

    let dry: RunResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &json!({"action": "run_brief", "dry_run": true}),
        &environment,
    )?)?;
    assert!(!dry.rejected, "dry run was rejected");
    assert_runs_report_selection_evidence(&database_text, &run.run_id, &message)?;
    Ok(())
}

fn assert_runs_report_selection_evidence(
    database_text: &str,
    run_id: &str,
    message: &str,
) -> Result<()> {
    let listing = invoke_cli_json(
        &["runs", "list", "--json", "--db", database_text],
        &BTreeMap::new(),
    )?;
    let runs = listing
        .get("runs")
        .and_then(|runs| runs.as_array())
        .context("runs list payload differs")?;
    assert_eq!(runs.len(), 2, "dry runs must not persist run rows");

    let detail = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", database_text],
        &BTreeMap::new(),
    )?;
    let stored_message = detail
        .pointer("/run/message")
        .and_then(|message| message.as_str())
        .context("run delivery message missing")?;
    assert_eq!(stored_message, message, "delivered message differs");
    let selected_flags: Vec<bool> = detail
        .pointer("/candidates")
        .and_then(|items| items.as_array())
        .context("candidates payload differs")?
        .iter()
        .filter_map(|item| item.get("selected").and_then(serde_json::Value::as_bool))
        .collect();
    assert_eq!(
        selected_flags,
        vec![true],
        "candidate selection flags differ"
    );
    assert_eq!(
        detail
            .pointer("/sent_items")
            .and_then(|items| items.as_array())
            .map(Vec::len),
        Some(1),
        "sent item count differs"
    );
    let missing = invoke(
        &["runs", "show", "missing-run", "--db", database_text],
        None,
        &BTreeMap::new(),
    )?;
    assert_eq!(missing.status.code(), Some(1), "unknown run: {missing:?}");
    Ok(())
}

#[test]
fn default_database_rejects_empty_home_and_blank_overrides() -> Result<()> {
    for database in [None, Some(" ")] {
        let mut environment = BTreeMap::new();
        environment.insert("HOME", "");
        if let Some(database) = database {
            environment.insert("SIFTWIRE_DATABASE_PATH", database);
        }
        let output = invoke(
            &["config"],
            Some(r#"{"action":"inspect_config"}"#),
            &environment,
        )?;
        assert_eq!(
            output.status.code(),
            Some(1),
            "empty HOME selected a database: {output:?}"
        );
        assert!(
            text(&output.stderr).contains("HOME is not set or empty"),
            "empty HOME diagnostic changed: {output:?}"
        );
    }
    Ok(())
}

fn assert_history_wrapper_rejected(
    database: &str,
    run_id: &str,
    message: &str,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    let wrapped = json!({
        "action": "record_delivery",
        "run_id": run_id,
        "message": format!("Current brief\n\n{message}")
    });
    let result: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &wrapped,
        environment,
    )?)?;
    assert!(result.rejected, "history wrapper was recorded");
    assert!(
        result
            .rejection_reason
            .contains("only the current brief body"),
        "wrapper rejection differs: {}",
        result.rejection_reason
    );
    Ok(())
}

fn invoke_json(
    arguments: &[&str],
    request: &serde_json::Value,
    environment: &BTreeMap<&str, &str>,
) -> Result<serde_json::Value> {
    let input = serde_json::to_string(request)?;
    let output = invoke(arguments, Some(&input), environment)?;
    if !output.status.success() {
        anyhow::bail!("siftwire {arguments:?}: {}", text(&output.stderr));
    }
    serde_json::from_slice(&output.stdout).context("decode siftwire output")
}

fn invoke(
    arguments: &[&str],
    input: Option<&str>,
    environment: &BTreeMap<&str, &str>,
) -> Result<Output> {
    let mut command = Command::new(BINARY);
    command
        .args(arguments)
        .env_remove("SIFTWIRE_DATABASE_PATH")
        .env_remove("SIFTWIRE_EVAL_ALLOW_FILE_URLS")
        .env_remove("XDG_DATA_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in environment {
        command.env(key, value);
    }
    let mut child = command.spawn().context("start siftwire")?;
    if let Some(input) = input {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .context("open siftwire stdin")?
            .write_all(input.as_bytes())?;
    }
    child.wait_with_output().context("wait for siftwire")
}

fn invoke_cli_json(
    arguments: &[&str],
    environment: &BTreeMap<&str, &str>,
) -> Result<serde_json::Value> {
    let output = invoke(arguments, None, environment)?;
    if !output.status.success() {
        anyhow::bail!("siftwire {arguments:?}: {}", text(&output.stderr));
    }
    serde_json::from_slice(&output.stdout).context("decode siftwire output")
}

fn decode<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> Result<T> {
    serde_json::from_value(value).context("decode typed siftwire result")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn source_commands_validate_store_and_list() -> Result<()> {
    let temp = TempDir::new()?;
    let database_text = temp
        .path()
        .join("sources.sqlite")
        .to_string_lossy()
        .into_owned();
    let feed_url = url::Url::parse("https://fixture.test/feed.xml")?;

    let invalid = invoke(
        &["source", "add", "--db", &database_text],
        Some(
            r#"{"key":"Bad Key","label":"Bad","kind":"rss","url":"https://fixture.test/feed","section":"technology","enabled":true}"#,
        ),
        &BTreeMap::new(),
    )?;
    assert_eq!(invalid.status.code(), Some(1), "invalid key: {invalid:?}");

    let added = invoke_json(
        &["source", "add", "--json", "--db", &database_text],
        &json!({
            "key": "fixture", "label": "Fixture", "kind": "rss",
            "url": feed_url, "section": "technology", "threshold": "medium",
            "enabled": true
        }),
        &BTreeMap::new(),
    )?;
    assert_eq!(
        added.get("key").and_then(|key| key.as_str()),
        Some("fixture"),
        "stored source key differs"
    );

    let listing = invoke_cli_json(
        &["source", "list", "--json", "--db", &database_text],
        &BTreeMap::new(),
    )?;
    let keys: Vec<&str> = listing
        .get("sources")
        .and_then(|sources| sources.as_array())
        .map(|sources| {
            sources
                .iter()
                .filter_map(|source| source.get("key").and_then(|key| key.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(keys, vec!["fixture"], "listed keys differ");
    Ok(())
}
