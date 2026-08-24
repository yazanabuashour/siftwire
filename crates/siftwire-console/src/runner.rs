use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

/// One invocation of the installed SiftWire runner.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub runner_bin: String,
    pub database: Option<String>,
    /// Full argument vector after the binary name, e.g. `["config"]`.
    pub arguments: Vec<String>,
    /// Optional JSON request written to stdin for protocol verbs.
    pub request: Option<Value>,
    pub timeout: Duration,
}

#[derive(Debug)]
pub enum Outcome {
    /// Exit 0 result JSON without a domain rejection.
    Ok(Value),
    /// Exit 0 result flagged `rejected: true` by the domain.
    Rejected { reason: String },
    /// Exit 1 runtime failure; carries stderr diagnostics.
    Failed(String),
    /// Exit 2 command-line misuse.
    Usage(String),
}

/// Runs one one-shot runner invocation and classifies its exit.
///
/// # Errors
///
/// Returns an error when the binary cannot spawn, its pipes fail, exit-0
/// output is not valid JSON, or the call exceeds its timeout.
pub async fn invoke(invocation: &Invocation) -> Result<Outcome> {
    let mut command = Command::new(&invocation.runner_bin);
    command
        // A timed-out invocation must not survive as an orphan that could
        // still complete a configuration write.
        .kill_on_drop(true)
        .args(&invocation.arguments)
        .stdin(if invocation.request.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(database) = &invocation.database {
        command.args(["--db", database]);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("spawn runner {}", invocation.runner_bin))?;
    if let Some(request) = &invocation.request {
        let payload = serde_json::to_vec(request).context("encode runner request")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&payload)
                .await
                .context("write runner request")?;
            stdin
                .write_all(b"\n")
                .await
                .context("terminate runner request")?;
        }
    }
    let output = match tokio::time::timeout(invocation.timeout, child.wait_with_output()).await {
        Ok(result) => result.context("collect runner output")?,
        Err(_) => {
            return Err(anyhow::anyhow!(
                "runner did not finish within {:?}",
                invocation.timeout
            ));
        }
    };
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    match output.status.code() {
        Some(0) => {
            let value: Value =
                serde_json::from_slice(&output.stdout).context("decode runner result JSON")?;
            if value.get("rejected").and_then(Value::as_bool) == Some(true) {
                let reason = value
                    .get("rejection_reason")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown rejection")
                    .to_owned();
                return Ok(Outcome::Rejected { reason });
            }
            Ok(Outcome::Ok(value))
        }
        Some(2) => Ok(Outcome::Usage(stderr)),
        _ => Ok(Outcome::Failed(stderr)),
    }
}
