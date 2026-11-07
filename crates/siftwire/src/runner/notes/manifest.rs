use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, SecondsFormat, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

pub const NOTE_PATH: &str = "technology/daily-briefs.md";
const TIMEZONE: Tz = chrono_tz::America::Chicago;
const SCHEMA_VERSION: u64 = 1;
// Notes rejects a section over 400 words.
const RECENT_DATES: usize = 14;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Slot {
    Am,
    Pm,
}

impl Slot {
    const fn label(self) -> &'static str {
        match self {
            Self::Am => "am",
            Self::Pm => "pm",
        }
    }
}

/// Maps a brief's start to its reporting date, matching the email's morning and evening cutoff.
#[must_use]
pub fn reporting_slot(started_at: DateTime<Utc>) -> (NaiveDate, Slot) {
    let local = started_at.with_timezone(&TIMEZONE);
    let slot = if local.hour() < 12 {
        Slot::Am
    } else {
        Slot::Pm
    };
    (local.date_naive(), slot)
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Edition {
    pub date: NaiveDate,
    pub slot: Slot,
    pub revision: u32,
    #[serde(rename = "editionId")]
    pub id: String,
    pub producer_run_id: String,
    pub generated_at: String,
    pub research_as_of: String,
    pub html_path: String,
    pub url: String,
    pub html_sha256: String,
}

impl Edition {
    #[must_use]
    pub fn id_for(date: NaiveDate, slot: Slot, revision: u32) -> String {
        let base = format!("brief-{date}-{}", slot.label());
        if revision > 1 {
            format!("{base}-r{revision}")
        } else {
            base
        }
    }

    fn link_label(&self) -> String {
        if self.revision > 1 {
            format!("{} r{}", self.slot.label(), self.revision)
        } else {
            self.slot.label().to_owned()
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SlotStatus {
    Missing,
    NotReady,
}

impl SlotStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::NotReady => "not ready",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OpenSlot {
    date: NaiveDate,
    slot: Slot,
    status: SlotStatus,
}

/// The index note's JSON block. Its editions are the record of what is published.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Manifest {
    schema_version: u64,
    timezone: String,
    updated_at: String,
    editions: Vec<Edition>,
    slots: Vec<OpenSlot>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            timezone: TIMEZONE.name().to_owned(),
            updated_at: String::new(),
            editions: Vec::new(),
            slots: Vec::new(),
        }
    }
}

impl Manifest {
    /// Reads the JSON block of an index note.
    ///
    /// # Errors
    ///
    /// Returns an error when the note has no readable block of this schema version.
    pub fn parse_note(text: &str) -> Result<Self> {
        let (_, block) = text
            .split_once("```json\n")
            .context("index note has no JSON manifest block")?;
        let (block, _) = block
            .split_once("\n```")
            .context("index note manifest block is not closed")?;
        let value: serde_json::Value =
            serde_json::from_str(block).context("index note manifest is not JSON")?;
        let version = value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64);
        if version != Some(SCHEMA_VERSION) {
            bail!("index note manifest has unsupported schemaVersion {version:?}");
        }
        let manifest: Self =
            serde_json::from_value(value).context("index note manifest has an invalid shape")?;
        if manifest.timezone != TIMEZONE.name() {
            bail!("index note manifest uses timezone {}", manifest.timezone);
        }
        Ok(manifest)
    }

    #[must_use]
    pub fn run_ids(&self) -> BTreeSet<&str> {
        self.editions
            .iter()
            .map(|edition| edition.producer_run_id.as_str())
            .collect()
    }

    #[must_use]
    pub fn edition_for_run(&self, run_id: &str) -> Option<&Edition> {
        self.editions
            .iter()
            .find(|edition| edition.producer_run_id == run_id)
    }

    #[must_use]
    pub fn next_revision(&self, date: NaiveDate, slot: Slot) -> u32 {
        self.editions
            .iter()
            .filter(|edition| edition.date == date && edition.slot == slot)
            .map(|edition| edition.revision)
            .max()
            .unwrap_or_default()
            .saturating_add(1)
    }

    /// Adds an edition and recomputes every open slot as of `now`.
    #[must_use]
    pub fn with_edition(&self, edition: Edition, now: DateTime<Utc>) -> Self {
        let mut editions = self.editions.clone();
        editions.push(edition);
        editions.sort_by_key(|edition| {
            std::cmp::Reverse((edition.date, edition.slot, edition.revision))
        });
        Self {
            slots: open_slots(&editions, now),
            updated_at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
            editions,
            ..Self::default()
        }
    }

    /// Renders the whole index note.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest cannot be encoded.
    pub fn render_note(&self) -> Result<String> {
        let manifest = serde_json::to_string_pretty(self).context("encode index manifest")?;
        Ok(format!(
            "---\nid: \"technology/daily-briefs\"\ntitle: \"Daily briefs\"\n---\n\n# Daily briefs\n\n\
             SiftWire writes this note each time it publishes an edition, so manual edits do not last.\n\n\
             Dates are {timezone} reporting dates. This note has no latest link: check the date before \
             you treat an edition as current. A date that is absent has no published edition, and the \
             slots below describe the state at {updated_at}.\n\n\
             ## Recent editions\n\n\
             The last {RECENT_DATES} dates are below. The manifest lists every edition.\n\n\
             {links}\n\n## Manifest\n\n```json\n{manifest}\n```\n",
            timezone = self.timezone,
            updated_at = self.updated_at,
            links = self.recent_links(),
        ))
    }

    fn recent_links(&self) -> String {
        let dates = self
            .editions
            .iter()
            .map(|edition| edition.date)
            .chain(self.slots.iter().map(|slot| slot.date))
            .collect::<BTreeSet<_>>();
        let mut lines = String::new();
        for date in dates.into_iter().rev().take(RECENT_DATES) {
            let parts = [Slot::Am, Slot::Pm]
                .into_iter()
                .filter_map(|slot| self.slot_summary(date, slot))
                .collect::<Vec<_>>();
            let separator = if lines.is_empty() { "" } else { "\n" };
            let _ignored = write!(lines, "{separator}- {date}: {}", parts.join(", "));
        }
        lines
    }

    fn slot_summary(&self, date: NaiveDate, slot: Slot) -> Option<String> {
        let latest = self
            .editions
            .iter()
            .filter(|edition| edition.date == date && edition.slot == slot)
            .max_by_key(|edition| edition.revision);
        latest
            .map(|edition| format!("[{}]({})", edition.link_label(), edition.url))
            .or_else(|| {
                self.slots
                    .iter()
                    .find(|open| open.date == date && open.slot == slot)
                    .map(|open| format!("{} {}", slot.label(), open.status.label()))
            })
    }
}

fn open_slots(editions: &[Edition], now: DateTime<Utc>) -> Vec<OpenSlot> {
    let local = now.with_timezone(&TIMEZONE);
    let today = local.date_naive();
    let filled = editions
        .iter()
        .map(|edition| (edition.date, edition.slot))
        .collect::<BTreeSet<_>>();
    let Some(first) = filled.iter().map(|(date, _)| *date).min() else {
        return Vec::new();
    };
    let mut slots = Vec::new();
    for date in first.iter_days().take_while(|date| *date <= today) {
        for slot in [Slot::Am, Slot::Pm] {
            if filled.contains(&(date, slot)) {
                continue;
            }
            let open = date == today && (slot == Slot::Pm || local.hour() < 12);
            slots.push(OpenSlot {
                date,
                slot,
                status: if open {
                    SlotStatus::NotReady
                } else {
                    SlotStatus::Missing
                },
            });
        }
    }
    slots.reverse();
    slots
}

#[cfg(test)]
#[expect(
    clippy::panic_in_result_fn,
    reason = "assertions report manifest contract failures"
)]
mod tests {
    use anyhow::{Context, Result};
    use chrono::{DateTime, NaiveDate, Utc};

    use super::{Edition, Manifest, Slot, SlotStatus, reporting_slot};

    fn at(value: &str) -> Result<DateTime<Utc>> {
        Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
    }

    fn date(value: &str) -> Result<NaiveDate> {
        Ok(value.parse()?)
    }

    fn edition(day: &str, slot: Slot, revision: u32) -> Result<Edition> {
        let date = date(day)?;
        let id = Edition::id_for(date, slot, revision);
        Ok(Edition {
            date,
            slot,
            revision,
            producer_run_id: format!("run-{id}"),
            generated_at: "2026-11-07T13:00:32Z".to_owned(),
            research_as_of: "2026-11-07T13:00:24Z".to_owned(),
            html_path: format!("html/{id}/index.html"),
            url: format!("https://notes.test/pages/{id}/"),
            html_sha256: "0".repeat(64),
            id,
        })
    }

    #[test]
    fn reporting_dates_follow_chicago_across_daylight_saving() -> Result<()> {
        for (instant, day, slot) in [
            // 19:00 CST on the 6th is already the 7th in UTC.
            ("2026-11-07T01:00:50Z", "2026-11-06", Slot::Pm),
            ("2026-11-07T13:00:24Z", "2026-11-07", Slot::Am),
            // Daylight time ended on November 1.
            ("2026-10-31T12:00:00Z", "2026-10-31", Slot::Am),
            ("2026-10-31T17:00:00Z", "2026-10-31", Slot::Pm),
            ("2026-11-01T17:59:59Z", "2026-11-01", Slot::Am),
            ("2026-11-01T18:00:00Z", "2026-11-01", Slot::Pm),
        ] {
            assert_eq!(
                reporting_slot(at(instant)?),
                (date(day)?, slot),
                "{instant}"
            );
        }
        Ok(())
    }

    #[test]
    fn edition_ids_add_a_revision_only_for_corrections() -> Result<()> {
        let day = date("2026-11-07")?;
        assert_eq!(Edition::id_for(day, Slot::Am, 1), "brief-2026-11-07-am");
        assert_eq!(Edition::id_for(day, Slot::Pm, 2), "brief-2026-11-07-pm-r2");
        Ok(())
    }

    #[test]
    fn open_slots_say_missing_or_not_ready_and_fill_gaps() -> Result<()> {
        let manifest = Manifest::default()
            .with_edition(
                edition("2026-11-04", Slot::Am, 1)?,
                at("2026-11-04T13:00:00Z")?,
            )
            .with_edition(
                edition("2026-11-07", Slot::Pm, 1)?,
                at("2026-11-08T01:00:00Z")?,
            );
        let status = |day: &str, slot: Slot| -> Result<Option<SlotStatus>> {
            let day = date(day)?;
            Ok(manifest
                .slots
                .iter()
                .find(|open| open.date == day && open.slot == slot)
                .map(|open| open.status))
        };
        assert_eq!(status("2026-11-04", Slot::Am)?, None);
        assert_eq!(status("2026-11-04", Slot::Pm)?, Some(SlotStatus::Missing));
        assert_eq!(status("2026-11-05", Slot::Am)?, Some(SlotStatus::Missing));
        assert_eq!(status("2026-11-07", Slot::Am)?, Some(SlotStatus::Missing));
        assert_eq!(status("2026-11-07", Slot::Pm)?, None);

        let morning = Manifest::default().with_edition(
            edition("2026-11-07", Slot::Am, 1)?,
            at("2026-11-07T13:01:00Z")?,
        );
        assert_eq!(morning.slots.len(), 1, "only pm stays open");
        assert_eq!(
            morning.slots.first().map(|open| open.status),
            Some(SlotStatus::NotReady)
        );

        // A retried evening edition must not call the current morning missing.
        let retried = Manifest::default().with_edition(
            edition("2026-11-06", Slot::Pm, 1)?,
            at("2026-11-07T13:01:00Z")?,
        );
        let current_morning = retried
            .slots
            .iter()
            .find(|open| open.date == date("2026-11-07").unwrap_or_default());
        assert_eq!(
            current_morning.map(|open| open.status),
            Some(SlotStatus::NotReady)
        );
        Ok(())
    }

    #[test]
    fn note_round_trips_and_never_labels_an_edition_current() -> Result<()> {
        let manifest = Manifest::default()
            .with_edition(
                edition("2026-11-06", Slot::Pm, 1)?,
                at("2026-11-07T01:01:00Z")?,
            )
            .with_edition(
                edition("2026-11-06", Slot::Pm, 2)?,
                at("2026-11-07T02:00:00Z")?,
            );
        let note = manifest.render_note()?;
        assert_eq!(Manifest::parse_note(&note)?, manifest);
        assert_eq!(
            manifest.next_revision(date("2026-11-06")?, Slot::Pm),
            3,
            "revisions count up"
        );
        assert!(
            note.contains(
                "- 2026-11-06: am missing, [pm r2](https://notes.test/pages/brief-2026-11-06-pm-r2/)"
            ),
            "{note}"
        );
        assert!(!note.to_lowercase().contains("today"), "{note}");
        Ok(())
    }

    #[test]
    fn links_stay_bounded_while_the_manifest_keeps_every_edition() -> Result<()> {
        let mut manifest = Manifest::default();
        let first = date("2026-01-01")?;
        for day in first.iter_days().take(40) {
            manifest = manifest.with_edition(
                edition(&day.to_string(), Slot::Am, 1)?,
                at(&format!("{day}T23:00:00Z"))?,
            );
        }
        let links = manifest.recent_links();
        assert_eq!(links.lines().count(), 14, "{links}");
        assert!(links.starts_with("- 2026-02-09:"), "{links}");
        assert_eq!(manifest.editions.len(), 40);
        Ok(())
    }

    #[test]
    fn unknown_or_damaged_manifests_are_refused() -> Result<()> {
        let note = Manifest::default().render_note()?;
        for (name, text) in [
            ("no block", "plain note".to_owned()),
            ("not JSON", note.replace("\"schemaVersion\": 1", "oops")),
            (
                "newer",
                note.replace("\"schemaVersion\": 1", "\"schemaVersion\": 2"),
            ),
            ("timezone", note.replace("America/Chicago\",\n", "UTC\",\n")),
            ("unknown field", note.replace("\"editions\"", "\"other\"")),
        ] {
            assert!(Manifest::parse_note(&text).is_err(), "{name}");
        }
        let empty = Manifest::parse_note(&note).context("empty manifest round trip")?;
        assert_eq!(empty, Manifest::default());
        Ok(())
    }
}
