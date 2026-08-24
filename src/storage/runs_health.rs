use std::collections::BTreeMap;

use anyhow::{Context, Result};
use rusqlite::{Transaction, params};

use crate::contract::HealthDelta;

use super::{
    FetchLog, Store, bool_i64, format_timestamp, go_zero_time, new_run_id, parse_timestamp_compat,
};

impl Store {
    /// Starts a persisted brief run and returns its random identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when identifier generation or the `SQLite` insert fails.
    pub fn start_run(&self, dry_run: bool) -> Result<String> {
        let id = new_run_id()?;
        self.connection
            .execute(
                "INSERT INTO brief_run (id, started_at, dry_run, status, summary) \
                 VALUES (?1, ?2, ?3, 'running', '')",
                params![id, format_timestamp(self.timestamp()), bool_i64(dry_run)],
            )
            .context("start brief run")?;
        Ok(id)
    }

    /// Marks a brief run as finished.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot update the run.
    pub fn finish_run(&self, id: &str, status: &str, summary: &str) -> Result<()> {
        self.connection
            .execute(
                "UPDATE brief_run SET finished_at = ?1, status = ?2, summary = ?3 \
                 WHERE id = ?4",
                params![format_timestamp(self.timestamp()), status, summary, id],
            )
            .with_context(|| format!("finish brief run {id}"))?;
        Ok(())
    }

    /// Reports whether a successful, finished brief run exists.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot inspect the run.
    pub fn brief_run_exists(&self, id: &str) -> Result<bool> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM brief_run \
                 WHERE id = ?1 AND status = 'ok' AND finished_at IS NOT NULL)",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .with_context(|| format!("inspect brief run {id}"))?;
        Ok(exists == 1)
    }

    /// Inserts one source fetch result.
    ///
    /// # Errors
    ///
    /// Returns an error when a count exceeds `SQLite`'s integer range or the insert fails.
    pub fn insert_fetch_log(&self, log: &FetchLog) -> Result<()> {
        let created_at = if log.created_at == go_zero_time() {
            self.timestamp()
        } else {
            log.created_at
        };
        let item_count = i64::try_from(log.item_count).context("fetch item count is too large")?;
        let new_item_count =
            i64::try_from(log.new_item_count).context("new fetch item count is too large")?;
        self.connection
            .execute(
                "INSERT INTO fetch_log \
                 (run_id, source_key, status, error, item_count, new_item_count, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    log.run_id,
                    log.source_key,
                    log.status,
                    log.error,
                    item_count,
                    new_item_count,
                    format_timestamp(created_at),
                ],
            )
            .with_context(|| format!("insert fetch log for source {}", log.source_key))?;
        Ok(())
    }

    /// Returns recent fetch logs in chronological order.
    ///
    /// A non-positive limit selects the compatibility default of 200 rows.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read a row or a count is invalid.
    pub fn recent_fetch_logs(&self, limit: i64) -> Result<Vec<FetchLog>> {
        let limit = if limit <= 0 { 200 } else { limit };
        let mut statement = self
            .connection
            .prepare(
                "SELECT run_id, source_key, status, error, item_count, new_item_count, created_at \
                 FROM fetch_log ORDER BY id DESC LIMIT ?1",
            )
            .context("prepare recent fetch log query")?;
        let rows = statement
            .query_map([limit], raw_fetch_log)
            .context("query recent fetch logs")?;
        let mut logs = Vec::new();
        for row in rows {
            logs.push(row.context("read fetch log row")?.try_into()?);
        }
        logs.reverse();
        Ok(logs)
    }

    /// Compares current warnings with active persisted warnings and optionally stores the change.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read or atomically update warning state.
    pub fn health_delta(
        &self,
        current: &BTreeMap<String, String>,
        mutate: bool,
    ) -> Result<HealthDelta> {
        let prior = self.active_warnings()?;
        let mut delta = HealthDelta::default();
        for (key, message) in current {
            if !prior.contains_key(key) {
                delta.new_warnings.push(message.clone());
            }
        }
        for (key, message) in &prior {
            if !current.contains_key(key) {
                delta.resolved_warnings.push(message.clone());
            }
        }
        delta.new_warnings.sort();
        delta.resolved_warnings.sort();
        if mutate {
            self.store_health_warnings(current, &prior)?;
        }
        Ok(delta)
    }

    fn active_warnings(&self) -> Result<BTreeMap<String, String>> {
        let mut statement = self
            .connection
            .prepare("SELECT key_name, message FROM health_warning WHERE active = 1")
            .context("prepare active warning query")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .context("query active warnings")?;
        let mut warnings = BTreeMap::new();
        for row in rows {
            let (key, message) = row.context("read active warning row")?;
            warnings.insert(key, message);
        }
        Ok(warnings)
    }

    fn store_health_warnings(
        &self,
        current: &BTreeMap<String, String>,
        prior: &BTreeMap<String, String>,
    ) -> Result<()> {
        let now = format_timestamp(self.timestamp());
        let transaction = self
            .connection
            .unchecked_transaction()
            .context("begin health warning update")?;
        for (key, message) in current {
            activate_warning(&transaction, key, message, &now)?;
        }
        for key in prior.keys().filter(|key| !current.contains_key(*key)) {
            resolve_warning(&transaction, key, &now)?;
        }
        transaction.commit().context("commit health warning update")
    }
}

pub(super) struct RawFetchLog {
    run_id: String,
    source_key: String,
    status: String,
    error: String,
    item_count: i64,
    new_item_count: i64,
    created_at: String,
}

impl TryFrom<RawFetchLog> for FetchLog {
    type Error = anyhow::Error;

    fn try_from(raw: RawFetchLog) -> Result<Self> {
        Ok(Self {
            run_id: raw.run_id,
            source_key: raw.source_key,
            status: raw.status,
            error: raw.error,
            item_count: usize::try_from(raw.item_count).context("invalid fetch item count")?,
            new_item_count: usize::try_from(raw.new_item_count)
                .context("invalid new fetch item count")?,
            created_at: parse_timestamp_compat(&raw.created_at),
        })
    }
}

pub(super) fn raw_fetch_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawFetchLog> {
    Ok(RawFetchLog {
        run_id: row.get(0)?,
        source_key: row.get(1)?,
        status: row.get(2)?,
        error: row.get(3)?,
        item_count: row.get(4)?,
        new_item_count: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn activate_warning(
    transaction: &Transaction<'_>,
    key: &str,
    message: &str,
    now: &str,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO health_warning \
             (key_name, message, active, first_seen_at, last_seen_at) \
             VALUES (?1, ?2, 1, ?3, ?3) ON CONFLICT(key_name) DO UPDATE SET \
             message = excluded.message, active = 1, last_seen_at = excluded.last_seen_at, \
             resolved_at = NULL",
            params![key, message, now],
        )
        .with_context(|| format!("activate health warning {key}"))?;
    Ok(())
}

fn resolve_warning(transaction: &Transaction<'_>, key: &str, now: &str) -> Result<()> {
    transaction
        .execute(
            "UPDATE health_warning SET active = 0, resolved_at = ?1, last_seen_at = ?1 \
             WHERE key_name = ?2",
            params![now, key],
        )
        .with_context(|| format!("resolve health warning {key}"))?;
    Ok(())
}
