use anyhow::{Context, Result, ensure};

use crate::types::{ADAPTER_PROTOCOL, Action, AdapterResult, Metrics, ParsedOutput, Request};

pub fn parse(output: &[u8], request: &Request) -> Result<ParsedOutput> {
    let result: AdapterResult =
        serde_json::from_slice(output).context("invalid adapter result JSON")?;
    ensure!(
        result.protocol == ADAPTER_PROTOCOL,
        "adapter protocol mismatch"
    );
    ensure!(
        !request.prompts.is_empty(),
        "adapter request has no prompts"
    );
    ensure!(
        result.turns.len() == request.prompts.len(),
        "adapter turn count differs from request prompt count"
    );
    ensure!(
        !result.runtime.adapter.trim().is_empty(),
        "runtime adapter must be nonempty"
    );
    for (field, value) in [
        ("model", &result.runtime.model),
        ("reasoning_effort", &result.runtime.reasoning_effort),
    ] {
        ensure!(
            value.as_ref().is_none_or(|value| !value.trim().is_empty()),
            "runtime {field} must be null or nonempty"
        );
    }

    let mut metrics = Metrics {
        assistant_calls: Some(0),
        ..Metrics::default()
    };
    let mut final_message = String::new();
    for (index, turn) in result.turns.into_iter().enumerate() {
        ensure!(
            !turn.final_message.trim().is_empty(),
            "adapter turn {index} final message is empty"
        );
        metrics.assistant_calls = match (metrics.assistant_calls, turn.assistant_calls) {
            (Some(total), Some(count)) => Some(
                total
                    .checked_add(count)
                    .context("adapter assistant call count overflow")?,
            ),
            _ => None,
        };
        for action in turn.actions {
            metrics.tool_calls = metrics.tool_calls.saturating_add(1);
            match action {
                Action::Command { command } => {
                    ensure!(!command.trim().is_empty(), "command action is empty");
                    metrics.command_executions = metrics.command_executions.saturating_add(1);
                    update_hygiene(&mut metrics, &command);
                }
                Action::Read { path } => {
                    ensure!(!path.trim().is_empty(), "read action path is empty");
                    if path != request.skill_path
                        && path != ".agents/skills/siftwire/SKILL.md"
                        && path != "./.agents/skills/siftwire/SKILL.md"
                    {
                        metrics.unexpected_command = true;
                        add_evidence(&mut metrics, "unexpected_read", &path);
                    }
                }
                Action::Other { name } => {
                    ensure!(!name.trim().is_empty(), "other action name is empty");
                    metrics.unexpected_command = true;
                    add_evidence(&mut metrics, "unexpected_tool", &name);
                }
            }
        }
        final_message = turn.final_message;
    }
    Ok(ParsedOutput {
        metrics,
        final_message,
        runtime: result.runtime,
    })
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
    allowed_skill_read(reader) && allowed_runner_command(runner)
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
    if literal_sed_address {
        return true;
    }
    let body = trim_shell_wrapper(lower);
    let body = body.strip_prefix("pwd && ").unwrap_or(body);
    let Some(reader) = body.strip_suffix(" .agents/skills/siftwire/skill.md") else {
        return false;
    };
    let bounded_sed = reader
        .strip_prefix("sed -n '1,")
        .and_then(|value| value.strip_suffix("p'"))
        .is_some_and(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()));
    reader == "cat" || reader == "sed -n '1,$p'" || bounded_sed
}
