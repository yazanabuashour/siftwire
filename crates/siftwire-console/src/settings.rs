use std::env;
use std::time::Duration;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Settings {
    pub bind: String,
    pub web_root: String,
    pub runner_bin: String,
    pub database: Option<String>,
    pub run_timeout: Duration,
}

impl Settings {
    /// Loads console settings from the environment with loopback defaults.
    ///
    /// # Errors
    ///
    /// Returns an error when an override cannot be parsed.
    pub fn from_env() -> Result<Self> {
        let bind =
            env_string("SIFTWIRE_CONSOLE_BIND").unwrap_or_else(|| "127.0.0.1:8790".to_owned());
        let web_root =
            env_string("SIFTWIRE_CONSOLE_WEB_ROOT").unwrap_or_else(|| "apps/web/dist".to_owned());
        let runner_bin =
            env_string("SIFTWIRE_CONSOLE_RUNNER_BIN").unwrap_or_else(|| "siftwire".to_owned());
        let database = env_string("SIFTWIRE_CONSOLE_DATABASE");
        // Receipt: a release `siftwire` invocation over `runs list` and
        // `config inspect_config` completed in about 2 ms on the operator
        // machine; 30 seconds is a tripwire far beyond healthy behavior and
        // only bounds a hung runner process.
        let run_timeout_secs = match env_string("SIFTWIRE_CONSOLE_RUN_TIMEOUT_SECS") {
            Some(raw) => raw
                .parse::<u64>()
                .context("SIFTWIRE_CONSOLE_RUN_TIMEOUT_SECS must be a number of seconds")?,
            None => 30,
        };
        Ok(Self {
            bind,
            web_root,
            runner_bin,
            database,
            run_timeout: Duration::from_secs(run_timeout_secs),
        })
    }
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
