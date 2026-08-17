use rusqlite::Connection;

use crate::types::{Metrics, Verification};
use crate::verify_checks::Checker;

pub fn scenario(
    database_path: &str,
    scenario_id: &str,
    final_message: &str,
    metrics: &Metrics,
) -> Verification {
    let database = match Connection::open(database_path) {
        Ok(database) => database,
        Err(error) => return open_failure(final_message, metrics, &error),
    };
    let mut checker = Checker::new(&database);
    check_assistant(&mut checker, final_message, metrics);
    if !verify_first_group(&mut checker, scenario_id, final_message)
        && !verify_second_group(&mut checker, scenario_id, final_message)
    {
        verify_third_group(&mut checker, scenario_id, final_message);
    }
    checker.finish()
}

fn check_assistant(checker: &mut Checker<'_>, message: &str, metrics: &Metrics) {
    if message.trim().is_empty() {
        checker.fail_assistant("assistant final message was empty or not exposed in JSONL");
    }
    if metrics.has_hygiene_failure() {
        checker.fail_assistant("agent used forbidden inspection path");
    }
}

fn verify_first_group(checker: &mut Checker<'_>, id: &str, message: &str) -> bool {
    match id {
        "empty-config-rejects-run-brief" => {
            checker.count("brief_source", 0);
            checker.contains_any(message, &["no enabled sources", "rejected"]);
        }
        "rss-source-first-run-candidate" => {
            checker.minimum_count("brief_source", 1);
            checker.minimum_count("source_state", 1);
            checker.count("delivery", 1);
            checker.count("sent_item", 1);
        }
        "rss-source-generic-processing-fields" => {
            checker.minimum_count("source_state", 1);
            checker.count("delivery", 1);
            checker.query_count("processing-field source", "SELECT COUNT(*) FROM brief_source WHERE key = 'github-blog' AND url_canonicalization = 'none' AND outlet_extraction = 'url_host' AND dedup_group = 'news' AND priority_rank = 10", 1);
        }
        "outlet-policy-watch-audit" => {
            checker.query_count("successful source fetch", "SELECT COUNT(*) FROM fetch_log WHERE source_key = 'github-blog' AND status = 'ok' AND item_count = 1", 1);
            checker.query_count("watch outlet policy", "SELECT COUNT(*) FROM outlet_policy WHERE name = 'fixture.example' AND policy = 'watch' AND enabled = 1", 1);
        }
        "configured-max-delivery-items" => {
            checker.minimum_count("brief_source", 3);
            checker.minimum_count("source_state", 3);
            checker.count("delivery", 1);
            checker.count("sent_item", 2);
            checker.runtime_value("max_delivery_items", "2");
            checker.bullet_count(message, 2);
            checker.recorded_delivery(message);
        }
        _ => return false,
    }
    true
}

fn verify_second_group(checker: &mut Checker<'_>, id: &str, message: &str) -> bool {
    match id {
        "brief-run-history" => {
            checker.minimum_count("brief_source", 1);
            checker.minimum_count("source_state", 1);
            checker.count("delivery", 3);
            checker.count("sent_item", 3);
            checker.bullet_count(message, 3);
            checker.contains_any(message, &["Current brief"]);
            checker.contains_any(message, &["Previous brief"]);
            checker.contains_all(
                message,
                &[
                    "- [Siftwire history story 1](<https://fixture.example/history-1>)",
                    "- [Siftwire history story 2](<https://fixture.example/history-2>)",
                    "- [Siftwire history story 3](<https://fixture.example/history-3>)",
                ],
            );
            checker.latest_delivery_is(
                "- [Siftwire history story 3](<https://fixture.example/history-3>)",
            );
        }
        "github-release-source-must-include" => {
            checker.minimum_count("brief_source", 1);
            checker.minimum_count("source_state", 1);
            checker.count("delivery", 1);
            checker.contains_all(message, &["fixture release", "v1.2.3", "codex"]);
        }
        "repeat-run-no-new-items" => {
            checker.minimum_count("brief_source", 1);
            checker.minimum_count("brief_run", 2);
            checker.count("delivery", 2);
            checker.count("sent_item", 1);
            checker.contains_all(message, &["NO_REPLY", "Previous brief"]);
        }
        "record-delivery-suppresses-repeats" => {
            checker.minimum_count("brief_run", 2);
            checker.minimum_count("delivery", 1);
            checker.minimum_count("sent_item", 1);
            checker.contains_any(message, &["suppress"]);
        }
        _ => return false,
    }
    true
}

fn verify_third_group(checker: &mut Checker<'_>, id: &str, message: &str) {
    match id {
        "feed-failure-health-footnote" => {
            checker.query_count(
                "active health warning",
                "SELECT COUNT(*) FROM health_warning WHERE active = 1",
                1,
            );
            checker.contains_all(message, &["health", "broken-feed"]);
        }
        "feed-recovery-resolves-warning" => {
            checker.minimum_count("brief_run", 2);
            checker.query_count(
                "resolved health warning",
                "SELECT COUNT(*) FROM health_warning WHERE active = 0 AND resolved_at IS NOT NULL",
                1,
            );
            checker.contains_all(message, &["RESOLVED", "changing-feed"]);
        }
        "invalid-source-config-rejects" => {
            checker.count("brief_source", 0);
            checker.contains_any(message, &["invalid", "Bad/Key", "rejected"]);
        }
        "routine-agent-hygiene" => {
            checker.contains_any(message, &["configured", "sources", "outlet"]);
        }
        _ => {}
    }
}

fn open_failure(message: &str, metrics: &Metrics, error: &rusqlite::Error) -> Verification {
    let mut details = Vec::new();
    let mut assistant_pass = true;
    if message.trim().is_empty() {
        assistant_pass = false;
        details.push("assistant final message was empty or not exposed in JSONL".to_owned());
    }
    if metrics.has_hygiene_failure() {
        assistant_pass = false;
        details.push("agent used forbidden inspection path".to_owned());
    }
    details.push(format!("open eval database: {error}"));
    Verification {
        passed: false,
        database_pass: false,
        assistant_pass,
        details: details.join("; "),
    }
}
