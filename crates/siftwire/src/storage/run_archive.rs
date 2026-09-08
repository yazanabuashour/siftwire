use anyhow::{Context, Result, bail};
use rusqlite::{OptionalExtension, params};

use super::{RunSummary, Store};

#[derive(Default)]
pub struct RunListOptions {
    pub delivered: bool,
    pub before: Option<String>,
    pub search: Option<String>,
}

#[derive(serde::Serialize)]
pub struct RunPage {
    pub runs: Vec<RunSummary>,
    pub next_before: Option<String>,
}

const RUN_JOIN: &str = "FROM brief_run r LEFT JOIN delivery_once ON delivery_once.run_id = r.id LEFT JOIN delivery ON delivery.id = delivery_once.delivery_id";

impl Store {
    pub fn list_runs(&self, limit: i64, options: &RunListOptions) -> Result<RunPage> {
        let probe_limit = limit
            .checked_add(1)
            .filter(|_| limit > 0)
            .context("limit must be positive and leave room for one pagination probe")?;
        // Stored UTC fractions omit trailing zeroes. Removing Z keeps .1 before
        // .12 and whole seconds before fractions, without losing nanoseconds.
        let order = if options.delivered {
            "rtrim(delivery.delivered_at, 'Z')"
        } else {
            "rtrim(r.started_at, 'Z')"
        };
        let cursor = if let Some(before) = &options.before {
            let timestamp = self
                .connection
                .query_row(
                    &format!("SELECT {order} {RUN_JOIN} WHERE r.id = ?1"),
                    [before],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten();
            Some(
                timestamp
                    .with_context(|| format!("unknown cursor in selected run order: {before}"))?,
            )
        } else {
            None
        };
        if options.before.as_ref().is_some_and(String::is_empty) {
            bail!("before requires a run id");
        }
        let query = format!(
            "SELECT r.id, r.started_at, r.finished_at, r.dry_run, r.status, r.summary, delivery.delivered_at, delivery.message {RUN_JOIN} \
             WHERE (?1 = 0 OR delivery.delivered_at IS NOT NULL) \
             AND (?2 IS NULL OR ({order}, r.id) < (?2, ?3)) \
             AND (?4 IS NULL OR instr(lower(r.id), lower(?4)) > 0 \
                 OR instr(lower(r.summary), lower(?4)) > 0 \
                 OR instr(r.started_at, ?4) > 0 OR instr(r.finished_at, ?4) > 0 \
                 OR instr(delivery.delivered_at, ?4) > 0) \
             ORDER BY {order} DESC, r.id DESC LIMIT ?5"
        );
        let mut statement = self
            .connection
            .prepare(&query)
            .context("prepare run archive")?;
        let mut runs = statement
            .query_map(
                params![
                    options.delivered,
                    cursor,
                    options.before,
                    options.search,
                    probe_limit
                ],
                super::run_items::run_summary_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let has_more = i64::try_from(runs.len()).context("run page length exceeds i64")? > limit;
        if has_more {
            runs.pop();
        }
        let next_before = if has_more {
            runs.last().map(|run| run.run_id.clone())
        } else {
            None
        };
        Ok(RunPage { runs, next_before })
    }
}
