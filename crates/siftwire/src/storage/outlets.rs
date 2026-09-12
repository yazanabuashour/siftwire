use anyhow::{Context, Result};
use rusqlite::params;

use crate::domain::normalize_outlet_policies;

use super::{OutletPolicy, Store, bool_i64, format_timestamp};

impl Store {
    /// Lists outlet policies ordered by name.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read a policy row.
    pub fn list_outlet_policies(&self) -> Result<Vec<OutletPolicy>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT name, aliases_json, policy, note, enabled \
                 FROM outlet_policy ORDER BY name",
            )
            .context("prepare outlet policy query")?;
        let rows = statement
            .query_map([], |row| {
                let aliases_json = row.get::<_, String>(1)?;
                let enabled = row.get::<_, i64>(4)?;
                let aliases = serde_json::from_str(&aliases_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(OutletPolicy {
                    name: row.get(0)?,
                    aliases,
                    policy: row.get(2)?,
                    note: row.get(3)?,
                    enabled: enabled == 1,
                })
            })
            .context("query outlet policies")?;
        let policies = rows
            .map(|row| row.context("read outlet policy row"))
            .collect::<Result<Vec<_>>>()?;
        normalize_outlet_policies(policies).context("invalid stored outlet policies")
    }

    /// Replaces the complete outlet policy set.
    ///
    /// # Errors
    ///
    /// Returns an error when validation or the atomic `SQLite` replacement fails.
    pub fn replace_outlet_policies(
        &self,
        policies: Vec<OutletPolicy>,
    ) -> Result<Vec<OutletPolicy>> {
        let policies = normalize_outlet_policies(policies)?;
        let transaction = self
            .connection
            .unchecked_transaction()
            .context("begin outlet policy replacement")?;
        transaction
            .execute("DELETE FROM outlet_policy", [])
            .context("delete existing outlet policies")?;
        let now = format_timestamp(self.timestamp());
        for policy in &policies {
            let aliases = serde_json::to_string(&policy.aliases)
                .with_context(|| format!("encode aliases for outlet policy {}", policy.name))?;
            transaction
                .execute(
                    "INSERT INTO outlet_policy \
                     (name, aliases_json, policy, note, enabled, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        policy.name,
                        aliases,
                        policy.policy,
                        policy.note,
                        bool_i64(policy.enabled),
                        now,
                        now,
                    ],
                )
                .with_context(|| format!("insert outlet policy {}", policy.name))?;
        }
        transaction
            .commit()
            .context("commit outlet policy replacement")?;
        self.list_outlet_policies()
    }
}
