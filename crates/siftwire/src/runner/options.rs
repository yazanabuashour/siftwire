use jiff::{SignedDuration, Timestamp};
use std::collections::BTreeMap;
use std::fmt::Display;

use anyhow::{Result, ensure};

use crate::contract::ConfigRequest;
use crate::engine::SportsOptions;
use crate::storage::{
    DEFAULT_MAX_DELIVERY_ITEMS, DEFAULT_SPORTS_POST_GAME_DAYS, DEFAULT_SPORTS_PRE_GAME_DAYS,
    MAX_DELIVERY_ITEMS_UPPER_BOUND, RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
    RUNTIME_CONFIG_READER_PRIORITIES, RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS,
    RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS, RUNTIME_CONFIG_SPORTS_TIMEZONE,
};

pub(super) struct BriefOptions {
    pub max_delivery_items: i64,
    pub sports: SportsOptions,
    pub reader_priorities: String,
    pub warnings: BTreeMap<String, String>,
}

pub(super) fn updates(
    request: &ConfigRequest,
    now: Timestamp,
) -> Result<Vec<(&'static str, String)>> {
    let mut values = Vec::new();
    if let Some(value) = request.max_delivery_items {
        values.push((
            RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
            delivery_limit(value)?.to_string(),
        ));
    }
    for (key, value) in [
        (
            RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS,
            request.sports_pre_game_days,
        ),
        (
            RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS,
            request.sports_post_game_days,
        ),
    ] {
        if let Some(days) = value {
            values.push((key, sports_days(key, days, now)?.to_string()));
        }
    }
    if let Some(raw) = &request.sports_timezone {
        let value = raw.trim();
        ensure!(
            crate::timezone::named(value).is_ok(),
            "{RUNTIME_CONFIG_SPORTS_TIMEZONE} must be an IANA time zone"
        );
        values.push((RUNTIME_CONFIG_SPORTS_TIMEZONE, value.to_owned()));
    }
    if let Some(value) = &request.reader_priorities {
        values.push((RUNTIME_CONFIG_READER_PRIORITIES, value.trim().to_owned()));
    }
    ensure!(
        !values.is_empty(),
        "set_brief_options requires at least one option"
    );
    Ok(values)
}

impl BriefOptions {
    pub(super) fn from_config(config: &BTreeMap<String, String>, now: Timestamp) -> Self {
        let mut warnings = BTreeMap::new();
        let max_delivery_items = stored(
            config,
            RUNTIME_CONFIG_MAX_DELIVERY_ITEMS,
            DEFAULT_MAX_DELIVERY_ITEMS,
            &mut warnings,
            |raw| delivery_limit(raw.parse().ok()?).ok(),
        );
        let timezone =
            config
                .get(RUNTIME_CONFIG_SPORTS_TIMEZONE)
                .map(|raw| raw.trim())
                .filter(|raw| !raw.is_empty())
                .and_then(|raw| {
                    crate::timezone::named(raw)
                        .map_err(|_error| {
                            let key = RUNTIME_CONFIG_SPORTS_TIMEZONE;
                            let default = crate::timezone::CENTRAL;
                            warnings.insert(format!("runtime:{key}"),
                    format!("`{key}` config value {raw:?} is invalid; using default {default}"));
                        })
                        .ok()
                })
                .unwrap_or_else(crate::timezone::central);
        let mut window = |key, default| {
            let days = stored(config, key, default, &mut warnings, |raw| {
                sports_days(key, raw.parse().ok()?, now).ok()
            });
            SignedDuration::from_secs(days.saturating_mul(86_400))
        };
        Self {
            max_delivery_items,
            sports: SportsOptions {
                pre_game_window: window(
                    RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS,
                    DEFAULT_SPORTS_PRE_GAME_DAYS,
                ),
                post_game_window: window(
                    RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS,
                    DEFAULT_SPORTS_POST_GAME_DAYS,
                ),
                timezone,
            },
            reader_priorities: config
                .get(RUNTIME_CONFIG_READER_PRIORITIES)
                .map(|value| value.trim().to_owned())
                .unwrap_or_default(),
            warnings,
        }
    }
}

fn stored<T: Display>(
    config: &BTreeMap<String, String>,
    key: &str,
    default: T,
    warnings: &mut BTreeMap<String, String>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> T {
    let Some(raw) = config
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return default;
    };
    parse(raw).unwrap_or_else(|| {
        warnings.insert(
            format!("runtime:{key}"),
            format!("`{key}` config value {raw:?} is invalid; using default {default}"),
        );
        default
    })
}

fn delivery_limit(value: i64) -> Result<i64> {
    ensure!(
        (1..=MAX_DELIVERY_ITEMS_UPPER_BOUND).contains(&value),
        "{RUNTIME_CONFIG_MAX_DELIVERY_ITEMS} must be between 1 and {MAX_DELIVERY_ITEMS_UPPER_BOUND}"
    );
    Ok(value)
}

fn sports_days(key: &str, value: i64, now: Timestamp) -> Result<i64> {
    let fits = value
        .checked_mul(86_400)
        .filter(|seconds| *seconds >= 0)
        .map(SignedDuration::from_secs)
        .is_some_and(|window| match key {
            RUNTIME_CONFIG_SPORTS_PRE_GAME_DAYS => now.checked_add(window).is_ok(),
            RUNTIME_CONFIG_SPORTS_POST_GAME_DAYS => now.checked_sub(window).is_ok(),
            _ => false,
        });
    ensure!(
        fits,
        "{key} must be a non-negative whole number of days that fits the date range"
    );
    Ok(value)
}
