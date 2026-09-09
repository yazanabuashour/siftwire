#![expect(
    clippy::expect_used,
    reason = "router tests assert their own setup failures explicitly"
)]

use std::os::unix::fs::PermissionsExt as _;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt as _;

use crate::settings::Settings;

fn state_with_runner(script: &str, web_root: &str) -> crate::AppState {
    crate::AppState {
        settings: Settings {
            bind: "127.0.0.1:0".to_owned(),
            web_root: web_root.to_owned(),
            runner_bin: script.to_owned(),
            database: None,
            run_timeout: std::time::Duration::from_secs(10),
        },
    }
}

/// Writes an executable fake runner that records its invocation and stdin.
fn write_runner(dir: &TempDir, stdout: &str, exit: i32) -> String {
    let path = dir.path().join("fake-runner.sh");
    std::fs::write(dir.path().join("output.json"), stdout).expect("write runner output");
    let body = format!(
        "#!/usr/bin/env bash\ncd -- '{}' || exit 1\nprintf '%s\\n' \"$*\" >args\ncat >input.json\ncat output.json\nexit {exit}\n",
        dir.path().display(),
    );
    std::fs::write(&path, body).expect("write fake runner");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake runner");
    path.to_string_lossy().into_owned()
}

async fn call(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.expect("in-process response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json body")
    };
    (status, value)
}

#[tokio::test]
async fn rejected_results_map_to_400() {
    let temp = TempDir::new().expect("temp dir");
    let script = write_runner(
        &temp,
        r#"{"rejected":true,"rejection_reason":"source key must be lowercase"}"#,
        0,
    );
    let app = crate::router(state_with_runner(
        &script,
        temp.path().to_str().expect("utf8"),
    ));
    let (status, body) = call(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/sources")
            .header("content-type", "application/json")
            .body(Body::from(json!({"key":"fixture"}).to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body.pointer("/error/message").and_then(Value::as_str),
        Some("source key must be lowercase")
    );
}

#[tokio::test]
async fn options_forward_sports_settings_to_the_runner() {
    let temp = TempDir::new().expect("temp dir");
    let input = temp.path().join("input.json");
    let script = write_runner(&temp, r#"{"rejected":false,"summary":"stored"}"#, 0);
    let app = crate::router(state_with_runner(
        &script,
        temp.path().to_str().expect("utf8"),
    ));
    let request = json!({
        "max_delivery_items": 7,
        "sports_pre_game_days": 7,
        "sports_post_game_days": 3,
        "sports_timezone": "America/Chicago"
    });
    let (status, _body) = call(
        app,
        Request::builder()
            .method("PUT")
            .uri("/api/v1/options")
            .header("content-type", "application/json")
            .body(Body::from(request.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let forwarded: Value =
        serde_json::from_slice(&std::fs::read(input).expect("read runner input"))
            .expect("decode runner input");
    assert_eq!(
        forwarded.get("sports_timezone").and_then(Value::as_str),
        Some("America/Chicago")
    );
    assert_eq!(
        forwarded
            .get("sports_pre_game_days")
            .and_then(Value::as_i64),
        Some(7)
    );
}

#[tokio::test]
async fn unknown_run_maps_to_404() {
    let temp = TempDir::new().expect("temp dir");
    // The CLI prints the not-found diagnostic on stderr and exits 1.
    let path = temp.path().join("missing-runner.sh");
    std::fs::write(
        &path,
        "#!/usr/bin/env bash\necho 'show run: run not found: nope' >&2\nexit 1\n",
    )
    .expect("write runner");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod runner");
    let app = crate::router(state_with_runner(
        &path.to_string_lossy(),
        temp.path().to_str().expect("utf8"),
    ));
    let (status, body) = call(
        app,
        Request::builder()
            .uri("/api/v1/runs/nope")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.get("error").is_some(), "expected error body: {body}");
}

fn json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

fn priority_items() -> Value {
    json!([
        {"priority_rank": i64::MIN, "source_key": "min"},
        {"priority_rank": i64::MAX, "source_key": "max"},
        {"priority_rank": 9_007_199_254_740_993_i64},
        {"priority_rank": -9_007_199_254_740_993_i64},
        {"priority_rank": 0, "enabled": true, "extra": {"priority_rank": 42}}
    ])
}

fn string_priority_items() -> Value {
    json!([
        {"priority_rank": "-9223372036854775808", "source_key": "min"},
        {"priority_rank": "9223372036854775807", "source_key": "max"},
        {"priority_rank": "9007199254740993"},
        {"priority_rank": "-9007199254740993"},
        {"priority_rank": "0", "enabled": true, "extra": {"priority_rank": 42}}
    ])
}

#[tokio::test]
async fn every_configuration_response_stringifies_source_priorities() {
    let mut output = json!({
        "runner_protocol": "siftwire-runner/v4",
        "capabilities": ["current-news/v1"],
        "sources": priority_items(), "summary": "stored", "rejected": false,
        "runtime_config": {"max_delivery_items": "7"},
        "outlets": [{"name": "fixture", "extra": {"priority_rank": 23}}],
        "unrelated_number": 9_007_199_254_740_993_i64
    });
    let mut expected = output.clone();
    *expected.get_mut("sources").expect("sources") = string_priority_items();
    output
        .get_mut("sources")
        .expect("sources")
        .as_array_mut()
        .expect("sources")
        .push(json!({"key": "zero"}));
    expected
        .get_mut("sources")
        .expect("sources")
        .as_array_mut()
        .expect("sources")
        .push(json!({
            "key": "zero", "priority_rank": "0"
        }));
    for (method, uri, request) in [
        ("GET", "/api/v1/config", Value::Null),
        ("POST", "/api/v1/sources", json!({"key": "fixture"})),
        ("DELETE", "/api/v1/sources/fixture", Value::Null),
        ("PUT", "/api/v1/options", json!({"max_delivery_items": 7})),
        ("PUT", "/api/v1/outlets", json!({"outlets": []})),
    ] {
        let temp = TempDir::new().expect("temp dir");
        let script = write_runner(&temp, &output.to_string(), 0);
        let app = crate::router(state_with_runner(
            &script,
            temp.path().to_str().expect("utf8"),
        ));
        let (status, body) = call(app, json_request(method, uri, &request)).await;
        assert_eq!(status, StatusCode::OK, "{method} {uri}: {body}");
        assert_eq!(body, expected, "{method} {uri}");
    }
}

#[tokio::test]
async fn archive_queries_proxy_filters_cursors_and_literal_search() {
    let output = json!({"runs": [{"run_id": "old"}], "next_before": "old"});
    for (uri, arguments) in [
        ("/api/v1/runs", "runs list --json\n"),
        (
            "/api/v1/runs?delivered=true&before=old&search=100%25%20literal&limit=1",
            "runs list --json --limit 1 --delivered --before old --search 100% literal\n",
        ),
        (
            "/api/v1/runs?delivered=false&search=--db",
            "runs list --json --search --db\n",
        ),
    ] {
        let temp = TempDir::new().expect("temp dir");
        let script = write_runner(&temp, &output.to_string(), 0);
        let app = crate::router(state_with_runner(
            &script,
            temp.path().to_str().expect("utf8"),
        ));
        let (status, body) = call(app, json_request("GET", uri, &Value::Null)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, output);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("args")).expect("args"),
            arguments
        );
    }
}

#[tokio::test]
async fn invalid_archive_queries_fail_before_spawning() {
    for query in [
        "limit=0",
        "limit=-1",
        "limit=9223372036854775807",
        "limit=9223372036854775808",
        "limit=no",
        "before=",
        "before=%20",
        "delivered=yes",
        "unknown=true",
        "limit=1&limit=2",
    ] {
        let temp = TempDir::new().expect("temp dir");
        let script = write_runner(&temp, "{}", 0);
        let app = crate::router(state_with_runner(
            &script,
            temp.path().to_str().expect("utf8"),
        ));
        let response = app
            .oneshot(json_request(
                "GET",
                &format!("/api/v1/runs?{query}"),
                &Value::Null,
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{query}");
        assert!(!temp.path().join("args").exists(), "spawned for {query}");
    }
}

#[tokio::test]
async fn historical_priorities_are_strings_without_rewriting_other_evidence() {
    let output = json!({
        "run": {"run_id": "fixture", "dry_run": false},
        "delivery_html": "<!doctype html>\n<html><body>Exact saved HTML &amp; legacy evidence</body></html>",
        "must_include": priority_items(), "candidates": priority_items(),
        "dropped": [{"detail": {"priority_rank": 9_007_199_254_740_993_i64}}],
        "fetch": [
            {"items": 7, "new_items": 3},
            {"items": 10, "new_items": null, "current_news": {
                "since": "2026-09-06T08:30:00Z", "until": "2026-09-07T08:30:00Z",
                "eligible_items": 4, "stale_items": 3, "undated_items": 2, "future_items": 1
            }},
            {"items": 0, "new_items": null, "status": "error"}
        ], "sent_items": []
    });
    let mut output = output;
    output
        .pointer_mut("/must_include/0")
        .expect("historical item")
        .as_object_mut()
        .expect("item object")
        .extend([
            ("kind".to_owned(), json!("atom")),
            ("always_report".to_owned(), json!(true)),
            ("threshold".to_owned(), json!("audit")),
            ("reporting".to_owned(), json!("observe")),
        ]);
    let mut expected = output.clone();
    for collection in ["must_include", "candidates"] {
        for (item, string_item) in expected
            .get_mut(collection)
            .expect("collection")
            .as_array_mut()
            .expect("items")
            .iter_mut()
            .zip(string_priority_items().as_array().expect("string items"))
        {
            *item.get_mut("priority_rank").expect("priority") = string_item
                .get("priority_rank")
                .expect("string priority")
                .clone();
        }
    }
    let temp = TempDir::new().expect("temp dir");
    let script = write_runner(&temp, &output.to_string(), 0);
    let app = crate::router(state_with_runner(
        &script,
        temp.path().to_str().expect("utf8"),
    ));
    let (status, body) = call(
        app,
        json_request("GET", "/api/v1/runs/fixture", &Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, expected);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("args")).expect("args"),
        "runs show --json fixture\n"
    );
}

#[tokio::test]
async fn source_writes_forward_exact_i64_numbers_and_preserve_omission() {
    for (priority, expected_rank) in [
        (None, None),
        (Some(json!("-9223372036854775808")), Some(i64::MIN)),
        (Some(json!("9223372036854775807")), Some(i64::MAX)),
        (Some(json!("9007199254740993")), Some(9_007_199_254_740_993)),
        (
            Some(json!("-9007199254740993")),
            Some(-9_007_199_254_740_993),
        ),
        (Some(json!("00042")), Some(42)),
        (Some(json!("-00042")), Some(-42)),
        (Some(json!("-0")), Some(0)),
        (
            Some(json!(9_007_199_254_740_991_i64)),
            Some(9_007_199_254_740_991),
        ),
        (
            Some(json!(-9_007_199_254_740_991_i64)),
            Some(-9_007_199_254_740_991),
        ),
        (Some(json!(0)), Some(0)),
    ] {
        let temp = TempDir::new().expect("temp dir");
        let script = write_runner(&temp, r#"{"rejected":false}"#, 0);
        let app = crate::router(state_with_runner(
            &script,
            temp.path().to_str().expect("utf8"),
        ));
        let mut source = json!({"key": "fixture", "enabled": true, "label": "Fixture", "extra": {"priority_rank": 12}});
        let mut expected = source.clone();
        if let Some(rank) = priority {
            source
                .as_object_mut()
                .expect("source")
                .insert("priority_rank".to_owned(), rank);
        }
        if let Some(rank) = expected_rank {
            expected
                .as_object_mut()
                .expect("source")
                .insert("priority_rank".to_owned(), json!(rank));
        }
        let (status, body) = call(app, json_request("POST", "/api/v1/sources", &source)).await;
        assert_eq!(status, StatusCode::OK, "{source}: {body}");
        let forwarded: Value = serde_json::from_slice(
            &std::fs::read(temp.path().join("input.json")).expect("runner input"),
        )
        .expect("decode input");
        assert_eq!(
            forwarded,
            json!({"action": "upsert_source", "source": expected}),
            "{source}"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("args")).expect("args"),
            "config\n"
        );
    }
}

#[tokio::test]
async fn invalid_browser_priorities_are_rejected_before_spawning() {
    for rank in [
        json!(""),
        json!("-"),
        json!("+1"),
        json!(" 1"),
        json!("1 "),
        json!("1\n"),
        json!("1.0"),
        json!("1e3"),
        json!("--1"),
        json!("１２"),
        json!("−1"),
        json!("9223372036854775808"),
        json!("-9223372036854775809"),
        json!(9_007_199_254_740_992_i64),
        json!(-9_007_199_254_740_992_i64),
        json!(i64::MIN),
        json!(i64::MAX),
        json!(u64::MAX),
        json!(1.0),
        json!(1.5),
        json!(1e3),
        Value::Null,
        json!(true),
        json!(false),
        json!([]),
        json!({}),
    ] {
        let temp = TempDir::new().expect("temp dir");
        let script = write_runner(&temp, r#"{"rejected":false}"#, 0);
        let app = crate::router(state_with_runner(
            &script,
            temp.path().to_str().expect("utf8"),
        ));
        let source = json!({"key": "fixture", "priority_rank": rank});
        let (status, body) = call(app, json_request("POST", "/api/v1/sources", &source)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{source}: {body}");
        assert!(
            body.pointer("/error/message")
                .expect("error message")
                .as_str()
                .expect("error")
                .contains("priority_rank"),
            "{body}"
        );
        assert!(
            !temp.path().join("args").exists(),
            "runner spawned for {source}"
        );
    }
}

#[tokio::test]
async fn invalid_runner_priority_shapes_fail_visibly() {
    for collection in ["sources", "must_include", "candidates"] {
        let mut invalid_items = vec![Value::Null, json!({}), json!([null]), json!([1])];
        for rank in [
            json!("1"),
            json!(1.0),
            Value::Null,
            json!(false),
            json!([]),
            json!({}),
            json!(u64::MAX),
        ] {
            invalid_items.push(json!([{"priority_rank": rank}]));
        }
        if collection != "sources" {
            invalid_items.push(json!([{}]));
        }
        for items in invalid_items {
            let mut output = json!({"sources": [], "must_include": [], "candidates": []});
            *output.get_mut(collection).expect("collection") = items;
            let temp = TempDir::new().expect("temp dir");
            let script = write_runner(&temp, &output.to_string(), 0);
            let app = crate::router(state_with_runner(
                &script,
                temp.path().to_str().expect("utf8"),
            ));
            let uri = if collection == "sources" {
                "/api/v1/config"
            } else {
                "/api/v1/runs/fixture"
            };
            let (status, body) = call(app, json_request("GET", uri, &Value::Null)).await;
            assert_eq!(status, StatusCode::BAD_GATEWAY, "{output}: {body}");
            let message = body
                .pointer("/error/message")
                .and_then(Value::as_str)
                .expect("error");
            assert!(
                message.contains("invalid runner response") && message.contains(collection),
                "{body}"
            );
        }
    }
}

#[tokio::test]
async fn static_fallback_serves_spa_routes() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(temp.path().join("index.html"), "<html>siftwire</html>").expect("write index");
    let app = crate::router(state_with_runner(
        "/bin/true",
        temp.path().to_str().expect("utf8"),
    ));
    for uri in ["/", "/sources"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("in-process response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-cache");
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        assert!(
            String::from_utf8_lossy(&bytes).contains("siftwire"),
            "index body differs"
        );
    }
}

#[tokio::test]
async fn foreign_host_headers_are_rejected() {
    let temp = TempDir::new().expect("temp dir");
    let app = crate::router(state_with_runner(
        "/bin/true",
        temp.path().to_str().expect("utf8"),
    ));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header("host", "attacker.example.com")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("in-process response");
    assert_eq!(response.status(), StatusCode::MISDIRECTED_REQUEST);
}

#[tokio::test]
async fn lan_ip_host_headers_are_accepted() {
    let temp = TempDir::new().expect("temp dir");
    let app = crate::router(state_with_runner(
        "/bin/true",
        temp.path().to_str().expect("utf8"),
    ));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header("host", "192.168.0.143:8790")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("in-process response");
    assert_eq!(response.status(), StatusCode::OK);
}
