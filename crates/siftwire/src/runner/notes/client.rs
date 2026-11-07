use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use ureq::Agent;

// Tripwire for a stalled Notes request.
const REQUEST_TIMEOUT: Duration = Duration::from_mins(1);
// Notes reads return at most 16,000 characters of text per page.
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

pub struct Note {
    pub hash: String,
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change<'a> {
    pub path: &'a str,
    pub base_hash: Option<&'a str>,
    pub text: &'a str,
}

pub enum Saved {
    Commit(String),
    Conflict(Vec<String>),
    Rejected(Vec<String>),
}

#[derive(Serialize)]
struct ReadRequest<'a> {
    path: &'a str,
    offset: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadPage {
    kind: String,
    hash: String,
    text: String,
    next_offset: Option<usize>,
}

#[derive(Serialize)]
struct SaveRequest<'a> {
    changes: &'a [Change<'a>],
}

#[derive(Deserialize)]
struct SavedPath {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase", tag = "status")]
enum SaveResponse {
    Saved { commit: String },
    Unchanged { commit: String },
    Conflict { conflicts: Vec<SavedPath> },
    Rejected { diagnostics: Vec<String> },
}

/// The Notes API's read and save routes. Errors never include the bearer token.
pub struct Client {
    agent: Agent,
    api_url: String,
    authorization: String,
}

impl Client {
    pub fn new(api_url: String, token: &str) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .user_agent("siftwire/0")
            .proxy(None)
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self {
            agent,
            api_url,
            authorization: format!("Bearer {token}"),
        }
    }

    /// Reads a whole note across pages, or `None` when it does not exist.
    pub fn read_note(&self, path: &str) -> Result<Option<Note>> {
        let mut note: Option<Note> = None;
        let mut offset = 0;
        loop {
            let (status, body) = self.post("/v1/read", &ReadRequest { path, offset })?;
            if status == 404 && note.is_none() {
                return Ok(None);
            }
            if status != 200 {
                bail!("Notes read returned HTTP {status}: {}", error_text(&body));
            }
            let page: ReadPage = serde_json::from_slice(&body).context("decode Notes read")?;
            if page.kind != "note" {
                bail!("{path} is a {} in Notes, not a note", page.kind);
            }
            let note = note.get_or_insert_with(|| Note {
                hash: page.hash.clone(),
                text: String::new(),
            });
            if note.hash != page.hash {
                bail!("{path} changed while it was being read");
            }
            note.text.push_str(&page.text);
            match page.next_offset {
                Some(next) => offset = next,
                None => break,
            }
        }
        Ok(note)
    }

    /// Saves every change in one Notes commit, or none of them.
    pub fn save(&self, changes: &[Change<'_>]) -> Result<Saved> {
        let (status, body) = self.post("/v1/save", &SaveRequest { changes })?;
        if status != 200 {
            bail!("Notes save returned HTTP {status}: {}", error_text(&body));
        }
        let response = serde_json::from_slice(&body).context("decode Notes save")?;
        Ok(match response {
            SaveResponse::Saved { commit } | SaveResponse::Unchanged { commit } => {
                Saved::Commit(commit)
            }
            SaveResponse::Conflict { conflicts } => {
                Saved::Conflict(conflicts.into_iter().map(|file| file.path).collect())
            }
            SaveResponse::Rejected { diagnostics } => Saved::Rejected(diagnostics),
        })
    }

    fn post(&self, route: &str, request: &impl Serialize) -> Result<(u16, Vec<u8>)> {
        let body = serde_json::to_vec(request).context("encode Notes request")?;
        let mut response = self
            .agent
            .post(format!("{}{route}", self.api_url))
            .header("Authorization", &self.authorization)
            .header("Content-Type", "application/json")
            .send(body.as_slice())
            .with_context(|| format!("request Notes {route}"))?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .with_context(|| format!("read Notes {route} response"))?;
        Ok((status, body))
    }
}

fn error_text(body: &[u8]) -> String {
    #[derive(Deserialize)]
    struct Failure {
        error: String,
    }
    serde_json::from_slice::<Failure>(body).map_or_else(
        |_| "unreadable error body".to_owned(),
        |failure| failure.error,
    )
}
