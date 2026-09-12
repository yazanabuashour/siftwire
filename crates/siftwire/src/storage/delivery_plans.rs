use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use crate::contract::{DeliveryItem, SportsUpdate};

use super::Store;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct RunDeliveryContext {
    pub max_delivery_items: i64,
    pub sports_updates: Vec<SportsUpdate>,
    pub sports_timezone: String,
    pub health_footnote: String,
}

#[derive(Clone, Debug)]
pub struct DeliveryPlan {
    pub id: String,
    pub run_id: String,
    pub candidate_indexes: Vec<usize>,
    pub message: String,
    pub text: String,
    pub html: String,
    pub items: Vec<DeliveryItem>,
}

pub struct DeliveryPreparation {
    pub started_at: String,
    pub status: String,
    pub dry_run: bool,
    pub delivered: bool,
}

impl Store {
    /// Reads preparation eligibility without archive evidence or a saved plan.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or the eligibility fields are invalid.
    pub fn delivery_preparation(&self, run_id: &str) -> Result<Option<DeliveryPreparation>> {
        self.connection
            .query_row(
                "SELECT r.started_at, r.status, r.dry_run, delivery.delivered_at IS NOT NULL \
                 FROM brief_run r \
                 LEFT JOIN delivery_once ON delivery_once.run_id = r.id \
                 LEFT JOIN delivery ON delivery.id = delivery_once.delivery_id \
                 WHERE r.id = ?1",
                [run_id],
                |row| {
                    Ok(DeliveryPreparation {
                        started_at: row.get(0)?,
                        status: row.get(1)?,
                        dry_run: row.get::<_, i64>(2)? == 1,
                        delivered: row.get(3)?,
                    })
                },
            )
            .optional()
            .with_context(|| format!("read delivery eligibility for run {run_id}"))
    }

    /// Persists the delivery inputs that must survive beyond `run_brief`.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or the database insert fails.
    pub fn insert_run_delivery_context(
        &self,
        run_id: &str,
        context: &RunDeliveryContext,
    ) -> Result<()> {
        let context = serde_json::to_string(context).context("serialize run delivery context")?;
        self.connection
            .execute(
                "INSERT INTO brief_run_delivery_context (run_id, context_json) VALUES (?1, ?2)",
                params![run_id, context],
            )
            .with_context(|| format!("insert delivery context for run {run_id}"))?;
        Ok(())
    }

    /// Reads the persisted delivery inputs for one run.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or stored JSON is invalid.
    pub fn run_delivery_context(&self, run_id: &str) -> Result<Option<RunDeliveryContext>> {
        let context = self
            .connection
            .query_row(
                "SELECT context_json FROM brief_run_delivery_context WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .with_context(|| format!("read delivery context for run {run_id}"))?;
        context
            .map(|value| serde_json::from_str(&value).context("decode stored delivery context"))
            .transpose()
    }

    /// Creates or replays the one immutable delivery plan for a run.
    ///
    /// `None` means the run already has a plan with different candidate indexes.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or storage fails.
    pub fn insert_delivery_plan(&self, plan: &DeliveryPlan) -> Result<Option<DeliveryPlan>> {
        let candidate_indexes = serde_json::to_string(&plan.candidate_indexes)
            .context("serialize delivery candidate indexes")?;
        let items = serde_json::to_string(&plan.items).context("serialize delivery plan items")?;
        self.connection
            .execute(
                "INSERT INTO delivery_plan \
                 (id, run_id, candidate_indexes_json, message, text_body, html_body, \
                  items_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(run_id) DO NOTHING",
                params![
                    plan.id,
                    plan.run_id,
                    candidate_indexes,
                    plan.message,
                    plan.text,
                    plan.html,
                    items,
                ],
            )
            .with_context(|| format!("insert delivery plan for run {}", plan.run_id))?;
        let stored = self
            .delivery_plan_for_run(&plan.run_id)?
            .context("delivery plan disappeared after insert")?;
        if stored.candidate_indexes != plan.candidate_indexes {
            return Ok(None);
        }
        Ok(Some(stored))
    }

    /// Reads one delivery plan by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or stored JSON is invalid.
    pub fn delivery_plan(&self, id: &str) -> Result<Option<DeliveryPlan>> {
        self.query_delivery_plan("id", id)
    }

    /// Reads the immutable delivery plan for one brief run.
    ///
    /// # Errors
    ///
    /// Returns an error when storage or stored JSON is invalid.
    pub fn delivery_plan_for_run(&self, run_id: &str) -> Result<Option<DeliveryPlan>> {
        self.query_delivery_plan("run_id", run_id)
    }

    fn query_delivery_plan(&self, field: &str, value: &str) -> Result<Option<DeliveryPlan>> {
        let query = format!(
            "SELECT id, run_id, candidate_indexes_json, message, text_body, html_body, \
             items_json FROM delivery_plan WHERE {field} = ?1"
        );
        let row = self
            .connection
            .query_row(&query, [value], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .optional()
            .with_context(|| format!("read delivery plan by {field}"))?;
        let Some((id, run_id, candidate_indexes, message, text, html, items)) = row else {
            return Ok(None);
        };
        Ok(Some(DeliveryPlan {
            id,
            run_id,
            candidate_indexes: serde_json::from_str(&candidate_indexes)
                .context("decode stored delivery candidate indexes")?,
            message,
            text,
            html,
            items: serde_json::from_str(&items).context("decode stored delivery plan items")?,
        }))
    }
}
