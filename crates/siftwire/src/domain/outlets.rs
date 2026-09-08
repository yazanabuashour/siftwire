use std::collections::{BTreeMap, BTreeSet};

use super::OutletPolicy;

#[derive(Clone, Debug, serde::Serialize)]
pub struct OutletConflict {
    pub matcher: String,
    pub names: Vec<String>,
}

pub fn outlet_conflicts(policies: &[OutletPolicy]) -> Vec<OutletConflict> {
    let mut matchers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for policy in policies.iter().filter(|policy| policy.enabled) {
        for name in std::iter::once(&policy.name).chain(&policy.aliases) {
            matchers
                .entry(normalize_outlet_name(name))
                .or_default()
                .insert(policy.name.clone());
        }
    }
    matchers
        .into_iter()
        .filter(|(_, names)| names.len() > 1)
        .map(|(matcher, names)| OutletConflict {
            matcher,
            names: names.into_iter().collect(),
        })
        .collect()
}

pub fn matching_outlet_policy<'a>(
    outlet: &str,
    policies: &'a [OutletPolicy],
) -> Option<&'a OutletPolicy> {
    if outlet.trim().is_empty() {
        return None;
    }
    let normalized = normalize_outlet_name(outlet);
    policies.iter().find(|policy| {
        policy.enabled
            && std::iter::once(&policy.name)
                .chain(&policy.aliases)
                .any(|name| normalize_outlet_name(name) == normalized)
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
