mod delivery;
mod outlets;
mod run_items;
mod runs_health;
mod runtime_config;
mod schema;
mod sources;
mod state;

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::fmt::Write as _;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Timelike, Utc};
use fs2::FileExt;
use rusqlite::Connection;

pub use crate::domain::{OutletPolicy, Source, normalize_source};
pub use delivery::normalize_title_key;
pub use run_items::{
    RUN_ITEM_CANDIDATE, RUN_ITEM_DROPPED, RUN_ITEM_MUST_INCLUDE, RunDetail, RunItemRow, RunSummary,
};

pub const RUNTIME_CONFIG_CONFIGURATION_VERSION: &str = "configuration_version";
pub const RUNTIME_CONFIG_MAX_DELIVERY_ITEMS: &str = "max_delivery_items";
pub const CONFIGURATION_VERSION_V2: &str = "v2";
pub const DEFAULT_MAX_DELIVERY_ITEMS: i64 = 7;
pub const MAX_DELIVERY_ITEMS_UPPER_BOUND: i64 = 25;
pub const DELIVERY_MESSAGE_CONFLICT: &str = "run_id was already delivered with a different message";

#[derive(Clone, Debug)]
pub struct SourceState {
    pub source_key: String,
    pub latest_identity: String,
    pub latest_feed_identity: String,
    pub latest_title: String,
    pub latest_url: String,
    pub latest_published_at: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct FetchLog {
    pub run_id: String,
    pub source_key: String,
    pub status: String,
    pub error: String,
    pub item_count: usize,
    pub new_item_count: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct Delivery {
    pub run_id: String,
    pub message: String,
    pub delivered_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredSentItem {
    pub title: String,
    pub url: String,
    pub sent_at: DateTime<Utc>,
}

impl Default for SourceState {
    fn default() -> Self {
        Self {
            source_key: String::new(),
            latest_identity: String::new(),
            latest_feed_identity: String::new(),
            latest_title: String::new(),
            latest_url: String::new(),
            latest_published_at: String::new(),
            checked_at: go_zero_time(),
        }
    }
}

impl Default for FetchLog {
    fn default() -> Self {
        Self {
            run_id: String::new(),
            source_key: String::new(),
            status: String::new(),
            error: String::new(),
            item_count: 0,
            new_item_count: 0,
            created_at: go_zero_time(),
        }
    }
}

impl Default for Delivery {
    fn default() -> Self {
        Self {
            run_id: String::new(),
            message: String::new(),
            delivered_at: go_zero_time(),
        }
    }
}

impl Default for StoredSentItem {
    fn default() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
            sent_at: go_zero_time(),
        }
    }
}

#[derive(Debug)]
pub enum DeliveryInsertError {
    Conflict,
    Storage(anyhow::Error),
}

impl DeliveryInsertError {
    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict)
    }
}

impl fmt::Display for DeliveryInsertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => formatter.write_str(DELIVERY_MESSAGE_CONFLICT),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

impl Error for DeliveryInsertError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Conflict => None,
            Self::Storage(error) => Some(error.as_ref()),
        }
    }
}

impl From<anyhow::Error> for DeliveryInsertError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage(error)
    }
}

pub struct Store {
    connection: Connection,
    now: fn() -> DateTime<Utc>,
}

impl Store {
    /// Opens and migrates a Siftwire `SQLite` database.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is empty, its directory cannot be created,
    /// or `SQLite` configuration or migration fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() || path.to_string_lossy().trim().is_empty() {
            bail!("database path is required");
        }
        let parent = database_parent(path);
        create_database_directory(&parent)?;
        let sidecars = sidecar_states(path)?;
        let _created = create_database_file(path)?;
        let migration_lock = open_migration_lock(path)?;
        migration_lock
            .lock_exclusive()
            .context("lock sqlite migration")?;
        let connection = Connection::open(path)
            .with_context(|| format!("open sqlite database {}", path.display()))?;
        let store = Self {
            connection,
            now: Utc::now,
        };
        store.configure()?;
        protect_new_sidecars(&sidecars)?;
        store.init_schema()?;
        protect_new_sidecars(&sidecars)?;
        drop(migration_lock);
        Ok(store)
    }

    fn timestamp(&self) -> DateTime<Utc> {
        (self.now)()
    }
}

fn database_parent(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_owned()
}

fn create_database_directory(path: &Path) -> Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(path)
        .with_context(|| format!("create database directory {}", path.display()))
}

fn open_migration_lock(path: &Path) -> Result<File> {
    let path = sidecar_path(path, ".siftwire-migration.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
        .open(&path)
        .with_context(|| format!("open sqlite migration lock {}", path.display()))
}

fn create_database_file(path: &Path) -> Result<bool> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    match options.open(path) {
        Ok(file) => {
            drop(file);
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("create sqlite database {}", path.display()))
        }
    }
}

struct SidecarState {
    path: PathBuf,
    existed: bool,
}

fn sidecar_states(path: &Path) -> Result<[SidecarState; 2]> {
    let state = |suffix| -> Result<SidecarState> {
        let path = sidecar_path(path, suffix);
        let existed = path
            .try_exists()
            .with_context(|| format!("inspect sqlite sidecar {}", path.display()))?;
        Ok(SidecarState { path, existed })
    };
    Ok([state("-wal")?, state("-shm")?])
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(unix)]
fn protect_new_sidecars(sidecars: &[SidecarState]) -> Result<()> {
    for sidecar in sidecars {
        if !sidecar.existed
            && sidecar.path.try_exists().with_context(|| {
                format!(
                    "inspect newly created sqlite sidecar {}",
                    sidecar.path.display()
                )
            })?
        {
            fs::set_permissions(&sidecar.path, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("protect sqlite sidecar {}", sidecar.path.display()))?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn protect_new_sidecars(_sidecars: &[SidecarState]) -> Result<()> {
    Ok(())
}

fn bool_i64(value: bool) -> i64 {
    i64::from(value)
}

fn format_timestamp(value: DateTime<Utc>) -> String {
    let mut timestamp = value.format("%Y-%m-%dT%H:%M:%S").to_string();
    let nanoseconds = value.nanosecond();
    if nanoseconds != 0 {
        let fraction = format!("{nanoseconds:09}");
        timestamp.push('.');
        timestamp.push_str(fraction.trim_end_matches('0'));
    }
    timestamp.push('Z');
    timestamp
}

fn parse_timestamp_compat(value: &str) -> DateTime<Utc> {
    // The Go store treated malformed legacy timestamps as the zero time.
    DateTime::parse_from_rfc3339(value).map_or_else(
        |_| go_zero_time(),
        |timestamp| timestamp.with_timezone(&Utc),
    )
}

fn go_zero_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("0001-01-01T00:00:00Z")
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .unwrap_or_default()
}

fn new_run_id() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).context("generate run id")?;
    let mut id = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut id, "{byte:02x}").context("format run id")?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests;
