use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Deserialize)]
struct ResultBody {
    rejected: bool,
    #[serde(default)]
    rejection_reason: String,
    summary: String,
    #[serde(default)]
    runtime_config: serde_json::Value,
    #[serde(default)]
    sources: serde_json::Value,
}

fn main() -> Result<()> {
    let (binary, database) = arguments()?;
    let mut child = Command::new(binary)
        .args(["config", "--db", database.as_str()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start siftwire config")?;
    let mut stdin = child.stdin.take().context("open siftwire standard input")?;
    stdin.write_all(b"{\"action\":\"inspect_config\"}\n")?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .context("wait for siftwire config")?;
    if !output.status.success() {
        bail!(
            "siftwire config: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let response: ResultBody =
        serde_json::from_slice(&output.stdout).context("decode siftwire result")?;
    if response.rejected {
        bail!("siftwire rejected request: {}", response.rejection_reason);
    }
    let displayed = serde_json::json!({
        "rejected": response.rejected,
        "summary": response.summary,
        "runtime_config": response.runtime_config,
        "sources": response.sources,
    });
    println!("{}", serde_json::to_string_pretty(&displayed)?);
    Ok(())
}

fn arguments() -> Result<(String, String)> {
    let mut binary = "siftwire".to_owned();
    let mut database = None;
    let mut values = std::env::args().skip(1);
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--binary" => binary = values.next().context("--binary requires a value")?,
            "--db" => database = Some(values.next().context("--db requires a value")?),
            _ => bail!("unknown argument {argument:?}"),
        }
    }
    Ok((binary, database.context("--db is required")?))
}
