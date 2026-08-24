use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use path_clean::PathClean;

use crate::contract::Paths;

pub const DATABASE_ENV: &str = "SIFTWIRE_DATABASE_PATH";

/// Resolves the selected database path from an explicit flag, the canonical
/// environment variable, or the SiftWire default.
///
/// # Errors
///
/// Returns an error when the user home cannot be determined or the derived
/// default path is not valid UTF-8.
pub fn resolve(explicit: Option<&str>) -> Result<Paths> {
    if let Some(path) = explicit.filter(|value| !value.trim().is_empty()) {
        return Ok(resolution(path));
    }
    let current = env::var(DATABASE_ENV).unwrap_or_default();
    if !current.trim().is_empty() {
        return Ok(resolution(&current));
    }
    resolution_path(&default_data_root()?.join("siftwire/siftwire.sqlite"))
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

fn resolution(path: &str) -> Paths {
    let cleaned = PathBuf::from(path).clean();
    Paths {
        data_dir: display_parent(&cleaned),
        database_path: cleaned.display().to_string(),
    }
}

fn resolution_path(path: &Path) -> Result<Paths> {
    let value = path
        .to_str()
        .ok_or_else(|| anyhow!("database path is not valid UTF-8"))?;
    Ok(resolution(value))
}

fn display_parent(path: &Path) -> String {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .display()
        .to_string()
}
