use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::runner::{Invocation, Outcome};

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({ "error": { "message": self.message } });
        (self.status, Json(body)).into_response()
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

/// Runs one runner invocation and maps outcomes onto HTTP semantics.
async fn run(
    state: &AppState,
    arguments: Vec<String>,
    request: Option<Value>,
) -> Result<Value, ApiError> {
    let invocation = Invocation {
        runner_bin: state.settings.runner_bin.clone(),
        database: state.settings.database.clone(),
        arguments,
        request,
        timeout: state.settings.run_timeout,
    };
    match crate::runner::invoke(&invocation).await {
        Ok(Outcome::Ok(value)) => Ok(value),
        Ok(Outcome::Rejected { reason }) => Err(ApiError::new(StatusCode::BAD_REQUEST, reason)),
        Ok(Outcome::Usage(stderr)) => Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("runner rejected the console arguments: {}", stderr.trim()),
        )),
        Ok(Outcome::Failed(stderr)) if stderr.contains("unknown cursor in selected run order") => {
            Err(ApiError::bad_request(stderr.trim()))
        }
        Ok(Outcome::Failed(stderr)) if stderr.contains("run not found") => {
            Err(ApiError::new(StatusCode::NOT_FOUND, stderr.trim()))
        }
        Ok(Outcome::Failed(stderr)) => Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("runner failed: {}", stderr.trim()),
        )),
        Err(error) => Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("runner unavailable: {error:#}"),
        )),
    }
}

// Receipt: the browser compatibility contract uses Number.MAX_SAFE_INTEGER.
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

fn parse_priority(value: &Value) -> Result<i64, ApiError> {
    let rank = match value {
        Value::String(raw) => {
            let digits = raw.strip_prefix('-').unwrap_or(raw);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                None
            } else {
                raw.parse::<i64>().ok()
            }
        }
        Value::Number(number) => number
            .as_i64()
            .filter(|rank| (-JS_MAX_SAFE_INTEGER..=JS_MAX_SAFE_INTEGER).contains(rank)),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    };
    rank.ok_or_else(|| {
        ApiError::bad_request(
            "priority_rank must be a signed decimal i64 string or a JSON integer within ±9007199254740991",
        )
    })
}

/// Converts only the runner collections that carry source priorities.
fn priority_response(mut value: Value, collections: &[&str]) -> ApiResult {
    let invalid = |path: &str, expected: &str| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("invalid runner response: {path} must be {expected}"),
        )
    };
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("result", "an object"))?;
    for &collection in collections {
        // ConfigResult omits empty sources; Source omits a zero priority.
        if collection == "sources" && !object.contains_key(collection) {
            continue;
        }
        let items = object
            .get_mut(collection)
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid(collection, "an array"))?;
        for (index, item) in items.iter_mut().enumerate() {
            let path = format!("{collection}[{index}]");
            let item = item
                .as_object_mut()
                .ok_or_else(|| invalid(&path, "an object"))?;
            let rank = match item.get("priority_rank") {
                None if collection == "sources" => 0,
                rank => rank.and_then(Value::as_i64).ok_or_else(|| {
                    invalid(
                        &format!("{path}.priority_rank"),
                        "a signed i64 JSON integer",
                    )
                })?,
            };
            item.insert("priority_rank".to_owned(), Value::String(rank.to_string()));
        }
    }
    Ok(Json(value))
}

fn protocol_arguments(verb: &str) -> Vec<String> {
    vec![verb.to_owned()]
}

fn cli_arguments(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunsQuery {
    limit: Option<i64>,
    delivered: Option<bool>,
    before: Option<String>,
    search: Option<String>,
}

pub async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

/// Returns `inspect_config` with source priorities encoded as decimal strings.
///
/// # Errors
///
/// Returns an API error when the runner invocation fails or is rejected.
pub async fn config(State(state): State<AppState>) -> ApiResult {
    let value = run(
        &state,
        protocol_arguments("config"),
        Some(json!({ "action": "inspect_config" })),
    )
    .await?;
    priority_response(value, &["sources"])
}

/// Stores one source through `upsert_source`.
///
/// # Errors
///
/// Returns an API error when the body is malformed or the runner invocation
/// fails or is rejected.
pub async fn upsert_source(
    State(state): State<AppState>,
    Json(mut source): Json<Value>,
) -> ApiResult {
    if !source.is_object() {
        return Err(ApiError::bad_request("body must be a source object"));
    }
    if let Some(rank) = source.get_mut("priority_rank") {
        *rank = Value::from(parse_priority(rank)?);
    }
    let value = run(
        &state,
        protocol_arguments("config"),
        Some(json!({ "action": "upsert_source", "source": source })),
    )
    .await?;
    priority_response(value, &["sources"])
}

/// Deletes one source by key.
///
/// # Errors
///
/// Returns an API error when the runner invocation fails or is rejected.
pub async fn delete_source(State(state): State<AppState>, Path(key): Path<String>) -> ApiResult {
    if key.trim().is_empty() {
        return Err(ApiError::bad_request("source key is required"));
    }
    let value = run(
        &state,
        protocol_arguments("config"),
        Some(json!({ "action": "delete_source", "key": key })),
    )
    .await?;
    priority_response(value, &["sources"])
}

#[derive(Deserialize)]
pub struct OptionsRequest {
    max_delivery_items: Option<i64>,
    sports_pre_game_days: Option<i64>,
    sports_post_game_days: Option<i64>,
    sports_timezone: Option<String>,
}

/// Sets brief and sports options through `set_brief_options`.
///
/// # Errors
///
/// Returns an API error when the runner invocation fails or is rejected.
pub async fn set_options(
    State(state): State<AppState>,
    Json(body): Json<OptionsRequest>,
) -> ApiResult {
    let value = run(
        &state,
        protocol_arguments("config"),
        Some(json!({
            "action": "set_brief_options",
            "max_delivery_items": body.max_delivery_items,
            "sports_pre_game_days": body.sports_pre_game_days,
            "sports_post_game_days": body.sports_post_game_days,
            "sports_timezone": body.sports_timezone,
        })),
    )
    .await?;
    priority_response(value, &["sources"])
}

/// Replaces the full outlet policy list through `replace_outlet_policies`.
///
/// # Errors
///
/// Returns an API error when the body is malformed or the runner invocation
/// fails or is rejected.
pub async fn replace_outlets(State(state): State<AppState>, Json(body): Json<Value>) -> ApiResult {
    let Value::Object(map) = &body else {
        return Err(ApiError::bad_request("body must be an object"));
    };
    let Some(outlets) = map.get("outlets") else {
        return Err(ApiError::bad_request("outlets field is required"));
    };
    if !outlets.is_array() {
        return Err(ApiError::bad_request("outlets must be an array"));
    }
    let value = run(
        &state,
        protocol_arguments("config"),
        Some(json!({ "action": "replace_outlet_policies", "outlets": outlets })),
    )
    .await?;
    priority_response(value, &["sources"])
}

pub async fn runs(State(state): State<AppState>, Query(query): Query<RunsQuery>) -> ApiResult {
    if query
        .limit
        .is_some_and(|limit| limit <= 0 || limit == i64::MAX)
    {
        return Err(ApiError::bad_request("limit must be positive"));
    }
    let mut arguments = cli_arguments(&["runs", "list", "--json"]);
    if let Some(limit) = query.limit {
        arguments.push("--limit".to_owned());
        arguments.push(limit.to_string());
    }
    if query.delivered == Some(true) {
        arguments.push("--delivered".to_owned());
    }
    if let Some(before) = query.before {
        if before.trim().is_empty() {
            return Err(ApiError::bad_request("before requires a run id"));
        }
        arguments.extend(["--before".to_owned(), before]);
    }
    if let Some(search) = query.search {
        arguments.extend(["--search".to_owned(), search]);
    }
    let value = run(&state, arguments, None).await?;
    Ok(Json(value))
}

pub async fn run_detail(State(state): State<AppState>, Path(run_id): Path<String>) -> ApiResult {
    if run_id.trim().is_empty() {
        return Err(ApiError::bad_request("run id is required"));
    }
    let value = run(
        &state,
        cli_arguments(&["runs", "show", "--json", &run_id]),
        None,
    )
    .await?;
    priority_response(value, &["must_include", "candidates"])
}
