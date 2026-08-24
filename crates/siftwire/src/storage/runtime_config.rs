use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use rusqlite::params;

use super::{Store, format_timestamp};

impl Store {
    /// Returns all runtime configuration ordered by key.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read the configuration.
    pub fn runtime_config(&self) -> Result<BTreeMap<String, String>> {
        let mut statement = self
            .connection
            .prepare("SELECT key_name, value_text FROM runtime_config ORDER BY key_name")
            .context("prepare runtime config query")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .context("query runtime config")?;
        let mut values = BTreeMap::new();
        for row in rows {
            let (key, value) = row.context("read runtime config row")?;
            values.insert(key, value);
        }
        Ok(values)
    }

    /// Sets one runtime configuration value.
    ///
    /// # Errors
    ///
    /// Returns an error when the key is empty or `SQLite` cannot store the value.
    pub fn set_runtime_config(&self, key: &str, value: &str) -> Result<()> {
        let key = key.trim();
        if key.is_empty() {
            bail!("runtime config key is required");
        }
        self.connection
            .execute(
                "INSERT INTO runtime_config (key_name, value_text, updated_at) \
                 VALUES (?1, ?2, ?3) ON CONFLICT(key_name) DO UPDATE SET \
                 value_text = excluded.value_text, updated_at = excluded.updated_at",
                params![key, value, format_timestamp(self.timestamp())],
            )
            .with_context(|| format!("set runtime config {key}"))?;
        Ok(())
    }
}
