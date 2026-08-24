use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use anyhow::{Context, Result};
use chrono::{SecondsFormat, TimeDelta, Utc};

use crate::contract::{
    BriefItem, BriefRequest, BriefResult, FetchStatus, Paths, PreviousBrief, SuppressedItem,
    SuppressedPolicyItem, SuppressedUnresolvedItem,
};
use crate::domain::{OUTLET_POLICY_BLOCK, Source};
use crate::engine::{
    FetchOutput, Fetcher, RecentSuppression, add_fetch_failure_warning,
    add_recurring_failure_warnings, add_stale_heartbeat_warning, brief_summary,
    build_health_footnote, classify_and_dedupe, collect_new_items, enabled_source_keys,
    process_source_items, sort_brief_items, suppress_recent_candidates,
};
use crate::storage::{
    DEFAULT_MAX_DELIVERY_ITEMS, Delivery, FetchLog, MAX_DELIVERY_ITEMS_UPPER_BOUND,
    RUN_ITEM_CANDIDATE, RUN_ITEM_DROPPED, RUN_ITEM_MUST_INCLUDE, RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
    RunItemRow, SourceState, Store,
};

use super::delivery;

const SOURCE_FETCH_CONCURRENCY: usize = 4;

pub(super) fn run_action(
    paths: Paths,
    store: &Store,
    request: &BriefRequest,
) -> Result<BriefResult> {
    match request.action.as_str() {
        "validate" => Ok(BriefResult {
            paths,
            summary: "valid".to_owned(),
            ..BriefResult::default()
        }),
        "run_brief" => run(paths, store, request.dry_run),
        "record_delivery" => delivery::record(paths, store, request),
        _ => Ok(delivery::rejected(
            paths,
            &format!("unsupported brief action {:?}", request.action),
        )),
    }
}

fn run(paths: Paths, store: &Store, dry_run: bool) -> Result<BriefResult> {
    let sources = store.list_sources(true)?;
    if sources.is_empty() {
        return Ok(delivery::rejected(paths, "no enabled sources configured"));
    }
    let previous = store.recent_deliveries(2)?;
    let run_id = if dry_run {
        "dry-run".to_owned()
    } else {
        store.start_run(false)?
    };
    let now = Utc::now();
    let recent_since = now
        .checked_sub_signed(TimeDelta::hours(24))
        .context("calculate recent delivery window")?;
    let recent = store.recent_sent_items(recent_since)?;
    let policies = store.list_outlet_policies()?;
    let fetched = fetch_sources(&sources);
    let source_run = SourceRun {
        store,
        policies: &policies,
        run_id: &run_id,
        dry_run,
        now,
    };
    let mut accumulator = Accumulator::default();
    for (source, output) in sources.iter().zip(fetched) {
        process_source(source, output, &source_run, &mut accumulator)?;
    }
    let mut classified = classify_and_dedupe(accumulator.collected);
    let recent_result = suppress_recent_candidates(classified.candidates, &recent);
    let run_rows = if dry_run {
        Vec::new()
    } else {
        run_item_rows(
            &classified.must_include,
            &recent_result.candidates,
            &recent_result,
            &classified.suppressed,
            &accumulator.suppressed_policy,
            &accumulator.suppressed_unresolved,
        )
    };
    classified.suppressed.extend(recent_result.suppressed);
    let runtime_config = store.runtime_config()?;
    let options = brief_options(&runtime_config);
    if let Some(warning) = options.warning {
        let _previous = accumulator.warnings.insert(
            format!("runtime:{RUNTIME_CONFIG_MAX_DELIVERY_ITEMS}"),
            warning,
        );
    }
    add_stale_heartbeat_warning(&runtime_config, &mut accumulator.warnings, now);
    if !dry_run {
        let logs = store.recent_fetch_logs(500)?;
        add_recurring_failure_warnings(
            &logs,
            &enabled_source_keys(&sources),
            &mut accumulator.warnings,
        );
    }
    let health_delta = store.health_delta(&accumulator.warnings, !dry_run)?;
    if !dry_run {
        store.set_runtime_config(
            "last_check",
            &now.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        )?;
    }
    let health_footnote = build_health_footnote(&health_delta);
    sort_brief_items(&mut classified.must_include);
    let mut candidates = recent_result.candidates;
    sort_brief_items(&mut candidates);
    accumulator
        .statuses
        .sort_by(|left, right| left.source_key.cmp(&right.source_key));
    let summary = brief_summary(&classified.must_include, &candidates, &health_footnote);
    if !dry_run {
        store.insert_run_items(&run_id, &run_rows)?;
        store.finish_run(&run_id, "ok", &summary)?;
    }
    Ok(BriefResult {
        paths,
        run_id,
        must_include: classified.must_include,
        candidates,
        previous_briefs: previous.into_iter().map(previous_brief).collect(),
        delivery_message_scope: "current_brief_only".to_owned(),
        recent_sent: recent.into_iter().map(delivery::sent_item).collect(),
        suppressed: classified.suppressed,
        suppressed_recent: recent_result.suppressed_recent,
        suppressed_policy: accumulator.suppressed_policy,
        suppressed_unresolved: accumulator.suppressed_unresolved,
        fetch_status: accumulator.statuses,
        health_footnote,
        health_delta,
        max_delivery_items: options.max_delivery_items,
        summary,
        ..BriefResult::default()
    })
}

#[derive(Default)]
struct Accumulator {
    collected: Vec<crate::engine::CollectedItem>,
    suppressed_policy: Vec<crate::contract::SuppressedPolicyItem>,
    suppressed_unresolved: Vec<crate::contract::SuppressedUnresolvedItem>,
    statuses: Vec<FetchStatus>,
    warnings: BTreeMap<String, String>,
}

struct SourceRun<'a> {
    store: &'a Store,
    policies: &'a [crate::domain::OutletPolicy],
    run_id: &'a str,
    dry_run: bool,
    now: chrono::DateTime<Utc>,
}

fn process_source(
    source: &Source,
    output: Result<FetchOutput>,
    run: &SourceRun<'_>,
    accumulator: &mut Accumulator,
) -> Result<()> {
    let state = run.store.source_state(&source.key)?;
    let output = match output {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            add_fetch_failure_warning(&source.key, &message, &mut accumulator.warnings);
            accumulator.statuses.push(FetchStatus {
                source_key: source.key.clone(),
                status: "error".to_owned(),
                error: message.clone(),
                ..FetchStatus::default()
            });
            if !run.dry_run {
                let _inserted = run.store.insert_fetch_log(&FetchLog {
                    run_id: run.run_id.to_owned(),
                    source_key: source.key.clone(),
                    status: "error".to_owned(),
                    error: message,
                    ..FetchLog::default()
                });
            }
            return Ok(());
        }
    };
    let item_count = output.items.len();
    let unresolved_count = output.unresolved.len();
    let processed = process_source_items(source, output, run.policies, state.as_ref());
    let new_count = processed.new_items.len();
    let policy_count = processed.suppressed_policy.len();
    accumulator
        .collected
        .extend(collect_new_items(source, &processed.new_items));
    accumulator
        .suppressed_policy
        .extend(processed.suppressed_policy);
    accumulator
        .suppressed_unresolved
        .extend(processed.suppressed_unresolved);
    if !run.dry_run {
        if let Some(top) = processed.items.first() {
            run.store.upsert_source_state(&SourceState {
                source_key: source.key.clone(),
                latest_identity: top.identity.clone(),
                latest_feed_identity: top.feed_identity().to_owned(),
                latest_title: top.title.clone(),
                latest_url: top.url.clone(),
                latest_published_at: top.published_at.clone(),
                checked_at: run.now,
            })?;
        }
        let _inserted = run.store.insert_fetch_log(&FetchLog {
            run_id: run.run_id.to_owned(),
            source_key: source.key.clone(),
            status: "ok".to_owned(),
            item_count,
            new_item_count: new_count,
            ..FetchLog::default()
        });
    }
    accumulator.statuses.push(FetchStatus {
        source_key: source.key.clone(),
        status: "ok".to_owned(),
        items: item_count,
        new_items: new_count,
        suppressed_policy: policy_count,
        suppressed_unresolved: unresolved_count,
        ..FetchStatus::default()
    });
    Ok(())
}

fn fetch_sources(sources: &[Source]) -> Vec<Result<FetchOutput>> {
    let fetcher = Fetcher::new();
    run_bounded_ordered(sources, SOURCE_FETCH_CONCURRENCY, |source| {
        fetcher.fetch(source)
    })
}

fn run_bounded_ordered<Input, Output, Work>(
    inputs: &[Input],
    concurrency: usize,
    work: Work,
) -> Vec<Result<Output>>
where
    Input: Sync,
    Output: Send,
    Work: Fn(&Input) -> Result<Output> + Sync,
{
    if inputs.is_empty() {
        return Vec::new();
    }
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        let workers = (0..concurrency.min(inputs.len()))
            .map(|_worker| {
                let next = &next;
                let work = &work;
                scope.spawn(move || {
                    let mut outputs = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(input) = inputs.get(index) else {
                            break;
                        };
                        outputs.push((index, work(input)));
                    }
                    outputs
                })
            })
            .collect::<Vec<_>>();
        let mut ordered = std::iter::repeat_with(|| None)
            .take(inputs.len())
            .collect::<Vec<_>>();
        for worker in workers {
            let Ok(outputs) = worker.join() else {
                continue;
            };
            for (index, output) in outputs {
                if let Some(slot) = ordered.get_mut(index) {
                    *slot = Some(output);
                }
            }
        }
        ordered
            .into_iter()
            .map(|output| {
                output.unwrap_or_else(|| Err(anyhow::anyhow!("source fetch worker panicked")))
            })
            .collect()
    })
}

struct BriefOptions {
    max_delivery_items: i64,
    warning: Option<String>,
}

/// Converts one finished run's outcome into persisted selection evidence.
/// Dropped rows keep their own context because suppression happens before the
/// full item view exists.
fn run_item_rows(
    must_include: &[BriefItem],
    candidates: &[BriefItem],
    recent_result: &RecentSuppression,
    same_run_suppressed: &[SuppressedItem],
    suppressed_policy: &[SuppressedPolicyItem],
    suppressed_unresolved: &[SuppressedUnresolvedItem],
) -> Vec<RunItemRow> {
    let mut rows: Vec<RunItemRow> = must_include
        .iter()
        .map(|item| item_row(RUN_ITEM_MUST_INCLUDE, item, "", ""))
        .collect();
    rows.extend(
        candidates
            .iter()
            .map(|item| item_row(RUN_ITEM_CANDIDATE, item, "", "")),
    );
    rows.extend(same_run_suppressed.iter().map(|item| RunItemRow {
        category: RUN_ITEM_DROPPED.to_owned(),
        source_key: item.source_key.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
        reason: item.reason.clone(),
        ..RunItemRow::default()
    }));
    rows.extend(recent_result.suppressed_recent.iter().map(|item| {
        RunItemRow {
            category: RUN_ITEM_DROPPED.to_owned(),
            source_key: item.source_key.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            reason: "recently_sent".to_owned(),
            detail: serde_json::json!({
                "matched_prior_title": item.matched_prior_title,
                "prior_sent_at": item.prior_sent_at,
            })
            .to_string(),
            ..RunItemRow::default()
        }
    }));
    // Only blocked outlets remove items; watch and allow audits stay queryable
    // through the candidate row that survives.
    rows.extend(
        suppressed_policy
            .iter()
            .filter(|item| item.policy == OUTLET_POLICY_BLOCK)
            .map(|item| RunItemRow {
                category: RUN_ITEM_DROPPED.to_owned(),
                source_key: item.source_key.clone(),
                outlet: item.outlet.clone(),
                title: item.title.clone(),
                url: item.url.clone(),
                reason: "outlet_policy".to_owned(),
                detail: serde_json::json!({ "policy": item.policy }).to_string(),
                ..RunItemRow::default()
            }),
    );
    rows.extend(suppressed_unresolved.iter().map(|item| RunItemRow {
        category: RUN_ITEM_DROPPED.to_owned(),
        source_key: item.source_key.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
        reason: "unresolved".to_owned(),
        detail: serde_json::json!({ "reason": item.reason }).to_string(),
        ..RunItemRow::default()
    }));
    rows
}

fn item_row(category: &str, item: &BriefItem, reason: &str, detail: &str) -> RunItemRow {
    RunItemRow {
        category: category.to_owned(),
        source_key: item.source_key.clone(),
        source_label: item.source_label.clone(),
        kind: item.kind.clone(),
        section: item.section.clone(),
        threshold: item.threshold.clone(),
        priority_rank: item.priority_rank,
        always_report: item.always_report,
        published_at: item.published_at.clone(),
        outlet: item.outlet.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
        reason: reason.to_owned(),
        detail: detail.to_owned(),
    }
}

fn brief_options(config: &BTreeMap<String, String>) -> BriefOptions {
    let Some(raw) = config
        .get(RUNTIME_CONFIG_MAX_DELIVERY_ITEMS)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return BriefOptions {
            max_delivery_items: DEFAULT_MAX_DELIVERY_ITEMS,
            warning: None,
        };
    };
    if let Ok(value) = raw.parse::<i64>()
        && (1..=MAX_DELIVERY_ITEMS_UPPER_BOUND).contains(&value)
    {
        return BriefOptions {
            max_delivery_items: value,
            warning: None,
        };
    }
    BriefOptions {
        max_delivery_items: DEFAULT_MAX_DELIVERY_ITEMS,
        warning: Some(format!(
            "`{RUNTIME_CONFIG_MAX_DELIVERY_ITEMS}` config value {raw:?} is invalid; using default {DEFAULT_MAX_DELIVERY_ITEMS}"
        )),
    }
}

fn previous_brief(delivery: Delivery) -> PreviousBrief {
    PreviousBrief {
        run_id: delivery.run_id,
        delivered_at: delivery
            .delivered_at
            .to_rfc3339_opts(SecondsFormat::AutoSi, true),
        message: delivery.message,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Condvar, Mutex, mpsc};
    use std::time::Duration;

    use anyhow::{Context, Result};

    use super::run_bounded_ordered;

    #[test]
    fn bounded_workers_start_new_work_when_a_slot_opens() {
        let inputs = (0_u8..8).collect::<Vec<_>>();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_gate = Arc::clone(&gate);
        let (started_sender, started_receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            run_bounded_ordered(&inputs, 4, |input| -> Result<u8> {
                started_sender
                    .send(*input)
                    .context("report started bounded work")?;
                if *input == 0 {
                    let (lock, condition) = &*worker_gate;
                    let mut released = lock
                        .lock()
                        .map_err(|error| anyhow::anyhow!("lock bounded worker gate: {error}"))?;
                    while !*released {
                        released = condition.wait(released).map_err(|error| {
                            anyhow::anyhow!("wait for bounded worker gate: {error}")
                        })?;
                    }
                    drop(released);
                }
                Ok(*input)
            })
        });

        let mut started = Vec::new();
        for _expected in 0..5 {
            let Ok(input) = started_receiver.recv_timeout(Duration::from_secs(1)) else {
                break;
            };
            started.push(input);
        }
        let (lock, condition) = &*gate;
        let mut released = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *released = true;
        condition.notify_all();
        drop(released);
        let results = worker.join().unwrap_or_default();
        assert!(
            started.contains(&4),
            "fifth input did not start while the first input remained blocked: {started:?}"
        );
        let outputs = results
            .into_iter()
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        assert_eq!(
            outputs,
            (0_u8..8).collect::<Vec<_>>(),
            "bounded worker results lost source order"
        );
    }
}
