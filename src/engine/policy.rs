use crate::contract::SuppressedPolicyItem;
use crate::domain::{OUTLET_POLICY_BLOCK, OutletPolicy, Source};

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
        let Some(policy) = matching_policy(&item, policies) else {
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

fn matching_policy<'a>(
    item: &FetchedItem,
    policies: &'a [OutletPolicy],
) -> Option<&'a OutletPolicy> {
    let outlet = item.outlet.trim();
    if outlet.is_empty() {
        return None;
    }
    let normalized = normalize_outlet_name(outlet);
    policies.iter().find(|policy| {
        policy.enabled
            && (normalize_outlet_name(&policy.name) == normalized
                || policy
                    .aliases
                    .iter()
                    .any(|alias| normalize_outlet_name(alias) == normalized))
    })
}

fn normalize_outlet_name(value: &str) -> String {
    let mut without_suffixes = String::new();
    let lowercase = value.trim().to_lowercase();
    let mut characters = lowercase.chars();
    while let Some(character) = characters.next() {
        if character != '.' {
            without_suffixes.push(character);
            continue;
        }
        let extension = characters
            .clone()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>();
        let boundary = characters
            .clone()
            .nth(extension.len())
            .is_none_or(|next| !next.is_ascii_alphanumeric());
        let removed_extension = matches!(
            extension.as_str(),
            "com" | "co" | "net" | "org" | "io" | "ai" | "news" | "uk"
        );
        if boundary && removed_extension {
            for _character in extension.chars() {
                let _consumed = characters.next();
            }
            without_suffixes.push(' ');
        } else {
            without_suffixes.push(character);
        }
    }
    without_suffixes
        .chars()
        .map(|character| {
            if character.is_ascii_lowercase() || character.is_ascii_digit() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
