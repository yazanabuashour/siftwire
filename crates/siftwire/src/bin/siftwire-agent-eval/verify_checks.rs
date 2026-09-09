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

    pub fn prepared_deliveries(&mut self) {
        self.query_count(
            "deliveries without matching immutable plans",
            "SELECT COUNT(*) FROM delivery AS d LEFT JOIN delivery_plan AS p ON p.run_id = d.run_id WHERE p.id IS NULL OR p.message != d.message",
            0,
        );
    }

    // These synthetic scenarios contain normal feed items, not sports updates.
    pub fn selected_evidence(&mut self) {
        self.query_count(
            "prepared items differing from required and selected evidence",
            "WITH candidates AS (
                SELECT *, ROW_NUMBER() OVER (PARTITION BY run_id ORDER BY id) - 1 AS candidate_index
                FROM brief_run_item WHERE category = 'candidate'
             ), expected AS (
                SELECT p.run_id, CAST(r.id AS TEXT) AS item_id, r.title, r.url, r.kind
                FROM delivery_plan p JOIN brief_run_item r ON r.run_id = p.run_id
                WHERE r.category = 'must_include'
                UNION ALL
                SELECT p.run_id, CAST(c.id AS TEXT), c.title, c.url, c.kind
                FROM delivery_plan p, json_each(p.candidate_indexes_json) i
                JOIN candidates c ON c.run_id = p.run_id AND c.candidate_index = i.value
             ), actual AS (
                SELECT p.run_id, json_extract(i.value, '$.run_item_ids[0]') AS item_id,
                    json_extract(i.value, '$.title') AS title,
                    json_extract(i.value, '$.url') AS url,
                    json_extract(i.value, '$.kind') AS kind
                FROM delivery_plan p, json_each(p.items_json) i
             ), missing AS (SELECT * FROM expected EXCEPT SELECT * FROM actual),
             extra AS (SELECT * FROM actual EXCEPT SELECT * FROM expected)
             SELECT (SELECT COUNT(*) FROM missing) + (SELECT COUNT(*) FROM extra)",
            0,
        );
        self.query_count(
            "sent items differing from prepared evidence",
            "WITH expected AS (
                SELECT d.id AS delivery_id, p.run_id,
                    json_extract(i.value, '$.title') AS title,
                    json_extract(i.value, '$.url') AS url,
                    json_extract(i.value, '$.kind') AS kind
                FROM delivery d JOIN delivery_plan p ON p.run_id = d.run_id,
                    json_each(p.items_json) i
             ), actual AS (SELECT delivery_id, run_id, title, url, kind FROM sent_item),
             missing AS (SELECT * FROM expected EXCEPT SELECT * FROM actual),
             extra AS (SELECT * FROM actual EXCEPT SELECT * FROM expected)
             SELECT (SELECT COUNT(*) FROM missing) + (SELECT COUNT(*) FROM extra)",
            0,
        );
    }

    pub fn recorded_delivery(&mut self, final_answer: &str) {
        match self.delivery_answer() {
            Ok(expected) if expected == final_answer => {}
            Ok(_) => {
                self.fail_assistant("final answer differs from recorded current/history bodies");
            }
            Err(error) => self.fail_database(format!("delivery history: {error}")),
        }
    }

    fn delivery_answer(&self) -> anyhow::Result<String> {
        let mut statement = self.database.prepare(
            "SELECT message, delivered_at FROM delivery ORDER BY delivered_at DESC, id DESC LIMIT 3",
        )?;
        let mut rows = statement.query([])?;
        let current = rows.next()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let mut answer = format!("Current brief\n\n{}", current.get::<_, String>(0)?);
        while let Some(previous) = rows.next()? {
            let delivered_at =
                chrono::DateTime::parse_from_rfc3339(&previous.get::<_, String>(1)?)?
                    .with_timezone(&chrono::Utc)
                    .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
            answer.push_str("\n\nPrevious brief (");
            answer.push_str(&delivered_at);
            answer.push_str(")\n\n");
            answer.push_str(&previous.get::<_, String>(0)?);
        }
        Ok(answer)
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
