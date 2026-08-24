use anyhow::Result;
use chrono::{SecondsFormat, Utc};
use regex::Regex;

use crate::contract::{BriefRequest, BriefResult, DeliveryRecord, Paths, SentItem};
use crate::storage::{Delivery, Store, StoredSentItem};

pub(super) fn record(paths: Paths, store: &Store, request: &BriefRequest) -> Result<BriefResult> {
    if request.run_id.trim().is_empty() {
        return Ok(rejected(paths, "run_id is required"));
    }
    if request.message.trim().is_empty() {
        return Ok(rejected(paths, "message is required"));
    }
    if contains_history_wrapper(&request.message) {
        return Ok(rejected(
            paths,
            "message must contain only the current brief body, without Current brief or Previous brief sections",
        ));
    }
    if !store.brief_run_exists(&request.run_id)? {
        return Ok(rejected(
            paths,
            "run_id was not produced by the current SiftWire database",
        ));
    }
    let items = delivery_items(&request.message)?;
    let stored = match store.insert_delivery(&request.run_id, &request.message, items) {
        Ok(value) => value,
        Err(error) if error.is_conflict() => return Ok(rejected(paths, &error.to_string())),
        Err(error) => return Err(error.into()),
    };
    let mut result = BriefResult {
        paths,
        run_id: request.run_id.clone(),
        summary: format!("recorded delivery with {} sent items", stored.len()),
        sent_items: stored.into_iter().map(sent_item).collect(),
        ..BriefResult::default()
    };
    let Ok(deliveries) = store.recent_deliveries(3) else {
        result.final_answer = final_answer(&request.run_id, &request.message, &[]);
        return Ok(result);
    };
    result.deliveries = deliveries.into_iter().map(delivery_record).collect();
    result.final_answer = final_answer(&request.run_id, &request.message, &result.deliveries);
    Ok(result)
}

fn contains_history_wrapper(message: &str) -> bool {
    message
        .lines()
        .map(str::trim)
        .any(|line| line == "Current brief" || line.starts_with("Previous brief ("))
}

fn delivery_items(message: &str) -> Result<Vec<StoredSentItem>> {
    let pattern = Regex::new(r"(?m)^-\s+\[([^\]]+)\]\(<([^>]+)>\)\s*$")?;
    Ok(pattern
        .captures_iter(message)
        .filter_map(|captures| {
            let title = captures.get(1)?.as_str().trim();
            let url = captures.get(2)?.as_str().trim();
            if title.is_empty() || url.is_empty() {
                return None;
            }
            Some(StoredSentItem {
                title: title.to_owned(),
                url: url.to_owned(),
                sent_at: chrono::DateTime::<Utc>::default(),
            })
        })
        .collect())
}

pub(super) fn sent_item(item: StoredSentItem) -> SentItem {
    SentItem {
        title: item.title,
        url: item.url,
        sent_at: item.sent_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
    }
}

pub(super) fn delivery_record(delivery: Delivery) -> DeliveryRecord {
    DeliveryRecord {
        run_id: delivery.run_id,
        delivered_at: delivery
            .delivered_at
            .to_rfc3339_opts(SecondsFormat::AutoSi, true),
        message: delivery.message,
    }
}

fn final_answer(run_id: &str, message: &str, deliveries: &[DeliveryRecord]) -> String {
    let mut answer = format!("Current brief\n\n{message}");
    let mut deliveries = deliveries.iter();
    if !deliveries
        .next()
        .is_some_and(|delivery| delivery.run_id == run_id && delivery.message == message)
    {
        return answer;
    }
    for delivery in deliveries.take(2) {
        answer.push_str("\n\nPrevious brief (");
        answer.push_str(&delivery.delivered_at);
        answer.push_str(")\n\n");
        answer.push_str(&delivery.message);
    }
    answer
}

pub(super) fn rejected(paths: Paths, reason: &str) -> BriefResult {
    BriefResult {
        rejected: true,
        rejection_reason: reason.to_owned(),
        paths,
        summary: reason.to_owned(),
        ..BriefResult::default()
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::DeliveryRecord;

    use super::final_answer;

    fn delivery(run_id: &str, delivered_at: &str, message: &str) -> DeliveryRecord {
        DeliveryRecord {
            run_id: run_id.to_owned(),
            delivered_at: delivered_at.to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn latest_delivery_includes_two_prior_runs_even_when_messages_repeat() {
        let deliveries = [
            delivery("run-3", "2026-04-23T03:00:00Z", "NO_REPLY"),
            delivery("run-2", "2026-04-23T02:00:00Z", "NO_REPLY"),
            delivery("run-1", "2026-04-23T01:00:00Z", "Earlier"),
        ];
        assert_eq!(
            final_answer("run-3", "NO_REPLY", &deliveries),
            "Current brief\n\nNO_REPLY\n\nPrevious brief (2026-04-23T02:00:00Z)\n\nNO_REPLY\n\nPrevious brief (2026-04-23T01:00:00Z)\n\nEarlier",
            "delivery history was deduplicated by message instead of run identity"
        );
    }

    #[test]
    fn retry_of_an_older_delivery_returns_only_that_run() {
        let deliveries = [
            delivery("run-3", "2026-04-23T03:00:00Z", "Newer"),
            delivery("run-2", "2026-04-23T02:00:00Z", "Retried"),
            delivery("run-1", "2026-04-23T01:00:00Z", "Earlier"),
        ];
        assert_eq!(
            final_answer("run-2", "Retried", &deliveries),
            "Current brief\n\nRetried",
            "retry of an older run included newer delivery history"
        );
    }
}
