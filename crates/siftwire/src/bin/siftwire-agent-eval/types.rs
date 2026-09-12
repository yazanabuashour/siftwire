use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const ADAPTER_PROTOCOL: &str = "siftwire-agent-eval/v1";

#[derive(Clone)]
pub struct Turn {
    pub prompt: String,
}

#[derive(Clone)]
pub struct Scenario {
    pub id: &'static str,
    pub turns: Vec<Turn>,
}

#[derive(Default)]
pub struct RunOptions {
    pub run_root: String,
    pub scenario: String,
    pub adapter: String,
    pub report_dir: String,
    pub report_name: String,
}

#[derive(Serialize)]
pub struct Request {
    pub protocol: &'static str,
    pub workspace: String,
    pub skill_path: String,
    pub artifact_dir: String,
    pub prompts: Vec<String>,
    pub tool_env: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub adapter: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub reasoning_effort: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterResult {
    pub protocol: String,
    pub runtime: Runtime,
    pub turns: Vec<AgentTurn>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTurn {
    pub final_message: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub assistant_calls: Option<usize>,
    pub actions: Vec<Action>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Command { command: String },
    Read { path: String },
    Other { name: String },
}

#[derive(Serialize)]
pub struct RunResult {
    pub run_root: String,
    pub scenario_count: usize,
    #[serde(rename = "scenario_results")]
    pub results: Vec<JobResult>,
    pub elapsed_seconds: f64,
}

#[derive(Clone, Default, Serialize)]
pub struct JobResult {
    pub scenario_id: String,
    pub prompts: Vec<String>,
    pub run_dir: String,
    pub database: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Runtime>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    pub seconds: f64,
    pub metrics: Metrics,
    pub verification: Verification,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub final_message: String,
}

#[derive(Clone, Default, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "eval output records four independent hygiene receipts"
)]
pub struct Metrics {
    pub assistant_calls: Option<usize>,
    pub tool_calls: usize,
    pub command_executions: usize,
    pub direct_sqlite_access: bool,
    pub broad_repo_search: bool,
    pub environment_access: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unexpected_command: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hygiene_evidence: Vec<String>,
}

impl Metrics {
    pub const fn has_hygiene_failure(&self) -> bool {
        self.direct_sqlite_access
            || self.broad_repo_search
            || self.environment_access
            || self.unexpected_command
    }
}

#[derive(Clone, Default, Serialize)]
pub struct Verification {
    pub passed: bool,
    pub database_pass: bool,
    pub assistant_pass: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub details: String,
}

pub struct ParsedOutput {
    pub metrics: Metrics,
    pub final_message: String,
    pub runtime: Runtime,
}
