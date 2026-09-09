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
mod schedule;
mod selection;

pub use dedup::{CollectedItem, classify_and_dedupe, sort_brief_items};
pub use fetcher::Fetcher;
pub use health::{
    add_fetch_failure_warning, add_news_date_warning, add_recurring_failure_warnings,
    add_stale_heartbeat_warning, brief_summary, build_health_footnote, enabled_source_keys,
};
pub use model::FetchOutput;
pub use process::{collect_items, process_source_items};
pub use recent::{RecentSuppression, suppress_recent_candidates};
pub use schedule::{
    SportsOptions, prepare_sports_updates, render_sports_section, sports_update_item,
};
pub use selection::CURRENT_NEWS_HOURS;

#[cfg(test)]
mod tests;
