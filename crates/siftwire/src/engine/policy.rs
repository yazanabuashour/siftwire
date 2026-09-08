use crate::contract::SuppressedPolicyItem;
use crate::domain::{OUTLET_POLICY_BLOCK, OutletPolicy, Source, matching_outlet_policy};

use super::model::FetchedItem;

#[derive(Clone, Debug, Default)]
pub struct OutletPolicyResult {
    pub items: Vec<FetchedItem>,
    pub audit: Vec<SuppressedPolicyItem>,
}

#[must_use]
pub fn apply_outlet_policies(
    source: &Source,
    items: Vec<FetchedItem>,
    policies: &[OutletPolicy],
) -> OutletPolicyResult {
    if items.is_empty() || policies.is_empty() {
        return OutletPolicyResult {
            items,
            audit: Vec::new(),
        };
    }
    let mut kept = Vec::new();
    let mut audit = Vec::new();
    for item in items {
        let Some(policy) = matching_outlet_policy(&item.outlet, policies) else {
            kept.push(item);
            continue;
        };
        audit.push(SuppressedPolicyItem {
            source_key: source.key.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            outlet: item.outlet.trim().to_owned(),
            policy: policy.policy.clone(),
        });
        if policy.policy != OUTLET_POLICY_BLOCK {
            kept.push(item);
        }
    }
    OutletPolicyResult { items: kept, audit }
}
