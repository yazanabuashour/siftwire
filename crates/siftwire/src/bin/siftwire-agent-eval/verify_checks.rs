use rusqlite::{Connection, params};

use crate::types::Verification;

pub struct Checker<'connection> {
    database: &'connection Connection,
    result: Verification,
    details: Vec<String>,
}

impl<'connection> Checker<'connection> {
    pub const fn new(database: &'connection Connection) -> Self {
        Self {
            database,
            result: Verification {
                passed: true,
                database_pass: true,
                assistant_pass: true,
                details: String::new(),
            },
            details: Vec::new(),
        }
    }

    pub fn fail_assistant(&mut self, detail: impl Into<String>) {
        self.result.assistant_pass = false;
        self.details.push(detail.into());
    }

    pub fn count(&mut self, table: &str, wanted: i64) {
        let query = format!("SELECT COUNT(*) FROM {table}");
        match self
            .database
            .query_row(&query, [], |row| row.get::<_, i64>(0))
        {
            Ok(actual) if actual == wanted => {}
            Ok(actual) => self.fail_database(format!("{table} count = {actual}, want {wanted}")),
            Err(error) => self.fail_database(format!("count {table}: {error}")),
        }
    }

    pub fn minimum_count(&mut self, table: &str, wanted: i64) {
        let query = format!("SELECT COUNT(*) FROM {table}");
        match self
            .database
            .query_row(&query, [], |row| row.get::<_, i64>(0))
        {
            Ok(actual) if actual >= wanted => {}
            Ok(actual) => {
                self.fail_database(format!("{table} count = {actual}, want at least {wanted}"));
            }
            Err(error) => self.fail_database(format!("count {table}: {error}")),
        }
    }

    pub fn query_count(&mut self, label: &str, query: &str, wanted: i64) {
        match self
            .database
            .query_row(query, [], |row| row.get::<_, i64>(0))
        {
            Ok(actual) if actual == wanted => {}
            Ok(actual) => self.fail_database(format!("{label} count = {actual}, want {wanted}")),
            Err(error) => self.fail_database(format!("count {label}: {error}")),
        }
    }

    pub fn runtime_value(&mut self, key: &str, wanted: &str) {
        let value = self.database.query_row(
            "SELECT value_text FROM runtime_config WHERE key_name = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match value {
            Ok(actual) if actual == wanted => {}
            Ok(actual) => self.fail_database(format!(
                "runtime_config {key} = {actual:?}, want {wanted:?}"
            )),
            Err(error) => self.fail_database(format!("runtime_config {key}: {error}")),
        }
    }

    pub fn recorded_delivery(&mut self, final_answer: &str) {
        let wanted = current_brief(final_answer);
        match self.latest_delivery() {
            Ok(actual) if actual == wanted => {}
            Ok(_) => self
                .fail_database("delivery message did not match current delivered brief".to_owned()),
            Err(error) => self.fail_database(format!("delivery message: {error}")),
        }
    }

    pub fn latest_delivery_is(&mut self, wanted: &str) {
        match self.latest_delivery() {
            Ok(actual) if actual == wanted => {}
            Ok(actual) => self.fail_database(format!(
                "latest delivery message = {actual:?}, want {wanted:?}"
            )),
            Err(error) => self.fail_database(format!("delivery message: {error}")),
        }
    }

    fn latest_delivery(&self) -> rusqlite::Result<String> {
        self.database.query_row(
            "SELECT message FROM delivery ORDER BY delivered_at DESC, id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
    }

    pub fn bullet_count(&mut self, message: &str, wanted: usize) {
        let actual = message
            .lines()
            .filter(|line| line.trim().starts_with("- ["))
            .count();
        if actual != wanted {
            self.fail_assistant(format!("assistant bullet count = {actual}, want {wanted}"));
        }
    }

    pub fn contains_any(&mut self, message: &str, values: &[&str]) {
        let lower = message.to_lowercase();
        if !values
            .iter()
            .any(|value| lower.contains(&value.to_lowercase()))
        {
            self.fail_assistant(format!(
                "assistant message did not contain any of {values:?}"
            ));
        }
    }

    pub fn contains_all(&mut self, message: &str, values: &[&str]) {
        for value in values {
            if !message.contains(value) {
                self.fail_assistant(format!("assistant message did not contain {value:?}"));
            }
        }
    }

    pub fn finish(mut self) -> Verification {
        self.result.passed = self.result.database_pass && self.result.assistant_pass;
        if !self.details.is_empty() {
            self.result.details = self.details.join("; ");
        }
        self.result
    }

    fn fail_database(&mut self, detail: String) {
        self.result.database_pass = false;
        self.details.push(detail);
    }
}

fn current_brief(final_answer: &str) -> &str {
    let without_heading = final_answer
        .strip_prefix("Current brief\n\n")
        .unwrap_or(final_answer);
    without_heading
        .split_once("\n\nPrevious brief (")
        .map_or(without_heading, |(current, _)| current)
}
