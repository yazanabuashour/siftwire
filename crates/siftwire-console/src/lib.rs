mod api;
mod runner;
mod settings;
#[cfg(test)]
mod tests;

use std::net::IpAddr;
use std::path::PathBuf;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse as _, Response};
use tower_http::services::{ServeDir, ServeFile};

pub use settings::Settings;

/// Shared application state handed to every handler.
#[derive(Clone, Debug)]
pub struct AppState {
    pub settings: Settings,
}

/// Serves the JSON API plus the built web application from disk.
pub fn router(state: AppState) -> Router {
    let web_root = PathBuf::from(&state.settings.web_root);
    let index = ServeFile::new(web_root.join("index.html"));
    let static_files = ServeDir::new(&web_root).fallback(index);
    Router::new()
        .route("/api/v1/health", get(api::health))
        .route("/api/v1/config", get(api::config))
        .route("/api/v1/sources", post(api::upsert_source))
        .route("/api/v1/sources/{key}", delete(api::delete_source))
        .route("/api/v1/options", put(api::set_options))
        .route("/api/v1/outlets", put(api::replace_outlets))
        .route("/api/v1/runs", get(api::runs))
        .route("/api/v1/runs/{run_id}", get(api::run_detail))
        .fallback_service(static_files)
        .layer(middleware::map_response(revalidate_html))
        .layer(middleware::from_fn(deny_rebound_hosts))
        .with_state(state)
}

// Cached HTML can keep referencing an old build's hashed assets after an upgrade.
async fn revalidate_html(mut response: Response) -> Response {
    if response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|value| value.as_bytes().starts_with(b"text/html"))
    {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    response
}

/// Blocks browser-based DNS rebinding: every Host must be an IP literal or
/// `localhost`, so a foreign domain cannot be aimed at this server.
async fn deny_rebound_hosts(request: Request, next: Next) -> Response {
    let Some(header) = request.headers().get(axum::http::header::HOST) else {
        return next.run(request).await;
    };
    let Ok(host) = header.to_str() else {
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    };
    // Strip a numeric port from an RFC 3986 authority; a bracketed IPv6 host
    // keeps its colons because its name already ends with ']'.
    let name = match host.rsplit_once(':') {
        Some((name, port))
            if !port.is_empty()
                && port.bytes().all(|byte| byte.is_ascii_digit())
                && !name.ends_with(']') =>
        {
            name
        }
        _ => host,
    };
    let name = name.trim_start_matches('[').trim_end_matches(']');
    let allowed =
        name.parse::<IpAddr>().is_ok() || name.eq_ignore_ascii_case("localhost") || name.is_empty();
    if allowed {
        next.run(request).await
    } else {
        StatusCode::MISDIRECTED_REQUEST.into_response()
    }
}

use axum::routing::{delete, get, post, put};
