use anyhow::{Context, Result, bail};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::{
    DEFAULT_MAX_DELIVERY_ITEMS, DEFAULT_SPORTS_POST_GAME_DAYS, DEFAULT_SPORTS_PRE_GAME_DAYS,
    DEFAULT_SPORTS_TIMEZONE, RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
    RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS, RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS,
    RUNTIME_CONFIG_SPORTS_TIMEZONE, Store, format_timestamp,
};

const SCHEMA: &[&str] = &[
    "CREATE TABLE runtime_config (\
        key_name TEXT PRIMARY KEY,\
        value_text TEXT NOT NULL,\
        updated_at TEXT NOT NULL\
    );",
    "CREATE TABLE brief_source (\
        key TEXT PRIMARY KEY,\
        label TEXT NOT NULL,\
        kind TEXT NOT NULL,\
        url TEXT NOT NULL DEFAULT '',\
        repo TEXT NOT NULL DEFAULT '',\
        section TEXT NOT NULL,\
        threshold TEXT NOT NULL,\
        enabled INTEGER NOT NULL,\
        url_canonicalization TEXT NOT NULL DEFAULT 'none',\
        outlet_extraction TEXT NOT NULL DEFAULT 'none',\
        dedup_group TEXT NOT NULL DEFAULT '',\
        priority_rank INTEGER NOT NULL DEFAULT 0,\
        schedule_format TEXT NOT NULL DEFAULT '',\
        schedule_filter TEXT NOT NULL DEFAULT 'all',\
        api_key TEXT NOT NULL DEFAULT '',\
        created_at TEXT NOT NULL,\
        updated_at TEXT NOT NULL\
    );",
    "CREATE TABLE outlet_policy (\
        name TEXT PRIMARY KEY,\
        aliases_json TEXT NOT NULL,\
        policy TEXT NOT NULL,\
        note TEXT NOT NULL,\
        enabled INTEGER NOT NULL,\
        created_at TEXT NOT NULL,\
        updated_at TEXT NOT NULL\
    );",
    "CREATE TABLE source_state (\
        source_key TEXT PRIMARY KEY REFERENCES brief_source(key) ON DELETE CASCADE,\
        latest_identity TEXT NOT NULL,\
        latest_feed_identity TEXT NOT NULL DEFAULT '',\
        latest_title TEXT NOT NULL,\
        latest_url TEXT NOT NULL,\
        latest_published_at TEXT NOT NULL,\
        checked_at TEXT NOT NULL\
    );",
    "CREATE TABLE brief_run (\
        id TEXT PRIMARY KEY,\
        started_at TEXT NOT NULL,\
        finished_at TEXT,\
        dry_run INTEGER NOT NULL,\
        status TEXT NOT NULL,\
        summary TEXT NOT NULL\
    );",
    "CREATE TABLE fetch_log (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        run_id TEXT NOT NULL,\
        source_key TEXT NOT NULL,\
        status TEXT NOT NULL,\
        error TEXT NOT NULL,\
        item_count INTEGER NOT NULL,\
        created_at TEXT NOT NULL,\
        source_label TEXT NOT NULL,\
        selection_json TEXT NOT NULL\
    );",
    "CREATE TABLE health_warning (\
        key_name TEXT PRIMARY KEY,\
        message TEXT NOT NULL,\
        active INTEGER NOT NULL,\
        first_seen_at TEXT NOT NULL,\
        last_seen_at TEXT NOT NULL,\
        resolved_at TEXT\
    );",
    "CREATE TABLE delivery (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        run_id TEXT NOT NULL,\
        message TEXT NOT NULL,\
        delivered_at TEXT NOT NULL\
    );",
    "CREATE TABLE delivery_once (\
        run_id TEXT PRIMARY KEY,\
        message TEXT NOT NULL,\
        delivery_id INTEGER REFERENCES delivery(id) ON DELETE CASCADE\
    );",
    "CREATE TABLE sent_item (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        delivery_id INTEGER NOT NULL REFERENCES delivery(id) ON DELETE CASCADE,\
        run_id TEXT NOT NULL,\
        title TEXT NOT NULL,\
        url TEXT NOT NULL,\
        kind TEXT NOT NULL DEFAULT '',\
        title_key TEXT NOT NULL,\
        sent_at TEXT NOT NULL\
    );",
    "CREATE INDEX idx_sent_item_sent_at ON sent_item(sent_at DESC);",
    "CREATE INDEX idx_fetch_log_run_id ON fetch_log(run_id);",
    "CREATE TABLE brief_run_item (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        run_id TEXT NOT NULL REFERENCES brief_run(id) ON DELETE CASCADE,\
        category TEXT NOT NULL,\
        source_key TEXT NOT NULL,\
        source_label TEXT NOT NULL DEFAULT '',\
        kind TEXT NOT NULL DEFAULT '',\
        section TEXT NOT NULL DEFAULT '',\
        threshold TEXT NOT NULL DEFAULT '',\
        priority_rank INTEGER NOT NULL DEFAULT 0,\
        published_at TEXT NOT NULL DEFAULT '',\
        outlet TEXT NOT NULL DEFAULT '',\
        title TEXT NOT NULL,\
        url TEXT NOT NULL,\
        reason TEXT NOT NULL DEFAULT '',\
        detail TEXT NOT NULL DEFAULT ''\
    );",
    "CREATE INDEX idx_brief_run_item_run ON brief_run_item(run_id, category);",
    "CREATE TABLE brief_run_delivery_context(\
        run_id TEXT PRIMARY KEY REFERENCES brief_run(id) ON DELETE CASCADE,\
        context_json TEXT NOT NULL\
    );",
    "CREATE TABLE delivery_plan(\
        id TEXT PRIMARY KEY,\
        run_id TEXT NOT NULL UNIQUE REFERENCES brief_run(id) ON DELETE CASCADE,\
        candidate_indexes_json TEXT NOT NULL,\
        message TEXT NOT NULL,\
        text_body TEXT NOT NULL,\
        html_body TEXT NOT NULL,\
        items_json TEXT NOT NULL\
    );",
];

impl Store {
    pub(super) fn configure(&self) -> Result<()> {
        for statement in [
            "PRAGMA foreign_keys = ON",
            "PRAGMA busy_timeout = 5000",
            "PRAGMA journal_mode = WAL",
            "PRAGMA synchronous = NORMAL",
        ] {
            self.connection
                .execute_batch(statement)
                .context("configure sqlite")?;
        }
        Ok(())
    }

    pub(super) fn init_schema(&self) -> Result<()> {
        if !validate_schema(&self.connection)? {
            return self.configure();
        }
        self.configure()?;
        let transaction =
            Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)
                .context("begin sqlite initialization")?;
        create_schema(&transaction)?;
        seed_runtime_config(&transaction, &format_timestamp(self.timestamp()))?;
        transaction.execute_batch("PRAGMA user_version = 1")?;
        transaction.commit().context("commit sqlite initialization")
    }
}

// Reject incompatible schemas and return whether the database needs initialization.
pub(super) fn validate_schema(connection: &Connection) -> Result<bool> {
    let actual = schema_objects(connection)?;
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application_id: i64 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if application_id != 0 {
        bail!("incompatible database application_id; select a new empty database");
    }
    if actual.is_empty() && version == 0 {
        return Ok(true);
    }
    let expected = Connection::open_in_memory().context("open schema reference")?;
    create_schema(&expected)?;
    if version != 1 || actual != schema_objects(&expected)? {
        bail!(
            "incompatible SiftWire database schema; select a new empty database (automatic migration is not supported)"
        );
    }
    Ok(false)
}

fn create_schema(connection: &Connection) -> Result<()> {
    for statement in SCHEMA {
        connection
            .execute_batch(statement)
            .context("initialize sqlite schema")?;
    }
    Ok(())
}

// Compare the complete application schema, including constraints and indexes,
// rather than trusting a version marker on a partial or modified database.
fn schema_objects(connection: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut statement = connection.prepare(
        "SELECT type, name, sql FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*' ORDER BY type, name",
    )?;
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()
        .context("inspect sqlite schema")
}

fn seed_runtime_config(connection: &Connection, now: &str) -> Result<()> {
    for (key, value) in [
        (
            RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
            DEFAULT_MAX_DELIVERY_ITEMS.to_string(),
        ),
        (
            RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS,
            DEFAULT_SPORTS_PRE_GAME_DAYS.to_string(),
        ),
        (
            RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS,
            DEFAULT_SPORTS_POST_GAME_DAYS.to_string(),
        ),
        (
            RUNTIME_CONFIG_SPORTS_TIMEZONE,
            DEFAULT_SPORTS_TIMEZONE.to_string(),
        ),
    ] {
        connection
            .execute(
                "INSERT INTO runtime_config (key_name, value_text, updated_at) VALUES (?1, ?2, ?3)",
                params![key, value, now],
            )
            .with_context(|| format!("seed runtime config {key}"))?;
    }
    Ok(())
}
