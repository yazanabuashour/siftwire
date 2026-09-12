use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use anyhow::{Context, Result};
use chrono::{SecondsFormat, TimeDelta, Utc};
use chrono_tz::Tz;

use crate::contract::ItemDisposition;
use crate::contract::{
    BriefItem, BriefRequest, BriefResult, FetchStatus, HealthDelta, Paths, PreviousBrief,
    SportsUpdate, SuppressedItem, SuppressedPolicyItem, SuppressedUnresolvedItem,
};
use crate::domain::{OUTLET_POLICY_BLOCK, SOURCE_KIND_SCHEDULE, Source};
use crate::engine::{
    CURRENT_NEWS_HOURS, FetchOutput, Fetcher, RecentSuppression, SportsOptions,
    add_fetch_failure_warning, add_news_date_warning, add_recurring_failure_warnings,
    add_stale_heartbeat_warning, brief_summary, build_health_footnote, classify_and_dedupe,
    collect_items, enabled_source_keys, prepare_sports_updates, process_source_items,
    render_sports_section, sort_brief_items, suppress_recent_candidates,
};
use crate::storage::RUN_ITEM_ANNOTATION;
use crate::storage::{
    DEFAULT_MAX_DELIVERY_ITEMS, DEFAULT_SPORTS_POST_GAME_DAYS, DEFAULT_SPORTS_PRE_GAME_DAYS,
    DEFAULT_SPORTS_TIMEZONE, Delivery, FetchLog, MAX_DELIVERY_ITEMS_UPPER_BOUND,
    RUN_ITEM_CANDIDATE, RUN_ITEM_DROPPED, RUN_ITEM_MUST_INCLUDE, RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
    RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS, RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS,
    RUNTIME_CONFIG_SPORTS_TIMEZONE, RunDeliveryContext, RunItemRow, SourceState, Store,
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
        "prepare_delivery" => delivery::prepare(paths, store, request),
        "confirm_delivery" => delivery::confirm(paths, store, request),
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
    let runtime_config = store.runtime_config()?;
    let options = brief_options(&runtime_config);
    let recent = recent_sent_items(store, now)?;
    let mut accumulator = collect_sources(store, &sources, &run_id, dry_run, now, &options.sports)?;
    let mut classified = classify_and_dedupe(std::mem::take(&mut accumulator.collected));
    let recent_result = suppress_recent_candidates(classified.candidates, &recent);
    let mut run_rows = if dry_run {
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
    for row in &mut run_rows {
        if let Some(source) = sources.iter().find(|source| source.key == row.source_key) {
            row.source_label.clone_from(&source.label);
        }
    }
    let health_delta = finish_health(
        store,
        &sources,
        &runtime_config,
        &options,
        &mut accumulator,
        now,
        dry_run,
    )?;
    let health_footnote = build_health_footnote(&health_delta);
    let sports_updates = prepare_sports_updates(std::mem::take(&mut accumulator.sports_updates));
    let sports_section = render_sports_section(&sports_updates, options.sports.timezone);
    sort_brief_items(&mut classified.must_include);
    let mut candidates = recent_result.candidates;
    sort_brief_items(&mut candidates);
    accumulator
        .statuses
        .sort_by(|left, right| left.source_key.cmp(&right.source_key));
    let summary = brief_summary(
        &classified.must_include,
        &candidates,
        sports_updates.len(),
        &health_footnote,
    );
    if !dry_run {
        store.insert_run_items(&run_id, &run_rows)?;
        persist_delivery_context(store, &run_id, &options, &sports_updates, &health_footnote)?;
        store.finish_run(&run_id, "ok", &summary)?;
    }
    let candidate_slots = candidate_slots(&classified.must_include, options.max_delivery_items);
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
        sports_section,
        sports_updates,
        health_footnote,
        health_delta,
        max_delivery_items: options.max_delivery_items,
        candidate_slots,
        summary,
        ..BriefResult::default()
    })
}

fn recent_sent_items(
    store: &Store,
    now: chrono::DateTime<Utc>,
) -> Result<Vec<crate::storage::StoredSentItem>> {
    let since = now
        .checked_sub_signed(TimeDelta::hours(CURRENT_NEWS_HOURS))
        .context("calculate recent delivery window")?;
    store.recent_sent_items(since)
}

#[derive(Default)]
struct Accumulator {
    collected: Vec<crate::engine::CollectedItem>,
    sports_updates: Vec<SportsUpdate>,
    suppressed_policy: Vec<crate::contract::SuppressedPolicyItem>,
    suppressed_unresolved: Vec<crate::contract::SuppressedUnresolvedItem>,
    statuses: Vec<FetchStatus>,
    warnings: BTreeMap<String, String>,
}

fn finish_health(
    store: &Store,
    sources: &[Source],
    runtime_config: &BTreeMap<String, String>,
    options: &BriefOptions,
    accumulator: &mut Accumulator,
    now: chrono::DateTime<Utc>,
    dry_run: bool,
) -> Result<HealthDelta> {
    add_runtime_option_warnings(options, &mut accumulator.warnings);
    add_stale_heartbeat_warning(runtime_config, &mut accumulator.warnings, now);
    if !dry_run {
        let logs = store.recent_fetch_logs(500)?;
        add_recurring_failure_warnings(
            &logs,
            &enabled_source_keys(sources),
            &mut accumulator.warnings,
        );
    }
    let delta = store.health_delta(&accumulator.warnings, !dry_run)?;
    if !dry_run {
        store.set_runtime_config(
            "last_check",
            &now.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        )?;
    }
    Ok(delta)
}

fn collect_sources(
    store: &Store,
    sources: &[Source],
    run_id: &str,
    dry_run: bool,
    now: chrono::DateTime<Utc>,
    sports_options: &SportsOptions,
) -> Result<Accumulator> {
    let policies = store.list_outlet_policies()?;
    let run = SourceRun {
        store,
        policies: &policies,
        run_id,
        dry_run,
        now,
    };
    let mut accumulator = Accumulator::default();
    for (source, output) in sources
        .iter()
        .zip(fetch_sources(sources, now, sports_options))
    {
        process_source(source, output, &run, &mut accumulator)?;
    }
    Ok(accumulator)
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
    let state = if source.is_current_news() {
        None
    } else {
        run.store.source_state(&source.key)?
    };
    let output = match output {
        Ok(value) => value,
        Err(error) => {
            let message = error.to_string();
            add_fetch_failure_warning(&source.key, &message, &mut accumulator.warnings);
            accumulator.statuses.push(FetchStatus {
                source_label: source.label.clone(),
                source_key: source.key.clone(),
                status: "error".to_owned(),
                error: message.clone(),
                ..FetchStatus::default()
            });
            if !run.dry_run {
                run.store.insert_fetch_log(&FetchLog {
                    source_label: source.label.clone(),
                    run_id: run.run_id.to_owned(),
                    source_key: source.key.clone(),
                    status: "error".to_owned(),
                    error: message,
                    ..FetchLog::default()
                })?;
            }
            return Ok(());
        }
    };
    let item_count = if source.kind == SOURCE_KIND_SCHEDULE {
        output.sports_updates.len()
    } else {
        output.items.len()
    };
    let unresolved_count = output.unresolved.len();
    let processed = process_source_items(source, output, run.policies, state.as_ref(), run.now);
    let current_news = processed.current_news.clone();
    add_news_date_warning(
        &source.key,
        current_news.as_ref(),
        &mut accumulator.warnings,
    );
    let new_count = if source.kind == SOURCE_KIND_SCHEDULE {
        processed.sports_updates.len()
    } else {
        processed.eligible_items.len()
    };
    let policy_count = processed.suppressed_policy.len();
    accumulator
        .collected
        .extend(collect_items(source, &processed.eligible_items));
    accumulator.sports_updates.extend(processed.sports_updates);
    accumulator
        .suppressed_policy
        .extend(processed.suppressed_policy);
    accumulator
        .suppressed_unresolved
        .extend(processed.suppressed_unresolved);
    if !run.dry_run {
        if !source.is_current_news()
            && let Some(top) = processed.items.first()
        {
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
        run.store.insert_fetch_log(&FetchLog {
            source_label: source.label.clone(),
            run_id: run.run_id.to_owned(),
            source_key: source.key.clone(),
            status: "ok".to_owned(),
            item_count,
            new_item_count: current_news.is_none().then_some(new_count),
            current_news: current_news.clone(),
            ..FetchLog::default()
        })?;
    }
    accumulator.statuses.push(FetchStatus {
        source_label: source.label.clone(),
        source_key: source.key.clone(),
        status: "ok".to_owned(),
        items: item_count,
        new_items: current_news.is_none().then_some(new_count),
        current_news,
        suppressed_policy: policy_count,
        suppressed_unresolved: unresolved_count,
        ..FetchStatus::default()
    });
    Ok(())
}

fn fetch_sources(
    sources: &[Source],
    now: chrono::DateTime<Utc>,
    sports_options: &SportsOptions,
) -> Vec<Result<FetchOutput>> {
    let fetcher = Fetcher::new();
    run_bounded_ordered(sources, SOURCE_FETCH_CONCURRENCY, |source| {
        fetcher.fetch(source, now, sports_options)
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
    sports: SportsOptions,
    warnings: Vec<(String, String)>,
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
    rows.extend(
        suppressed_policy
            .iter()
            .map(|item| RunItemRow {
                category: if item.policy == OUTLET_POLICY_BLOCK { RUN_ITEM_DROPPED } else { RUN_ITEM_ANNOTATION }.to_owned(),
                source_key: item.source_key.clone(),
                outlet: item.outlet.clone(),
                title: item.title.clone(),
                url: item.url.clone(),
                reason: "outlet_policy".to_owned(),
                detail: serde_json::json!({ "policy": item.policy, "outlet": item.outlet,
                    "disposition": if item.policy == OUTLET_POLICY_BLOCK { ItemDisposition::Dropped } else { ItemDisposition::Retained }
                }).to_string(),
                ..RunItemRow::default()
            }),
    );
    rows.extend(suppressed_unresolved.iter().map(|item| {
        RunItemRow {
            category: if item.disposition == ItemDisposition::Dropped {
                RUN_ITEM_DROPPED
            } else {
                RUN_ITEM_ANNOTATION
            }
            .to_owned(),
            source_key: item.source_key.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            reason: "unresolved".to_owned(),
            detail: serde_json::json!({ "reason": item.reason, "disposition": item.disposition })
                .to_string(),
            ..RunItemRow::default()
        }
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
        published_at: item.published_at.clone(),
        outlet: item.outlet.clone(),
        title: item.title.clone(),
        url: item.url.clone(),
        reason: reason.to_owned(),
        detail: detail.to_owned(),
        ..RunItemRow::default()
    }
}

fn brief_options(config: &BTreeMap<String, String>) -> BriefOptions {
    let mut warnings = Vec::new();
    let max_delivery_items = config
        .get(RUNTIME_CONFIG_MAX_DELIVERY_ITEMS)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .and_then(|raw| {
            raw.parse::<i64>()
                .ok()
                .filter(|value| (1..=MAX_DELIVERY_ITEMS_UPPER_BOUND).contains(value))
                .or_else(|| {
                    warnings.push((
                        RUNTIME_CONFIG_MAX_DELIVERY_ITEMS.to_owned(),
                        format!(
                            "`{RUNTIME_CONFIG_MAX_DELIVERY_ITEMS}` config value {raw:?} is invalid; using default {DEFAULT_MAX_DELIVERY_ITEMS}"
                        ),
                    ));
                    None
                })
        })
        .unwrap_or(DEFAULT_MAX_DELIVERY_ITEMS);
    let pre_game_window = sports_window(
        config,
        RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS,
        DEFAULT_SPORTS_PRE_GAME_DAYS,
        &mut warnings,
    );
    let post_game_window = sports_window(
        config,
        RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS,
        DEFAULT_SPORTS_POST_GAME_DAYS,
        &mut warnings,
    );
    let timezone = config
        .get(RUNTIME_CONFIG_SPORTS_TIMEZONE)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .and_then(|raw| {
            raw.parse::<Tz>().ok().or_else(|| {
                warnings.push((
                    RUNTIME_CONFIG_SPORTS_TIMEZONE.to_owned(),
                    format!(
                        "`{RUNTIME_CONFIG_SPORTS_TIMEZONE}` config value {raw:?} is invalid; using default {DEFAULT_SPORTS_TIMEZONE}"
                    ),
                ));
                None
            })
        })
        .unwrap_or(DEFAULT_SPORTS_TIMEZONE);
    BriefOptions {
        max_delivery_items,
        sports: SportsOptions {
            pre_game_window,
            post_game_window,
            timezone,
        },
        warnings,
    }
}

fn add_runtime_option_warnings(options: &BriefOptions, warnings: &mut BTreeMap<String, String>) {
    for (key, warning) in &options.warnings {
        let _previous = warnings.insert(format!("runtime:{key}"), warning.clone());
    }
}

fn sports_window(
    config: &BTreeMap<String, String>,
    key: &str,
    default: i64,
    warnings: &mut Vec<(String, String)>,
) -> TimeDelta {
    let default_window = TimeDelta::days(default);
    let Some(raw) = config
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return default_window;
    };
    if let Ok(value) = raw.parse::<i64>()
        && value >= 0
        && TimeDelta::try_days(value).is_some_and(|window| {
            if key == RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS {
                Utc::now().checked_add_signed(window).is_some()
            } else {
                Utc::now().checked_sub_signed(window).is_some()
            }
        })
    {
        return TimeDelta::days(value);
    }
    warnings.push((
        key.to_owned(),
        format!("`{key}` config value {raw:?} is invalid; using default {default}"),
    ));
    default_window
}

fn persist_delivery_context(
    store: &Store,
    run_id: &str,
    options: &BriefOptions,
    sports_updates: &[SportsUpdate],
    health_footnote: &str,
) -> Result<()> {
    store.insert_run_delivery_context(
        run_id,
        &RunDeliveryContext {
            max_delivery_items: options.max_delivery_items,
            sports_updates: sports_updates.to_vec(),
            sports_timezone: options.sports.timezone.to_string(),
            health_footnote: health_footnote.to_owned(),
        },
    )
}

fn candidate_slots(must_include: &[BriefItem], max_items: i64) -> usize {
    usize::try_from(max_items)
        .unwrap_or_default()
        .saturating_sub(must_include.len())
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
