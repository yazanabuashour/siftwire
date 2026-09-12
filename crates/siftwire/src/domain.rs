use std::collections::BTreeSet;

mod outlets;
mod reporting;
pub use outlets::{matching_outlet_policy, outlet_conflicts};
pub use reporting::Reporting;
use std::env;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use url::Url;

pub const SOURCE_KIND_RSS: &str = "rss";
pub const SOURCE_KIND_GITHUB_RELEASE: &str = "github_release";
pub const SOURCE_KIND_SCHEDULE: &str = "sports_schedule";
pub const SCHEDULE_FORMAT_ESPN: &str = "espn";
pub const SCHEDULE_FORMAT_ESPN_SCOREBOARD: &str = "espn_scoreboard";
pub const SCHEDULE_FORMAT_RIOT: &str = "riot";
pub const SCHEDULE_FILTER_ALL: &str = "all";
pub const SCHEDULE_FILTER_STANDINGS_TOP_TWO: &str = "standings_top_two";
pub const THRESHOLD_ALWAYS: &str = "always";
pub const THRESHOLD_MEDIUM: &str = "medium";
pub const THRESHOLD_HIGH: &str = "high";
pub const URL_CANONICALIZATION_NONE: &str = "none";
pub const URL_CANONICALIZATION_GOOGLE_NEWS_ARTICLE: &str = "google_news_article_url";
pub const OUTLET_EXTRACTION_NONE: &str = "none";
pub const OUTLET_EXTRACTION_TITLE_SUFFIX: &str = "title_suffix";
pub const OUTLET_POLICY_ALLOW: &str = "allow";
pub const OUTLET_POLICY_BLOCK: &str = "block";
pub const OUTLET_POLICY_WATCH: &str = "watch";
pub const EVAL_ALLOW_FILE_URLS_ENV: &str = "SIFTWIRE_EVAL_ALLOW_FILE_URLS";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Source {
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub key: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub label: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub kind: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub url: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub repo: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub section: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub threshold: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub enabled: bool,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub url_canonicalization: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub outlet_extraction: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub dedup_group: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "is_default"
    )]
    pub priority_rank: i64,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub schedule_format: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub schedule_filter: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub api_key: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutletPolicy {
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub name: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub aliases: Vec<String>,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub policy: String,
    #[serde(
        deserialize_with = "crate::serde_util::null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub note: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    pub enabled: bool,
}

/// Normalizes and validates one source.
/// # Errors
/// Returns an error when a required field or supported value is invalid.
pub fn normalize_source(mut source: Source) -> Result<Source> {
    source.key = source.key.trim().to_lowercase();
    source.label = source.label.trim().to_owned();
    source.kind = source.kind.trim().to_lowercase();
    source.url = source.url.trim().to_owned();
    source.repo = source.repo.trim().to_owned();
    source.section = source.section.trim().to_lowercase();
    source.threshold = source.threshold.trim().to_lowercase();
    source.url_canonicalization = source.url_canonicalization.trim().to_lowercase();
    source.outlet_extraction = source.outlet_extraction.trim().to_lowercase();
    source.dedup_group = source.dedup_group.trim().to_lowercase();
    source.schedule_format = source.schedule_format.trim().to_lowercase();
    source.schedule_filter = source.schedule_filter.trim().to_lowercase();
    // API keys are case-sensitive; trim only.
    source.api_key = source.api_key.trim().to_owned();
    validate_source_fields(&mut source)?;
    Ok(source)
}

/// Normalizes and validates a source set.
/// # Errors
/// Returns an error when a source is invalid or normalized keys repeat.
pub fn normalize_sources(sources: Vec<Source>) -> Result<Vec<Source>> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(sources.len());
    for source in sources {
        let source = normalize_source(source)?;
        if !seen.insert(source.key.clone()) {
            bail!("duplicate source key {:?}", source.key);
        }
        normalized.push(source);
    }
    Ok(normalized)
}

/// Normalizes and validates outlet policies.
/// # Errors
/// Returns an error when a policy is invalid or normalized names repeat.
pub fn normalize_outlet_policies(policies: Vec<OutletPolicy>) -> Result<Vec<OutletPolicy>> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(policies.len());
    for mut policy in policies {
        policy.name = trim_owned(policy.name);
        policy.policy = policy.policy.trim().to_lowercase();
        policy.note = trim_owned(policy.note);
        if policy.name.is_empty() {
            bail!("outlet policy name is required");
        }
        if policy.policy.is_empty() {
            OUTLET_POLICY_ALLOW.clone_into(&mut policy.policy);
        }
        if !matches!(
            policy.policy.as_str(),
            OUTLET_POLICY_ALLOW | OUTLET_POLICY_BLOCK | OUTLET_POLICY_WATCH
        ) {
            bail!(
                "outlet policy {:?} policy must be allow, block, or watch",
                policy.name
            );
        }
        policy.aliases = compact_strings(policy.aliases);
        if !seen.insert(policy.name.to_lowercase()) {
            bail!("duplicate outlet policy {:?}", policy.name);
        }
        normalized.push(policy);
    }
    if let Some(conflict) = outlet_conflicts(&normalized).first() {
        bail!(
            "colliding outlet matcher {:?}: {}",
            conflict.matcher,
            conflict.names.join(", ")
        );
    }
    Ok(normalized)
}

fn validate_source_fields(source: &mut Source) -> Result<()> {
    if !valid_source_key(&source.key) {
        bail!("source key must be lowercase letters, numbers, dot, underscore, or hyphen");
    }
    if source.label.is_empty() {
        bail!("source {:?} label is required", source.key);
    }
    if source.section.is_empty() {
        bail!("source {:?} section is required", source.key);
    }
    set_source_defaults(source);
    validate_source_options(source)?;
    match source.kind.as_str() {
        SOURCE_KIND_RSS => validate_fetch_url(&source.url)
            .map_err(|error| anyhow::anyhow!("source {:?} url: {error}", source.key)),
        SOURCE_KIND_GITHUB_RELEASE => validate_github_source(source),
        SOURCE_KIND_SCHEDULE => validate_schedule_source(source),
        _ => bail!(
            "source {:?} kind must be rss, github_release, or sports_schedule",
            source.key
        ),
    }
}

fn set_source_defaults(source: &mut Source) {
    if source.threshold.is_empty() {
        THRESHOLD_MEDIUM.clone_into(&mut source.threshold);
    }
    if source.url_canonicalization.is_empty() {
        URL_CANONICALIZATION_NONE.clone_into(&mut source.url_canonicalization);
    }
    if source.outlet_extraction.is_empty() {
        OUTLET_EXTRACTION_NONE.clone_into(&mut source.outlet_extraction);
    }
    if source.kind == SOURCE_KIND_SCHEDULE && source.schedule_format.is_empty() {
        SCHEDULE_FORMAT_ESPN.clone_into(&mut source.schedule_format);
    }
    if source.schedule_filter.is_empty() {
        SCHEDULE_FILTER_ALL.clone_into(&mut source.schedule_filter);
    }
}

fn validate_source_options(source: &Source) -> Result<()> {
    if !matches!(
        source.threshold.as_str(),
        THRESHOLD_ALWAYS | THRESHOLD_MEDIUM | THRESHOLD_HIGH
    ) {
        bail!(
            "source {:?} threshold must be always, medium, or high",
            source.key
        );
    }
    if !matches!(
        source.url_canonicalization.as_str(),
        URL_CANONICALIZATION_NONE | URL_CANONICALIZATION_GOOGLE_NEWS_ARTICLE
    ) {
        bail!(
            "source {:?} url_canonicalization must be none or google_news_article_url",
            source.key
        );
    }
    if !matches!(
        source.outlet_extraction.as_str(),
        OUTLET_EXTRACTION_NONE | OUTLET_EXTRACTION_TITLE_SUFFIX
    ) {
        bail!(
            "source {:?} outlet_extraction must be none or title_suffix",
            source.key
        );
    }
    if !matches!(
        source.schedule_filter.as_str(),
        SCHEDULE_FILTER_ALL | SCHEDULE_FILTER_STANDINGS_TOP_TWO
    ) {
        bail!(
            "source {:?} schedule_filter must be all or standings_top_two",
            source.key
        );
    }
    if source.kind != SOURCE_KIND_SCHEDULE
        && (!source.schedule_format.is_empty()
            || source.schedule_filter != SCHEDULE_FILTER_ALL
            || !source.api_key.is_empty())
    {
        bail!(
            "source {:?} schedule options apply only to sports_schedule sources",
            source.key
        );
    }
    Ok(())
}

fn validate_schedule_source(source: &Source) -> Result<()> {
    validate_fetch_url(&source.url)
        .map_err(|error| anyhow::anyhow!("source {:?} url: {error}", source.key))?;
    if !matches!(
        source.schedule_format.as_str(),
        SCHEDULE_FORMAT_ESPN | SCHEDULE_FORMAT_ESPN_SCOREBOARD | SCHEDULE_FORMAT_RIOT
    ) {
        bail!(
            "source {:?} schedule_format must be espn, espn_scoreboard, or riot",
            source.key
        );
    }
    if source.schedule_filter == SCHEDULE_FILTER_STANDINGS_TOP_TWO {
        if source.schedule_format != SCHEDULE_FORMAT_RIOT {
            bail!(
                "source {:?} standings_top_two schedule_filter applies only to the riot schedule format",
                source.key
            );
        }
        validate_riot_league_id(source)?;
    }
    if source.schedule_format == SCHEDULE_FORMAT_RIOT && source.api_key.is_empty() {
        bail!(
            "source {:?} api_key is required for the riot schedule format",
            source.key
        );
    }
    if source.schedule_format != SCHEDULE_FORMAT_RIOT && !source.api_key.is_empty() {
        bail!(
            "source {:?} api_key applies only to the riot schedule format",
            source.key
        );
    }
    Ok(())
}

fn validate_riot_league_id(source: &Source) -> Result<()> {
    let url = Url::parse(&source.url)?;
    if !url.path().contains("/persisted/gw/") {
        bail!(
            "source {:?} standings_top_two URL must use Riot's persisted Gateway",
            source.key
        );
    }
    let league_ids = url
        .query_pairs()
        .filter(|(name, _value)| name == "leagueId")
        .map(|(_name, value)| value)
        .collect::<Vec<_>>();
    let [league_id] = league_ids.as_slice() else {
        bail!(
            "source {:?} standings_top_two URL must contain exactly one leagueId",
            source.key
        );
    };
    if league_id.trim().is_empty() || league_id.contains(',') {
        bail!(
            "source {:?} standings_top_two URL must contain exactly one leagueId",
            source.key
        );
    }
    Ok(())
}

fn validate_github_source(source: &Source) -> Result<()> {
    if !source.url.is_empty() {
        bail!(
            "source {:?} url override is not supported for github_release; use repo owner/name",
            source.key
        );
    }
    if !valid_github_repo(&source.repo) {
        bail!("source {:?} repo must be owner/name", source.key);
    }
    Ok(())
}

fn validate_fetch_url(value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("is required");
    }
    let parsed = Url::parse(value)?;
    if parsed.scheme() == "file"
        && env::var(EVAL_ALLOW_FILE_URLS_ENV).is_ok_and(|value| value == "1")
    {
        if parsed.path().is_empty() {
            bail!("file URL must include a path");
        }
        return Ok(());
    }
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("must be http or https");
    }
    if parsed.host().is_none() {
        bail!("must include a host");
    }
    let has_userinfo = value
        .split_once("://")
        .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
        .is_some_and(|authority| authority.contains('@'));
    if has_userinfo || !parsed.username().is_empty() || parsed.password().is_some() {
        bail!("must not include credentials");
    }
    Ok(())
}

fn valid_source_key(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
}

fn valid_github_repo(value: &str) -> bool {
    let mut parts = value.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    parts.next().is_none() && valid_repo_part(owner, false) && valid_repo_part(name, true)
}

fn valid_repo_part(value: &str, dot_allowed: bool) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| {
            byte.is_ascii_alphanumeric()
                || byte == b'-'
                || (dot_allowed && matches!(byte, b'.' | b'_'))
        })
}

fn trim_owned(mut value: String) -> String {
    let start = value.len().saturating_sub(value.trim_start().len());
    let mut value = value.split_off(start);
    value.truncate(value.trim_end().len());
    value
}

fn compact_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let value = value.trim().to_owned();
            (!value.is_empty() && seen.insert(value.to_lowercase())).then_some(value)
        })
        .collect()
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

#[cfg(test)]
mod tests;
