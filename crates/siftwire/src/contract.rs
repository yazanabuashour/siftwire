use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{OutletPolicy, Source};

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
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub max_delivery_items: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct ConfigResult {
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
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct BriefResult {
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
    pub health_footnote: String,
    pub health_delta: HealthDelta,
    #[serde(skip_serializing_if = "is_default")]
    pub max_delivery_items: i64,
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
    #[serde(skip_serializing_if = "is_default")]
    pub always_report: bool,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub published_at: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub outlet: String,
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

#[derive(Clone, Debug, Default, Serialize)]
pub struct SuppressedUnresolvedItem {
    pub source_key: String,
    pub title: String,
    pub url: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FetchStatus {
    pub source_key: String,
    pub status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    pub items: usize,
    pub new_items: usize,
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
