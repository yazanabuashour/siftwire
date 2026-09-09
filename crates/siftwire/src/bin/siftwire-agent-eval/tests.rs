use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, anyhow, ensure};
use serde_json::json;

use crate::codex;
use crate::fixtures;
use crate::output;
use crate::run;
use crate::scenarios::RUNNER_ONLY_INSTRUCTION;
use crate::types::{RunOptions, Scenario, Turn};

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
fn codex_arguments_preserve_isolation_and_resume() -> Result<()> {
    let single = scenario("single", &["single prompt"]);
    let single_turn = single
        .turns
        .first()
        .ok_or_else(|| anyhow!("single scenario has no turn"))?;
    let arguments = codex::args_for_turn(
        Path::new("run-root/single/workspace"),
        Path::new("run-root/single"),
        &single,
        single_turn,
        1,
        "",
        "synthetic-fast",
    );
    ensure!(
        arguments.iter().any(|value| value == "--ephemeral"),
        "single turn must be ephemeral: {arguments:?}"
    );
    ensure!(
        arguments
            .iter()
            .any(|value| value == "--ignore-user-config"),
        "user config must be ignored: {arguments:?}"
    );
    let prompt = arguments.last().map_or("", String::as_str);
    ensure!(
        prompt.contains(RUNNER_ONLY_INSTRUCTION)
            && prompt.contains("no real email")
            && prompt.contains("confirm_delivery")
            && prompt.contains("siftwire-runner/v4")
            && prompt.contains("prepared-delivery/v1")
            && prompt.contains("current-news/v1"),
        "runner-only or simulated transport instruction missing: {prompt}"
    );

    let multiple = scenario("multi", &["first", "second"]);
    let second_turn = multiple
        .turns
        .get(1)
        .ok_or_else(|| anyhow!("multi scenario has no second turn"))?;
    let resumed = codex::args_for_turn(
        Path::new("run-root/multi/workspace"),
        Path::new("run-root/multi"),
        &multiple,
        second_turn,
        2,
        "session-123",
        "synthetic-fast",
    );
    ensure!(
        !resumed.iter().any(|value| value == "--ephemeral"),
        "resume must not be ephemeral: {resumed:?}"
    );
    ensure!(
        resumed.iter().rev().nth(1).map(String::as_str) == Some("session-123"),
        "session ID must precede the prompt"
    );
    Ok(())
}

#[test]
fn eval_environment_excludes_maintainer_state() {
    let values = codex::eval_env(Path::new("run-root"), Path::new("run-root/scenario"));
    let keys = values
        .iter()
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    for forbidden in [
        "SIFTWIRE_DATABASE_PATH",
        "AWS_SECRET_ACCESS_KEY",
        "AI_MODEL_ROLES_FILE",
        "XDG_CONFIG_HOME",
    ] {
        assert!(
            !keys.iter().any(|key| key == forbidden),
            "forbidden environment key exposed: {keys:?}"
        );
    }
    assert!(
        values
            .iter()
            .any(|(key, value)| key == OsStr::new("CODEX_HOME")
                && value == OsStr::new("run-root/codex-home")),
        "isolated CODEX_HOME missing: {values:?}"
    );
}

#[test]
fn codex_home_links_only_dedicated_auth() -> Result<()> {
    let source = test_directory("codex-source")?;
    fs::create_dir_all(source.join("sessions"))?;
    fs::write(source.join("auth.json"), br#"{"token":"secret"}"#)?;
    fs::write(source.join("config.toml"), b"model = \"custom\"")?;
    let root = test_directory("codex-root")?;
    codex::setup_home_from_source(&root, &source)?;
    let home = root.join("codex-home");
    let auth = home.join("auth.json");
    ensure!(
        auth.symlink_metadata()?.file_type().is_symlink(),
        "eval auth must be linked"
    );
    ensure!(
        !home.join("config.toml").try_exists()?,
        "config must not be copied"
    );
    ensure!(
        std::ops::BitAnd::bitand(home.metadata()?.permissions().mode(), 0o077) == 0,
        "Codex home must exclude group and other access"
    );
    fs::remove_dir_all(source)?;
    fs::remove_dir_all(root)?;
    Ok(())
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

#[test]
fn output_parser_detects_hygiene_and_final_message() -> Result<()> {
    let events = [
        json!({"type":"thread.started","thread_id":"thread-123"}),
        json!({"type":"tool.call","cmd":"sqlite3 siftwire.sqlite 'select * from brief_source'"}),
        json!({"type":"assistant.message","message":"NO_REPLY"}),
    ];
    let text = events
        .iter()
        .map(serde_json::to_string)
        .collect::<serde_json::Result<Vec<_>>>()?
        .join("\n");
    let parsed = output::parse(text.as_bytes());
    ensure!(parsed.session_id == "thread-123", "session ID differs");
    ensure!(parsed.final_message == "NO_REPLY", "final message differs");
    ensure!(
        parsed.metrics.assistant_calls == 1,
        "assistant count differs"
    );
    ensure!(
        parsed.metrics.direct_sqlite_access,
        "direct SQLite access was not detected"
    );
    ensure!(
        parsed.metrics.command_executions == 1,
        "command count differs"
    );
    Ok(())
}

#[test]
fn output_parser_allows_skill_and_runner_commands() {
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
        let event =
            json!({"type":"item.started","item":{"type":"command_execution","command":command}});
        let parsed = output::parse(event.to_string().as_bytes());
        assert!(
            !parsed.metrics.unexpected_command,
            "allowed command was flagged: {command}"
        );
        assert!(
            parsed.metrics.hygiene_evidence.is_empty(),
            "allowed command produced evidence: {command}"
        );
    }
}

#[test]
fn output_parser_flags_compound_substituted_and_unrecognized_commands() {
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
        let event =
            json!({"type":"item.started","item":{"type":"command_execution","command":command}});
        let parsed = output::parse(event.to_string().as_bytes());
        assert!(
            parsed.metrics.unexpected_command,
            "unrecognized command was allowed: {command}"
        );
    }
}
