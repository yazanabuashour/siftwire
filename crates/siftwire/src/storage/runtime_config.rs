use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use rusqlite::{Transaction, TransactionBehavior, params};

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

    /// Sets runtime configuration values in one transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when a key is empty or `SQLite` cannot store the values.
    pub fn set_runtime_config_values(&self, values: &[(&str, String)]) -> Result<()> {
        let transaction =
            Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate)
                .context("serialize runtime config update")?;
        let updated_at = format_timestamp(self.timestamp());
        for (key, value) in values {
            let key = key.trim();
            if key.is_empty() {
                bail!("runtime config key is required");
            }
            transaction
                .execute(
                    "INSERT INTO runtime_config (key_name, value_text, updated_at) \
                     VALUES (?1, ?2, ?3) ON CONFLICT(key_name) DO UPDATE SET \
                     value_text = excluded.value_text, updated_at = excluded.updated_at",
                    params![key, value, updated_at],
                )
                .with_context(|| format!("set runtime config {key}"))?;
        }
        transaction.commit().context("commit runtime config update")
    }

    /// Sets one runtime configuration value.
    ///
    /// # Errors
    ///
    /// Returns an error when the key is empty or `SQLite` cannot store the value.
    pub fn set_runtime_config(&self, key: &str, value: &str) -> Result<()> {
        self.set_runtime_config_values(&[(key, value.to_owned())])
    }
}
