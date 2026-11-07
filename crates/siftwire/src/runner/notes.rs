mod client;
mod manifest;

use std::fmt::Write as _;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::ExitCode;

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use self::client::{Change, Client, Note, Saved};
use self::manifest::{Edition, Manifest, NOTE_PATH, Slot, reporting_slot};
use super::{
    EXIT_FAILURE, EXIT_OK, EXIT_USAGE_CODE, decode_request, open_store, split_database_flag,
    write_result,
};
use crate::storage::{PublishablePlan, Store};

// Matches the Notes authority's own limit for a save that loses races.
const MAX_SAVE_ATTEMPTS: u32 = 5;
const SCHEMA_VERSION: &str = "siftwire-notes-publication/v1";
const USAGE: &str = "usage: siftwire notes publish [--db path] < request.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    api_url: String,
    public_url: String,
    token_file: String,
    start_date: String,
}

#[derive(Serialize)]
struct Receipt {
    edition_id: String,
    run_id: String,
    url: String,
    html_sha256: String,
    notes_commit: String,
}

#[derive(Serialize)]
struct Report {
    schema_version: &'static str,
    published: Vec<Receipt>,
    already_published: usize,
    pending: Vec<String>,
    error: String,
}

struct Target {
    public_url: String,
    start_date: NaiveDate,
}

struct Brief {
    plan: PublishablePlan,
    date: NaiveDate,
    slot: Slot,
    started_at: DateTime<Utc>,
}

pub fn run(arguments: &[String]) -> ExitCode {
    let (database, rest) = match split_database_flag(arguments) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_USAGE_CODE);
        }
    };
    if rest != ["publish"] {
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE_CODE);
    }
    let report = decode_request::<Request>()
        .context("decode notes request")
        .and_then(|request| {
            let (_, store) = open_store(database.as_deref())?;
            publish(&store, &request)
        });
    match report {
        Ok(report) => {
            let finished = report.pending.is_empty();
            let written = write_result(&report, "notes");
            if written == EXIT_OK && !finished {
                EXIT_FAILURE
            } else {
                written
            }
        }
        Err(error) => {
            eprintln!("publish to Notes: {error:#}");
            EXIT_FAILURE
        }
    }
}

fn publish(store: &Store, request: &Request) -> Result<Report> {
    let api_url = base_url("api_url", &request.api_url)?;
    ensure_private_transport(&api_url)?;
    let target = Target {
        public_url: base_url("public_url", &request.public_url)?
            .as_str()
            .trim_end_matches('/')
            .to_owned(),
        start_date: NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
            .context("start_date must be YYYY-MM-DD")?,
    };
    let token = read_token(&request.token_file)?;
    let client = Client::new(api_url.as_str().trim_end_matches('/').to_owned(), &token);
    let briefs = briefs_to_publish(store, target.start_date)?;
    let mut report = Report {
        schema_version: SCHEMA_VERSION,
        published: Vec::new(),
        already_published: 0,
        pending: Vec::new(),
        error: String::new(),
    };
    if briefs.is_empty() {
        return Ok(report);
    }
    // One read settles every published run, so only unpublished briefs read again.
    let (_, manifest) = read_index(&client)?;
    let published = manifest.run_ids();
    let (known, unpublished): (Vec<_>, Vec<_>) = briefs
        .into_iter()
        .partition(|brief| published.contains(brief.plan.run_id.as_str()));
    report.already_published = known.len();
    let mut unpublished = unpublished.into_iter();
    for brief in unpublished.by_ref() {
        match publish_brief(&client, &target, &brief) {
            Ok(Some(receipt)) => report.published.push(receipt),
            Ok(None) => report.already_published = report.already_published.saturating_add(1),
            Err(error) => {
                report.error = format!("{}: {error:#}", brief.plan.run_id);
                report.pending.push(brief.plan.run_id);
                break;
            }
        }
    }
    report
        .pending
        .extend(unpublished.map(|brief| brief.plan.run_id));
    Ok(report)
}

fn read_index(client: &Client) -> Result<(Option<Note>, Manifest)> {
    let note = client.read_note(NOTE_PATH)?;
    let manifest = note.as_ref().map_or_else(
        || Ok(Manifest::default()),
        |note| Manifest::parse_note(&note.text),
    )?;
    Ok((note, manifest))
}

fn briefs_to_publish(store: &Store, start_date: NaiveDate) -> Result<Vec<Brief>> {
    let mut briefs = Vec::new();
    for plan in store.publishable_plans(&start_date.to_string())? {
        let started_at = DateTime::parse_from_rfc3339(&plan.started_at)
            .with_context(|| format!("run {} has an invalid start time", plan.run_id))?
            .with_timezone(&Utc);
        let (date, slot) = reporting_slot(started_at);
        if date >= start_date {
            briefs.push(Brief {
                plan,
                date,
                slot,
                started_at,
            });
        }
    }
    briefs.sort_by(|left, right| {
        (left.started_at, &left.plan.run_id).cmp(&(right.started_at, &right.plan.run_id))
    });
    Ok(briefs)
}

// The manifest decides identity, so a retry after a lost response converges.
fn publish_brief(client: &Client, target: &Target, brief: &Brief) -> Result<Option<Receipt>> {
    let html_sha256 = sha256_hex(&brief.plan.html);
    let mut occupied = 0_u32;
    for _ in 0..MAX_SAVE_ATTEMPTS {
        let (note, manifest) = read_index(client)?;
        if let Some(edition) = manifest.edition_for_run(&brief.plan.run_id) {
            ensure!(
                edition.html_sha256 == html_sha256,
                "{} holds different HTML from this run's plan",
                edition.id
            );
            return Ok(None);
        }
        let revision = manifest
            .next_revision(brief.date, brief.slot)
            .saturating_add(occupied);
        let edition = edition(target, brief, revision, &html_sha256);
        let text = manifest
            .with_edition(edition.clone(), Utc::now())
            .render_note()?;
        let changes = [
            Change {
                path: &edition.html_path,
                base_hash: None,
                text: &brief.plan.html,
            },
            Change {
                path: NOTE_PATH,
                base_hash: note.as_ref().map(|note| note.hash.as_str()),
                text: &text,
            },
        ];
        match client.save(&changes)? {
            Saved::Commit(notes_commit) => {
                return Ok(Some(Receipt {
                    edition_id: edition.id,
                    run_id: brief.plan.run_id.clone(),
                    url: edition.url,
                    html_sha256,
                    notes_commit,
                }));
            }
            Saved::Conflict(paths) => {
                if paths.contains(&edition.html_path) {
                    occupied = occupied.saturating_add(1);
                }
            }
            Saved::Rejected(diagnostics) => {
                bail!("Notes rejected the save: {}", diagnostics.join("; "));
            }
        }
    }
    bail!("Notes kept conflicting after {MAX_SAVE_ATTEMPTS} saves")
}

fn edition(target: &Target, brief: &Brief, revision: u32, html_sha256: &str) -> Edition {
    let edition_id = Edition::id_for(brief.date, brief.slot, revision);
    Edition {
        date: brief.date,
        slot: brief.slot,
        revision,
        producer_run_id: brief.plan.run_id.clone(),
        generated_at: brief.plan.finished_at.clone(),
        research_as_of: brief.plan.started_at.clone(),
        html_path: format!("html/{edition_id}/index.html"),
        url: format!("{}/pages/{edition_id}/", target.public_url),
        html_sha256: html_sha256.to_owned(),
        id: edition_id,
    }
}

fn base_url(name: &str, value: &str) -> Result<Url> {
    let url = Url::parse(value).with_context(|| format!("{name} is not a URL"))?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "{name} must be a bare http or https origin"
    );
    Ok(url)
}

// A bearer token crosses plain HTTP only on this machine.
fn ensure_private_transport(url: &Url) -> Result<()> {
    let loopback = match url.host() {
        Some(Host::Domain(domain)) => domain == "localhost",
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    ensure!(
        url.scheme() == "https" || loopback,
        "api_url must use https or a loopback address"
    );
    Ok(())
}

fn read_token(path: &str) -> Result<String> {
    let metadata = fs::symlink_metadata(path).context("inspect Notes token file")?;
    ensure!(
        metadata.is_file(),
        "Notes token file must be a regular file"
    );
    #[cfg(unix)]
    ensure!(
        metadata.permissions().mode() & 0o777 == 0o600,
        "Notes token file must have mode 600"
    );
    let contents = fs::read_to_string(path).context("read Notes token file")?;
    let token = contents.strip_suffix('\n').unwrap_or(&contents);
    ensure!(
        !token.is_empty() && token.chars().all(|character| character.is_ascii_graphic()),
        "Notes token file must hold one token on one line"
    );
    Ok(token.to_owned())
}

fn sha256_hex(text: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let mut hex = String::new();
    for byte in digest.as_ref() {
        let _ignored = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    #[test]
    fn sha256_matches_the_published_test_vector() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
