#![expect(
    clippy::panic_in_result_fn,
    reason = "assertions report normalized delivery contracts"
)]

use super::*;
use crate::contract::{BriefRequest, Paths};
use crate::storage::Store;
use anyhow::{Context, Result};

fn normalized_fixture() -> Result<(tempfile::TempDir, Store, String)> {
    let temp = tempfile::TempDir::new()?;
    let database = temp.path().join("normalized.sqlite");
    let store = Store::open(&database)?;
    let run_id = store.start_run(false)?;
    rusqlite::Connection::open(&database)?.execute(
        "INSERT INTO sqlite_sequence(name, seq) VALUES ('brief_run_item', 9007199254740992)",
        [],
    )?;
    let update = SportsUpdate {
        source_key: "sports".to_owned(),
        status: "upcoming".to_owned(),
        title: "Alpha at Beta".to_owned(),
        competition: "League".to_owned(),
        url: "https://EXAMPLE.test:443/card".to_owned(),
        starts_at: chrono::DateTime::from_timestamp(1_800_000_000, 0).unwrap_or_default(),
        ..SportsUpdate::default()
    };
    let mut second = update.clone();
    second.starts_at = second
        .starts_at
        .checked_add_signed(chrono::TimeDelta::days(1))
        .context("next fixture time")?;
    let context = RunDeliveryContext {
        max_delivery_items: 7,
        sports_updates: vec![update.clone(), second],
        sports_timezone: "UTC".to_owned(),
        health_footnote: String::new(),
    };
    let mut rows = context
        .sports_updates
        .iter()
        .map(|update| {
            let projected = crate::engine::sports_update_item(update, chrono_tz::UTC);
            RunItemRow {
                category: RUN_ITEM_MUST_INCLUDE.to_owned(),
                source_key: update.source_key.clone(),
                kind: SOURCE_KIND_SCHEDULE.to_owned(),
                title: projected.title,
                url: projected.url,
                published_at: projected.published_at,
                ..RunItemRow::default()
            }
        })
        .collect::<Vec<_>>();
    rows.extend([
        RunItemRow {
            category: RUN_ITEM_CANDIDATE.to_owned(),
            kind: "rss".to_owned(),
            title: "Normalized".to_owned(),
            url: "https://EXAMPLE.test:443".to_owned(),
            ..RunItemRow::default()
        },
        RunItemRow {
            category: RUN_ITEM_CANDIDATE.to_owned(),
            kind: "rss".to_owned(),
            title: update.title,
            url: update.url,
            ..RunItemRow::default()
        },
    ]);
    store.insert_run_items(&run_id, &rows)?;
    store.insert_run_delivery_context(&run_id, &context)?;
    store.finish_run(&run_id, "ok", "fixture")?;
    Ok((temp, store, run_id))
}

#[test]
fn references_disambiguate_identical_sports_links_and_preserve_large_ids() -> Result<()> {
    let (_temp, store, run_id) = normalized_fixture()?;
    let request = BriefRequest {
        run_id: run_id.clone(),
        candidate_indexes: vec![0],
        ..BriefRequest::default()
    };
    let prepared = crate::runner::delivery::prepare(Paths::default(), &store, &request)?;
    assert!(!prepared.rejected);
    let wire = serde_json::to_value(&prepared.prepared_items)?;
    assert_eq!(
        wire.pointer("/0/run_item_ids/0")
            .and_then(serde_json::Value::as_str),
        Some("9007199254740995")
    );
    assert_eq!(
        wire.pointer("/0/url").and_then(serde_json::Value::as_str),
        Some("https://example.test/")
    );
    let replay = crate::runner::delivery::prepare(Paths::default(), &store, &request)?;
    assert_eq!(prepared.prepared_items, replay.prepared_items);
    assert_eq!(prepared.html, replay.html);
    crate::runner::delivery::confirm(
        Paths::default(),
        &store,
        &BriefRequest {
            delivery_plan_id: prepared.delivery_plan_id,
            ..BriefRequest::default()
        },
    )?;
    let mut detail = store.run_detail(&run_id)?.context("detail")?;
    assert_eq!(
        detail
            .items
            .iter()
            .map(|row| delivery_status(row, &detail))
            .collect::<Vec<_>>(),
        [
            DeliveryStatus::SentAsSports,
            DeliveryStatus::SentAsSports,
            DeliveryStatus::Sent,
            DeliveryStatus::NotSelected
        ]
    );
    assert_eq!(
        detail.delivery_html.as_deref(),
        Some(prepared.html.as_str())
    );
    detail.delivery_plan = None;
    // A parsed link shared by sports and ordinary news cannot prove either row.
    detail.sent_items.truncate(2);
    assert_eq!(
        detail
            .items
            .iter()
            .map(|row| delivery_status(row, &detail))
            .collect::<Vec<_>>(),
        [
            DeliveryStatus::Unknown,
            DeliveryStatus::Unknown,
            DeliveryStatus::Sent,
            DeliveryStatus::Unknown
        ]
    );
    Ok(())
}
