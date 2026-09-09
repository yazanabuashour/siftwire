use std::env;
use std::time::Duration;

use anyhow::{Context, Result, ensure};

#[derive(Clone, Debug)]
pub struct Settings {
    pub bind: String,
    pub allowed_hosts: Vec<String>,
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
            allowed_hosts: parse_allowed_hosts(
                env_string("SIFTWIRE_CONSOLE_ALLOWED_HOSTS").as_deref(),
            )?,
            web_root,
            runner_bin,
            database,
            run_timeout: Duration::from_secs(run_timeout_secs),
        })
    }
}

fn parse_allowed_hosts(raw: Option<&str>) -> Result<Vec<String>> {
    let raw = raw.unwrap_or_default().trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(|host| {
            let host = host.trim();
            ensure!(
                host.split('.').all(|label| {
                    !label.is_empty()
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                }),
                "SIFTWIRE_CONSOLE_ALLOWED_HOSTS requires comma-separated DNS hostnames without schemes, ports, paths, or wildcards"
            );
            Ok(host.to_ascii_lowercase())
        })
        .collect()
}

#[cfg(test)]
#[test]
#[expect(
    clippy::expect_used,
    reason = "configuration tests assert parsing failures explicitly"
)]
fn allowed_hosts_are_explicit_dns_names() {
    assert!(parse_allowed_hosts(None).expect("unset hosts").is_empty());
    assert!(
        parse_allowed_hosts(Some("  "))
            .expect("blank hosts")
            .is_empty()
    );
    assert_eq!(
        parse_allowed_hosts(Some(" Console.Example.com , other.example.com "))
            .expect("explicit hosts"),
        ["console.example.com", "other.example.com"]
    );
    for invalid in [
        "*",
        "*.example.com",
        "https://console.example.com",
        "console.example.com:8443",
        "console.example.com/path",
        "user@console.example.com",
        "console.example.com,",
        "console..example.com",
        "-console.example.com",
        "console-.example.com",
        "console.example.com.",
    ] {
        assert!(parse_allowed_hosts(Some(invalid)).is_err(), "{invalid}");
    }
}

fn env_string(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
