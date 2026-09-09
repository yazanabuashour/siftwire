use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Transaction, params};

use crate::domain::{SOURCE_KIND_ATOM, SOURCE_KIND_RSS, THRESHOLD_ALWAYS, normalize_sources};

use super::{Source, Store, bool_i64, format_timestamp, normalize_source};

const SOURCE_COLUMNS: &str = "key, label, kind, url, repo, section, threshold, enabled, \
    url_canonicalization, outlet_extraction, dedup_group, priority_rank, always_report, \
    schedule_format, schedule_filter, api_key";

impl Store {
    /// Lists effective sources ordered by key without rewriting legacy rows.
    /// Enabled-only reads reject unsupported configuration before fetching.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read a row or an enabled-only read
    /// encounters unsupported source configuration.
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
        rows.map(|row| {
            let source = row.context("read source row")?;
            if enabled_only {
                let key = source.key.clone();
                normalize_source(source).map_err(|error| {
                    anyhow::anyhow!(
                        "enabled source {key:?} is unsupported: {error}; select supported configuration or disable the source"
                    )
                })
            } else {
                Ok(source)
            }
        })
        .collect()
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
    let kind: String = row.get(2)?;
    // Adapt only the effective configuration. Raw rows and their state stay intact.
    let kind = if kind == SOURCE_KIND_ATOM {
        SOURCE_KIND_RSS.to_owned()
    } else {
        kind
    };
    let threshold = if always_report == 1 {
        THRESHOLD_ALWAYS.to_owned()
    } else {
        row.get(6)?
    };
    Ok(Source {
        key: row.get(0)?,
        label: row.get(1)?,
        kind,
        url: row.get(3)?,
        repo: row.get(4)?,
        section: row.get(5)?,
        threshold,
        enabled: enabled == 1,
        url_canonicalization: row.get(8)?,
        outlet_extraction: row.get(9)?,
        dedup_group: row.get(10)?,
        priority_rank: row.get(11)?,
        schedule_format: row.get(13)?,
        schedule_filter: row.get(14)?,
        api_key: row.get(15)?,
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
             schedule_format, schedule_filter, api_key, created_at, updated_at) VALUES \
             (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18) \
             ON CONFLICT(key) DO UPDATE SET label = excluded.label, kind = excluded.kind, \
             url = excluded.url, repo = excluded.repo, section = excluded.section, \
             threshold = excluded.threshold, enabled = excluded.enabled, \
             url_canonicalization = excluded.url_canonicalization, \
             outlet_extraction = excluded.outlet_extraction, dedup_group = excluded.dedup_group, \
             priority_rank = excluded.priority_rank, always_report = excluded.always_report, \
             schedule_format = excluded.schedule_format, schedule_filter = excluded.schedule_filter, \
             api_key = excluded.api_key, updated_at = excluded.updated_at",
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
                0,
                source.schedule_format,
                source.schedule_filter,
                source.api_key,
                timestamp,
                timestamp,
            ],
        )
        .with_context(|| format!("upsert source {}", source.key))?;
    Ok(())
}
