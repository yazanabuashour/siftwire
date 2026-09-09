use std::collections::BTreeMap;

use serde_json::Value;

use crate::types::{Metrics, ParsedOutput};

type StringsByKey = BTreeMap<String, Vec<String>>;

pub fn parse(output: &[u8]) -> ParsedOutput {
    let mut parsed = ParsedOutput {
        metrics: Metrics::default(),
        final_message: String::new(),
        session_id: String::new(),
    };
    for raw_line in String::from_utf8_lossy(output).lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        process_event(&event, &mut parsed);
    }
    parsed
}

fn process_event(event: &Value, parsed: &mut ParsedOutput) {
    let mut values = StringsByKey::new();
    collect_strings("", event, &mut values);
    let event_types = values.get("type").map_or(&[][..], Vec::as_slice);
    let event_type = first_value(&values, "type");
    if (contains_exact(event_types, "thread.started") || contains_exact(event_types, "session"))
        && parsed.session_id.is_empty()
    {
        parsed.session_id = first_nonempty([
            first_value(&values, "thread_id"),
            first_value(&values, "session_id"),
            first_value(&values, "id"),
        ]);
    }
    if contains_value(event_types, "assistant") || contains_value(event_types, "agent_message") {
        parsed.metrics.assistant_calls = parsed.metrics.assistant_calls.saturating_add(1);
        let message = likely_assistant_message(&values);
        if !message.is_empty() {
            parsed.final_message = message;
        }
    }
    if event_type.contains("tool")
        || event_type.contains("exec")
        || contains_value(event_types, "command")
    {
        parsed.metrics.tool_calls = parsed.metrics.tool_calls.saturating_add(1);
    }
    for command in command_strings(&values) {
        parsed.metrics.command_executions = parsed.metrics.command_executions.saturating_add(1);
        update_hygiene(&mut parsed.metrics, &command);
    }
}

fn collect_strings(key: &str, value: &Value, output: &mut StringsByKey) {
    match value {
        Value::Object(object) => {
            for (child_key, child) in object {
                collect_strings(&child_key.to_lowercase(), child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_strings(key, child, output);
            }
        }
        Value::String(value) => output
            .entry(key.to_owned())
            .or_default()
            .push(value.to_owned()),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn first_value(values: &StringsByKey, key: &str) -> String {
    values
        .get(&key.to_lowercase())
        .and_then(|candidates| candidates.iter().find(|value| !value.trim().is_empty()))
        .map_or_else(String::new, |value| value.trim().to_owned())
}

fn first_nonempty<const N: usize>(values: [String; N]) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .map_or_else(String::new, |value| value.trim().to_owned())
}

fn contains_exact(values: &[String], wanted: &str) -> bool {
    values.iter().any(|value| value.trim() == wanted)
}

fn contains_value(values: &[String], wanted: &str) -> bool {
    let lower_wanted = wanted.to_lowercase();
    values
        .iter()
        .any(|value| value.to_lowercase().contains(&lower_wanted))
}

fn likely_assistant_message(values: &StringsByKey) -> String {
    for key in ["message", "content", "text"] {
        if let Some(candidates) = values.get(key)
            && let Some(candidate) = candidates
                .iter()
                .rev()
                .map(|value| value.trim())
                .find(|value| !value.is_empty() && !looks_like_command(value))
        {
            return candidate.to_owned();
        }
    }
    String::new()
}

fn command_strings(values: &StringsByKey) -> Vec<String> {
    values
        .iter()
        .filter(|(key, _)| {
            matches!(
                key.as_str(),
                "cmd" | "command" | "arguments" | "shell_command"
            )
        })
        .flat_map(|(key, candidates)| {
            candidates.iter().filter_map(move |candidate| {
                let trimmed = candidate.trim();
                (key == "command" || looks_like_command(trimmed)).then(|| trimmed.to_owned())
            })
        })
        .collect()
}

fn looks_like_command(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains("\n\n") {
        return false;
    }
    [
        "/bin/sh ",
        "/bin/zsh ",
        "siftwire ",
        "sqlite3",
        "rg ",
        "grep ",
        "find ",
        "cat ",
        "sed ",
        "ls ",
        "env",
        "printenv",
        "go ",
        "mise ",
        "./",
    ]
    .iter()
    .any(|prefix| trimmed == prefix.trim() || trimmed.starts_with(prefix))
}

fn update_hygiene(metrics: &mut Metrics, command: &str) {
    let lower = command.to_lowercase();
    if lower.contains("sqlite3") || lower.contains("select ") {
        metrics.direct_sqlite_access = true;
        add_evidence(metrics, "direct_sqlite", command);
    }
    if lower.contains("rg ") || lower.contains("grep ") || lower.contains("find ") {
        metrics.broad_repo_search = true;
        add_evidence(metrics, "broad_repo_search", command);
    }
    if lower.contains("env") || lower.contains("printenv") {
        metrics.environment_access = true;
        add_evidence(metrics, "environment_access", command);
    }
    if !allowed_skill_read(&lower)
        && !allowed_runner_command(&lower)
        && !allowed_skill_then_runner(&lower)
    {
        metrics.unexpected_command = true;
        add_evidence(metrics, "unexpected_command", command);
    }
}

fn add_evidence(metrics: &mut Metrics, label: &str, command: &str) {
    metrics.hygiene_evidence.push(format!("{label}: {command}"));
}

fn allowed_skill_then_runner(lower: &str) -> bool {
    let Some((reader, runner)) = trim_shell_wrapper(lower).split_once(" && ") else {
        return false;
    };
    // Only one literal read of the named skill may precede the runner call.
    let Some(reader) = reader.strip_suffix(" .agents/skills/siftwire/skill.md") else {
        return false;
    };
    let bounded_sed = reader
        .strip_prefix("sed -n '1,")
        .and_then(|value| value.strip_suffix("p'"))
        .is_some_and(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()));
    (reader == "cat" || bounded_sed) && allowed_runner_command(runner)
}

fn allowed_runner_command(lower: &str) -> bool {
    let body = trim_shell_wrapper(lower);
    if body.contains(" && ") && allowed_runner_chain(body) {
        return true;
    }
    if ["&&", "||", ";"]
        .iter()
        .any(|separator| body.contains(separator))
    {
        return false;
    }
    let body = trim_database_assignment(body);
    if body.starts_with("siftwire ") {
        if body.contains('\n') {
            return allowed_direct_quoted_heredoc(body);
        }
        if let Some((runner, payload)) = body.split_once("<<<") {
            return !has_shell_substitution(body)
                && runner_command_tail(runner.trim())
                && payload
                    .trim()
                    .strip_prefix('\'')
                    .and_then(|value| value.strip_suffix('\''))
                    .is_some_and(|value| !value.contains('\''));
        }
        return !has_shell_substitution(body) && runner_command_tail(body);
    }
    if body.matches('|').count() != 1 {
        return false;
    }
    let Some((producer, consumer)) = body.split_once('|') else {
        return false;
    };
    let producer = trim_database_assignment(producer.trim());
    if producer.starts_with("printf ") {
        let arguments = producer.strip_prefix("printf '%s\n' ").unwrap_or(producer);
        return !has_shell_substitution(body)
            && !arguments.contains('\n')
            && runner_command_tail(trim_database_assignment(consumer.trim()));
    }
    allowed_quoted_heredoc(producer, consumer)
}

fn allowed_runner_chain(body: &str) -> bool {
    // Chain only literal printf pipelines, never general shell commands or heredocs.
    body.split(" && ").all(|command| {
        let Some((producer, consumer)) = command.split_once(" | ") else {
            return false;
        };
        [
            "printf '%s' '",
            "printf '%s\n' '",
            "printf '%s\\n' '",
            "printf '%s\\\\n' '",
        ]
        .iter()
        .find_map(|prefix| producer.strip_prefix(prefix))
        .and_then(|payload| payload.strip_suffix('\''))
        .is_some_and(|payload| {
            !payload.contains(['\'', '\n', '\r']) && !has_shell_substitution(payload)
        }) && runner_command_tail(consumer)
    })
}

fn has_shell_substitution(command: &str) -> bool {
    command.contains(['$', '`']) || command.contains("<(") || command.contains(">(")
}

fn allowed_direct_quoted_heredoc(body: &str) -> bool {
    let Some((command, payload)) = body.split_once('\n') else {
        return false;
    };
    let Some((runner, declaration)) = command.rsplit_once(" <<") else {
        return false;
    };
    let Some(delimiter) = quoted_delimiter(declaration) else {
        return false;
    };
    valid_heredoc_payload(runner, payload, delimiter)
}

fn allowed_quoted_heredoc(producer: &str, consumer: &str) -> bool {
    let Some(delimiter) = quoted_heredoc_delimiter(producer) else {
        return false;
    };
    let Some((runner, payload)) = consumer.split_once('\n') else {
        return false;
    };
    valid_heredoc_payload(runner, payload, delimiter)
}

fn valid_heredoc_payload(runner: &str, payload: &str, delimiter: &str) -> bool {
    let mut lines = payload.lines();
    runner_command_tail(trim_database_assignment(runner.trim()))
        && lines.next_back().is_some_and(|line| {
            line.trim_matches(|character| matches!(character, '\'' | '"')) == delimiter
        })
        && lines.all(|line| line != delimiter)
}

fn quoted_heredoc_delimiter(producer: &str) -> Option<&str> {
    let value = producer.strip_prefix("cat ")?.trim().strip_prefix("<<")?;
    quoted_delimiter(value)
}

fn quoted_delimiter(value: &str) -> Option<&str> {
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') || !value.ends_with(quote) {
        return None;
    }
    let delimiter = value.strip_prefix(quote)?.strip_suffix(quote)?;
    (!delimiter.is_empty()
        && delimiter
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_'))
    .then_some(delimiter)
}

fn trim_database_assignment(command: &str) -> &str {
    command
        .strip_prefix("siftwire_database_path=")
        .and_then(|assignment| assignment.split_once(' '))
        .map_or(command, |(_value, body)| body.trim_start())
}

fn trim_shell_wrapper(command: &str) -> &str {
    let mut body = command;
    for prefix in [
        "/usr/bin/bash -c ",
        "/bin/bash -c ",
        "/usr/bin/zsh -lc ",
        "/bin/zsh -lc ",
    ] {
        if let Some(stripped) = body.strip_prefix(prefix) {
            body = stripped.trim();
            break;
        }
    }
    let body = body.trim();
    body.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            body.strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(body)
        .trim()
}

fn runner_command_tail(command: &str) -> bool {
    ["siftwire config", "siftwire brief"].iter().any(|allowed| {
        command
            .strip_prefix(allowed)
            .is_some_and(|tail| tail.trim().is_empty())
    })
}

fn allowed_skill_read(lower: &str) -> bool {
    // The dollar is literal only in these complete quoting/wrapper contexts.
    let literal_sed_address = matches!(
        lower,
        "sed -n '1,$p' .agents/skills/siftwire/skill.md"
            | "/usr/bin/bash -c \"sed -n '1,\"'$p'\"' .agents/skills/siftwire/skill.md\""
    );
    if !lower.contains(".agents/skills/siftwire/skill.md")
        && !lower.contains("skills/.system/siftwire/skill.md")
    {
        return false;
    }
    if [
        "sqlite3", "select ", " rg ", " grep ", " find ", " env", "printenv",
    ]
    .iter()
    .any(|forbidden| lower.contains(forbidden))
        || (has_shell_substitution(lower) && !literal_sed_address)
        || [";", "|"].iter().any(|separator| lower.contains(separator))
    {
        return false;
    }
    let without_prelude = lower.replace("pwd &&", "");
    !without_prelude.contains(['\n', '\r'])
        && !without_prelude.contains("&&")
        && (without_prelude.contains("sed ") || without_prelude.contains("cat "))
}

pub fn merge(mut left: Metrics, right: Metrics) -> Metrics {
    left.assistant_calls = left.assistant_calls.saturating_add(right.assistant_calls);
    left.tool_calls = left.tool_calls.saturating_add(right.tool_calls);
    left.command_executions = left
        .command_executions
        .saturating_add(right.command_executions);
    left.direct_sqlite_access |= right.direct_sqlite_access;
    left.broad_repo_search |= right.broad_repo_search;
    left.environment_access |= right.environment_access;
    left.unexpected_command |= right.unexpected_command;
    left.hygiene_evidence.extend(right.hygiene_evidence);
    left
}
