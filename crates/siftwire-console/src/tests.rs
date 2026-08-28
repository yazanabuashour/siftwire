#![expect(
    clippy::expect_used,
    reason = "router tests assert their own setup failures explicitly"
)]

use std::io::Write as _;
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
    let mut file = std::fs::File::create(&path).expect("create fake runner");
    let body = format!(
        "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >\"$FAKE_RUNNER_ARGS\"\ncat >\"$FAKE_RUNNER_STDIN\" || true\nprintf '%s\\n' '{stdout}'\nexit {exit}\n",
    );
    file.write_all(body.as_bytes()).expect("write fake runner");
    drop(file);
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
    let script = temp.path().join("options-runner.sh");
    std::fs::write(
        &script,
        format!(
            "#!/usr/bin/env bash\ncat >'{}'\nprintf '%s\\n' '{{\"rejected\":false,\"summary\":\"stored\"}}'\n",
            input.display()
        ),
    )
    .expect("write runner");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("chmod runner");
    let app = crate::router(state_with_runner(
        &script.to_string_lossy(),
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

#[tokio::test]
async fn static_fallback_serves_spa_routes() {
    let temp = TempDir::new().expect("temp dir");
    std::fs::write(temp.path().join("index.html"), "<html>siftwire</html>").expect("write index");
    let app = crate::router(state_with_runner(
        "/bin/true",
        temp.path().to_str().expect("utf8"),
    ));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/sources")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("in-process response");
    assert_eq!(response.status(), StatusCode::OK);
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
