#[cfg(test)]
mod architecture_tests;
mod delivery;
mod delivery_plans;
mod outlets;
mod run_archive;
mod run_items;
pub use run_archive::{RunListOptions, RunPage};
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
use rusqlite::{Connection, OpenFlags};

pub use crate::domain::{OutletPolicy, Source, normalize_source};
pub use delivery::normalize_title_key;
pub use delivery_plans::{DeliveryPlan, RunDeliveryContext};
pub use run_items::{
    RUN_ITEM_ANNOTATION, RUN_ITEM_CANDIDATE, RUN_ITEM_DROPPED, RUN_ITEM_MUST_INCLUDE, RunDetail,
    RunItemRow, RunSummary,
};

pub const RUNTIME_CONFIG_MAX_DELIVERY_ITEMS: &str = "max_delivery_items";
pub const RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS: &str = "sports_post_game_days";
pub const RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS: &str = "sports_pre_game_days";
pub const RUNTIME_CONFIG_SPORTS_TIMEZONE: &str = "sports_timezone";
pub const DEFAULT_MAX_DELIVERY_ITEMS: i64 = 7;
// Receipt: `docs/architecture/schedule-source-adr.md` records the operator's
// requested recurring windows and the retained Central-time default.
pub const DEFAULT_SPORTS_POST_GAME_DAYS: i64 = 3;
pub const DEFAULT_SPORTS_PRE_GAME_DAYS: i64 = 7;
pub const DEFAULT_SPORTS_TIMEZONE: chrono_tz::Tz = chrono_tz::America::Chicago;
pub const MAX_DELIVERY_ITEMS_UPPER_BOUND: i64 = 25;
pub const DELIVERY_MESSAGE_CONFLICT: &str = "run_id was already delivered with a different message";

#[derive(Clone, Debug, Default)]
pub struct SourceState {
    pub source_key: String,
    pub latest_identity: String,
    pub latest_feed_identity: String,
    pub latest_title: String,
    pub latest_url: String,
    pub latest_published_at: String,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default)]
pub struct FetchLog {
    pub source_label: String,
    pub run_id: String,
    pub source_key: String,
    pub status: String,
    pub error: String,
    pub item_count: usize,
    pub new_item_count: Option<usize>,
    pub current_news: Option<crate::contract::CurrentNewsStatus>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default)]
pub struct Delivery {
    pub run_id: String,
    pub message: String,
    pub delivered_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default)]
pub struct StoredSentItem {
    pub title: String,
    pub url: String,
    pub kind: String,
    pub sent_at: DateTime<Utc>,
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
    /// Opens a current SiftWire `SQLite` database or initializes an empty one.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is empty, its directory cannot be created,
    /// the database schema is incompatible, or `SQLite` initialization fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.as_os_str().is_empty() || path.to_string_lossy().trim().is_empty() {
            bail!("database path is required");
        }
        let parent = database_parent(path);
        create_database_directory(&parent)?;
        let sidecars = sidecar_states(path)?;
        let _created = create_database_file(path)?;
        let initialization_lock = open_initialization_lock(path)?;
        initialization_lock
            .lock_exclusive()
            .context("lock sqlite initialization")?;
        let opened = (|| {
            // Read/write connection cleanup can checkpoint WAL even on rejection.
            // Read-only inspection must include WAL, so do not use immutable=1.
            let inspection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| format!("inspect sqlite database {}", path.display()))?;
            schema::validate_schema(&inspection)?;
            drop(inspection);
            let connection = Connection::open(path)
                .with_context(|| format!("open sqlite database {}", path.display()))?;
            let store = Self {
                connection,
                now: Utc::now,
            };
            store.init_schema()?;
            Ok(store)
        })();
        protect_new_sidecars(&sidecars)?;
        drop(initialization_lock);
        opened
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

fn open_initialization_lock(path: &Path) -> Result<File> {
    let path = sidecar_path(path, ".siftwire-initialization.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
        .open(&path)
        .with_context(|| format!("open sqlite initialization lock {}", path.display()))
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

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .with_context(|| format!("invalid stored timestamp {value:?}"))
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
