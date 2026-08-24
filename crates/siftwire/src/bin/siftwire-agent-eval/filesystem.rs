use std::env;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use path_clean::PathClean;

pub fn create_dir_all(path: &Path, mode: u32) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(mode).create(path)
}

pub fn create_dir(path: &Path, mode: u32) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.mode(mode).create(path)
}

pub fn write_file(path: &Path, content: &[u8], mode: u32) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent, 0o755)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true).mode(mode);
    let mut file = options.open(path)?;
    std::io::Write::write_all(&mut file, content)
}

pub fn create_new_file(path: &Path, content: &[u8], mode: u32) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(mode);
    let mut file = options.open(path)?;
    std::io::Write::write_all(&mut file, content)?;
    Ok(file)
}

pub fn copy_file(source: &Path, target: &Path, mode: u32) -> Result<()> {
    let content =
        fs::read(source).with_context(|| format!("read source file {}", source.display()))?;
    write_file(target, &content, mode)
        .with_context(|| format!("write target file {}", target.display()))
}

pub fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("resolve repo root")?;
    if !output.status.success() {
        bail!("resolve repo root: {}", output.status);
    }
    let value = String::from_utf8(output.stdout).context("repo root is not valid UTF-8")?;
    fs::canonicalize(value.trim()).context("make repo root absolute")
}

pub fn absolute(path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .context("resolve current directory")?
            .join(path)
    };
    Ok(joined.clean())
}

pub fn is_within(child: &Path, parent: &Path) -> bool {
    child.strip_prefix(parent).is_ok()
}

pub fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
