use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow, ensure};
use serde_json::{Value, json};

use crate::adapter;
use crate::fixtures;
use crate::output;
use crate::run;
use crate::scenarios::RUNNER_ONLY_INSTRUCTION;
use crate::types::{ADAPTER_PROTOCOL, ParsedOutput, Request, RunOptions, Scenario, Turn};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn test_directory(label: &str) -> Result<PathBuf> {
    let nonce = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "siftwire-agent-eval-{label}-{}-{nonce}",
        std::process::id()
    ));
    if path.try_exists()? {
        fs::remove_dir_all(&path)?;
    }
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn scenario(id: &'static str, prompts: &[&str]) -> Scenario {
    Scenario {
        id,
        turns: prompts
            .iter()
            .map(|prompt| Turn {
                prompt: (*prompt).to_owned(),
            })
            .collect(),
    }
}

#[test]
fn adapter_request_keeps_scenario_prompts_and_artifacts_together() {
    let root = Path::new("run-root");
    let run_dir = root.join("scenario");
    let request = adapter::request(root, &run_dir, vec!["first".into(), "second".into()]);
    assert_eq!(request.protocol, ADAPTER_PROTOCOL);
    assert_eq!(request.workspace, "run-root/scenario/workspace");
    assert_eq!(
        request.skill_path,
        "run-root/scenario/workspace/.agents/skills/siftwire/SKILL.md"
    );
    assert_eq!(request.artifact_dir, "run-root/scenario/adapter");
    assert_eq!(request.prompts.len(), 2);
    for (prompt, original) in request.prompts.iter().zip(["first", "second"]) {
        assert!(prompt.starts_with(original), "original prompt changed");
        assert!(
            prompt.ends_with(RUNNER_ONLY_INSTRUCTION),
            "runner-only instruction missing"
        );
        assert_eq!(prompt.matches(RUNNER_ONLY_INSTRUCTION).count(), 1);
    }
    assert_eq!(request.tool_env, adapter::tool_env(root, &run_dir));
}

#[test]
fn eval_tool_environment_excludes_maintainer_state() {
    let values = adapter::tool_env(Path::new("run-root"), Path::new("run-root/scenario"));
    for forbidden in [
        "AWS_SECRET_ACCESS_KEY",
        "AI_MODEL_ROLES_FILE",
        "XDG_CONFIG_HOME",
        "PI_CODING_AGENT_DIR",
        "PI_SESSION_FILE",
        "BASH_ENV",
    ] {
        assert!(
            !values.contains_key(forbidden),
            "forbidden tool environment key: {forbidden}"
        );
    }
    assert_eq!(
        values.get("SIFTWIRE_DATABASE_PATH").map(String::as_str),
        Some("run-root/scenario/siftwire.sqlite")
    );
    assert_eq!(
        values.get("HOME").map(String::as_str),
        Some("run-root/scenario/home")
    );
    assert_eq!(
        values.get("PATH").map(String::as_str),
        Some("run-root/bin:/usr/bin:/bin")
    );
}

#[test]
fn fixtures_rewrite_feed_missing_and_generated_urls() -> Result<()> {
    let root = test_directory("fixtures")?;
    let started = chrono::Utc::now().timestamp();
    let prepared = fixtures::prepare(
        scenario(
            "configured-max-delivery-items",
            &[
                "Use https://github.blog/feed/, https://example.com/siftwire-missing.xml, https://example.com/siftwire-limit-1.xml, https://example.com/siftwire-limit-2.xml, and https://example.com/siftwire-limit-3.xml named Fixture Outlet.",
            ],
        ),
        &root,
    )?;
    let prompt = prepared
        .turns
        .first()
        .map_or("", |turn| turn.prompt.as_str());
    ensure!(
        !prompt.contains("https://github.blog/feed/"),
        "feed URL was not rewritten: {prompt}"
    );
    ensure!(
        prompt.matches("file://").count() == 5,
        "all fixture URLs must be local: {prompt}"
    );
    ensure!(
        !root.join("fixtures/missing.xml").try_exists()?,
        "missing fixture must stay absent"
    );
    ensure!(
        fs::read_to_string(root.join("fixtures/github-blog.xml"))?
            .contains("SiftWire fixture story - Fixture Outlet")
            && prompt.contains("named Fixture Outlet"),
        "title-suffix publisher does not match the outlet policy"
    );
    fixtures::prepare(
        scenario("brief-run-history", &["Use the synthetic history feeds."]),
        &root,
    )?;
    let finished = chrono::Utc::now().timestamp();
    for entry in fs::read_dir(root.join("fixtures"))? {
        let content = fs::read_to_string(entry?.path())?;
        let published = content
            .split_once("<pubDate>")
            .and_then(|(_, rest)| rest.split_once("</pubDate>"))
            .map(|(published, _)| published)
            .ok_or_else(|| anyhow!("fixture publication date missing"))?;
        let timestamp = chrono::DateTime::parse_from_rfc2822(published)?.timestamp();
        ensure!(
            (started..=finished).contains(&timestamp),
            "fixture publication date must reflect creation time: {published}"
        );
    }
    let github = scenario(
        "github-release-source-config",
        &["Configure repository openai/codex with key codex-releases."],
    );
    let prepared_github = fixtures::prepare(github.clone(), &root)?;
    ensure!(
        prepared_github.turns.first().map(|turn| &turn.prompt)
            == github.turns.first().map(|turn| &turn.prompt)
            && !root.join("fixtures/codex-releases.json").try_exists()?,
        "GitHub config fixture reintroduced a custom source URL"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn run_root_rejects_unmarked_existing_directory_without_deleting_it() -> Result<()> {
    let parent = test_directory("unmarked-run-root")?;
    let repository = parent.join("repository");
    let shared = parent.join("shared");
    fs::create_dir_all(&repository)?;
    fs::create_dir_all(&shared)?;
    let sentinel = shared.join("brief-run-history");
    fs::create_dir_all(&sentinel)?;
    let options = RunOptions {
        run_root: shared.to_string_lossy().into_owned(),
        ..RunOptions::default()
    };
    ensure!(
        run::prepare_run_root(&options, &repository).is_err(),
        "unmarked run root was accepted"
    );
    ensure!(sentinel.try_exists()?, "unrelated data was deleted");
    fs::remove_dir_all(parent)?;
    Ok(())
}

#[test]
fn run_root_lock_rejects_concurrent_owner() -> Result<()> {
    let parent = test_directory("run-root-lock")?;
    let repository = parent.join("repository");
    fs::create_dir_all(&repository)?;
    let root = parent.join("owned");
    let options = RunOptions {
        run_root: root.to_string_lossy().into_owned(),
        ..RunOptions::default()
    };
    let first = run::prepare_run_root(&options, &repository)?;
    ensure!(
        run::prepare_run_root(&options, &repository).is_err(),
        "concurrent run-root owner was accepted"
    );
    drop(first);
    let next = run::prepare_run_root(&options, &repository)?;
    drop(next);
    fs::remove_dir_all(parent)?;
    Ok(())
}

fn request(turns: usize) -> Request {
    adapter::request(
        Path::new("run-root"),
        Path::new("run-root/scenario"),
        vec!["prompt".to_owned(); turns],
    )
}

fn adapter_result(turns: Value) -> Value {
    Value::Object(serde_json::Map::from_iter([
        ("protocol".to_owned(), json!(ADAPTER_PROTOCOL)),
        (
            "runtime".to_owned(),
            json!({"adapter": "fixture adapter", "model": null, "reasoning_effort": null}),
        ),
        ("turns".to_owned(), turns),
    ]))
}

fn agent_turn(final_message: &str, assistant_calls: Option<usize>, actions: Value) -> Value {
    Value::Object(serde_json::Map::from_iter([
        ("final_message".to_owned(), json!(final_message)),
        ("assistant_calls".to_owned(), json!(assistant_calls)),
        ("actions".to_owned(), actions),
    ]))
}

fn parse_result(result: &Value, request: &Request) -> Result<ParsedOutput> {
    output::parse(&serde_json::to_vec(result)?, request)
}

fn parse_action(action: Value) -> Result<ParsedOutput> {
    parse_result(
        &adapter_result(json!([agent_turn(
            "NO_REPLY",
            Some(2),
            Value::Array(vec![action])
        )])),
        &request(1),
    )
}

#[test]
fn output_parser_checks_all_turns_and_preserves_last_body_and_opaque_runtime() -> Result<()> {
    let body = "  siftwire brief\r\n\u{0085}\u{2028}\u{2029}\nNO_REPLY  \n";
    let mut result = adapter_result(json!([
        agent_turn(
            "earlier answer",
            Some(2),
            json!([
                {"kind": "command", "command": "sqlite3 siftwire.sqlite 'select * from brief_source'"},
                {"kind": "command", "command": "rg secret; printenv"},
                {"kind": "read", "path": "secret"},
                {"kind": "other", "name": "future_tool"}
            ])
        ),
        agent_turn(
            body,
            Some(3),
            json!([
                {"kind": "command", "command": "siftwire brief"}
            ])
        )
    ]));
    let runtime = json!({
        "adapter": "fixture adapter",
        "model": "opaque model ID: no provider syntax required",
        "reasoning_effort": "adapter-defined effort"
    });
    *result.get_mut("runtime").context("runtime missing")? = runtime.clone();
    let parsed = parse_result(&result, &request(2))?;
    ensure!(parsed.final_message == body, "final body changed");
    ensure!(
        serde_json::to_value(parsed.runtime)? == runtime,
        "opaque runtime changed"
    );
    ensure!(
        parsed.metrics.assistant_calls == Some(5),
        "assistant count differs"
    );
    ensure!(parsed.metrics.tool_calls == 5, "action count differs");
    ensure!(
        parsed.metrics.command_executions == 3,
        "command count differs"
    );
    ensure!(
        parsed.metrics.direct_sqlite_access,
        "earlier SQLite access missed"
    );
    ensure!(
        parsed.metrics.broad_repo_search,
        "earlier broad search missed"
    );
    ensure!(
        parsed.metrics.environment_access,
        "earlier environment access missed"
    );
    ensure!(
        parsed.metrics.unexpected_command,
        "earlier unexpected action missed"
    );
    for evidence in ["unexpected_read: secret", "unexpected_tool: future_tool"] {
        ensure!(
            parsed
                .metrics
                .hygiene_evidence
                .iter()
                .any(|item| item == evidence),
            "missing evidence: {evidence}"
        );
    }
    Ok(())
}

#[test]
fn output_parser_preserves_unknown_assistant_counts() -> Result<()> {
    for (counts, expected) in [
        (vec![Some(0)], Some(0)),
        (vec![Some(2), None], None),
        (vec![None, Some(3)], None),
    ] {
        let turns = counts
            .iter()
            .map(|count| agent_turn("NO_REPLY", *count, json!([])))
            .collect::<Vec<_>>();
        let parsed = parse_result(&adapter_result(json!(turns)), &request(counts.len()))?;
        ensure!(
            parsed.metrics.assistant_calls == expected,
            "count changed for {counts:?}"
        );
        ensure!(
            parsed.runtime.model.is_none() && parsed.runtime.reasoning_effort.is_none(),
            "null runtime fields changed"
        );
    }
    Ok(())
}

#[test]
fn output_parser_requires_one_strict_result_object() -> Result<()> {
    let result = adapter_result(json!([agent_turn("NO_REPLY", Some(1), json!([]))]));
    let text = serde_json::to_string(&result)?;
    output::parse(format!(" \n{text}\n ").as_bytes(), &request(1))?;
    for raw in [
        "not json".to_owned(),
        "[]".to_owned(),
        r#"{"type":"agent_end","messages":[]}"#.to_owned(),
        format!("{text}\n{text}"),
    ] {
        ensure!(
            output::parse(raw.as_bytes(), &request(1)).is_err(),
            "accepted: {raw}"
        );
    }
    for pointer in ["", "/runtime", "/turns/0"] {
        let mut unknown = result.clone();
        unknown
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .context("fixture object missing")?
            .insert("native_event".to_owned(), json!({}));
        ensure!(
            parse_result(&unknown, &request(1)).is_err(),
            "unknown field accepted at {pointer}"
        );
    }
    for pointer in [
        "/protocol",
        "/runtime/adapter",
        "/runtime/model",
        "/runtime/reasoning_effort",
        "/turns/0/final_message",
        "/turns/0/assistant_calls",
        "/turns/0/actions",
    ] {
        let mut missing = result.clone();
        let (parent, field) = pointer.rsplit_once('/').context("field pointer missing")?;
        missing
            .pointer_mut(parent)
            .and_then(Value::as_object_mut)
            .context("fixture object missing")?
            .remove(field);
        ensure!(
            parse_result(&missing, &request(1)).is_err(),
            "missing field accepted: {pointer}"
        );
    }
    let mut wrong_protocol = result.clone();
    *wrong_protocol
        .get_mut("protocol")
        .context("protocol missing")? = json!("siftwire-agent-eval/v2");
    ensure!(
        parse_result(&wrong_protocol, &request(1)).is_err(),
        "wrong protocol accepted"
    );
    ensure!(
        parse_result(&result, &request(2)).is_err(),
        "turn count mismatch accepted"
    );
    ensure!(
        parse_result(&adapter_result(json!([])), &request(1)).is_err(),
        "empty turns accepted"
    );
    ensure!(
        parse_result(&adapter_result(json!([])), &request(0)).is_err(),
        "empty prompts accepted"
    );
    Ok(())
}

#[test]
fn output_parser_rejects_empty_receipts_and_invalid_actions() -> Result<()> {
    let result = adapter_result(json!([
        agent_turn("earlier answer", Some(1), json!([])),
        agent_turn("NO_REPLY", Some(1), json!([]))
    ]));
    for pointer in [
        "/runtime/adapter",
        "/runtime/model",
        "/runtime/reasoning_effort",
        "/turns/0/final_message",
        "/turns/1/final_message",
    ] {
        let mut empty = result.clone();
        *empty
            .pointer_mut(pointer)
            .context("fixture field missing")? = json!(" \n\t");
        ensure!(
            parse_result(&empty, &request(2)).is_err(),
            "empty field accepted: {pointer}"
        );
    }
    for action in [
        json!({"kind": "command", "command": " "}),
        json!({"kind": "read", "path": " "}),
        json!({"kind": "other", "name": " "}),
        json!({"kind": "command"}),
        json!({"kind": "bash", "command": "siftwire brief"}),
        json!({"kind": "command", "command": "siftwire brief", "native_id": "extra"}),
    ] {
        ensure!(
            parse_action(action.clone()).is_err(),
            "invalid action accepted: {action}"
        );
    }
    Ok(())
}

#[test]
fn output_parser_allows_skill_and_runner_commands() -> Result<()> {
    for command in [
        "/bin/zsh -lc \"pwd && sed -n '1,220p' .agents/skills/siftwire/SKILL.md\"",
        "/bin/zsh -lc \"cat <<'JSON' | siftwire brief\n{\\\"action\\\":\\\"run_brief\\\",\\\"dry_run\\\":false}\nJSON\"",
        "/bin/bash -c \"cat <<'JSON' | siftwire brief\n{\\\"action\\\":\\\"confirm_delivery\\\",\\\"delivery_plan_id\\\":\\\"plan `broken` $(literal)\\\"}\nJSON\"",
        "/usr/bin/bash -c \"siftwire brief <<'JSON'\n{\\\"action\\\":\\\"run_brief\\\"}\nJSON\"",
        "/usr/bin/bash -c \"SIFTWIRE_DATABASE_PATH=/tmp/eval printf '%s' '{\\\"action\\\":\\\"run_brief\\\"}' | siftwire brief\"",
        "/usr/bin/bash -c \"printf '%s' '{\\\"action\\\":\\\"run_brief\\\"}' | SIFTWIRE_DATABASE_PATH=/tmp/eval siftwire brief\"",
        "/usr/bin/bash -c \"printf '%s\n' '{\\\"action\\\":\\\"inspect_config\\\"}' | siftwire config\"",
        "/usr/bin/bash -c \"siftwire config <<< '{\\\"action\\\":\\\"inspect_config\\\"}'\"",
        "/usr/bin/bash -c \"sed -n '1,\"'$p'\"' .agents/skills/siftwire/SKILL.md\"",
        "sed -n '1,$p' .agents/skills/siftwire/SKILL.md",
        "/usr/bin/bash -c \"printf '%s\n' '{\\\"action\\\":\\\"replace_outlet_policies\\\",\\\"outlets\\\":[{\\\"name\\\":\\\"Fixture Outlet\\\",\\\"aliases\\\":[],\\\"policy\\\":\\\"watch\\\",\\\"enabled\\\":true}]}' | siftwire config && printf '%s\n' '{\\\"action\\\":\\\"upsert_source\\\",\\\"source\\\":{\\\"key\\\":\\\"github-blog\\\",\\\"label\\\":\\\"GitHub Blog\\\",\\\"kind\\\":\\\"rss\\\",\\\"url\\\":\\\"file:///eval/fixtures/github-blog.xml\\\",\\\"section\\\":\\\"technology\\\",\\\"threshold\\\":\\\"medium\\\",\\\"enabled\\\":true,\\\"outlet_extraction\\\":\\\"title_suffix\\\"}}' | siftwire config\"",
        "printf '%s' '{}' | siftwire config && printf '%s\\n' '{}' | siftwire brief",
        "/usr/bin/bash -c \"printf '%s\\\\n' '{}' | siftwire config && printf '%s\\\\n' '{}' | siftwire brief\"",
        "/usr/bin/bash -c \"sed -n '1,240p' .agents/skills/siftwire/SKILL.md && siftwire config <<'JSON'\n{\\\"action\\\":\\\"inspect_config\\\"}\nJSON\"",
    ] {
        let parsed = parse_action(json!({"kind": "command", "command": command}))?;
        ensure!(
            !parsed.metrics.unexpected_command,
            "allowed command was flagged: {command}"
        );
        ensure!(
            parsed.metrics.hygiene_evidence.is_empty(),
            "allowed command produced evidence: {command}"
        );
    }
    Ok(())
}

#[test]
fn output_parser_flags_compound_substituted_and_unrecognized_commands() -> Result<()> {
    for command in [
        "/bin/zsh -lc \"sed -n '1,220p' .agents/skills/siftwire/SKILL.md && sqlite3 siftwire.sqlite 'select * from brief_source'\"",
        "/usr/bin/bash -c \"python3 -c 'print(open(\\\"secret\\\").read())'\"",
        "/bin/bash -c \"printf '%s' '`cat secret`' | siftwire config\"",
        "/bin/bash -c \"printf '%s' '$(cat secret)' | siftwire config\"",
        "/bin/bash -c \"printf '%s' <(cat secret) | siftwire config\"",
        "/bin/bash -c \"printf '%s' >(cat secret) | siftwire config\"",
        "/bin/bash -c \"printf '%s' '$SECRET' | siftwire config\"",
        "/bin/bash -c \"cat <<JSON | siftwire config\n{\\\"action\\\":\\\"inspect_config\\\"}\nJSON\"",
        "/bin/bash -c \"siftwire brief <<JSON\n{\\\"action\\\":\\\"run_brief\\\"}\nJSON\"",
        "/bin/bash -c \"cat <<'JSON' | siftwire config\n{}\nJSON\ncat secret\nJSON\"",
        "/usr/bin/bash -c \"printf '%s\n' '{}'\ncat secret | siftwire config\"",
        "/usr/bin/bash -c \"printf '%s\n' '{}' | siftwire config\ncat secret\"",
        "/usr/bin/bash -c \"printf '%s\n' '$(cat secret)' | siftwire config\"",
        "/usr/bin/bash -c \"siftwire config <<< '{}' 'extra'\"",
        "/usr/bin/bash -c \"siftwire config <<< '$(cat secret)'\"",
        "/usr/bin/bash -c \"siftwire config <<< '{}'\ncat secret\"",
        "/usr/bin/bash -c \"sed -n '1,$p' .agents/skills/siftwire/SKILL.md && cat secret\"",
        "/usr/bin/bash -c \"sed -n \"$p\" .agents/skills/siftwire/SKILL.md\"",
        "sed -n p .agents/skills/siftwire/SKILL.md \"'1,$p'\"",
        "sed -n '1,\"'$p'\"' .agents/skills/siftwire/SKILL.md",
        "cat secret .agents/skills/siftwire/SKILL.md",
        "sed -n '1,240p' secret .agents/skills/siftwire/SKILL.md",
        "cat skills/.system/siftwire/SKILL.md",
        "cat secret .agents/skills/siftwire/SKILL.md && siftwire config",
        "printf '%s' '{}' | siftwire config && cat secret",
        "cat secret && printf '%s' '{}' | siftwire config",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}' < secret | siftwire config",
        "printf '%s' '{}' | siftwire config && printf '%s' '$(cat secret)' | siftwire config",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}' | siftwire config > secret",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}' | siftwire config; cat secret",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}' | siftwire config || cat secret",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}' | siftwire config & cat secret",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}'\ncat secret | siftwire config",
        "printf '%s' '{}' | siftwire config && printf '%s' '{}'; cat secret; printf '%s' '{}' | siftwire config",
        "printf '%s' '{}' | siftwire config && cat <<'JSON' | siftwire config\n{}\nJSON\ncat secret\nJSON",
        "sed -n '1,240p' secret .agents/skills/siftwire/SKILL.md && siftwire config",
        "sed -n '1,240p' .agents/skills/siftwire/SKILL.md && siftwire config && cat secret",
        "sed -n '1,240p' .agents/skills/siftwire/SKILL.md && siftwire config <<JSON\n{}\nJSON",
        "sed -n '1,240p' .agents/skills/siftwire/SKILL.md && siftwire config <<'JSON'\n{}\nJSON\ncat secret\nJSON",
    ] {
        let parsed = parse_action(json!({"kind": "command", "command": command}))?;
        ensure!(
            parsed.metrics.unexpected_command,
            "unrecognized command was allowed: {command}"
        );
    }
    Ok(())
}

#[test]
fn output_parser_limits_reads_and_flags_other_actions() -> Result<()> {
    for path in [
        ".agents/skills/siftwire/SKILL.md",
        "./.agents/skills/siftwire/SKILL.md",
        "run-root/scenario/workspace/.agents/skills/siftwire/SKILL.md",
    ] {
        let parsed = parse_action(json!({"kind": "read", "path": path}))?;
        ensure!(
            !parsed.metrics.has_hygiene_failure(),
            "named skill read flagged: {path}"
        );
        ensure!(
            parsed.metrics.tool_calls == 1 && parsed.metrics.command_executions == 0,
            "read counted as command"
        );
    }
    for path in [
        "secret",
        ".agents/skills/siftwire/skill.md",
        "skills/.system/siftwire/SKILL.md",
        "prefix/.agents/skills/siftwire/SKILL.md",
        "run-root/another/workspace/.agents/skills/siftwire/SKILL.md",
        "run-root/scenario/workspace/./.agents/skills/siftwire/SKILL.md",
        "run-root/scenario/workspace/../workspace/.agents/skills/siftwire/SKILL.md",
        ".agents/skills/siftwire/SKILL.md/../secret",
        ".agents/skills/siftwire/SKILL.md ",
    ] {
        let parsed = parse_action(json!({"kind": "read", "path": path}))?;
        ensure!(
            parsed.metrics.unexpected_command,
            "unapproved read accepted: {path}"
        );
        ensure!(
            parsed.metrics.hygiene_evidence == vec![format!("unexpected_read: {path}")],
            "read evidence differs"
        );
    }
    let parsed = parse_action(json!({"kind": "other", "name": "sqlite3 not a command"}))?;
    ensure!(parsed.metrics.unexpected_command, "other action ignored");
    ensure!(
        !parsed.metrics.direct_sqlite_access,
        "other action name parsed as command"
    );
    ensure!(
        parsed.metrics.tool_calls == 1 && parsed.metrics.command_executions == 0,
        "other action counted as command"
    );
    ensure!(
        parsed.metrics.hygiene_evidence == vec!["unexpected_tool: sqlite3 not a command"],
        "other action evidence differs"
    );
    Ok(())
}
