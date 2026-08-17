use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Transaction, params};

use crate::domain::normalize_sources;

use super::{Source, Store, bool_i64, format_timestamp, normalize_source};

const SOURCE_COLUMNS: &str = "key, label, kind, url, repo, section, threshold, enabled, \
    url_canonicalization, outlet_extraction, dedup_group, priority_rank, always_report";

impl Store {
    /// Lists sources ordered by key.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read a source row.
    pub fn list_sources(&self, enabled_only: bool) -> Result<Vec<Source>> {
        let filter = if enabled_only {
            " WHERE enabled = 1"
        } else {
            ""
        };
        let query = format!("SELECT {SOURCE_COLUMNS} FROM brief_source{filter} ORDER BY key");
        let mut statement = self
            .connection
            .prepare(&query)
            .context("prepare source query")?;
        let rows = statement
            .query_map([], source_from_row)
            .context("query sources")?;
        rows.map(|row| row.context("read source row")).collect()
    }

    /// Replaces the complete source set.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or the atomic `SQLite` replacement fails.
    pub fn replace_sources(&self, sources: Vec<Source>) -> Result<Vec<Source>> {
        let sources = normalize_sources(sources)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .context("begin source replacement")?;
        transaction
            .execute("DELETE FROM brief_source", [])
            .context("delete existing sources")?;
        for source in &sources {
            upsert_source_tx(&transaction, source, self.timestamp())?;
        }
        transaction.commit().context("commit source replacement")?;
        self.list_sources(false)
    }

    /// Inserts or updates one source.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or the atomic `SQLite` write fails.
    pub fn upsert_source(&self, source: Source) -> Result<Source> {
        let source = normalize_source(source)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .context("begin source upsert")?;
        upsert_source_tx(&transaction, &source, self.timestamp())?;
        transaction.commit().context("commit source upsert")?;
        Ok(source)
    }

    /// Deletes one source and its cascading state.
    ///
    /// # Errors
    ///
    /// Returns an error when the key is empty or `SQLite` cannot delete the source.
    pub fn delete_source(&self, key: &str) -> Result<()> {
        let key = key.trim();
        if key.is_empty() {
            bail!("source key is required");
        }
        self.connection
            .execute("DELETE FROM brief_source WHERE key = ?1", [key])
            .with_context(|| format!("delete source {key}"))?;
        Ok(())
    }
}

fn source_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    let enabled = row.get::<_, i64>(7)?;
    let always_report = row.get::<_, i64>(12)?;
    Ok(Source {
        key: row.get(0)?,
        label: row.get(1)?,
        kind: row.get(2)?,
        url: row.get(3)?,
        repo: row.get(4)?,
        section: row.get(5)?,
        threshold: row.get(6)?,
        enabled: enabled == 1,
        url_canonicalization: row.get(8)?,
        outlet_extraction: row.get(9)?,
        dedup_group: row.get(10)?,
        priority_rank: row.get(11)?,
        always_report: always_report == 1,
    })
}

fn upsert_source_tx(
    transaction: &Transaction<'_>,
    source: &Source,
    now: DateTime<Utc>,
) -> Result<()> {
    let timestamp = format_timestamp(now);
    transaction
        .execute(
            "INSERT INTO brief_source (key, label, kind, url, repo, section, threshold, enabled, \
             url_canonicalization, outlet_extraction, dedup_group, priority_rank, always_report, \
             created_at, updated_at) VALUES \
             (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15) \
             ON CONFLICT(key) DO UPDATE SET label = excluded.label, kind = excluded.kind, \
             url = excluded.url, repo = excluded.repo, section = excluded.section, \
             threshold = excluded.threshold, enabled = excluded.enabled, \
             url_canonicalization = excluded.url_canonicalization, \
             outlet_extraction = excluded.outlet_extraction, dedup_group = excluded.dedup_group, \
             priority_rank = excluded.priority_rank, always_report = excluded.always_report, \
             updated_at = excluded.updated_at",
            params![
                source.key,
                source.label,
                source.kind,
                source.url,
                source.repo,
                source.section,
                source.threshold,
                bool_i64(source.enabled),
                source.url_canonicalization,
                source.outlet_extraction,
                source.dedup_group,
                source.priority_rank,
                bool_i64(source.always_report),
                timestamp,
                timestamp,
            ],
        )
        .with_context(|| format!("upsert source {}", source.key))?;
    Ok(())
}
