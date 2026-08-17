use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};

use super::{SourceState, Store, format_timestamp, go_zero_time, parse_timestamp_compat};

impl Store {
    /// Returns one source state, or `None` when the source has no state.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read the state row.
    pub fn source_state(&self, source_key: &str) -> Result<Option<SourceState>> {
        let row = self
            .connection
            .query_row(
                "SELECT source_key, latest_identity, latest_feed_identity, latest_title, \
                 latest_url, latest_published_at, checked_at \
                 FROM source_state WHERE source_key = ?1",
                [source_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .with_context(|| format!("read source state {source_key}"))?;
        Ok(row.map(|row| SourceState {
            source_key: row.0,
            latest_identity: row.1,
            latest_feed_identity: row.2,
            latest_title: row.3,
            latest_url: row.4,
            latest_published_at: row.5,
            checked_at: parse_timestamp_compat(&row.6),
        }))
    }

    /// Inserts or updates one source state.
    ///
    /// # Errors
    ///
    /// Returns an error when required identities are empty or `SQLite` cannot store the state.
    pub fn upsert_source_state(&self, state: &SourceState) -> Result<()> {
        if state.source_key.trim().is_empty() || state.latest_identity.trim().is_empty() {
            bail!("source state key and latest identity are required");
        }
        let checked_at = if state.checked_at == go_zero_time() {
            self.timestamp()
        } else {
            state.checked_at
        };
        self.connection
            .execute(
                "INSERT INTO source_state \
                 (source_key, latest_identity, latest_feed_identity, latest_title, latest_url, \
                 latest_published_at, checked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(source_key) DO UPDATE SET latest_identity = excluded.latest_identity, \
                 latest_feed_identity = excluded.latest_feed_identity, \
                 latest_title = excluded.latest_title, latest_url = excluded.latest_url, \
                 latest_published_at = excluded.latest_published_at, \
                 checked_at = excluded.checked_at",
                params![
                    state.source_key,
                    state.latest_identity,
                    state.latest_feed_identity,
                    state.latest_title,
                    state.latest_url,
                    state.latest_published_at,
                    format_timestamp(checked_at),
                ],
            )
            .with_context(|| format!("upsert source state {}", state.source_key))?;
        Ok(())
    }
}
