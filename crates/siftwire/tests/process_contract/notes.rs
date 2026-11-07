#![expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::significant_drop_tightening,
    reason = "the fixture counts in memory, holds its lock per request, and panics on bad JSON paths"
)]

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write as _};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::{invoke, invoke_json, text};

const TOKEN: &str = "fixture-notes-token";
const PAGE_CHARACTERS: usize = 16_000;
const INDEX: &str = "technology/daily-briefs.md";

#[derive(Default)]
struct Vault {
    files: BTreeMap<String, String>,
    commits: usize,
    requests: usize,
    fail_next_read: bool,
    drop_next_save_response: bool,
}

/// Enforces the path, compare-and-swap, atomic-save, and paging rules of Notes.
struct FakeNotes {
    url: String,
    vault: Arc<Mutex<Vault>>,
}

impl FakeNotes {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}", listener.local_addr()?);
        let vault = Arc::new(Mutex::new(Vault::default()));
        let shared = Arc::clone(&vault);
        drop(std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if handle(stream, &shared).is_err() {
                    break;
                }
            }
        }));
        Ok(Self { url, vault })
    }

    fn vault(&self) -> MutexGuard<'_, Vault> {
        self.vault.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn file(&self, path: &str) -> Option<String> {
        self.vault().files.get(path).cloned()
    }

    fn manifest(&self) -> Result<Value> {
        let note = self.file(INDEX).context("index note")?;
        let (_, block) = note.split_once("```json\n").context("manifest block")?;
        let (block, _) = block.split_once("\n```").context("manifest close")?;
        Ok(serde_json::from_str(block)?)
    }
}

fn hash_of(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:040x}", hasher.finish())
}

fn lock(vault: &Mutex<Vault>) -> MutexGuard<'_, Vault> {
    vault.lock().unwrap_or_else(PoisonError::into_inner)
}

fn handle(mut stream: TcpStream, vault: &Mutex<Vault>) -> Result<()> {
    let mut received = Vec::new();
    let mut buffer = [0_u8; 8192];
    let (headers, mut body) = loop {
        let count = stream.read(&mut buffer)?;
        received.extend_from_slice(buffer.get(..count).context("read length")?);
        let request = String::from_utf8_lossy(&received);
        if let Some((headers, body)) = request.split_once("\r\n\r\n") {
            break (headers.to_owned(), body.as_bytes().to_vec());
        }
    };
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length: ")?
                .parse()
                .ok()
        })
        .unwrap_or(0_usize);
    while body.len() < length {
        let count = stream.read(&mut buffer)?;
        body.extend_from_slice(buffer.get(..count).context("read length")?);
    }
    lock(vault).requests += 1;
    let (status, response) = if headers.contains(&format!("uthorization: Bearer {TOKEN}\r\n"))
        || headers.ends_with(&format!("uthorization: Bearer {TOKEN}"))
    {
        let request: Value = serde_json::from_slice(&body)?;
        let route = headers.split_whitespace().nth(1).unwrap_or_default();
        match route {
            "/v1/read" => read(vault, &request),
            "/v1/save" => {
                let dropped = std::mem::take(&mut lock(vault).drop_next_save_response);
                let result = save(vault, &request);
                if dropped {
                    return Ok(());
                }
                result
            }
            _ => (404, json!({"error": "Unknown route."})),
        }
    } else {
        (
            401,
            json!({"error": "A valid Notes bearer token is required."}),
        )
    };
    let payload = response.to_string();
    write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    )?;
    Ok(())
}

fn read(vault: &Mutex<Vault>, request: &Value) -> (u16, Value) {
    let mut vault = lock(vault);
    if std::mem::take(&mut vault.fail_next_read) {
        return (503, json!({"error": "fixture outage"}));
    }
    let path = request["path"].as_str().unwrap_or_default();
    let Some(file) = vault.files.get(path) else {
        return (404, json!({"error": "No file."}));
    };
    let offset = request["offset"].as_u64().unwrap_or(0);
    let characters = file.chars().collect::<Vec<_>>();
    let start = usize::try_from(offset).unwrap_or(0);
    let end = characters.len().min(start + PAGE_CHARACTERS);
    let page = characters
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .collect::<String>();
    let mut result = json!({
        "kind": if is_note(path) { "note" } else { "text" },
        "hash": hash_of(file),
        "text": page,
    });
    if end < characters.len() {
        result["nextOffset"] = json!(end);
    }
    (200, result)
}

fn is_note(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension == "md")
}

fn allowed(path: &str) -> bool {
    let page = path
        .strip_prefix("html/")
        .and_then(|rest| rest.strip_suffix("/index.html"))
        .is_some_and(|slug| {
            !slug.is_empty()
                && slug.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        });
    page || (path.starts_with("technology/") && is_note(path))
}

fn save(vault: &Mutex<Vault>, request: &Value) -> (u16, Value) {
    let mut vault = lock(vault);
    let commit = |count: usize| format!("{count:040x}");
    let changes = request["changes"].as_array().cloned().unwrap_or_default();
    let mut writes = Vec::new();
    let mut conflicts = Vec::new();
    for change in &changes {
        let path = change["path"].as_str().unwrap_or_default();
        let text = change["text"].as_str().unwrap_or_default();
        if !allowed(path) {
            return (
                200,
                json!({"status": "rejected", "commit": commit(vault.commits),
                "diagnostics": [format!("{path}: read-only")]}),
            );
        }
        if text.contains("fixture-rejected-page") {
            return (
                200,
                json!({"status": "rejected", "commit": commit(vault.commits),
                "diagnostics": [format!("{path}: rule failed")]}),
            );
        }
        let current = vault.files.get(path).map(|file| hash_of(file));
        if current.as_deref() == Some(hash_of(text).as_str()) {
            continue;
        }
        if current.as_deref() == change["baseHash"].as_str() {
            writes.push((path.to_owned(), text.to_owned()));
        } else {
            conflicts.push(json!({"path": path, "hash": current}));
        }
    }
    if !conflicts.is_empty() {
        return (
            200,
            json!({"status": "conflict", "commit": commit(vault.commits), "conflicts": conflicts}),
        );
    }
    if writes.is_empty() {
        return (
            200,
            json!({"status": "unchanged", "commit": commit(vault.commits), "files": []}),
        );
    }
    vault.files.extend(writes);
    vault.commits += 1;
    (
        200,
        json!({"status": "saved", "commit": commit(vault.commits), "files": []}),
    )
}

struct Fixture {
    temp: TempDir,
    database: String,
    token: PathBuf,
    notes: FakeNotes,
}

impl Fixture {
    fn start() -> Result<Self> {
        let temp = TempDir::new()?;
        let database = temp.path().join("notes.sqlite").display().to_string();
        let token = temp.path().join("token");
        fs::write(&token, format!("{TOKEN}\n"))?;
        fs::set_permissions(&token, fs::Permissions::from_mode(0o600))?;
        let fixture = Self {
            temp,
            database,
            token,
            notes: FakeNotes::start()?,
        };
        let feed = url::Url::from_file_path(fixture.feed())
            .map_err(|()| anyhow::anyhow!("feed path is not absolute"))?;
        let upsert = json!({"action": "upsert_source", "source": {
            "key": "fixture", "label": "Fixture", "kind": "rss", "url": feed,
            "section": "technology", "threshold": "high", "enabled": true}});
        fixture.write_feed("seed")?;
        invoke_json(
            &["config", "--db", &fixture.database],
            &upsert,
            &Self::environment(),
        )?;
        Ok(fixture)
    }

    fn feed(&self) -> PathBuf {
        self.temp.path().join("feed.xml")
    }

    fn environment() -> BTreeMap<&'static str, &'static str> {
        BTreeMap::from([("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")])
    }

    fn write_feed(&self, story: &str) -> Result<()> {
        fs::write(
            self.feed(),
            format!(
                r#"<?xml version="1.0"?><rss version="2.0"><channel><title>F</title><item><title>Story {story}</title><link>https://example.test/{story}</link><guid>{story}</guid><pubDate>{}</pubDate></item></channel></rss>"#,
                chrono::Utc::now().to_rfc2822()
            ),
        )?;
        Ok(())
    }

    /// Prepares one real plan, then moves its run to a fixed start time.
    fn brief(&self, story: &str, started_at: &str) -> Result<(String, String)> {
        self.write_feed(story)?;
        let arguments = ["brief", "--db", self.database.as_str()];
        let run = invoke_json(
            &arguments,
            &json!({"action": "run_brief", "dry_run": false}),
            &Self::environment(),
        )?;
        let run_id = run["run_id"].as_str().context("run id")?.to_owned();
        let prepare =
            json!({"action": "prepare_delivery", "run_id": run_id, "candidate_indexes": [0]});
        let prepared = invoke_json(&arguments, &prepare, &Self::environment())?;
        let html = prepared["html"]
            .as_str()
            .context("prepared html")?
            .to_owned();
        rusqlite::Connection::open(&self.database)?.execute(
            "UPDATE brief_run SET started_at = ?1 WHERE id = ?2",
            rusqlite::params![started_at, run_id],
        )?;
        Ok((run_id, html))
    }

    fn request(&self, start_date: &str) -> String {
        json!({
            "api_url": self.notes.url,
            "public_url": "https://notes.test/",
            "token_file": self.token,
            "start_date": start_date,
        })
        .to_string()
    }

    fn publish_with(&self, request: &str) -> Result<Output> {
        invoke(
            &["notes", "publish", "--db", &self.database],
            Some(request),
            &BTreeMap::new(),
        )
    }

    fn publish(&self, start_date: &str) -> Result<(Output, Value)> {
        let output = self.publish_with(&self.request(start_date))?;
        let report = serde_json::from_slice(&output.stdout)
            .with_context(|| format!("publication report: {output:?}"))?;
        Ok((output, report))
    }
}

fn sha256(text: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let mut hex = String::new();
    for byte in digest.as_ref() {
        let _ignored = write!(hex, "{byte:02x}");
    }
    hex
}

#[test]
fn notes_publish_lands_one_commit_and_replays_to_nothing() -> Result<()> {
    let fixture = Fixture::start()?;
    let started_at = "2026-11-07T01:00:50.822363807Z";
    let (run_id, html) = fixture.brief("evening", started_at)?;

    let (output, report) = fixture.publish("2026-11-06")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        report["published"][0]["edition_id"], "brief-2026-11-06-pm",
        "{report}"
    );
    assert_eq!(report["pending"], json!([]));
    assert_eq!(
        fixture.notes.vault().commits,
        1,
        "HTML and index share a commit"
    );
    assert_eq!(
        fixture
            .notes
            .file("html/brief-2026-11-06-pm/index.html")
            .as_deref(),
        Some(html.as_str()),
        "the page is the email body, byte for byte"
    );

    let manifest = fixture.notes.manifest()?;
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["timezone"], "America/Chicago");
    let edition = &manifest["editions"][0];
    assert_eq!(edition["date"], "2026-11-06");
    assert_eq!(edition["slot"], "pm");
    assert_eq!(edition["editionId"], "brief-2026-11-06-pm");
    assert_eq!(edition["producerRunId"], run_id.as_str());
    assert_eq!(edition["researchAsOf"], started_at);
    assert!(
        edition["generatedAt"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(edition["htmlPath"], "html/brief-2026-11-06-pm/index.html");
    assert_eq!(
        edition["url"],
        "https://notes.test/pages/brief-2026-11-06-pm/"
    );
    assert_eq!(edition["htmlSha256"], sha256(&html).as_str());
    assert!(
        manifest["slots"]
            .as_array()
            .context("slots")?
            .contains(&json!({"date": "2026-11-06", "slot": "am", "status": "missing"})),
        "{manifest}"
    );

    let (output, report) = fixture.publish("2026-11-06")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(report["already_published"], 1, "{report}");
    assert_eq!(report["published"], json!([]));
    assert_eq!(fixture.notes.vault().commits, 1, "a replay writes nothing");
    Ok(())
}

#[test]
fn notes_failures_stay_pending_and_a_retry_converges() -> Result<()> {
    let fixture = Fixture::start()?;
    fixture.brief("first", "2026-11-07T13:00:00Z")?;

    fixture.notes.vault().fail_next_read = true;
    let output = fixture.publish_with(&fixture.request("2026-11-07"))?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        output.stdout.is_empty(),
        "an unread index proves nothing: {output:?}"
    );
    assert!(
        text(&output.stderr).contains("fixture outage"),
        "{output:?}"
    );
    assert_eq!(fixture.notes.vault().commits, 0);

    let (output, report) = fixture.publish("2026-11-07")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(report["published"][0]["edition_id"], "brief-2026-11-07-am");

    let (second, _) = fixture.brief("second", "2026-11-08T01:00:00Z")?;
    fixture.notes.vault().drop_next_save_response = true;
    let (output, report) = fixture.publish("2026-11-07")?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(report["pending"], json!([second]), "{report}");
    assert_eq!(
        fixture.notes.vault().commits,
        2,
        "the lost save still landed"
    );

    let (output, report) = fixture.publish("2026-11-07")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(report["already_published"], 2, "{report}");
    assert_eq!(
        fixture.notes.vault().commits,
        2,
        "the retry adds no commit or revision"
    );
    Ok(())
}

#[test]
fn notes_corrections_take_a_new_path_and_never_replace_content() -> Result<()> {
    let fixture = Fixture::start()?;
    let (_, original) = fixture.brief("original", "2026-11-07T13:00:00Z")?;
    assert!(fixture.publish("2026-11-07")?.0.status.success());

    let (_, correction) = fixture.brief("correction", "2026-11-07T13:30:00Z")?;
    let (output, report) = fixture.publish("2026-11-07")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        report["published"][0]["edition_id"], "brief-2026-11-07-am-r2",
        "{report}"
    );
    assert_eq!(
        fixture.notes.file("html/brief-2026-11-07-am/index.html"),
        Some(original)
    );
    assert_eq!(
        fixture.notes.file("html/brief-2026-11-07-am-r2/index.html"),
        Some(correction)
    );

    let foreign = "<p>placed by hand</p>".to_owned();
    fixture.notes.vault().files.insert(
        "html/brief-2026-11-08-am/index.html".to_owned(),
        foreign.clone(),
    );
    let (_, next) = fixture.brief("next", "2026-11-08T13:00:00Z")?;
    let (output, report) = fixture.publish("2026-11-07")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        report["published"][0]["edition_id"], "brief-2026-11-08-am-r2",
        "{report}"
    );
    assert_eq!(
        fixture.notes.file("html/brief-2026-11-08-am/index.html"),
        Some(foreign)
    );
    assert_eq!(
        fixture.notes.file("html/brief-2026-11-08-am-r2/index.html"),
        Some(next)
    );
    Ok(())
}

#[test]
fn notes_publish_honors_the_start_date_and_keeps_a_damaged_index() -> Result<()> {
    let fixture = Fixture::start()?;
    // This run's UTC date is the start date, but its reporting date is the day before.
    fixture.brief("before", "2026-11-07T01:00:00Z")?;
    let (output, report) = fixture.publish("2026-11-07")?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(report["published"], json!([]), "{report}");
    assert_eq!(
        fixture.notes.vault().requests,
        0,
        "nothing to publish, nothing to send"
    );

    fixture.brief("after", "2026-11-07T13:00:00Z")?;
    fixture
        .notes
        .vault()
        .files
        .insert(INDEX.to_owned(), "hand-written\n".to_owned());
    let output = fixture.publish_with(&fixture.request("2026-11-07"))?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        output.stdout.is_empty(),
        "an unread index proves nothing: {output:?}"
    );
    assert!(
        text(&output.stderr).contains("no JSON manifest"),
        "{output:?}"
    );
    assert_eq!(fixture.notes.file(INDEX).as_deref(), Some("hand-written\n"));
    assert_eq!(fixture.notes.vault().commits, 0);
    Ok(())
}

#[test]
fn notes_publish_refuses_unsafe_settings_without_exposing_the_token() -> Result<()> {
    let fixture = Fixture::start()?;
    fixture.brief("story", "2026-11-07T13:00:00Z")?;
    let refused = |request: &str, expected: &str| -> Result<()> {
        let output = fixture.publish_with(request)?;
        let diagnostic = text(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{expected}: {output:?}");
        assert!(output.stdout.is_empty(), "{expected}: {output:?}");
        assert!(diagnostic.contains(expected), "{expected}: {diagnostic}");
        assert!(!diagnostic.contains(TOKEN), "token leaked: {diagnostic}");
        assert_eq!(fixture.notes.vault().requests, 0, "{expected}");
        Ok(())
    };
    let mut request: Value = serde_json::from_str(&fixture.request("2026-11-07"))?;

    request["api_url"] = json!("http://notes.test");
    refused(&request.to_string(), "https or a loopback")?;
    request["api_url"] = json!(fixture.notes.url.replace("http://", "http://user:secret@"));
    refused(&request.to_string(), "bare http or https origin")?;
    request["api_url"] = json!(fixture.notes.url);

    request["start_date"] = json!("yesterday");
    refused(&request.to_string(), "start_date")?;
    request["start_date"] = json!("2026-11-07");

    fs::set_permissions(&fixture.token, fs::Permissions::from_mode(0o640))?;
    refused(&request.to_string(), "mode 600")?;
    fs::set_permissions(&fixture.token, fs::Permissions::from_mode(0o600))?;

    let link = fixture.temp.path().join("token-link");
    symlink(&fixture.token, &link)?;
    request["token_file"] = json!(link);
    refused(&request.to_string(), "regular file")?;

    let two_lines: &Path = &fixture.temp.path().join("two-lines");
    fs::write(two_lines, format!("{TOKEN}\nextra\n"))?;
    fs::set_permissions(two_lines, fs::Permissions::from_mode(0o600))?;
    request["token_file"] = json!(two_lines);
    refused(&request.to_string(), "one token on one line")?;
    Ok(())
}

#[test]
fn notes_publish_rejections_and_wrong_tokens_stay_visible() -> Result<()> {
    let fixture = Fixture::start()?;
    fixture.brief("story", "2026-11-07T13:00:00Z")?;

    fs::write(&fixture.token, "wrong-token\n")?;
    let output = fixture.publish_with(&fixture.request("2026-11-07"))?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        output.stdout.is_empty(),
        "an unread index proves nothing: {output:?}"
    );
    assert!(text(&output.stderr).contains("HTTP 401"), "{output:?}");
    fs::write(&fixture.token, format!("{TOKEN}\n"))?;

    rusqlite::Connection::open(&fixture.database)?.execute(
        "UPDATE delivery_plan SET html_body = 'fixture-rejected-page'",
        [],
    )?;
    let (output, report) = fixture.publish("2026-11-07")?;
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        report["error"]
            .as_str()
            .is_some_and(|error| error.contains("rule failed")),
        "{report}"
    );
    assert_eq!(fixture.notes.vault().commits, 0);
    Ok(())
}

#[test]
fn notes_reads_a_manifest_longer_than_one_api_page() -> Result<()> {
    let fixture = Fixture::start()?;
    for (index, day) in (1..=30).enumerate() {
        fixture.brief(
            &format!("story-{index}"),
            &format!("2026-10-{day:02}T13:00:00Z"),
        )?;
    }
    assert!(fixture.publish("2026-10-01")?.0.status.success());
    let note_length = fixture.notes.file(INDEX).context("index")?.chars().count();
    assert!(
        note_length > PAGE_CHARACTERS,
        "fixture must span pages: {note_length}"
    );
    assert_eq!(
        fixture.notes.manifest()?["editions"]
            .as_array()
            .map(Vec::len),
        Some(30)
    );

    let requests = fixture.notes.vault().requests;
    let (_, report) = fixture.publish("2026-10-01")?;
    assert_eq!(report["already_published"], 30, "{report}");
    assert_eq!(
        fixture.notes.vault().requests - requests,
        note_length.div_ceil(PAGE_CHARACTERS),
        "a run reads the index once, however many editions exist"
    );
    Ok(())
}
