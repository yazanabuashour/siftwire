#![expect(
    clippy::panic_in_result_fn,
    reason = "process tests return Result for setup while assertions report contract failures"
)]

#[path = "process_contract/architecture.rs"]
mod architecture;

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
    runner_protocol: String,
    capabilities: Vec<String>,
    rejected: bool,
    #[serde(default)]
    runtime_config: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RunResult {
    runner_protocol: String,
    capabilities: Vec<String>,
    rejected: bool,
    run_id: String,
    #[serde(default)]
    delivery_message_scope: String,
    #[serde(default)]
    must_include: Vec<Item>,
    #[serde(default)]
    candidates: Vec<Item>,
    #[serde(default)]
    sports_section: String,
    #[serde(default)]
    sports_updates: Vec<serde_json::Value>,
    #[serde(default)]
    candidate_slots: usize,
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
    delivery_plan_id: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    html: String,
    #[serde(default)]
    prepared_items: Vec<Item>,
    #[serde(default)]
    rejection_reason: String,
    #[serde(default)]
    final_answer: String,
    #[serde(default)]
    sent_items: Vec<Item>,
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
    assert_eq!(configured.runner_protocol, "siftwire-runner/v3");
    assert!(
        configured
            .capabilities
            .iter()
            .any(|capability| capability == "prepared-delivery/v1"),
        "prepared delivery capability is missing"
    );

    let run: RunResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &json!({"action": "run_brief", "dry_run": false}),
        &environment,
    )?)?;
    assert!(!run.rejected, "run was rejected");
    assert_eq!(run.runner_protocol, "siftwire-runner/v3");
    assert!(
        run.capabilities
            .iter()
            .any(|capability| capability == "prepared-delivery/v1"),
        "run omitted prepared delivery capability"
    );
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
    let prepared =
        assert_normal_prepared_plan(&database_text, &run.run_id, &message, &environment)?;
    let request = json!({
        "action": "confirm_delivery",
        "run_id": run.run_id.clone(),
        "delivery_plan_id": prepared.delivery_plan_id
    });
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

    assert_delivery_conflict_and_followup_runs(&database_text, &run.run_id, &environment)?;
    assert_runs_report_selection_evidence(&database_text, &run.run_id, &message, &prepared.html)?;
    Ok(())
}

fn assert_delivery_conflict_and_followup_runs(
    database: &str,
    run_id: &str,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    let prepare_after_delivery: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "prepare_delivery", "run_id": run_id, "candidate_indexes": [0]}),
        environment,
    )?)?;
    assert!(
        prepare_after_delivery.rejected
            && prepare_after_delivery
                .rejection_reason
                .contains("already delivered"),
        "delivered run accepted another preparation"
    );
    let repeat: RunResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "run_brief", "dry_run": false}),
        environment,
    )?)?;
    assert!(repeat.candidates.is_empty(), "repeat candidate returned");
    assert!(
        repeat.suppressed_recent.is_empty(),
        "latest-seen repeat reached recent suppression"
    );
    let empty: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "prepare_delivery", "run_id": repeat.run_id, "candidate_indexes": []}),
        environment,
    )?)?;
    assert!(!empty.rejected, "empty preparation was rejected");
    assert_eq!(
        empty.message, "NO_REPLY",
        "empty brief must not invent content"
    );
    assert!(
        empty.html.is_empty(),
        "empty brief must not produce rich email"
    );
    let confirmed: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "confirm_delivery", "run_id": repeat.run_id, "delivery_plan_id": empty.delivery_plan_id}),
        environment,
    )?)?;
    assert!(!confirmed.rejected, "empty delivery was rejected");
    let dry: RunResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "run_brief", "dry_run": true}),
        environment,
    )?)?;
    assert!(!dry.rejected, "dry run was rejected");
    Ok(())
}

fn assert_normal_prepared_plan(
    database: &str,
    run_id: &str,
    message: &str,
    environment: &BTreeMap<&str, &str>,
) -> Result<DeliveryResult> {
    let unprepared = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", database],
        environment,
    )?;
    assert_eq!(
        unprepared.get("delivery_html"),
        Some(&serde_json::Value::Null),
        "undelivered run must expose null HTML"
    );
    let prepared: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "prepare_delivery", "run_id": run_id, "candidate_indexes": [0]}),
        environment,
    )?)?;
    assert!(!prepared.rejected, "normal delivery plan was rejected");
    assert_eq!(prepared.message, message, "prepared normal message differs");
    assert!(!prepared.html.is_empty(), "fixture must prepare rich HTML");
    let unconfirmed = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", database],
        environment,
    )?;
    assert_eq!(
        unconfirmed.get("delivery_html"),
        Some(&serde_json::Value::Null),
        "prepared HTML must stay hidden until confirmation"
    );
    let changed: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "prepare_delivery", "run_id": run_id, "candidate_indexes": []}),
        environment,
    )?)?;
    assert!(
        changed.rejected,
        "run accepted a second delivery plan with different candidates"
    );
    Ok(prepared)
}

fn assert_runs_report_selection_evidence(
    database_text: &str,
    run_id: &str,
    message: &str,
    html: &str,
) -> Result<()> {
    let configured: ConfigStatus = decode(invoke_json(
        &["config", "--db", database_text],
        &json!({"action": "set_brief_options", "sports_timezone": "Pacific/Auckland"}),
        &BTreeMap::new(),
    )?)?;
    assert!(!configured.rejected, "updated timezone was rejected");
    let listing = invoke_cli_json(
        &["runs", "list", "--json", "--db", database_text],
        &BTreeMap::new(),
    )?;
    let runs = listing
        .get("runs")
        .and_then(|runs| runs.as_array())
        .context("runs list payload differs")?;
    assert_eq!(runs.len(), 2, "dry runs must not persist run rows");
    assert!(
        runs.iter().all(|run| run.get("delivery_html").is_none()),
        "runs list must not expose delivery HTML"
    );

    let detail = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", database_text],
        &BTreeMap::new(),
    )?;
    assert_eq!(
        detail.get("delivery_html").and_then(|value| value.as_str()),
        Some(html),
        "confirmed HTML must exactly match the immutable prepared body"
    );
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

fn write_sports_schedule(temp: &TempDir) -> Result<url::Url> {
    let now = chrono::Utc::now();
    let upcoming = now
        .checked_add_signed(chrono::TimeDelta::days(6))
        .context("calculate upcoming fixture")?;
    let completed = now
        .checked_sub_signed(chrono::TimeDelta::days(1))
        .context("calculate completed fixture")?;
    let schedule = temp.path().join("schedule.json");
    fs::write(
        &schedule,
        serde_json::to_vec(&json!({
            "events": [
                {
                    "id": "upcoming-1",
                    "date": upcoming.to_rfc3339(),
                    "name": "Team Beta at Team Alpha",
                    "links": [{"href": "https://example.test/events/upcoming-1", "rel": ["web"]}],
                    "competitions": [{"competitors": [
                        {"homeAway": "home", "team": {"displayName": "Team Alpha"}},
                        {"homeAway": "away", "team": {"displayName": "Team Beta"}}
                    ]}]
                },
                {
                    "id": "final-1",
                    "date": completed.to_rfc3339(),
                    "name": "Team Gamma at Team Alpha",
                    "links": [{"href": "https://example.test/events/final-1", "rel": ["web"]}],
                    "status": {"type": {"completed": true}},
                    "competitions": [{"competitors": [
                        {"homeAway": "home", "score": "2", "winner": true, "team": {"displayName": "Team Alpha"}},
                        {"homeAway": "away", "score": "0", "winner": false, "team": {"displayName": "Team Gamma"}}
                    ]}]
                }
            ],
            "season": {"displayName": "Example League"}
        }))?,
    )?;
    url::Url::from_file_path(&schedule)
        .map_err(|()| anyhow::anyhow!("fixture path is not an absolute file URL"))
}

#[test]
fn sports_updates_use_their_own_recurring_section() -> Result<()> {
    let temp = TempDir::new()?;
    let schedule_url = write_sports_schedule(&temp)?;
    let database_text = temp
        .path()
        .join("sports.sqlite")
        .to_string_lossy()
        .into_owned();
    let mut environment = BTreeMap::new();
    environment.insert("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1");

    let configured: ConfigStatus = decode(invoke_json(
        &["config", "--db", &database_text],
        &json!({
            "action": "set_brief_options",
            "max_delivery_items": 5,
            "sports_pre_game_days": 7,
            "sports_post_game_days": 3,
            "sports_timezone": "UTC"
        }),
        &environment,
    )?)?;
    assert!(!configured.rejected, "sports options were rejected");
    assert_eq!(
        configured
            .runtime_config
            .get("sports_timezone")
            .map(String::as_str),
        Some("UTC"),
        "sports timezone was not stored"
    );

    let source: ConfigStatus = decode(invoke_json(
        &["config", "--db", &database_text],
        &json!({
            "action": "upsert_source",
            "source": {
                "key": "team_alpha", "label": "Team Alpha", "kind": "sports_schedule",
                "url": schedule_url, "section": "sports", "threshold": "medium",
                "schedule_format": "espn", "enabled": true
            }
        }),
        &environment,
    )?)?;
    assert!(!source.rejected, "sports source was rejected");

    let run: RunResult = decode(invoke_json(
        &["brief", "--db", &database_text],
        &json!({"action": "run_brief", "dry_run": false}),
        &environment,
    )?)?;
    assert_eq!(
        run.must_include.len(),
        1,
        "legacy upcoming fixture compatibility changed"
    );
    assert!(
        run.candidates.is_empty(),
        "sports entered candidate selection"
    );
    assert_eq!(
        run.candidate_slots, 5,
        "sports consumed normal candidate capacity"
    );
    assert_eq!(run.sports_updates.len(), 2, "sports updates are incomplete");
    assert!(run.sports_section.contains("### Upcoming fixtures"));
    assert!(run.sports_section.contains("### Recent results"));
    assert_sports_prepared_plan(&database_text, &run, &environment)?;
    Ok(())
}

fn assert_sports_prepared_plan(
    database: &str,
    run: &RunResult,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    let prepared: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action": "prepare_delivery", "run_id": run.run_id, "candidate_indexes": []}),
        environment,
    )?)?;
    assert!(!prepared.rejected, "sports delivery plan was rejected");
    assert_eq!(
        prepared.message, run.sports_section,
        "prepared message duplicated or omitted sports"
    );
    assert!(
        prepared.text.contains("Upcoming fixtures"),
        "plain text omitted upcoming fixtures"
    );
    assert!(
        prepared.text.contains("Recent results"),
        "plain text omitted recent results"
    );
    assert!(
        prepared.html.contains(">Sports</h2>"),
        "HTML omitted sports section"
    );
    assert_eq!(
        prepared.prepared_items.len(),
        2,
        "prepared sports item evidence differs"
    );
    let delivery: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({
            "action": "confirm_delivery",
            "run_id": run.run_id,
            "delivery_plan_id": prepared.delivery_plan_id
        }),
        environment,
    )?)?;
    assert!(!delivery.rejected, "sports delivery was rejected");
    assert_eq!(
        delivery.sent_items.len(),
        2,
        "confirmed sports item evidence differs"
    );
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
fn retired_delivery_writes_fail_but_recorded_messages_remain_readable() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("legacy.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::new();
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"init"}),
        &environment,
    )?;
    let db = rusqlite::Connection::open(&database)?;
    db.execute_batch("INSERT INTO brief_run (id,started_at,dry_run,status,summary) VALUES ('old','2026-01-01T00:00:00Z',0,'ok','recorded'); INSERT INTO delivery (id,run_id,message,delivered_at) VALUES (1,'old','Exact legacy body','2026-01-01T00:01:00Z'); INSERT INTO delivery_once VALUES ('old','Exact legacy body',1);")?;
    let retired = invoke_json(
        &["brief", "--db", &database],
        &json!({"action":"record_delivery","run_id":"old"}),
        &environment,
    )?;
    assert_eq!(
        retired.get("rejected"),
        Some(&json!(true)),
        "manual delivery action remained writable"
    );
    let body = invoke(
        &["brief", "--db", &database],
        Some(r#"{"action":"record_delivery","run_id":"old","message":"changed"}"#),
        &environment,
    )?;
    assert_eq!(
        body.status.code(),
        Some(1),
        "removed message field must fail decoding"
    );
    assert!(body.stdout.is_empty(), "decode failure emitted a result");
    let detail = invoke_cli_json(
        &["runs", "show", "old", "--json", "--db", &database],
        &environment,
    )?;
    assert_eq!(
        detail.pointer("/run/message"),
        Some(&json!("Exact legacy body")),
        "old body changed"
    );
    assert_eq!(
        detail.get("delivery_html"),
        Some(&serde_json::Value::Null),
        "old delivery invented HTML"
    );
    let removed = invoke(&["source", "list", "--db", &database], None, &environment)?;
    assert_eq!(
        removed.status.code(),
        Some(2),
        "removed source command remained available"
    );
    assert!(
        removed.stdout.is_empty(),
        "removed command emitted result JSON"
    );
    Ok(())
}
