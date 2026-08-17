use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::Value;
use url::Url;

use super::http::HttpClient;
use super::model::ResolveError;

const ARTICLE_BASE_URL: &str = "https://news.google.com/articles/";
const BATCH_ENDPOINT: &str = "https://news.google.com/_/DotsSplashUi/data/batchexecute";
const BACKOFF_INTERVAL: Duration = Duration::from_millis(250);

pub struct GoogleResolver {
    client: HttpClient,
    calls: Mutex<HashMap<String, Arc<MemoCall>>>,
    next_start: Mutex<Option<Instant>>,
}

struct MemoCall {
    result: Mutex<Option<Result<String, ResolveError>>>,
    ready: Condvar,
}

enum MemoClaim {
    Resolve(Arc<MemoCall>),
    Wait(Arc<MemoCall>),
}

struct DecodingParams {
    article_id: String,
    signature: String,
    timestamp: String,
}

impl GoogleResolver {
    pub fn new(client: HttpClient) -> Self {
        Self {
            client,
            calls: Mutex::new(HashMap::new()),
            next_start: Mutex::new(None),
        }
    }

    pub fn resolve(&self, article_url: &str, deadline: Instant) -> Result<String, ResolveError> {
        match self.claim(article_url)? {
            MemoClaim::Wait(call) => wait_for_call(&call, deadline),
            MemoClaim::Resolve(call) => {
                let mut result = self
                    .wait_for_backoff(deadline)
                    .and_then(|()| self.resolve_uncached(article_url, deadline));
                if let Err(error) = self.record_backoff(result.as_ref().err()) {
                    result = Err(error);
                }
                store_result(&call, &result)?;
                if result.is_err() {
                    self.remove_failed_call(article_url, &call)?;
                }
                result
            }
        }
    }

    fn claim(&self, article_url: &str) -> Result<MemoClaim, ResolveError> {
        let mut calls = lock(&self.calls, "Google News memo")?;
        if let Some(call) = calls.get(article_url) {
            return Ok(MemoClaim::Wait(Arc::clone(call)));
        }
        let call = Arc::new(MemoCall {
            result: Mutex::new(None),
            ready: Condvar::new(),
        });
        let _previous = calls.insert(article_url.to_owned(), Arc::clone(&call));
        drop(calls);
        Ok(MemoClaim::Resolve(call))
    }

    fn remove_failed_call(
        &self,
        article_url: &str,
        failed: &Arc<MemoCall>,
    ) -> Result<(), ResolveError> {
        let mut calls = lock(&self.calls, "Google News memo")?;
        if calls
            .get(article_url)
            .is_some_and(|current| Arc::ptr_eq(current, failed))
        {
            let _removed = calls.remove(article_url);
        }
        drop(calls);
        Ok(())
    }

    fn resolve_uncached(
        &self,
        article_url: &str,
        deadline: Instant,
    ) -> Result<String, ResolveError> {
        let article_id = google_news_article_id(article_url)?;
        let article_endpoint = format!("{ARTICLE_BASE_URL}{article_id}");
        let (body, _final_url) = self
            .client
            .get_until(&article_endpoint, deadline)
            .map_err(normalize_google_error)?;
        let html = String::from_utf8_lossy(&body);
        let params = decoding_params(&html, &article_id)?;
        let request = build_batch_request(&params)?;
        let response = self
            .client
            .post_form_until(BATCH_ENDPOINT, "f.req", &request, deadline)
            .map_err(normalize_google_error)?;
        let resolved = parse_batch_response(&String::from_utf8_lossy(&response))?;
        if !is_publisher_url(&resolved) {
            return Err(ResolveError::Message(
                "google decode RPC returned non-publisher URL".to_owned(),
            ));
        }
        Ok(resolved)
    }

    fn wait_for_backoff(&self, deadline: Instant) -> Result<(), ResolveError> {
        let wait_until = {
            let mut next_start = lock(&self.next_start, "Google News backoff")?;
            let now = Instant::now();
            let Some(wait_until) = *next_start else {
                return Ok(());
            };
            if now >= wait_until {
                return Ok(());
            }
            *next_start = wait_until
                .checked_add(BACKOFF_INTERVAL)
                .or(Some(wait_until));
            wait_until
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = wait_until.saturating_duration_since(Instant::now());
        if wait >= remaining {
            thread::sleep(remaining);
            return Err(ResolveError::Timeout);
        }
        thread::sleep(wait);
        Ok(())
    }

    fn record_backoff(&self, error: Option<&ResolveError>) -> Result<(), ResolveError> {
        if !error.is_some_and(ResolveError::is_backoff_signal) {
            return Ok(());
        }
        let proposed = Instant::now().checked_add(BACKOFF_INTERVAL);
        {
            let mut next_start = lock(&self.next_start, "Google News backoff")?;
            if proposed.is_some_and(|next| next_start.is_none_or(|current| current < next)) {
                *next_start = proposed;
            }
        }
        Ok(())
    }
}

pub fn is_google_news_article_url(value: &str) -> bool {
    google_news_article_id(value).is_ok()
}

fn google_news_article_id(value: &str) -> Result<String, ResolveError> {
    let pattern = Regex::new(r"^https://news\.google\.com/rss/articles/([^?/#]+)")
        .map_err(|error| ResolveError::Message(error.to_string()))?;
    pattern
        .captures(value.trim())
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| ResolveError::Message("not a Google News article URL".to_owned()))
}

fn decoding_params(html: &str, article_id: &str) -> Result<DecodingParams, ResolveError> {
    let signature = first_submatch(html, r#"data-n-a-sg="([^"]+)""#)?;
    let timestamp = first_submatch(html, r#"data-n-a-ts="([^"]+)""#)?;
    if signature.is_empty() || timestamp.is_empty() {
        return Err(ResolveError::Message(format!(
            "missing decode params for Google News article {article_id}"
        )));
    }
    Ok(DecodingParams {
        article_id: article_id.to_owned(),
        signature,
        timestamp,
    })
}

fn first_submatch(text: &str, pattern: &str) -> Result<String, ResolveError> {
    Regex::new(pattern)
        .map_err(|error| ResolveError::Message(error.to_string()))?
        .captures(text)
        .and_then(|captures| captures.get(1))
        .map_or_else(
            || Ok(String::new()),
            |value| Ok(value.as_str().trim().to_owned()),
        )
}

fn build_batch_request(params: &DecodingParams) -> Result<String, ResolveError> {
    let inner = format!(
        r#"["garturlreq",[["X","X",["X","X"],null,null,1,1,"US:en",null,1,null,null,null,null,null,0,1],"X","X",1,[1,1,1],1,1,null,0,0,null,0],"{}",{},"{}"]"#,
        params.article_id, params.timestamp, params.signature
    );
    serde_json::to_string(&[[["Fbv4je".to_owned(), inner]]])
        .map_err(|error| ResolveError::Message(error.to_string()))
}

fn parse_batch_response(raw: &str) -> Result<String, ResolveError> {
    let payload = raw.split("\n\n").nth(1).ok_or_else(no_publisher_url)?;
    let rows: Vec<Value> = serde_json::from_str(payload).map_err(|error| {
        ResolveError::Message(format!("parse Google decode RPC response: {error}"))
    })?;
    rows.iter()
        .filter_map(Value::as_array)
        .find_map(|row| publisher_url_from_row(row))
        .ok_or_else(no_publisher_url)
}

fn publisher_url_from_row(row: &[Value]) -> Option<String> {
    if row.first()?.as_str()? != "wrb.fr" || row.get(1)?.as_str()? != "Fbv4je" {
        return None;
    }
    let encoded = row.get(2)?.as_str()?;
    let decoded: Vec<Value> = serde_json::from_str(encoded).ok()?;
    let resolved = decoded.get(1)?.as_str()?.trim();
    (!resolved.is_empty()).then(|| resolved.to_owned())
}

fn is_publisher_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https")
            && parsed
                .host_str()
                .is_some_and(|host| !host.eq_ignore_ascii_case("news.google.com"))
    })
}

fn normalize_google_error(error: ResolveError) -> ResolveError {
    let ResolveError::Message(message) = error else {
        return error;
    };
    let mut words = message.split_whitespace();
    if words.next() == Some("HTTP")
        && let Some(status) = words.next()
    {
        return ResolveError::Message(format!(
            "HTTP {status} while fetching Google News article page"
        ));
    }
    ResolveError::Message(message)
}

fn no_publisher_url() -> ResolveError {
    ResolveError::Message("google decode RPC returned no publisher URL".to_owned())
}

fn lock<'a, T>(mutex: &'a Mutex<T>, name: &str) -> Result<MutexGuard<'a, T>, ResolveError> {
    mutex
        .lock()
        .map_err(|error| ResolveError::Message(format!("{name} lock poisoned: {error}")))
}

fn store_result(
    call: &MemoCall,
    result: &Result<String, ResolveError>,
) -> Result<(), ResolveError> {
    *lock(&call.result, "Google News memo call")? = Some(result.clone());
    call.ready.notify_all();
    Ok(())
}

fn wait_for_call(call: &MemoCall, deadline: Instant) -> Result<String, ResolveError> {
    let mut result = lock(&call.result, "Google News memo call")?;
    loop {
        if let Some(ready) = result.as_ref() {
            return ready.clone();
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ResolveError::Timeout);
        }
        let waited = call
            .ready
            .wait_timeout(result, remaining)
            .map_err(|error| {
                ResolveError::Message(format!("Google News memo lock poisoned: {error}"))
            })?;
        result = waited.0;
        if waited.1.timed_out() && result.is_none() {
            return Err(ResolveError::Timeout);
        }
    }
}
