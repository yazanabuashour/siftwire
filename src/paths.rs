use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use path_clean::PathClean;

use crate::contract::Paths;

pub const DATABASE_ENV: &str = "SIFTWIRE_DATABASE_PATH";
pub const LEGACY_DATABASE_ENV: &str = "OPENBRIEF_DATABASE_PATH";

pub struct Resolution {
    pub paths: Paths,
    pub deprecated_environment: bool,
}

pub fn resolve(explicit: Option<&str>) -> Result<Resolution> {
    if let Some(path) = explicit.filter(|value| !value.trim().is_empty()) {
        return Ok(resolution(path, false));
    }
    let current = env::var(DATABASE_ENV).unwrap_or_default();
    let legacy = env::var(LEGACY_DATABASE_ENV).unwrap_or_default();
    if !current.trim().is_empty() && !legacy.trim().is_empty() && !same_path(&current, &legacy)? {
        bail!("{DATABASE_ENV} and {LEGACY_DATABASE_ENV} select different databases; unset one");
    }
    if !current.trim().is_empty() {
        return Ok(resolution(&current, false));
    }
    if !legacy.trim().is_empty() {
        return Ok(resolution(&legacy, true));
    }
    resolve_default()
}

fn resolve_default() -> Result<Resolution> {
    let root = default_data_root()?;
    let current = root.join("siftwire/siftwire.sqlite").clean();
    let legacy = root.join("openbrief/openbrief.sqlite").clean();
    if !current.try_exists().context("inspect default database")?
        && legacy.try_exists().context("inspect OpenBrief database")?
    {
        bail!(
            "existing OpenBrief database found at {}; select it explicitly with --db or {DATABASE_ENV}",
            legacy.display()
        );
    }
    path_resolution(&current, false)
}

fn default_data_root() -> Result<PathBuf> {
    if let Some(value) = env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("resolve user home: HOME is not set or empty"))?;
    Ok(PathBuf::from(home).join(".local/share"))
}

fn resolution(path: &str, deprecated_environment: bool) -> Resolution {
    let cleaned = PathBuf::from(path).clean();
    let parent = display_parent(&cleaned);
    Resolution {
        paths: Paths {
            data_dir: parent,
            database_path: cleaned.display().to_string(),
        },
        deprecated_environment,
    }
}

fn path_resolution(path: &Path, deprecated_environment: bool) -> Result<Resolution> {
    let value = path
        .to_str()
        .ok_or_else(|| anyhow!("database path is not valid UTF-8"))?;
    Ok(resolution(value, deprecated_environment))
}

fn display_parent(path: &Path) -> String {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .display()
        .to_string()
}

fn same_path(left: &str, right: &str) -> Result<bool> {
    if let (Ok(left_path), Ok(right_path)) = (fs::canonicalize(left), fs::canonicalize(right)) {
        return Ok(left_path == right_path);
    }
    Ok(absolute_clean(left)? == absolute_clean(right)?)
}

fn absolute_clean(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return Ok(path.clean());
    }
    Ok(env::current_dir()
        .context("resolve current directory")?
        .join(path)
        .clean())
}
