use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration, Utc};
use chrono_tz::America::Chicago;
use serde::Deserialize;

use crate::domain::{
    SCHEDULE_FORMAT_ESPN, SCHEDULE_FORMAT_ESPN_CORE, SCHEDULE_FORMAT_RIOT, Source,
};

use super::http::HttpClient;
use super::model::{FetchOutput, FetchedItem};

// Receipt: ADR `schedule-source-adr.md`. Briefs run every 12 hours; a fixture
// appears in every run that starts within this window before kickoff.
const FIXTURE_WINDOW: Duration = Duration::hours(24);
// Receipt: ESPN's edge (Akamai) answers 403 for unrecognized user agents while
// allowing recognized scripted-client agents; probed 2026-08-24, "siftwire/0.4"
// -> 403, "curl/8.5.0" and "python-requests/2.31.0" -> 200. The runner is a
// scripted client and identifies as one for these endpoints.
const ESPN_USER_AGENT: &str = "curl/8";

/// Fetches and normalizes one schedule source, emitting only fixtures whose
/// kickoff falls inside the pre-kickoff window.
pub(super) fn fetch_schedule(
    client: &HttpClient,
    source: &Source,
    now: DateTime<Utc>,
) -> Result<FetchOutput> {
    let items = match source.schedule_format.as_str() {
        SCHEDULE_FORMAT_ESPN => espn_items(
            source,
            &client.get_with_headers(&source.url, &[("user-agent", ESPN_USER_AGENT)])?,
            now,
        )?,
        SCHEDULE_FORMAT_ESPN_CORE => espn_core_items(client, source, now)?,
        SCHEDULE_FORMAT_RIOT => {
            let body =
                client.get_with_headers(&source.url, &[("x-api-key", source.api_key.as_str())])?;
            riot_items(source, &body, now)?
        }
        other => bail!("unsupported schedule format {other:?}"),
    };
    Ok(FetchOutput {
        items,
        unresolved: Vec::new(),
        truncated: false,
    })
}

/// The stable cross-source key for one fixture: provider identity when present,
/// otherwise the item URL.
pub(super) fn fixture_key(identity: &str, url: &str) -> String {
    let id = identity.trim();
    if id.is_empty() {
        url.to_owned()
    } else {
        format!("id:{id}")
    }
}

/// A fixture is upcoming when kickoff is in the future within the pre-kickoff
/// window. Past and far-future fixtures never emit.
fn in_window(kickoff: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let Some(limit) = now.checked_add_signed(FIXTURE_WINDOW) else {
        return false;
    };
    kickoff > now && kickoff <= limit
}

fn parse_kickoff(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim();
    if let Ok(kickoff) = DateTime::parse_from_rfc3339(trimmed) {
        return Some(kickoff.with_timezone(&Utc));
    }
    // ESPN emits compact stamps like "2026-03-17T20:00Z"; RFC 3339 wants
    // seconds, so normalize the trailing Z into an explicit UTC offset.
    let without_zone = trimmed.strip_suffix('Z')?;
    let kickoff = DateTime::parse_from_rfc3339(&format!("{without_zone}:00+00:00")).ok()?;
    Some(kickoff.with_timezone(&Utc))
}

fn fixture_title(teams: &str, competition: &str, kickoff: DateTime<Utc>) -> String {
    let local = kickoff.with_timezone(&Chicago);
    format!(
        "{teams} — {competition}, {}",
        local.format("%a %b %-d, %-I:%M %p %Z")
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnSchedule {
    events: Vec<EspnEvent>,
    season: Option<EspnSeason>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnSeason {
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnEvent {
    #[serde(default)]
    id: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    competitions: Vec<EspnCompetition>,
    #[serde(default)]
    links: Vec<EspnLink>,
    #[serde(rename = "$ref")]
    reference: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCompetition {
    #[serde(default)]
    competitors: Vec<EspnCompetitor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCompetitor {
    #[serde(default)]
    home_away: String,
    #[serde(default)]
    team: EspnTeam,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EspnTeam {
    #[serde(default)]
    display_name: String,
}

#[derive(Deserialize)]
struct EspnLink {
    #[serde(default)]
    href: String,
    #[serde(default)]
    rel: Vec<String>,
}

#[derive(Deserialize)]
struct EspnCoreEventList {
    items: Vec<EspnReference>,
}

#[derive(Deserialize)]
struct EspnReference {
    #[serde(rename = "$ref")]
    reference: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RiotScheduleResponse {
    #[serde(default)]
    data: Option<RiotScheduleData>,
}

#[derive(Deserialize)]
struct RiotScheduleData {
    #[serde(default)]
    schedule: Option<RiotSchedule>,
}

#[derive(Deserialize)]
struct RiotSchedule {
    events: Vec<RiotEvent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RiotEvent {
    #[serde(default)]
    start_time: String,
    league: Option<RiotLeague>,
    #[serde(rename = "match")]
    match_up: Option<RiotMatch>,
}

#[derive(Deserialize)]
struct RiotLeague {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct RiotMatch {
    #[serde(default)]
    id: String,
    #[serde(default)]
    teams: Vec<RiotTeam>,
}

#[derive(Deserialize)]
struct RiotTeam {
    #[serde(default)]
    name: String,
}

fn espn_items(source: &Source, body: &[u8], now: DateTime<Utc>) -> Result<Vec<FetchedItem>> {
    let schedule: EspnSchedule =
        serde_json::from_slice(body).context("parse ESPN schedule JSON")?;
    let competition = competition_name(schedule.season.as_ref(), source);
    let items = schedule
        .events
        .iter()
        .filter_map(|event| espn_event_item(source, event, competition.as_str(), now))
        .collect();
    Ok(items)
}

fn espn_event_item(
    source: &Source,
    event: &EspnEvent,
    competition: &str,
    now: DateTime<Utc>,
) -> Option<FetchedItem> {
    let kickoff = parse_kickoff(&event.date)?;
    if !in_window(kickoff, now) {
        return None;
    }
    let teams = event_teams(event);
    let title = fixture_title(&teams, competition, kickoff);
    let url = event_url(source, event);
    Some(FetchedItem {
        title,
        url: url.clone(),
        published_at: kickoff.to_rfc3339(),
        identity: fixture_identity(event.id.as_str(), &url),
        feed_identity: String::new(),
        outlet: String::new(),
        rss_source: String::new(),
    })
}

fn event_teams(event: &EspnEvent) -> String {
    let home = event
        .competitions
        .first()
        .map(|competition| {
            competition
                .competitors
                .iter()
                .find(|side| side.home_away == "home")
        })
        .unwrap_or_default();
    let away = event
        .competitions
        .first()
        .map(|competition| {
            competition
                .competitors
                .iter()
                .find(|side| side.home_away == "away")
        })
        .unwrap_or_default();
    let (Some(home), Some(away)) = (home, away) else {
        return event.name.clone();
    };
    format!("{} vs {}", away.team.display_name, home.team.display_name)
}

fn event_url(source: &Source, event: &EspnEvent) -> String {
    for wanted in ["web", "canonical", "mobile"] {
        if let Some(link) = event
            .links
            .iter()
            .find(|link| link.rel.iter().any(|rel| rel == wanted) && !link.href.is_empty())
        {
            return link.href.clone();
        }
    }
    if let Some(reference) = &event.reference {
        return reference.clone();
    }
    source.url.clone()
}

fn fixture_identity(id: &str, url: &str) -> String {
    if id.trim().is_empty() {
        url.to_owned()
    } else {
        id.trim().to_owned()
    }
}

fn competition_name(season: Option<&EspnSeason>, source: &Source) -> String {
    season
        .map(|season| season.display_name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| source.label.clone())
}

fn espn_core_items(
    client: &HttpClient,
    source: &Source,
    now: DateTime<Utc>,
) -> Result<Vec<FetchedItem>> {
    let endpoint = espn_core_range_endpoint(&source.url, now);
    let body = client.get_with_headers(&endpoint, &[("user-agent", ESPN_USER_AGENT)])?;
    let list: EspnCoreEventList =
        serde_json::from_slice(&body).context("parse ESPN event list JSON")?;
    let mut items = Vec::new();
    for reference in &list.items {
        let body =
            client.get_with_headers(&reference.reference, &[("user-agent", ESPN_USER_AGENT)])?;
        let event: EspnEvent = serde_json::from_slice(&body).context("parse ESPN event JSON")?;
        if let Some(item) = espn_core_event_item(source, &event, now) {
            items.push(item);
        }
    }
    items.sort_by(|left, right| left.published_at.cmp(&right.published_at));
    Ok(items)
}

fn espn_core_range_endpoint(endpoint: &str, now: DateTime<Utc>) -> String {
    let today = now.format("%Y%m%d");
    let horizon = now
        .checked_add_signed(FIXTURE_WINDOW)
        .unwrap_or(now)
        .format("%Y%m%d");
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    format!("{endpoint}{separator}dates={today}-{horizon}")
}

fn espn_core_event_item(
    source: &Source,
    event: &EspnEvent,
    now: DateTime<Utc>,
) -> Option<FetchedItem> {
    let kickoff = parse_kickoff(&event.date)?;
    if !in_window(kickoff, now) {
        return None;
    }
    let teams = if event.name.trim().is_empty() {
        event_teams(event)
    } else {
        event.name.trim().to_owned()
    };
    let url = event
        .reference
        .clone()
        .filter(|reference| !reference.trim().is_empty())
        .unwrap_or_else(|| source.url.clone());
    Some(FetchedItem {
        title: fixture_title(&teams, &source.label, kickoff),
        url: url.clone(),
        published_at: kickoff.to_rfc3339(),
        identity: fixture_identity(event.id.as_str(), &url),
        feed_identity: String::new(),
        outlet: String::new(),
        rss_source: String::new(),
    })
}

fn riot_items(source: &Source, body: &[u8], now: DateTime<Utc>) -> Result<Vec<FetchedItem>> {
    let response: RiotScheduleResponse =
        serde_json::from_slice(body).context("parse Riot schedule JSON")?;
    let events = response
        .data
        .as_ref()
        .and_then(|data| data.schedule.as_ref())
        .map(|schedule| &schedule.events)
        .context("Riot schedule JSON has no schedule events")?;
    let items = events
        .iter()
        .filter_map(|event| riot_event_item(source, event, now))
        .collect();
    Ok(items)
}

fn riot_event_item(source: &Source, event: &RiotEvent, now: DateTime<Utc>) -> Option<FetchedItem> {
    let kickoff = parse_kickoff(&event.start_time)?;
    if !in_window(kickoff, now) {
        return None;
    }
    let match_up = event.match_up.as_ref()?;
    let mut teams = match_up
        .teams
        .iter()
        .map(|team| team.name.trim().to_owned())
        .filter(|name| !name.is_empty());
    let (first, second) = (teams.next()?, teams.next()?);
    let competition = event
        .league
        .as_ref()
        .map(|league| league.name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| source.label.clone());
    // Riot's API exposes no stable per-match page; the public schedule lists it.
    let url = "https://lolesports.com/schedule".to_owned();
    Some(FetchedItem {
        title: fixture_title(&format!("{first} vs {second}"), &competition, kickoff),
        url: url.clone(),
        published_at: kickoff.to_rfc3339(),
        identity: fixture_identity(match_up.id.as_str(), &format!("{url}#{}", match_up.id)),
        feed_identity: String::new(),
        outlet: String::new(),
        rss_source: String::new(),
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::panic_in_result_fn,
        reason = "behavior tests return Result for fixture decoding while assertions report contract failures"
    )]

    use super::{espn_items, in_window, parse_kickoff, riot_items};
    use crate::domain::{SCHEDULE_FORMAT_ESPN, SCHEDULE_FORMAT_RIOT, Source};
    use anyhow::{Result, bail};
    use chrono::{DateTime, Duration, Utc};

    fn schedule_source() -> Source {
        Source {
            key: "ucl_man_city".to_owned(),
            label: "Manchester City (UCL)".to_owned(),
            kind: crate::domain::SOURCE_KIND_SCHEDULE.to_owned(),
            url: "https://site.api.espn.com/apis/site/v2/sports/soccer/uefa.champions/teams/382/schedule".to_owned(),
            section: "sports".to_owned(),
            threshold: "medium".to_owned(),
            enabled: true,
            schedule_format: SCHEDULE_FORMAT_ESPN.to_owned(),
            ..Source::default()
        }
    }

    fn riot_source() -> Source {
        Source {
            key: "lol_lec".to_owned(),
            label: "LEC".to_owned(),
            kind: crate::domain::SOURCE_KIND_SCHEDULE.to_owned(),
            url: "https://esports-api.lolesports.com/persisted/gw/getSchedule?hl=en-US&leagueId=98767991302996019".to_owned(),
            section: "gaming".to_owned(),
            threshold: "medium".to_owned(),
            enabled: true,
            schedule_format: SCHEDULE_FORMAT_RIOT.to_owned(),
            api_key: "public-frontend-key".to_owned(),
            ..Source::default()
        }
    }

    fn now() -> Result<DateTime<Utc>> {
        let moment = DateTime::parse_from_rfc3339("2026-08-24T15:00:00Z")?;
        Ok(moment.with_timezone(&Utc))
    }

    #[test]
    fn espn_schedule_emits_only_in_window_fixtures() -> Result<()> {
        let body = br#"{"events":[
            {"id":"401","date":"2026-08-25T12:00Z","name":"Real Madrid at Manchester City",
             "competitions":[{"competitors":[
                {"homeAway":"home","team":{"displayName":"Manchester City"}},
                {"homeAway":"away","team":{"displayName":"Real Madrid"}}]}],
             "links":[{"href":"https://www.espn.com/soccer/story/_/id/401","rel":["web"]}]},
            {"id":"402","date":"2026-09-20T20:00Z","name":"Far away fixture",
             "competitions":[{"competitors":[
                {"homeAway":"home","team":{"displayName":"Manchester City"}},
                {"homeAway":"away","team":{"displayName":"Someone"}}]}]},
            {"id":"403","date":"2026-08-20T20:00Z","name":"Already played",
             "competitions":[{"competitors":[
                {"homeAway":"home","team":{"displayName":"Manchester City"}},
                {"homeAway":"away","team":{"displayName":"Someone"}}]}]}
        ],"season":{"displayName":"2025-26 UEFA Champions League"}}"#;
        let items = espn_items(&schedule_source(), body, now()?)?;
        if items.len() != 1 {
            bail!(
                "window must keep only the imminent fixture, got {}",
                items.len()
            );
        }
        let item = items
            .first()
            .ok_or_else(|| anyhow::anyhow!("missing fixture item"))?;
        assert!(
            item.title
                .starts_with("Real Madrid vs Manchester City — 2025-26 UEFA Champions League, "),
            "unexpected fixture title: {}",
            item.title
        );
        assert!(
            item.title.contains("CDT") || item.title.contains("CST"),
            "kickoff is not formatted in Central time: {}",
            item.title
        );
        assert_eq!(item.identity, "401", "fixture identity changed");
        assert_eq!(item.url, "https://www.espn.com/soccer/story/_/id/401");
        assert_eq!(item.published_at, "2026-08-25T12:00:00+00:00");
        Ok(())
    }

    #[test]
    fn espn_schedule_without_links_falls_back_to_reference() -> Result<()> {
        let body = br#"{"events":[
            {"id":"404","date":"2026-08-25T12:00Z","name":"Fallback fixture",
             "competitions":[{"competitors":[
                {"homeAway":"home","team":{"displayName":"Manchester City"}},
                {"homeAway":"away","team":{"displayName":"Arsenal"}}]}],
             "$ref":"http://sports.core.example/event/404"}
        ]}"#;
        let items = espn_items(&schedule_source(), body, now()?)?;
        if items.len() != 1 {
            bail!("expected the in-window fixture, got {}", items.len());
        }
        let item = items
            .first()
            .ok_or_else(|| anyhow::anyhow!("missing fixture item"))?;
        assert_eq!(
            item.url, "http://sports.core.example/event/404",
            "reference fallback changed"
        );
        Ok(())
    }

    #[test]
    fn espn_schedule_without_competition_uses_source_label() -> Result<()> {
        let body = br#"{"events":[
            {"id":"405","date":"2026-08-25T12:00Z","name":"Label fallback",
             "competitions":[{"competitors":[
                {"homeAway":"home","team":{"displayName":"Manchester City"}},
                {"homeAway":"away","team":{"displayName":"Arsenal"}}]}]}
        ]}"#;
        let items = espn_items(&schedule_source(), body, now()?)?;
        assert!(
            items
                .first()
                .is_some_and(|item| item.title.contains("Manchester City (UCL)")),
            "label fallback missing from title: {:?}",
            items.first().map(|item| &item.title)
        );
        Ok(())
    }

    #[test]
    fn riot_schedule_emits_fixture_with_league_name() -> Result<()> {
        let body = br#"{"data":{"schedule":{"events":[
            {"startTime":"2026-08-25T10:00:00Z","league":{"name":"LEC"},
             "match":{"id":"110850271387346448","teams":[{"name":"G2 Esports"},{"name":"Fnatic"}]}},
            {"startTime":"2026-09-15T18:00:00Z","league":{"name":"LEC"},
             "match":{"id":"2","teams":[{"name":"G2 Esports"},{"name":"Vitality"}]}}
        ]}}}"#;
        let items = riot_items(&riot_source(), body, now()?)?;
        if items.len() != 1 {
            bail!(
                "window must keep only the imminent match, got {}",
                items.len()
            );
        }
        let item = items
            .first()
            .ok_or_else(|| anyhow::anyhow!("missing riot item"))?;
        assert!(
            item.title.starts_with("G2 Esports vs Fnatic — LEC, "),
            "unexpected riot title: {}",
            item.title
        );
        assert_eq!(item.url, "https://lolesports.com/schedule");
        assert_eq!(item.identity, "110850271387346448");
        Ok(())
    }

    #[test]
    fn riot_schedule_with_single_team_is_skipped() -> Result<()> {
        let body = br#"{"data":{"schedule":{"events":[
            {"startTime":"2026-08-25T10:00:00Z","league":{"name":"LEC"},
             "match":{"id":"1","teams":[{"name":"G2 Esports"}]}}
        ]}}}"#;
        let items = riot_items(&riot_source(), body, now()?)?;
        assert!(items.is_empty(), "single-team matches must not emit");
        Ok(())
    }

    #[test]
    fn window_boundaries_match_two_run_receipt() -> Result<()> {
        let moment = now()?;
        if in_window(moment, moment) {
            bail!("kickoff at now is not upcoming");
        }
        assert!(
            in_window(moment + Duration::hours(24), moment),
            "24h is inside the window"
        );
        if in_window(moment + Duration::hours(25), moment) {
            bail!("25h must be outside the window");
        }
        Ok(())
    }

    #[test]
    fn kickoff_parser_handles_espn_and_riot_timestamps() {
        assert!(
            parse_kickoff("2026-03-17T20:00Z").is_some(),
            "compact ESPN stamp failed"
        );
        assert!(
            parse_kickoff("2026-03-17T20:00:00Z").is_some(),
            "full RFC 3339 failed"
        );
        assert!(
            parse_kickoff("not-a-date").is_none(),
            "garbage must not parse"
        );
    }
}
