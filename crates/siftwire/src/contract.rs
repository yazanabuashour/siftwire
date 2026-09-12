use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{OutletPolicy, Source};

pub const RUNNER_PROTOCOL: &str = "siftwire-runner/v5";
pub const CAPABILITY_PREPARED_DELIVERY: &str = "prepared-delivery/v1";
pub const CAPABILITY_SPORTS_UPDATES: &str = "sports-updates/v1";
pub const CAPABILITY_CURRENT_NEWS: &str = "current-news/v1";

fn capabilities() -> Vec<String> {
    vec![
        CAPABILITY_PREPARED_DELIVERY.to_owned(),
        CAPABILITY_SPORTS_UPDATES.to_owned(),
        CAPABILITY_CURRENT_NEWS.to_owned(),
    ]
}

#[derive(Debug, Serialize)]
pub struct RunnerMetadata {
    pub runner_protocol: String,
    pub capabilities: Vec<String>,
}

impl Default for RunnerMetadata {
    fn default() -> Self {
        Self {
            runner_protocol: RUNNER_PROTOCOL.to_owned(),
            capabilities: capabilities(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Paths {
    pub data_dir: String,
    pub database_path: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigRequest {
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub action: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub source: Source,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub sources: Vec<Source>,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub key: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub outlets: Vec<OutletPolicy>,
    pub max_delivery_items: Option<i64>,
    pub sports_pre_game_days: Option<i64>,
    pub sports_post_game_days: Option<i64>,
    pub sports_timezone: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ConfigResult {
    #[serde(flatten)]
    pub runner: RunnerMetadata,
    pub rejected: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rejection_reason: String,
    pub paths: Paths,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub runtime_config: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outlets: Vec<OutletPolicy>,
    pub source_reporting: BTreeMap<String, crate::domain::Reporting>,
    pub summary: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BriefRequest {
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub action: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub dry_run: bool,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub run_id: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub candidate_indexes: Vec<usize>,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub delivery_plan_id: String,
}

#[derive(Debug, Default, Serialize)]
pub struct BriefResult {
    #[serde(flatten)]
    pub runner: RunnerMetadata,
    pub rejected: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rejection_reason: String,
    pub paths: Paths,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub run_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub must_include: Vec<BriefItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<BriefItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub previous_briefs: Vec<PreviousBrief>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub delivery_message_scope: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_sent: Vec<SentItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressed: Vec<SuppressedItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressed_recent: Vec<SuppressedRecentItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressed_policy: Vec<SuppressedPolicyItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suppressed_unresolved: Vec<SuppressedUnresolvedItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fetch_status: Vec<FetchStatus>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sports_section: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sports_updates: Vec<SportsUpdate>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub health_footnote: String,
    pub health_delta: HealthDelta,
    #[serde(skip_serializing_if = "is_default")]
    pub max_delivery_items: i64,
    pub candidate_slots: usize,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub delivery_plan_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub message: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub html: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prepared_items: Vec<DeliveryItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sent_items: Vec<SentItem>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deliveries: Vec<DeliveryRecord>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub final_answer: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct BriefItem {
    pub source_key: String,
    pub source_label: String,
    pub kind: String,
    pub section: String,
    pub threshold: String,
    #[serde(skip_serializing_if = "is_default")]
    pub priority_rank: i64,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub published_at: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub outlet: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SportsUpdate {
    #[serde(skip)]
    pub fixture_identity: String,
    pub source_key: String,
    pub source_label: String,
    pub status: String,
    pub title: String,
    pub competition: String,
    pub url: String,
    pub starts_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<SportsImage>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SportsImage {
    pub url: String,
    pub alt: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PreviousBrief {
    pub run_id: String,
    pub delivered_at: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DeliveryRecord {
    pub run_id: String,
    pub delivered_at: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeliveryItem {
    /// Stable `SQLite` run-item identifiers, never JSON numbers.
    /// Sports updates carry an empty list.
    pub run_item_ids: Vec<String>,
    pub title: String,
    pub url: String,
    pub kind: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SentItem {
    pub title: String,
    pub url: String,
    pub sent_at: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuppressedItem {
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuppressedRecentItem {
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub matched_prior_title: String,
    pub prior_sent_at: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuppressedPolicyItem {
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub outlet: String,
    pub policy: String,
}

/// The producing step's decision, not a claim about later selection or delivery.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemDisposition {
    #[default]
    Retained,
    Dropped,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuppressedUnresolvedItem {
    pub disposition: ItemDisposition,
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CurrentNewsStatus {
    pub since: String,
    pub until: String,
    pub eligible_items: usize,
    pub stale_items: usize,
    pub undated_items: usize,
    pub future_items: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FetchStatus {
    pub source_key: String,
    pub source_label: String,
    pub status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    pub items: usize,
    pub new_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_news: Option<CurrentNewsStatus>,
    #[serde(skip_serializing_if = "is_default")]
    pub suppressed_policy: usize,
    #[serde(skip_serializing_if = "is_default")]
    pub suppressed_unresolved: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct HealthDelta {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_warnings: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resolved_warnings: Vec<String>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

#[cfg(test)]
mod tests {
    use super::BriefResult;

    #[test]
    fn zero_candidate_slots_remain_explicit() {
        let result = serde_json::to_value(BriefResult::default());
        assert_eq!(
            result
                .as_ref()
                .ok()
                .and_then(|value| value.get("candidate_slots"))
                .and_then(serde_json::Value::as_u64),
            Some(0),
            "zero candidate capacity disappeared from the wire contract"
        );
    }
}
