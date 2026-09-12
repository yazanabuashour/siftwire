use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use chrono::SecondsFormat;
use chrono_tz::Tz;
use url::Url;

use crate::contract::{
    BriefRequest, BriefResult, DeliveryItem, DeliveryRecord, Paths, SentItem, SportsUpdate,
};
use crate::domain::SOURCE_KIND_SCHEDULE;
use crate::engine::render_sports_section;
use crate::storage::{
    Delivery, DeliveryPlan, RUN_ITEM_CANDIDATE, RUN_ITEM_MUST_INCLUDE, RunDeliveryContext,
    RunItemRow, Store, StoredSentItem,
};

pub(super) fn prepare(paths: Paths, store: &Store, request: &BriefRequest) -> Result<BriefResult> {
    if request.run_id.trim().is_empty() {
        return Ok(rejected(paths, "run_id is required"));
    }
    let Some(run) = store.delivery_preparation(&request.run_id)? else {
        return Ok(rejected(
            paths,
            "run_id was not produced by the current SiftWire database",
        ));
    };
    if run.status != "ok" || run.dry_run {
        return Ok(rejected(paths, "run_id is not a finished durable brief"));
    }
    if run.delivered {
        return Ok(rejected(paths, "run_id is already delivered"));
    }
    if let Some(plan) = store.delivery_plan_for_run(&request.run_id)? {
        return Ok(if plan.candidate_indexes == request.candidate_indexes {
            prepared(paths, plan)
        } else {
            rejected(
                paths,
                "run_id already has a delivery plan with different candidate indexes",
            )
        });
    }
    let Some(context) = store.run_delivery_context(&request.run_id)? else {
        return Ok(rejected(paths, "run_id has no recorded delivery context"));
    };
    let plan = match build_plan(
        &request.run_id,
        &request.candidate_indexes,
        &run.started_at,
        &store.run_delivery_items(&request.run_id)?,
        &context,
    ) {
        Ok(value) => value,
        Err(error) => return Ok(rejected(paths, &error.to_string())),
    };
    let Some(plan) = store.insert_delivery_plan(&plan)? else {
        return Ok(rejected(
            paths,
            "run_id already has a delivery plan with different candidate indexes",
        ));
    };
    Ok(prepared(paths, plan))
}

fn prepared(paths: Paths, plan: DeliveryPlan) -> BriefResult {
    BriefResult {
        paths,
        run_id: plan.run_id,
        delivery_plan_id: plan.id,
        message: plan.message,
        text: plan.text,
        html: plan.html,
        prepared_items: plan.items,
        summary: "prepared immutable delivery plan".to_owned(),
        ..BriefResult::default()
    }
}

pub(super) fn confirm(paths: Paths, store: &Store, request: &BriefRequest) -> Result<BriefResult> {
    if request.delivery_plan_id.trim().is_empty() {
        return Ok(rejected(paths, "delivery_plan_id is required"));
    }
    let Some(plan) = store.delivery_plan(&request.delivery_plan_id)? else {
        return Ok(rejected(
            paths,
            "delivery_plan_id was not produced by the current SiftWire database",
        ));
    };
    if !request.run_id.trim().is_empty() && request.run_id != plan.run_id {
        return Ok(rejected(
            paths,
            "run_id does not match the prepared delivery plan",
        ));
    }
    record_message(paths, store, &plan)
}

fn build_plan(
    run_id: &str,
    candidate_indexes: &[usize],
    started_at: &str,
    rows: &[RunItemRow],
    context: &RunDeliveryContext,
) -> Result<DeliveryPlan> {
    let required = rows
        .iter()
        .filter(|row| row.category == RUN_ITEM_MUST_INCLUDE)
        .collect::<Vec<_>>();
    let candidates = rows
        .iter()
        .filter(|row| row.category == RUN_ITEM_CANDIDATE)
        .collect::<Vec<_>>();
    let max_items = usize::try_from(context.max_delivery_items).unwrap_or_default();
    let candidate_slots = max_items.saturating_sub(required.len());
    if candidate_indexes.len() > candidate_slots {
        anyhow::bail!(
            "candidate selection has {} indexes but only {candidate_slots} slots remain",
            candidate_indexes.len()
        );
    }
    let mut seen = BTreeSet::new();
    let mut selected = required;
    for index in candidate_indexes {
        if !seen.insert(*index) {
            anyhow::bail!("candidate selection contains duplicate index {index}");
        }
        selected.push(
            candidates
                .get(*index)
                .copied()
                .with_context(|| format!("candidate index {index} is out of range"))?,
        );
    }
    let selected = selected
        .into_iter()
        .map(|row| {
            let mut row = row.clone();
            row.url = normalize_delivery_url(&row.url).unwrap_or_default();
            row
        })
        .collect::<Vec<_>>();
    let selected_refs = selected.iter().collect::<Vec<_>>();
    let mut context = context.clone();
    for update in &mut context.sports_updates {
        update.url = normalize_delivery_url(&update.url).unwrap_or_default();
        update.images.retain_mut(|image| {
            let Some(url) = normalize_delivery_image_url(&image.url) else {
                return false;
            };
            image.url = url;
            true
        });
    }
    let mut items = selected
        .iter()
        .map(|row| DeliveryItem {
            run_item_ids: vec![row.id.clone()],
            title: row.title.clone(),
            url: row.url.clone(),
            kind: row.kind.clone(),
        })
        .collect::<Vec<_>>();
    items.extend(context.sports_updates.iter().map(|update| DeliveryItem {
        run_item_ids: Vec::new(),
        title: update.title.clone(),
        url: update.url.clone(),
        kind: SOURCE_KIND_SCHEDULE.to_owned(),
    }));
    let (message, text, html) = render_bodies(&selected_refs, &context, started_at)?;
    Ok(DeliveryPlan {
        id: format!("plan-{run_id}"),
        run_id: run_id.to_owned(),
        candidate_indexes: candidate_indexes.to_vec(),
        message,
        text,
        html,
        items,
    })
}

fn render_bodies(
    selected: &[&RunItemRow],
    context: &RunDeliveryContext,
    started_at: &str,
) -> Result<(String, String, String)> {
    let markdown_items = selected
        .iter()
        .map(|item| {
            if item.url.is_empty() {
                format!("- {}", markdown_link_text(&item.title))
            } else {
                format!("- [{}](<{}>)", markdown_link_text(&item.title), item.url)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let text_items = selected
        .iter()
        .map(|item| {
            let title = plain_text(&item.title);
            if item.url.is_empty() {
                format!("- {title}")
            } else {
                format!("- {title}\n  {}", item.url)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let timezone = context
        .sports_timezone
        .parse::<Tz>()
        .context("stored sports timezone is invalid")?;
    let sports_text = render_sports_text(&context.sports_updates, timezone);
    let sports_markdown = render_sports_section(&context.sports_updates, timezone);
    let message = join_parts(&[&markdown_items, &sports_markdown, &context.health_footnote]);
    let text = join_parts(&[&text_items, &sports_text, &context.health_footnote]);
    let html = super::email::render(selected, context, started_at, timezone)?;
    if message.is_empty() {
        return Ok(("NO_REPLY".to_owned(), "NO_REPLY".to_owned(), String::new()));
    }
    Ok((message, text, html))
}

fn render_sports_text(updates: &[SportsUpdate], timezone: Tz) -> String {
    if updates.is_empty() {
        return String::new();
    }
    let mut output = "Sports".to_owned();
    append_sports_text_group(
        &mut output,
        "Upcoming fixtures",
        "upcoming",
        updates,
        timezone,
    );
    append_sports_text_group(&mut output, "Recent results", "final", updates, timezone);
    output
}

fn append_sports_text_group(
    output: &mut String,
    heading: &str,
    status: &str,
    updates: &[SportsUpdate],
    timezone: Tz,
) {
    let mut matching = updates
        .iter()
        .filter(|update| update.status == status)
        .peekable();
    if matching.peek().is_none() {
        return;
    }
    let _result = write!(output, "\n\n{heading}");
    for update in matching {
        let when = update
            .starts_at
            .with_timezone(&timezone)
            .format("%a %b %-d, %-I:%M %p %Z");
        let final_label = if status == "final" { ", final" } else { "" };
        if update.url.is_empty() {
            let _result = write!(
                output,
                "\n- {} — {}{final_label}, {when}",
                plain_text(&update.title),
                plain_text(&update.competition)
            );
        } else {
            let _result = write!(
                output,
                "\n- {} — {}{final_label}, {when}\n  {}",
                plain_text(&update.title),
                plain_text(&update.competition),
                update.url
            );
        }
    }
}

fn plain_text(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn markdown_link_text(value: &str) -> String {
    plain_text(value)
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

pub(super) fn normalize_delivery_url(value: &str) -> Result<String> {
    let url = Url::parse(value).context("delivery item URL is invalid")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        anyhow::bail!("delivery item URL must be an absolute HTTP or HTTPS URL");
    }
    Ok(url.into())
}

fn normalize_delivery_image_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some("a.espncdn.com" | "static.lolesports.com")
        ))
    .then(|| url.into())
}

fn join_parts(parts: &[&str]) -> String {
    parts
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn record_message(paths: Paths, store: &Store, plan: &DeliveryPlan) -> Result<BriefResult> {
    let run_id = &plan.run_id;
    let message = &plan.message;
    let stored = match store.insert_delivery(plan) {
        Ok(value) => value,
        Err(error) if error.is_conflict() => return Ok(rejected(paths, &error.to_string())),
        Err(error) => return Err(error.into()),
    };
    let mut result = BriefResult {
        paths,
        run_id: run_id.to_owned(),
        summary: format!("recorded delivery with {} sent items", stored.len()),
        sent_items: stored.into_iter().map(sent_item).collect(),
        delivery_plan_id: plan.id.clone(),
        message: plan.message.clone(),
        prepared_items: plan.items.clone(),
        ..BriefResult::default()
    };
    let Ok(deliveries) = store.recent_deliveries(3) else {
        result.final_answer = final_answer(run_id, message, &[]);
        return Ok(result);
    };
    result.deliveries = deliveries.into_iter().map(delivery_record).collect();
    result.final_answer = final_answer(run_id, message, &result.deliveries);
    Ok(result)
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
    #![expect(
        clippy::panic_in_result_fn,
        reason = "behavior assertions report delivery contract failures"
    )]

    use crate::contract::DeliveryRecord;
    use crate::storage::{RUN_ITEM_MUST_INCLUDE, RunDeliveryContext, RunItemRow};

    use super::{build_plan, final_answer};

    fn delivery(run_id: &str, delivered_at: &str, message: &str) -> DeliveryRecord {
        DeliveryRecord {
            run_id: run_id.to_owned(),
            delivered_at: delivered_at.to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn prepared_delivery_renders_required_linkless_items_as_text() -> anyhow::Result<()> {
        let row = RunItemRow {
            category: RUN_ITEM_MUST_INCLUDE.to_owned(),
            kind: "rss".to_owned(),
            title: "Linkless item".to_owned(),
            ..RunItemRow::default()
        };
        let context = RunDeliveryContext {
            max_delivery_items: 7,
            sports_updates: Vec::new(),
            sports_timezone: "America/Chicago".to_owned(),
            health_footnote: String::new(),
        };
        let plan = build_plan(
            "run-linkless",
            &[],
            "2026-08-27T12:00:00Z",
            &[row],
            &context,
        )?;
        assert_eq!(plan.message, "- Linkless item");
        assert!(plan.html.contains(">Linkless item</span>"));
        assert!(!plan.html.contains("href=\"\""));
        Ok(())
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
