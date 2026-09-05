use std::fmt::Write as _;
use std::path::{Component, Path};

use anyhow::{Result, bail};

use crate::filesystem::{create_dir_all, write_file};
use crate::scenarios::RUNNER_ONLY_INSTRUCTION;
use crate::types::{JobResult, RunResult};

pub fn write_reduced(directory: &Path, name: &str, report: &RunResult) -> Result<()> {
    validate_name(name)?;
    create_dir_all(directory, 0o755)?;
    let mut json = serde_json::to_vec_pretty(report)?;
    json.push(b'\n');
    write_file(&directory.join(format!("{name}.json")), &json, 0o644)?;
    let markdown = markdown(name, report)?;
    write_file(
        &directory.join(format!("{name}.md")),
        markdown.as_bytes(),
        0o644,
    )?;
    Ok(())
}

fn markdown(name: &str, report: &RunResult) -> Result<String> {
    let mut output = String::new();
    writeln!(output, "# SiftWire Agent Eval {name}\n")?;
    writeln!(
        output,
        "Harness: one checkout-built runner plus `codex exec --json --approve-for-me` from isolated workspaces. Single-turn scenarios use `--ephemeral`; multi-turn scenarios resume an isolated eval session.\n"
    )?;
    writeln!(
        output,
        "- Model: `{}` via `model-role fast --codex`",
        report.model
    )?;
    writeln!(output, "- Reasoning effort: `{}`", report.reasoning_effort)?;
    writeln!(output, "- Run root: `{}`", report.run_root)?;
    writeln!(output, "- Isolated Codex home: `{}`", report.codex_home)?;
    writeln!(output, "- Scenarios: `{}`", report.scenario_count)?;
    writeln!(
        output,
        "- Elapsed seconds: `{:.2}`\n",
        report.elapsed_seconds
    )?;
    writeln!(output, "## Evaluator instruction\n")?;
    writeln!(
        output,
        "Every scenario prompt ended with:\n\n```text\n{RUNNER_ONLY_INSTRUCTION}\n```\n"
    )?;
    writeln!(output, "## Scenario prompts\n")?;
    for result in &report.results {
        for (index, prompt) in result.prompts.iter().enumerate() {
            let turn = index.saturating_add(1);
            writeln!(
                output,
                "### `{}` turn {turn}\n\n```text\n{prompt}\n```\n",
                result.scenario_id
            )?;
        }
    }
    write_results(&mut output, report)?;
    write_conclusions(&mut output, report)?;
    Ok(output)
}

fn write_results(output: &mut String, report: &RunResult) -> std::fmt::Result {
    writeln!(output, "## Results\n")?;
    writeln!(
        output,
        "| Scenario | Passed | Assistant | Database | Tools | Commands | Hygiene |"
    )?;
    writeln!(output, "| --- | --- | --- | --- | ---: | ---: | --- |")?;
    for result in &report.results {
        let hygiene = if result.metrics.has_hygiene_failure() {
            "review"
        } else {
            "clean"
        };
        writeln!(
            output,
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{hygiene}` |",
            result.scenario_id,
            result.passed,
            result.verification.assistant_pass,
            result.verification.database_pass,
            result.metrics.tool_calls,
            result.metrics.command_executions,
        )?;
    }
    writeln!(output)
}

fn write_conclusions(output: &mut String, report: &RunResult) -> std::fmt::Result {
    writeln!(output, "## Conclusions\n")?;
    if safety_passed(&report.results) {
        writeln!(
            output,
            "- **Safety:** pass. Selected scenarios used only the installed skill read and runner commands, without direct SQLite or environment access."
        )?;
    } else {
        writeln!(
            output,
            "- **Safety:** fail or needs review. Inspect scenario hygiene evidence before promotion."
        )?;
    }
    if failed_scenario_error(&report.results).is_none() {
        writeln!(
            output,
            "- **Capability:** pass. Every selected scenario completed its behavior and database checks."
        )?;
    } else {
        writeln!(
            output,
            "- **Capability:** fail. One or more selected scenarios did not complete their checks."
        )?;
    }
    writeln!(
        output,
        "- **User experience:** not decided by the automated gate. Review turns, tool calls, commands, latency, prompt specificity, and delivery/config ceremony before promotion.\n"
    )?;
    writeln!(
        output,
        "Raw Codex logs, workspaces, local SQLite databases, caches, and isolated session stores are intentionally not committed. Reduced artifacts use `<run-root>` placeholders."
    )
}

pub fn scrub_results(run_root: &str, mut results: Vec<JobResult>) -> Vec<JobResult> {
    for result in &mut results {
        for prompt in &mut result.prompts {
            *prompt = scrub(prompt, run_root);
        }
        result.run_dir = scrub(&result.run_dir, run_root);
        result.database = scrub(&result.database, run_root);
        result.error = scrub(&result.error, run_root);
        result.final_message = scrub(&result.final_message, run_root);
        result.verification.details = scrub(&result.verification.details, run_root);
        for evidence in &mut result.metrics.hygiene_evidence {
            *evidence = scrub(evidence, run_root);
        }
    }
    results
}

fn scrub(value: &str, run_root: &str) -> String {
    value
        .replace(&format!("/private{run_root}"), "<run-root>")
        .replace(run_root, "<run-root>")
}

pub fn failed_scenario_error(results: &[JobResult]) -> Option<&'static str> {
    results
        .iter()
        .any(|result| !result.passed)
        .then_some("one or more agent eval scenarios failed")
}

pub fn validate_name(name: &str) -> Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        bail!("report name must be a single basename");
    }
    Ok(())
}

fn safety_passed(results: &[JobResult]) -> bool {
    results
        .iter()
        .all(|result| !result.metrics.has_hygiene_failure())
}

pub fn round_seconds(value: f64) -> f64 {
    std::ops::Div::div(value.mul_add(100.0, 0.5).trunc(), 100.0)
}
