mod canonical;
mod dedup;
mod feed;
mod fetcher;
mod google;
mod health;
mod http;
mod model;
mod policy;
mod process;
mod recent;
mod selection;

pub use dedup::{CollectedItem, classify_and_dedupe, sort_brief_items};
pub use fetcher::Fetcher;
pub use health::{
    add_fetch_failure_warning, add_recurring_failure_warnings, add_stale_heartbeat_warning,
    brief_summary, build_health_footnote, enabled_source_keys,
};
pub use model::FetchOutput;
pub use process::{collect_new_items, process_source_items};
pub use recent::{RecentSuppression, suppress_recent_candidates};

#[cfg(test)]
mod tests;
