use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use super::{FetchLog, Store, StoredSentItem};

pub const RUN_ITEM_MUST_INCLUDE: &str = "must_include";
pub const RUN_ITEM_CANDIDATE: &str = "candidate";
pub const RUN_ITEM_DROPPED: &str = "dropped";
pub const RUN_ITEM_ANNOTATION: &str = "annotation";

/// One persisted selection-evidence item belonging to a finished brief run.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RunItemRow {
    pub id: String,
    pub category: String,
    pub source_key: String,
    pub source_label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub section: String,
    pub threshold: String,
    pub priority_rank: i64,
    pub published_at: String,
    pub outlet: String,
    pub title: String,
    pub url: String,
    pub reason: String,
    pub detail: String,
}

/// One persisted brief run with its optional delivery outcome.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub dry_run: bool,
    pub status: String,
    pub summary: String,
    pub delivered_at: Option<String>,
    pub message: Option<String>,
}

/// Everything stored for one brief run.
#[derive(Clone, Debug)]
pub struct RunDetail {
    pub summary: RunSummary,
    pub delivery_html: Option<String>,
    pub delivery_plan: Option<super::DeliveryPlan>,
    pub delivery_context: Option<super::RunDeliveryContext>,
    pub items: Vec<RunItemRow>,
    pub fetch_logs: Vec<FetchLog>,
    pub sent_items: Vec<StoredSentItem>,
}

impl Store {
    /// Persists selection evidence for one run inside a single transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when any item insert fails.
    pub fn insert_run_items(&self, run_id: &str, items: &[RunItemRow]) -> Result<()> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .context("begin run item insert")?;
        for item in items {
            transaction
                .execute(
                    "INSERT INTO brief_run_item \
                     (run_id, category, source_key, source_label, kind, section, threshold, \
                     priority_rank, published_at, outlet, title, url, \
                     reason, detail) VALUES \
                     (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        run_id,
                        item.category,
                        item.source_key,
                        item.source_label,
                        item.kind,
                        item.section,
                        item.threshold,
                        item.priority_rank,
                        item.published_at,
                        item.outlet,
                        item.title,
                        item.url,
                        item.reason,
                        item.detail,
                    ],
                )
                .with_context(|| {
                    format!("insert {} run item for {}", item.category, item.source_key)
                })?;
        }
        transaction.commit().context("commit run item insert")
    }

    /// Returns everything stored for one run, or `None` when unknown.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read run state.
    pub fn run_detail(&self, run_id: &str) -> Result<Option<RunDetail>> {
        let summary = self
            .connection
            .query_row(
                "SELECT r.id, r.started_at, r.finished_at, r.dry_run, r.status, r.summary, \
                 delivery.delivered_at, delivery.message \
                 FROM brief_run r \
                 LEFT JOIN delivery_once ON delivery_once.run_id = r.id \
                 LEFT JOIN delivery ON delivery.id = delivery_once.delivery_id \
                 WHERE r.id = ?1",
                [run_id],
                run_summary_from_row,
            )
            .optional()
            .with_context(|| format!("read run {run_id}"))?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let delivery_html = if summary.delivered_at.is_some() {
            self.delivery_plan_for_run(run_id)?
                .map(|plan| plan.html)
                .filter(|html| !html.is_empty())
        } else {
            None
        };
        Ok(Some(RunDetail {
            delivery_html,
            delivery_plan: self.delivery_plan_for_run(run_id)?,
            delivery_context: self.run_delivery_context(run_id)?,
            items: query_run_items(&self.connection, run_id)?,
            fetch_logs: query_fetch_logs_for_run(&self.connection, run_id)?,
            sent_items: query_sent_items_for_run(&self.connection, run_id)?,
            summary,
        }))
    }
}

pub(super) fn run_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunSummary> {
    Ok(RunSummary {
        run_id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        dry_run: row.get::<_, i64>(3)? == 1,
        status: row.get(4)?,
        summary: row.get(5)?,
        delivered_at: row.get(6)?,
        message: row.get(7)?,
    })
}

fn query_run_items(connection: &Connection, run_id: &str) -> Result<Vec<RunItemRow>> {
    let mut statement = connection
        .prepare(
            "SELECT category, source_key, source_label, kind, section, threshold, \
             priority_rank, published_at, outlet, title, url, reason, detail, CAST(id AS TEXT) \
             FROM brief_run_item WHERE run_id = ?1 ORDER BY id",
        )
        .context("prepare run item query")?;
    let rows = statement
        .query_map([run_id], |row| {
            Ok(RunItemRow {
                id: row.get(13)?,
                category: row.get(0)?,
                source_key: row.get(1)?,
                source_label: row.get(2)?,
                kind: row.get(3)?,
                section: row.get(4)?,
                threshold: row.get(5)?,
                priority_rank: row.get(6)?,
                published_at: row.get(7)?,
                outlet: row.get(8)?,
                title: row.get(9)?,
                url: row.get(10)?,
                reason: row.get(11)?,
                detail: row.get(12)?,
            })
        })
        .context("query run items")?;
    rows.map(|row| row.context("read run item row")).collect()
}

fn query_fetch_logs_for_run(connection: &Connection, run_id: &str) -> Result<Vec<FetchLog>> {
    let mut statement = connection
        .prepare(
            "SELECT run_id, source_key, status, error, item_count, created_at, source_label, selection_json \
             FROM fetch_log WHERE run_id = ?1 ORDER BY id",
        )
        .context("prepare run fetch log query")?;
    let rows = statement
        .query_map([run_id], super::runs_health::raw_fetch_log)
        .context("query run fetch logs")?;
    let mut logs = Vec::new();
    for row in rows {
        logs.push(row.context("read run fetch log row")?.try_into()?);
    }
    Ok(logs)
}

fn query_sent_items_for_run(connection: &Connection, run_id: &str) -> Result<Vec<StoredSentItem>> {
    let mut statement = connection
        .prepare("SELECT title, url, kind, sent_at FROM sent_item WHERE run_id = ?1 ORDER BY id")
        .context("prepare run sent item query")?;
    let rows = statement
        .query_map([run_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .context("query run sent items")?;
    rows.map(|row| {
        let (title, url, kind, sent_at) = row.context("read run sent item row")?;
        Ok(StoredSentItem {
            title,
            url,
            kind,
            sent_at: super::parse_timestamp(&sent_at)?,
        })
    })
    .collect()
}
