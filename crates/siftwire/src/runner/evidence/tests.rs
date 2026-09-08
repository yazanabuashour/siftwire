#![expect(
    clippy::panic_in_result_fn,
    reason = "evidence tests use Result for temporary storage setup"
)]

use anyhow::{Context, Result};
use tempfile::TempDir;

use super::*;
use crate::contract::{BriefRequest, Paths};
use crate::storage::{Store, StoredSentItem};

fn fixture() -> Result<(TempDir, Store, String)> {
    let temp = TempDir::new()?;
    let store = Store::open(temp.path().join("evidence.sqlite"))?;
    let run = store.start_run(false)?;
    let update = SportsUpdate {
        source_key: "sport".to_owned(),
        title: "Alpha vs Beta".to_owned(),
        url: "https://example.test/card".to_owned(),
        competition: "League".to_owned(),
        status: "upcoming".to_owned(),
        fixture_identity: "fixture-1".to_owned(),
        ..SportsUpdate::default()
    };
    let projected = crate::engine::sports_update_item(&update, chrono_tz::UTC);
    store.insert_run_items(
        &run,
        &[
            RunItemRow {
                category: RUN_ITEM_MUST_INCLUDE.to_owned(),
                kind: SOURCE_KIND_SCHEDULE.to_owned(),
                source_key: update.source_key.clone(),
                title: projected.title,
                url: update.url.clone(),
                published_at: projected.published_at,
                ..RunItemRow::default()
            },
            RunItemRow {
                category: RUN_ITEM_MUST_INCLUDE.to_owned(),
                kind: SOURCE_KIND_SCHEDULE.to_owned(),
                source_key: update.source_key.clone(),
                title: "Different bout on the same card".to_owned(),
                url: update.url.clone(),
                ..RunItemRow::default()
            },
            RunItemRow {
                category: RUN_ITEM_CANDIDATE.to_owned(),
                kind: "rss".to_owned(),
                title: update.title.clone(),
                url: update.url.clone(),
                ..RunItemRow::default()
            },
            RunItemRow {
                category: RUN_ITEM_CANDIDATE.to_owned(),
                kind: "rss".to_owned(),
                title: "Chosen news".to_owned(),
                url: "https://example.test/news".to_owned(),
                ..RunItemRow::default()
            },
        ],
    )?;
    store.insert_run_delivery_context(
        &run,
        &RunDeliveryContext {
            max_delivery_items: 7,
            sports_updates: vec![update],
            sports_timezone: "UTC".to_owned(),
            health_footnote: String::new(),
        },
    )?;
    store.finish_run(&run, "ok", "fixture")?;
    Ok((temp, store, run))
}

fn statuses(detail: &RunDetail) -> Vec<DeliveryStatus> {
    detail
        .items
        .iter()
        .map(|row| delivery_status(row, detail))
        .collect()
}

#[test]
fn prepared_references_survive_confirmation_and_do_not_select_shared_urls() -> Result<()> {
    let (temp, store, run) = fixture()?;
    let prepared = crate::runner::delivery::prepare(
        Paths::default(),
        &store,
        &BriefRequest {
            run_id: run.clone(),
            candidate_indexes: vec![1],
            ..BriefRequest::default()
        },
    )?;
    assert!(!prepared.rejected, "{}", prepared.rejection_reason);
    let detail = store.run_detail(&run)?.context("prepared detail")?;
    assert_eq!(statuses(&detail), vec![DeliveryStatus::NotDelivered; 4]);
    let plan = detail.delivery_plan.context("plan")?;
    assert!(
        plan.items
            .iter()
            .all(|item| item.run_item_ids.as_ref().is_some_and(|ids| ids.len() == 1)),
        "new plans carry stable row references"
    );
    let confirmed = crate::runner::delivery::confirm(
        Paths::default(),
        &store,
        &BriefRequest {
            delivery_plan_id: plan.id,
            ..BriefRequest::default()
        },
    )?;
    assert!(!confirmed.rejected, "{}", confirmed.rejection_reason);
    drop(store);
    let store = Store::open(temp.path().join("evidence.sqlite"))?;
    let detail = store.run_detail(&run)?.context("confirmed detail")?;
    assert_eq!(
        statuses(&detail),
        [
            DeliveryStatus::SentAsSports,
            DeliveryStatus::Unknown,
            DeliveryStatus::NotSelected,
            DeliveryStatus::Sent
        ]
    );
    Ok(())
}

#[test]
fn legacy_sports_needs_unique_recorded_projection_and_confirmation() -> Result<()> {
    let (_temp, store, run) = fixture()?;
    let mut detail = store.run_detail(&run)?.context("detail")?;
    detail.summary.delivered_at = Some("2026-01-01T00:00:00Z".to_owned());
    detail.sent_items.push(StoredSentItem {
        title: "Alpha vs Beta".to_owned(),
        url: "https://example.test/card".to_owned(),
        ..StoredSentItem::default()
    });
    // The ordinary candidate has the same short title and URL: neither link
    // parsing nor source kind can prove that the sports card was sent.
    assert_eq!(statuses(&detail).first(), Some(&DeliveryStatus::Unknown));
    detail
        .items
        .retain(|item| item.kind == SOURCE_KIND_SCHEDULE);
    assert_eq!(
        statuses(&detail),
        [DeliveryStatus::SentAsSports, DeliveryStatus::Unknown]
    );
    let context = detail.delivery_context.as_mut().context("context")?;
    let duplicate = context.sports_updates.first().context("update")?.clone();
    context.sports_updates.push(duplicate);
    assert_eq!(
        statuses(&detail),
        [DeliveryStatus::Unknown, DeliveryStatus::Unknown]
    );
    detail.delivery_context = None;
    assert_eq!(
        statuses(&detail),
        [DeliveryStatus::Unknown, DeliveryStatus::Unknown]
    );
    Ok(())
}

#[test]
fn legacy_plans_keep_exact_evidence_without_rewriting_stored_json() -> Result<()> {
    let (temp, store, run) = fixture()?;
    let prepared = crate::runner::delivery::prepare(
        Paths::default(),
        &store,
        &BriefRequest {
            run_id: run.clone(),
            candidate_indexes: vec![1],
            ..BriefRequest::default()
        },
    )?;
    let db = rusqlite::Connection::open(temp.path().join("evidence.sqlite"))?;
    let raw: String = db.query_row(
        "SELECT items_json FROM delivery_plan WHERE run_id = ?1",
        [&run],
        |row| row.get(0),
    )?;
    let mut items: serde_json::Value = serde_json::from_str(&raw)?;
    for item in items.as_array_mut().context("plan items")? {
        item.as_object_mut()
            .context("plan item")?
            .remove("run_item_ids");
    }
    let legacy = items.to_string();
    db.execute(
        "UPDATE delivery_plan SET items_json = ?1 WHERE run_id = ?2",
        [&legacy, &run],
    )?;
    let confirmed = crate::runner::delivery::confirm(
        Paths::default(),
        &store,
        &BriefRequest {
            delivery_plan_id: prepared.delivery_plan_id,
            ..BriefRequest::default()
        },
    )?;
    assert!(!confirmed.rejected, "{}", confirmed.rejection_reason);
    drop(store);
    let store = Store::open(temp.path().join("evidence.sqlite"))?;
    let detail = store.run_detail(&run)?.context("detail")?;
    assert_eq!(
        statuses(&detail),
        [
            DeliveryStatus::SentAsSports,
            DeliveryStatus::Unknown,
            DeliveryStatus::NotSelected,
            DeliveryStatus::Sent
        ]
    );
    let preserved: String = db.query_row(
        "SELECT items_json FROM delivery_plan WHERE run_id = ?1",
        [&run],
        |row| row.get(0),
    )?;
    assert_eq!(
        preserved, legacy,
        "reading legacy evidence must not rewrite it"
    );
    Ok(())
}
