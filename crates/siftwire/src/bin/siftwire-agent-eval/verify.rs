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
    checker.prepared_deliveries();
    checker.selected_evidence();
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
            checker.count("source_state", 0);
            checker.count("delivery", 1);
            checker.count("sent_item", 1);
            checker.recorded_delivery(message);
        }
        "rss-source-generic-processing-fields" => {
            checker.count("source_state", 0);
            checker.count("delivery", 1);
            checker.count("sent_item", 1);
            checker.recorded_delivery(message);
            checker.query_count("processing-field source", "SELECT COUNT(*) FROM brief_source WHERE key = 'github-blog' AND url_canonicalization = 'none' AND outlet_extraction = 'title_suffix' AND dedup_group = 'news' AND priority_rank = 10", 1);
        }
        "outlet-policy-watch-audit" => {
            checker.count("source_state", 0);
            checker.query_count("successful source fetch", "SELECT COUNT(*) FROM fetch_log WHERE source_key = 'github-blog' AND status = 'ok' AND item_count = 1", 1);
            checker.query_count("watch outlet policy", "SELECT COUNT(*) FROM outlet_policy WHERE name = 'Fixture Outlet' AND policy = 'watch' AND enabled = 1", 1);
            checker.query_count("retained watch candidate", "SELECT COUNT(*) FROM brief_run_item WHERE source_key = 'github-blog' AND category = 'candidate' AND outlet = 'Fixture Outlet'", 1);
            checker.query_count("watch policy annotation", "SELECT COUNT(*) FROM brief_run_item WHERE source_key = 'github-blog' AND category = 'annotation' AND reason = 'outlet_policy' AND outlet = 'Fixture Outlet' AND json_extract(detail, '$.policy') = 'watch'", 1);
        }
        "configured-max-delivery-items" => {
            checker.minimum_count("brief_source", 3);
            checker.count("source_state", 0);
            checker.count("delivery", 1);
            checker.count("sent_item", 2);
            checker.runtime_value("max_delivery_items", "2");
            checker.query_count("two selected candidates", "SELECT COUNT(*) FROM delivery_plan WHERE json_array_length(candidate_indexes_json) = 2", 1);
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
            checker.count("source_state", 0);
            checker.count("delivery", 3);
            checker.count("sent_item", 3);
            checker.recorded_delivery(message);
            checker.query_count("distinct history evidence", "SELECT COUNT(DISTINCT url) FROM sent_item WHERE url IN ('https://fixture.example/history-1', 'https://fixture.example/history-2', 'https://fixture.example/history-3')", 3);
        }
        "github-release-source-config" => {
            checker.query_count("repository-only release source", "SELECT COUNT(*) FROM brief_source WHERE key = 'codex-releases' AND kind = 'github_release' AND repo = 'openai/codex' AND url = '' AND threshold = 'always' AND enabled = 1", 1);
            checker.count("fetch_log", 0);
            checker.count("brief_run", 0);
            checker.contains_all(message, &["openai/codex"]);
        }
        "rss-source-must-include" => {
            checker.minimum_count("source_state", 1);
            checker.count("delivery", 1);
            checker.count("sent_item", 1);
            checker.query_count("required fixture", "SELECT COUNT(*) FROM brief_run_item WHERE source_key = 'required-feed' AND category = 'must_include' AND threshold = 'always'", 1);
            checker.query_count(
                "no optional selection",
                "SELECT COUNT(*) FROM delivery_plan WHERE candidate_indexes_json = '[]'",
                1,
            );
            checker.contains_all(
                message,
                &["SiftWire fixture story", "https://fixture.example/story"],
            );
            checker.recorded_delivery(message);
        }
        "repeat-run-no-new-items" => {
            checker.count("source_state", 0);
            checker.minimum_count("brief_source", 1);
            checker.minimum_count("brief_run", 2);
            checker.count("delivery", 2);
            checker.count("sent_item", 1);
            checker.query_count("no repeat candidates", "SELECT COUNT(*) FROM brief_run_item WHERE category = 'candidate' AND run_id = (SELECT id FROM brief_run ORDER BY started_at DESC, id DESC LIMIT 1)", 0);
            checker.query_count("confirmed repeat evidence", "SELECT COUNT(*) FROM brief_run_item WHERE reason = 'recently_sent' AND run_id = (SELECT id FROM brief_run ORDER BY started_at DESC, id DESC LIMIT 1)", 1);
            checker.contains_all(message, &["NO_REPLY", "Previous brief"]);
            checker.recorded_delivery(message);
        }
        _ => return false,
    }
    true
}

fn verify_third_group(checker: &mut Checker<'_>, id: &str, message: &str) {
    match id {
        "feed-failure-health-footnote" => {
            checker.count("source_state", 0);
            checker.query_count(
                "active health warning",
                "SELECT COUNT(*) FROM health_warning WHERE active = 1",
                1,
            );
            checker.contains_all(message, &["health", "broken-feed"]);
        }
        "feed-recovery-resolves-warning" => {
            checker.count("source_state", 0);
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
            checker.contains_all(
                message,
                &["source key must be lowercase letters, numbers, dot, underscore, or hyphen"],
            );
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

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    #[test]
    fn invalid_source_reports_the_rejection_constraint() -> anyhow::Result<()> {
        let database = Connection::open_in_memory()?;
        database.execute("CREATE TABLE brief_source (key TEXT)", [])?;
        let reason = "source key must be lowercase letters, numbers, dot, underscore, or hyphen";
        for message in [
            reason.to_owned(),
            format!("Production runner rejection:\n\n`{reason}`"),
            format!("Rejected Bad/Key: {reason}"),
        ] {
            let mut checker = Checker::new(&database);
            verify_third_group(&mut checker, "invalid-source-config-rejects", &message);
            ensure!(
                checker.finish().passed,
                "rejection was not recognized: {message}"
            );
        }
        for message in [
            "Configured Bad/Key successfully",
            "Rejected: invalid source URL",
            "Production runner rejection",
            "",
        ] {
            let mut checker = Checker::new(&database);
            verify_third_group(&mut checker, "invalid-source-config-rejects", message);
            let result = checker.finish();
            ensure!(
                result.database_pass && !result.assistant_pass,
                "wrong rejection accepted: {message}"
            );
        }
        database.execute("INSERT INTO brief_source VALUES ('Bad/Key')", [])?;
        let mut checker = Checker::new(&database);
        verify_third_group(&mut checker, "invalid-source-config-rejects", reason);
        let result = checker.finish();
        ensure!(
            !result.database_pass && result.assistant_pass,
            "persisted invalid source accepted"
        );
        Ok(())
    }
}
