use std::convert::Infallible;
use std::env;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result};

use crate::paths;

const LAUNCHER: &str = include_str!("../../automation/run-siftwire-automation.sh");
const RENDER_PROMPT: &str = include_str!("../../automation/siftwire-render.md");
const PROMPT_FILE: &str = "siftwire-render.md";
// Linux MAX_ARG_STRLEN: one argument cannot exceed 32 pages.
const _: () = assert!(LAUNCHER.len() < 128 * 1024, "bash -c script is too long");

/// Replaces this process with the launcher embedded in this binary.
pub fn run(arguments: &[String]) -> ExitCode {
    let Err(error) = launch(arguments);
    eprintln!("siftwire automation: {error:#}");
    ExitCode::FAILURE
}

fn launch(arguments: &[String]) -> Result<Infallible> {
    let mut command = Command::new("bash");
    command
        .args(["-c", LAUNCHER, "siftwire-automation"])
        .args(arguments);
    if env::var_os("SIFTWIRE_RENDER_PROMPT").is_none() {
        command.env(
            "SIFTWIRE_RENDER_PROMPT",
            install_prompt(&paths::automation_directory()?)?,
        );
    }
    if env::var_os("SIFTWIRE_BIN").is_none() {
        command.env(
            "SIFTWIRE_BIN",
            env::current_exe().context("resolve this binary")?,
        );
    }
    replace_process(&mut command)
}

#[cfg(unix)]
fn replace_process(command: &mut Command) -> Result<Infallible> {
    Err(command.exec()).context("start bash")
}

#[cfg(not(unix))]
fn replace_process(_command: &mut Command) -> Result<Infallible> {
    anyhow::bail!("the automation launcher requires a Unix host")
}

// Pi runs from this directory and trusts it, so it holds only the prompt.
fn install_prompt(directory: &Path) -> Result<PathBuf> {
    let path = directory.join(PROMPT_FILE);
    if fs::read(&path).is_ok_and(|current| current == RENDER_PROMPT.as_bytes()) {
        return Ok(path);
    }
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(directory)
        .with_context(|| format!("create {}", directory.display()))?;
    let temporary = directory.join(format!(".{PROMPT_FILE}.{}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
        .open(&temporary)
        .and_then(|mut file| file.write_all(RENDER_PROMPT.as_bytes()))
        .and_then(|()| fs::rename(&temporary, &path))
        .with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}
