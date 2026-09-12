use anyhow::{Context, Result, anyhow, ensure};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{
    Delivery, DeliveryInsertError, Store, StoredSentItem, format_timestamp, parse_timestamp,
};

impl Store {
    /// Returns recent deliveries newest first.
    ///
    /// The limit must be positive.
    ///
    /// # Errors
    ///
    /// Returns an error when the limit is non-positive or a stored row is invalid.
    pub fn recent_deliveries(&self, limit: i64) -> Result<Vec<Delivery>> {
        ensure!(limit > 0, "delivery limit must be positive");
        let mut statement = self
            .connection
            .prepare(
                "SELECT run_id, message, delivered_at FROM delivery \
                 ORDER BY delivered_at DESC, id DESC LIMIT ?1",
            )
            .context("prepare recent delivery query")?;
        let rows = statement
            .query_map([limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .context("query recent deliveries")?;
        rows.map(|row| {
            let (run_id, message, delivered_at) = row.context("read delivery row")?;
            Ok(Delivery {
                run_id,
                message,
                delivered_at: parse_timestamp(&delivered_at)?,
            })
        })
        .collect()
    }

    /// Returns sent items at or after the supplied UTC timestamp, newest first.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot read a sent item row.
    pub fn recent_sent_items(&self, since: DateTime<Utc>) -> Result<Vec<StoredSentItem>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT title, url, kind, sent_at FROM sent_item \
                 WHERE sent_at >= ?1 AND kind != 'sports_schedule' \
                 ORDER BY sent_at DESC, id DESC",
            )
            .context("prepare recent sent item query")?;
        let rows = statement
            .query_map([format_timestamp(since)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .context("query recent sent items")?;
        rows.map(|row| {
            let (title, url, kind, sent_at) = row.context("read sent item row")?;
            Ok(StoredSentItem {
                title,
                url,
                kind,
                sent_at: parse_timestamp(&sent_at)?,
            })
        })
        .collect()
    }

    /// Atomically records one idempotent delivery and its sent items.
    ///
    /// Repeating a run identifier with the same message returns the original items.
    /// Repeating it with a different message returns [`DeliveryInsertError::Conflict`].
    ///
    /// # Errors
    ///
    /// Returns a conflict for a changed message, or a storage error when validation or `SQLite`
    /// work fails.
    pub fn insert_delivery(
        &self,
        run_id: &str,
        message: &str,
        mut items: Vec<StoredSentItem>,
    ) -> Result<Vec<StoredSentItem>, DeliveryInsertError> {
        if run_id.trim().is_empty() {
            return Err(anyhow!("run_id is required").into());
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .context("begin delivery insert")?;
        let claimed = transaction
            .execute(
                "INSERT INTO delivery_once (run_id, message) VALUES (?1, ?2) \
                 ON CONFLICT(run_id) DO NOTHING",
                params![run_id, message],
            )
            .with_context(|| format!("claim delivery run {run_id}"))?;
        if claimed == 0 {
            return existing_delivery(transaction, run_id, message);
        }
        let delivered_at = self.timestamp();
        transaction
            .execute(
                "INSERT INTO delivery (run_id, message, delivered_at) VALUES (?1, ?2, ?3)",
                params![run_id, message, format_timestamp(delivered_at)],
            )
            .with_context(|| format!("insert delivery for run {run_id}"))?;
        let delivery_id = transaction.last_insert_rowid();
        for item in &mut items {
            item.sent_at = delivered_at;
            insert_sent_item(&transaction, delivery_id, run_id, item)?;
        }
        transaction
            .execute(
                "UPDATE delivery_once SET delivery_id = ?1 WHERE run_id = ?2",
                params![delivery_id, run_id],
            )
            .with_context(|| format!("complete delivery claim for run {run_id}"))?;
        transaction
            .commit()
            .with_context(|| format!("commit delivery for run {run_id}"))?;
        Ok(items)
    }
}

fn existing_delivery(
    transaction: Transaction<'_>,
    run_id: &str,
    message: &str,
) -> Result<Vec<StoredSentItem>, DeliveryInsertError> {
    let stored = transaction
        .query_row(
            "SELECT message, delivery_id FROM delivery_once WHERE run_id = ?1",
            [run_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()
        .with_context(|| format!("read delivery claim for run {run_id}"))?
        .ok_or_else(|| anyhow!("delivery idempotency record disappeared for run {run_id}"))?;
    if stored.0 != message {
        return Err(DeliveryInsertError::Conflict);
    }
    let delivery_id = stored
        .1
        .ok_or_else(|| anyhow!("delivery idempotency record is incomplete"))?;
    let items = sent_items_for_delivery(&transaction, delivery_id)?;
    transaction
        .commit()
        .with_context(|| format!("commit idempotent delivery for run {run_id}"))?;
    Ok(items)
}

fn insert_sent_item(
    transaction: &Transaction<'_>,
    delivery_id: i64,
    run_id: &str,
    item: &StoredSentItem,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO sent_item \
             (delivery_id, run_id, title, url, kind, title_key, sent_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                delivery_id,
                run_id,
                item.title,
                item.url,
                item.kind,
                super::normalize_title_key(&item.title),
                format_timestamp(item.sent_at),
            ],
        )
        .with_context(|| format!("insert sent item for run {run_id}"))?;
    Ok(())
}

fn sent_items_for_delivery(
    transaction: &Transaction<'_>,
    delivery_id: i64,
) -> Result<Vec<StoredSentItem>> {
    let mut statement = transaction
        .prepare(
            "SELECT title, url, kind, sent_at FROM sent_item WHERE delivery_id = ?1 ORDER BY id",
        )
        .context("prepare idempotent delivery item query")?;
    let rows = statement
        .query_map([delivery_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .context("query idempotent delivery items")?;
    rows.map(|row| {
        let (title, url, kind, sent_at) = row.context("read idempotent delivery item")?;
        Ok(StoredSentItem {
            title,
            url,
            kind,
            sent_at: parse_timestamp(&sent_at)?,
        })
    })
    .collect()
}

/// Produces the normalized key used for sent-title deduplication.
#[must_use]
pub fn normalize_title_key(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_space = false;
    for character in text.trim().to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            normalized.push(character);
            previous_space = false;
        } else if !previous_space {
            normalized.push(' ');
            previous_space = true;
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}
