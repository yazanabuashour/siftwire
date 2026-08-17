use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::{
    CONFIGURATION_VERSION_V2, DEFAULT_MAX_DELIVERY_ITEMS, RUNTIME_CONFIG_CONFIGURATION_VERSION,
    RUNTIME_CONFIG_MAX_DELIVERY_ITEMS, Store, format_timestamp,
};

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS runtime_config (\
        key_name TEXT PRIMARY KEY,\
        value_text TEXT NOT NULL,\
        updated_at TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS brief_source (\
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
        always_report INTEGER NOT NULL DEFAULT 0,\
        created_at TEXT NOT NULL,\
        updated_at TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS outlet_policy (\
        name TEXT PRIMARY KEY,\
        aliases_json TEXT NOT NULL,\
        policy TEXT NOT NULL,\
        note TEXT NOT NULL,\
        enabled INTEGER NOT NULL,\
        created_at TEXT NOT NULL,\
        updated_at TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS source_state (\
        source_key TEXT PRIMARY KEY REFERENCES brief_source(key) ON DELETE CASCADE,\
        latest_identity TEXT NOT NULL,\
        latest_feed_identity TEXT NOT NULL DEFAULT '',\
        latest_title TEXT NOT NULL,\
        latest_url TEXT NOT NULL,\
        latest_published_at TEXT NOT NULL,\
        checked_at TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS brief_run (\
        id TEXT PRIMARY KEY,\
        started_at TEXT NOT NULL,\
        finished_at TEXT,\
        dry_run INTEGER NOT NULL,\
        status TEXT NOT NULL,\
        summary TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS fetch_log (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        run_id TEXT NOT NULL,\
        source_key TEXT NOT NULL,\
        status TEXT NOT NULL,\
        error TEXT NOT NULL,\
        item_count INTEGER NOT NULL,\
        new_item_count INTEGER NOT NULL,\
        created_at TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS health_warning (\
        key_name TEXT PRIMARY KEY,\
        message TEXT NOT NULL,\
        active INTEGER NOT NULL,\
        first_seen_at TEXT NOT NULL,\
        last_seen_at TEXT NOT NULL,\
        resolved_at TEXT\
    );",
    "CREATE TABLE IF NOT EXISTS delivery (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        run_id TEXT NOT NULL,\
        message TEXT NOT NULL,\
        delivered_at TEXT NOT NULL\
    );",
    "CREATE TABLE IF NOT EXISTS delivery_once (\
        run_id TEXT PRIMARY KEY,\
        message TEXT NOT NULL,\
        delivery_id INTEGER REFERENCES delivery(id) ON DELETE CASCADE\
    );",
    "CREATE TABLE IF NOT EXISTS sent_item (\
        id INTEGER PRIMARY KEY AUTOINCREMENT,\
        delivery_id INTEGER NOT NULL REFERENCES delivery(id) ON DELETE CASCADE,\
        run_id TEXT NOT NULL,\
        title TEXT NOT NULL,\
        url TEXT NOT NULL,\
        title_key TEXT NOT NULL,\
        sent_at TEXT NOT NULL\
    );",
    "CREATE INDEX IF NOT EXISTS idx_sent_item_sent_at ON sent_item(sent_at DESC);",
    "CREATE INDEX IF NOT EXISTS idx_fetch_log_run_id ON fetch_log(run_id);",
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
        for statement in SCHEMA {
            self.connection
                .execute_batch(statement)
                .context("initialize sqlite schema")?;
        }
        let now = format_timestamp(self.timestamp());
        self.connection
            .execute(
                "INSERT INTO runtime_config (key_name, value_text, updated_at) \
                 VALUES (?1, ?2, ?3) ON CONFLICT(key_name) DO NOTHING",
                params![
                    RUNTIME_CONFIG_CONFIGURATION_VERSION,
                    CONFIGURATION_VERSION_V2,
                    now
                ],
            )
            .context("initialize runtime config")?;
        self.migrate_schema()
    }

    fn migrate_schema(&self) -> Result<()> {
        let transaction =
            Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)
                .context("serialize sqlite schema migration")?;
        for (name, definition) in [
            ("url_canonicalization", "TEXT NOT NULL DEFAULT 'none'"),
            ("outlet_extraction", "TEXT NOT NULL DEFAULT 'none'"),
            ("dedup_group", "TEXT NOT NULL DEFAULT ''"),
            ("priority_rank", "INTEGER NOT NULL DEFAULT 0"),
            ("always_report", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            ensure_column(&transaction, "brief_source", name, definition)?;
        }
        ensure_column(
            &transaction,
            "source_state",
            "latest_feed_identity",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        backfill_delivery_idempotency(&transaction)?;
        seed_runtime_config(&transaction, &format_timestamp(self.timestamp()))?;
        transaction
            .commit()
            .context("commit sqlite schema migration")
    }
}

fn backfill_delivery_idempotency(connection: &Connection) -> Result<()> {
    connection
        .execute(
            "INSERT OR REPLACE INTO delivery_once (run_id, message, delivery_id) \
             SELECT delivery.run_id, delivery.message, delivery.id FROM delivery \
             JOIN (SELECT run_id, MAX(id) AS id FROM delivery GROUP BY run_id) latest \
             ON latest.id = delivery.id",
            [],
        )
        .context("backfill delivery idempotency")?;
    Ok(())
}

fn seed_runtime_config(connection: &Connection, now: &str) -> Result<()> {
    connection
        .execute(
            "INSERT INTO runtime_config (key_name, value_text, updated_at) \
             VALUES (?1, ?2, ?3) ON CONFLICT(key_name) DO UPDATE SET \
             value_text = excluded.value_text, updated_at = excluded.updated_at",
            params![
                RUNTIME_CONFIG_CONFIGURATION_VERSION,
                CONFIGURATION_VERSION_V2,
                now
            ],
        )
        .context("migrate runtime config")?;
    connection
        .execute(
            "INSERT INTO runtime_config (key_name, value_text, updated_at) \
             VALUES (?1, ?2, ?3) ON CONFLICT(key_name) DO NOTHING",
            params![
                RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
                DEFAULT_MAX_DELIVERY_ITEMS.to_string(),
                now
            ],
        )
        .context("seed brief runtime config")?;
    Ok(())
}

fn ensure_column(connection: &Connection, table: &str, name: &str, definition: &str) -> Result<()> {
    let query = format!("PRAGMA table_info({table})");
    let mut statement = connection
        .prepare(&query)
        .with_context(|| format!("inspect {table} columns"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .with_context(|| format!("inspect {table} columns"))?;
    for column in names {
        if column.with_context(|| format!("inspect {table} columns"))? == name {
            return Ok(());
        }
    }
    connection
        .execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {name} {definition}"
        ))
        .with_context(|| format!("add {table}.{name}"))
}
