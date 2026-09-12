use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::filesystem::{create_dir_all, display, write_file};
use crate::scenarios::RUNNER_ONLY_INSTRUCTION;
use crate::types::{ADAPTER_PROTOCOL, Request};

pub fn resolve_executable(value: &str) -> Result<PathBuf> {
    let candidates = if Path::new(value).components().count() > 1 {
        vec![PathBuf::from(value)]
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|directory| directory.join(value))
            .collect()
    };
    for path in candidates {
        if let Ok(metadata) = path.metadata()
            && metadata.is_file()
            && std::ops::BitAnd::bitand(metadata.permissions().mode(), 0o111) != 0
        {
            return Ok(env::current_dir()
                .context("resolve adapter working directory")?
                .join(path));
        }
    }
    bail!("adapter executable not found or not executable: {value}");
}

pub fn request(run_root: &Path, run_dir: &Path, prompts: Vec<String>) -> Request {
    let workspace = run_dir.join("workspace");
    Request {
        protocol: ADAPTER_PROTOCOL,
        skill_path: display(&workspace.join(".agents/skills/siftwire/SKILL.md")),
        workspace: display(&workspace),
        artifact_dir: display(&run_dir.join("adapter")),
        prompts: prompts
            .into_iter()
            .map(|prompt| format!("{}\n\n{RUNNER_ONLY_INSTRUCTION}", prompt.trim()))
            .collect(),
        tool_env: tool_env(run_root, run_dir),
    }
}

pub fn tool_env(run_root: &Path, run_dir: &Path) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "PATH".to_owned(),
            format!("{}:/usr/bin:/bin", display(&run_root.join("bin"))),
        ),
        ("HOME".to_owned(), display(&run_dir.join("home"))),
        ("TMPDIR".to_owned(), display(&run_dir.join("tmp"))),
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
        (
            "SIFTWIRE_DATABASE_PATH".to_owned(),
            display(&run_dir.join("siftwire.sqlite")),
        ),
        ("SIFTWIRE_EVAL_ALLOW_FILE_URLS".to_owned(), "1".to_owned()),
    ])
}

pub fn run(executable: &Path, run_dir: &Path, request: &Request) -> Result<Vec<u8>> {
    for directory in [
        run_dir.join("home"),
        run_dir.join("tmp"),
        PathBuf::from(&request.artifact_dir),
    ] {
        create_dir_all(&directory, 0o700)?;
    }
    let request_path = run_dir.join("request.json");
    write_file(&request_path, &serde_json::to_vec(request)?, 0o600)?;
    let output_path = run_dir.join("response.json");
    let stderr_path = run_dir.join("adapter.stderr");
    let status = Command::new(executable)
        .current_dir(&request.workspace)
        .env("TMPDIR", run_dir.join("tmp"))
        .stdin(File::open(&request_path).context("open adapter request")?)
        .stdout(capture_file(&output_path)?)
        .stderr(capture_file(&stderr_path)?)
        .status()
        .context("launch agent adapter")?;
    if !status.success() {
        bail!("agent adapter {status}; inspect {}", display(&stderr_path));
    }
    fs::read(&output_path).context("read adapter result")
}

fn capture_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("create adapter capture {}", path.display()))
}
