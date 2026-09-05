use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};

use crate::filesystem::{copy_file, create_dir_all, display};
use crate::scenarios::RUNNER_ONLY_INSTRUCTION;
use crate::types::{Scenario, Turn};

const CODEX_HOME_ENV: &str = "SIFTWIRE_EVAL_CODEX_HOME";
pub const REASONING_EFFORT: &str = "medium";

pub fn resolve_fast_model(helper: &Path) -> Result<String> {
    let guidance = "resolve eval model with `model-role fast --codex`; install model-role on PATH and check the fast role in AI_MODEL_ROLES_FILE or ${XDG_CONFIG_HOME:-$HOME/.config}/ai/model-roles.json";
    let output = Command::new(helper)
        .args(["fast", "--codex"])
        .stdin(Stdio::null())
        .output()
        .context(guidance)?;
    if !output.status.success() {
        bail!(
            "{guidance}: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8(output.stdout).context(guidance)?;
    let model = stdout.strip_suffix('\n').unwrap_or_default();
    if model.is_empty()
        || model.starts_with('-')
        || model
            .chars()
            .any(|value| value.is_whitespace() || value.is_control())
    {
        bail!("{guidance}: expected a bare model slug followed by a newline");
    }
    Ok(model.to_owned())
}

pub struct CodexRun {
    pub output: Vec<u8>,
    pub error: Option<String>,
}

pub fn args_for_turn(
    workspace: &Path,
    run_dir: &Path,
    scenario: &Scenario,
    turn: &Turn,
    turn_number: usize,
    session_id: &str,
    model: &str,
) -> Vec<String> {
    let mut arguments = if scenario.turns.len() == 1 {
        initial_args(workspace, run_dir, true)
    } else if turn_number == 1 {
        initial_args(workspace, run_dir, false)
    } else {
        resume_args(workspace, run_dir)
    };
    arguments.extend(config(run_dir, model));
    if turn_number > 1 {
        arguments.push(session_id.to_owned());
    }
    arguments.push(eval_prompt(&turn.prompt));
    arguments
}

fn initial_args(workspace: &Path, run_dir: &Path, ephemeral: bool) -> Vec<String> {
    let mut arguments = vec!["exec".to_owned(), "--json".to_owned()];
    if ephemeral {
        arguments.push("--ephemeral".to_owned());
    }
    arguments.extend(common_args(workspace, run_dir));
    arguments
}

fn resume_args(workspace: &Path, run_dir: &Path) -> Vec<String> {
    let mut arguments = vec!["exec".to_owned()];
    arguments.extend([
        "-C".to_owned(),
        display(workspace),
        "--add-dir".to_owned(),
        display(run_dir),
        "--approve-for-me".to_owned(),
        "resume".to_owned(),
        "--json".to_owned(),
        "--skip-git-repo-check".to_owned(),
        "--ignore-user-config".to_owned(),
    ]);
    arguments
}

fn common_args(workspace: &Path, run_dir: &Path) -> Vec<String> {
    vec![
        "--approve-for-me".to_owned(),
        "--skip-git-repo-check".to_owned(),
        "--ignore-user-config".to_owned(),
        "-C".to_owned(),
        display(workspace),
        "--add-dir".to_owned(),
        display(run_dir),
    ]
}

fn config(run_dir: &Path, model: &str) -> Vec<String> {
    let run_root = run_dir.parent().unwrap_or(run_dir);
    let path = format!(
        "{}{}{}",
        display(&run_root.join("bin")),
        if cfg!(windows) { ';' } else { ':' },
        env::var("PATH").unwrap_or_default()
    );
    let shell_values = format!(
        "shell_environment_policy.set={{PATH={path:?},HOME={:?},TMPDIR={:?},SIFTWIRE_DATABASE_PATH={:?},SIFTWIRE_EVAL_ALLOW_FILE_URLS=\"1\",LANG=\"C.UTF-8\"}}",
        display(&run_dir.join("home")),
        display(&run_dir.join("tmp")),
        display(&run_dir.join("siftwire.sqlite")),
    );
    vec![
        "-m".to_owned(),
        model.to_owned(),
        "-c".to_owned(),
        format!("model_reasoning_effort={REASONING_EFFORT:?}"),
        "-c".to_owned(),
        "shell_environment_policy.inherit=\"none\"".to_owned(),
        "-c".to_owned(),
        shell_values,
        "-c".to_owned(),
        "shell_environment_policy.ignore_default_excludes=false".to_owned(),
        "-c".to_owned(),
        "shell_environment_policy.experimental_use_profile=false".to_owned(),
        "-c".to_owned(),
        "allow_login_shell=false".to_owned(),
    ]
}

fn eval_prompt(prompt: &str) -> String {
    format!("{}\n\n{RUNNER_ONLY_INSTRUCTION}", prompt.trim())
}

pub fn run(run_root: &Path, run_dir: &Path, arguments: &[String]) -> Result<CodexRun> {
    prepare_dirs(run_root, run_dir)?;
    let mut command = Command::new("codex");
    command
        .args(arguments)
        .env_clear()
        .envs(eval_env(run_root, run_dir));
    let capture_path = run_dir.join(".codex-output");
    let (output, status) = capture(&mut command, &capture_path)?;
    let error = match status {
        Ok(value) if value.success() => None,
        Ok(value) => Some(format!("codex {}: {value}", arguments.join(" "))),
        Err(error) => Some(format!("codex {}: {error}", arguments.join(" "))),
    };
    Ok(CodexRun { output, error })
}

fn capture(
    command: &mut Command,
    path: &Path,
) -> Result<(Vec<u8>, io::Result<std::process::ExitStatus>)> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("create command capture {}", path.display()))?;
    let stderr = file.try_clone().context("clone command capture")?;
    command
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(stderr));
    let status = command.status();
    let output =
        fs::read(path).with_context(|| format!("read command capture {}", path.display()))?;
    remove_capture(path)?;
    Ok((output, status))
}

fn remove_capture(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("remove command capture {}", path.display()))
        }
    }
}

pub fn eval_env(run_root: &Path, run_dir: &Path) -> Vec<(OsString, OsString)> {
    let mut values = vec![
        pair("CODEX_HOME", codex_home(run_root)),
        pair("HOME", run_root.join("codex-runtime-home")),
        pair("PATH", env::var_os("PATH").unwrap_or_default()),
        pair("TMPDIR", run_dir.join("tmp")),
        pair("LANG", "C.UTF-8"),
    ];
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        if let Some(value) = env::var_os(key) {
            values.push((OsString::from(key), value));
        }
    }
    values
}

fn pair(key: &str, value: impl Into<OsString>) -> (OsString, OsString) {
    (OsString::from(key), value.into())
}

fn prepare_dirs(run_root: &Path, run_dir: &Path) -> Result<()> {
    for directory in [
        run_root.join("codex-runtime-home"),
        run_dir.join("home"),
        run_dir.join("tmp"),
    ] {
        create_dir_all(&directory, 0o700)?;
    }
    Ok(())
}

pub fn codex_home(run_root: &Path) -> PathBuf {
    run_root.join("codex-home")
}

pub fn setup_home(run_root: &Path) -> Result<()> {
    let source = env::var(CODEX_HOME_ENV).unwrap_or_default();
    if source.trim().is_empty() {
        bail!(
            "{CODEX_HOME_ENV} must point to a dedicated authenticated Codex home; run CODEX_HOME=<path> codex login first"
        );
    }
    setup_home_from_source(run_root, Path::new(source.trim()))
}

pub fn setup_home_from_source(run_root: &Path, source_home: &Path) -> Result<()> {
    let source_auth = source_home.join("auth.json");
    match source_auth.try_exists() {
        Ok(true) => {}
        Ok(false) => bail!(
            "missing Codex auth at {}; authenticate the dedicated eval home",
            source_auth.display()
        ),
        Err(error) => return Err(error).context("inspect dedicated Codex auth"),
    }
    let target_home = codex_home(run_root);
    remove_dir_if_present(&target_home)?;
    create_dir_all(&target_home, 0o700)?;
    symlink(source_auth, target_home.join("auth.json")).context("link dedicated Codex auth")
}

fn remove_dir_if_present(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
    };
    let removed = if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    removed.with_context(|| format!("remove {}", path.display()))
}

pub fn install_skill(repo_root: &Path, workspace: &Path) -> Result<()> {
    copy_file(
        &repo_root.join("skills/siftwire/SKILL.md"),
        &workspace.join(".agents/skills/siftwire/SKILL.md"),
        0o644,
    )
}

pub fn build_binary(repo_root: &Path, run_root: &Path) -> Result<()> {
    let bin_dir = run_root.join("bin");
    create_dir_all(&bin_dir, 0o755)?;
    let build_dir = run_root.join("cargo-build");
    remove_dir_if_present(&build_dir)?;
    let mut command = Command::new("mise");
    command
        .args([
            "exec",
            "--",
            "cargo",
            "build",
            "--locked",
            "--release",
            "--bin",
            "siftwire",
            "--target-dir",
        ])
        .arg(&build_dir)
        .current_dir(repo_root);
    let (message, status) = capture(&mut command, &run_root.join(".build-output"))?;
    let build_result = match status {
        Ok(value) if value.success() => copy_file(
            &build_dir.join("release/siftwire"),
            &bin_dir.join("siftwire"),
            0o755,
        ),
        Ok(value) => Err(anyhow!(
            "build siftwire: {value}\n{}",
            String::from_utf8_lossy(&message).trim()
        )),
        Err(error) => Err(anyhow!("build siftwire: {error}")),
    };
    remove_dir_if_present(&build_dir)?;
    build_result
}
