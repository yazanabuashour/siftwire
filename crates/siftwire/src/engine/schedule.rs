use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;
use serde::Deserialize;
use url::Url;

use crate::contract::{SportsImage, SportsUpdate};
use crate::domain::{
    SCHEDULE_FILTER_STANDINGS_TOP_TWO, SCHEDULE_FORMAT_ESPN, SCHEDULE_FORMAT_ESPN_SCOREBOARD,
    SCHEDULE_FORMAT_RIOT, Source,
};

use super::http::HttpClient;
use super::model::{FetchOutput, FetchedItem};

pub const SPORTS_STATUS_FINAL: &str = "final";
pub const SPORTS_STATUS_UPCOMING: &str = "upcoming";

#[derive(Clone, Debug)]
pub struct SportsOptions {
    pub pre_game_window: TimeDelta,
    pub post_game_window: TimeDelta,
    pub timezone: Tz,
}

// Receipt: ESPN's edge (Akamai) answers 403 for unrecognized user agents while
// allowing recognized scripted-client agents; probed 2026-08-24, "siftwire/0.4"
// -> 403, "curl/8.5.0" and "python-requests/2.31.0" -> 200. The runner is a
// scripted client and identifies as one for these endpoints.
const ESPN_USER_AGENT: &str = "curl/8";

/// Fetches one schedule source and returns recurring fixture and result updates.
pub(super) fn fetch_schedule(
    client: &HttpClient,
    source: &Source,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Result<FetchOutput> {
    let sports_updates = match source.schedule_format.as_str() {
        SCHEDULE_FORMAT_ESPN => espn_updates(
            source,
            &client.get_with_headers(&source.url, &[("user-agent", ESPN_USER_AGENT)])?,
            now,
            options,
        )?,
        SCHEDULE_FORMAT_ESPN_SCOREBOARD => espn_scoreboard_updates(client, source, now, options)?,
        SCHEDULE_FORMAT_RIOT => riot_fetch_updates(client, source, now, options)?,
        other => bail!("unsupported schedule format {other:?}"),
    };
    let items = sports_updates
        .iter()
        .filter(|update| update.status == SPORTS_STATUS_UPCOMING)
        .map(|update| sports_update_item(update, options.timezone))
        .collect();
    Ok(FetchOutput {
        items,
        sports_updates,
        ..FetchOutput::default()
    })
}

pub fn sports_update_item(update: &SportsUpdate, timezone: Tz) -> FetchedItem {
    let when = update
        .starts_at
        .with_timezone(&timezone)
        .format("%a %b %-d, %-I:%M %p %Z");
    FetchedItem {
        title: format!("{} - {}, {when}", update.title, update.competition),
        url: update.url.clone(),
        published_at: update.starts_at.to_rfc3339(),
        identity: update.fixture_identity.clone(),
        ..FetchedItem::default()
    }
}

fn update_status(
    kickoff: DateTime<Utc>,
    completed: bool,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Option<&'static str> {
    if kickoff > now {
        let limit = now.checked_add_signed(options.pre_game_window)?;
        return (kickoff <= limit).then_some(SPORTS_STATUS_UPCOMING);
    }
    if completed {
        let limit = now.checked_sub_signed(options.post_game_window)?;
        return (kickoff >= limit).then_some(SPORTS_STATUS_FINAL);
    }
    None
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

#[must_use]
pub fn prepare_sports_updates(updates: Vec<SportsUpdate>) -> Vec<SportsUpdate> {
    let mut seen = BTreeSet::new();
    let mut updates = updates
        .into_iter()
        .filter(|update| seen.insert(update.fixture_identity.clone()))
        .collect::<Vec<_>>();
    updates.sort_by(|left, right| {
        u8::from(left.status != SPORTS_STATUS_UPCOMING)
            .cmp(&u8::from(right.status != SPORTS_STATUS_UPCOMING))
            .then_with(|| {
                if left.status == SPORTS_STATUS_FINAL {
                    right.starts_at.cmp(&left.starts_at)
                } else {
                    left.starts_at.cmp(&right.starts_at)
                }
            })
            .then_with(|| left.source_key.cmp(&right.source_key))
            .then_with(|| left.title.cmp(&right.title))
    });
    updates
}

#[must_use]
pub fn render_sports_section(updates: &[SportsUpdate], timezone: Tz) -> String {
    if updates.is_empty() {
        return String::new();
    }
    let mut section = "## Sports".to_owned();
    append_sports_group(
        &mut section,
        "Upcoming fixtures",
        SPORTS_STATUS_UPCOMING,
        updates,
        timezone,
    );
    append_sports_group(
        &mut section,
        "Recent results",
        SPORTS_STATUS_FINAL,
        updates,
        timezone,
    );
    section
}

fn append_sports_group(
    section: &mut String,
    heading: &str,
    status: &str,
    updates: &[SportsUpdate],
    timezone: Tz,
) {
    let mut matching = updates
        .iter()
        .filter(|update| update.status == status)
        .peekable();
    if matching.peek().is_none() {
        return;
    }
    let _result = write!(section, "\n\n### {heading}");
    for update in matching {
        let when = update
            .starts_at
            .with_timezone(&timezone)
            .format("%a %b %-d, %-I:%M %p %Z");
        let competition = markdown_inline_text(&update.competition);
        let detail = if status == SPORTS_STATUS_FINAL {
            format!("{competition}, final {when}")
        } else {
            format!("{competition}, {when}")
        };
        if update.url.is_empty() {
            let _result = write!(
                section,
                "\n- {} - {detail}",
                markdown_inline_text(&update.title)
            );
        } else {
            let _result = write!(
                section,
                "\n- [{}](<{}>) - {detail}",
                markdown_inline_text(&update.title),
                markdown_link_destination(&update.url)
            );
        }
    }
}

fn markdown_inline_text(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn markdown_link_destination(value: &str) -> String {
    value
        .replace('\\', "%5C")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace(' ', "%20")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnSchedule {
    events: Vec<EspnEvent>,
    season: Option<EspnSeason>,
    #[serde(default)]
    leagues: Vec<EspnLeague>,
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
    #[serde(default)]
    status: EspnStatus,
    #[serde(rename = "$ref")]
    reference: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCompetition {
    #[serde(default)]
    id: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    competitors: Vec<EspnCompetitor>,
    #[serde(default)]
    status: EspnStatus,
    #[serde(default, rename = "type")]
    kind: EspnCompetitionType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EspnCompetitor {
    #[serde(default)]
    id: String,
    #[serde(default, rename = "type")]
    kind: String,
    order: Option<i64>,
    #[serde(default)]
    home_away: String,
    #[serde(default)]
    team: EspnTeam,
    #[serde(default)]
    athlete: EspnAthlete,
    #[serde(default)]
    records: Vec<EspnRecord>,
    #[serde(default)]
    score: serde_json::Value,
    #[serde(default)]
    winner: bool,
}

#[derive(Deserialize, Default)]
struct EspnStatus {
    #[serde(default, rename = "displayClock")]
    display_clock: String,
    #[serde(default)]
    period: i64,
    #[serde(default, rename = "type")]
    kind: EspnStatusType,
}

#[derive(Deserialize, Default)]
struct EspnStatusType {
    #[serde(default)]
    completed: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EspnTeam {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    logos: Vec<EspnLogo>,
}

#[derive(Deserialize)]
struct EspnLeague {
    #[serde(default)]
    logos: Vec<EspnLogo>,
}

#[derive(Deserialize)]
struct EspnLogo {
    #[serde(default)]
    href: String,
    #[serde(default)]
    rel: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EspnAthlete {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    full_name: String,
}

#[derive(Deserialize)]
struct EspnRecord {
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    summary: String,
}

#[derive(Deserialize, Default)]
struct EspnCompetitionType {
    #[serde(default)]
    abbreviation: String,
}

#[derive(Deserialize)]
struct EspnLink {
    #[serde(default)]
    href: String,
    #[serde(default)]
    rel: Vec<String>,
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
    #[serde(default)]
    state: String,
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

#[derive(Clone, Deserialize)]
struct RiotTeam {
    #[serde(default, deserialize_with = "crate::serde_util::null_default")]
    name: String,
    #[serde(default, deserialize_with = "crate::serde_util::null_default")]
    code: String,
    #[serde(default, deserialize_with = "crate::serde_util::null_default")]
    image: String,
    #[serde(default, deserialize_with = "crate::serde_util::null_default")]
    record: RiotRecord,
    result: Option<RiotResult>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RiotResult {
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    outcome: String,
    game_wins: Option<i64>,
}

#[derive(Clone, Default, Deserialize)]
struct RiotRecord {
    #[serde(default)]
    wins: i64,
    #[serde(default)]
    losses: i64,
}

#[derive(Deserialize)]
struct RiotTournamentsResponse {
    data: RiotTournamentsData,
}

#[derive(Deserialize)]
struct RiotTournamentsData {
    leagues: Vec<RiotTournamentLeague>,
}

#[derive(Deserialize)]
struct RiotTournamentLeague {
    tournaments: Vec<RiotTournament>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RiotTournament {
    id: String,
    start_date: String,
    end_date: String,
}

#[derive(Deserialize)]
struct RiotStandingsResponse {
    data: RiotStandingsData,
}

#[derive(Deserialize)]
struct RiotStandingsData {
    standings: Vec<RiotStanding>,
}

#[derive(Deserialize)]
struct RiotStanding {
    stages: Vec<RiotStage>,
}

#[derive(Deserialize)]
struct RiotStage {
    sections: Vec<RiotSection>,
}

#[derive(Deserialize)]
struct RiotSection {
    #[serde(default)]
    rankings: Vec<RiotRanking>,
}

#[derive(Deserialize)]
struct RiotRanking {
    ordinal: i64,
    teams: Vec<RiotRankedTeam>,
}

#[derive(Clone, Deserialize)]
struct RiotRankedTeam {
    #[serde(default, deserialize_with = "crate::serde_util::null_default")]
    code: String,
    #[serde(default, deserialize_with = "crate::serde_util::null_default")]
    name: String,
}

#[derive(Default)]
struct RiotTeamSelection {
    codes: BTreeSet<String>,
    names: BTreeSet<String>,
}

fn espn_updates(
    source: &Source,
    body: &[u8],
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Result<Vec<SportsUpdate>> {
    let schedule: EspnSchedule =
        serde_json::from_slice(body).context("parse ESPN schedule JSON")?;
    let competition = competition_name(schedule.season.as_ref(), source);
    let updates = schedule
        .events
        .iter()
        .filter_map(|event| espn_event_update(source, event, competition.as_str(), now, options))
        .collect();
    Ok(updates)
}

fn espn_event_update(
    source: &Source,
    event: &EspnEvent,
    competition: &str,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Option<SportsUpdate> {
    let kickoff = parse_kickoff(&event.date)?;
    let status = update_status(kickoff, event_completed(event), now, options)?;
    let url = event_url(source, event)?;
    let title = event_title(event, status);
    Some(SportsUpdate {
        fixture_identity: fixture_identity("espn", event.id.as_str(), &url),
        source_key: source.key.clone(),
        source_label: source.label.clone(),
        status: status.to_owned(),
        title: if title.trim().is_empty() {
            source.label.clone()
        } else {
            title
        },
        competition: competition.to_owned(),
        url,
        starts_at: kickoff,
        images: espn_event_images(event),
    })
}

fn event_completed(event: &EspnEvent) -> bool {
    event.status.kind.completed
        || event
            .competitions
            .first()
            .is_some_and(|competition| competition.status.kind.completed)
}

fn event_title(event: &EspnEvent, status: &str) -> String {
    let Some((away, home)) = event_sides(event) else {
        return event.name.clone();
    };
    let away_name = away.team.display_name.trim();
    let home_name = home.team.display_name.trim();
    if away_name.is_empty() || home_name.is_empty() {
        return event.name.clone();
    }
    if status != SPORTS_STATUS_FINAL {
        return format!("{away_name} vs {home_name}");
    }
    if let Some((away_score, home_score)) = score_text(&away.score).zip(score_text(&home.score)) {
        return format!("{away_name} {away_score} - {home_score} {home_name}");
    }
    match (away.winner, home.winner) {
        (true, false) => format!("{away_name} defeated {home_name}"),
        (false, true) => format!("{home_name} defeated {away_name}"),
        _ => format!("{away_name} vs {home_name}"),
    }
}

fn score_text(score: &serde_json::Value) -> Option<String> {
    let score = score
        .as_object()
        .and_then(|value| value.get("displayValue").or_else(|| value.get("value")))
        .unwrap_or(score);
    match score {
        serde_json::Value::String(value) if !value.trim().is_empty() => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Array(_)
        | serde_json::Value::Object(_) => None,
    }
}

fn event_sides(event: &EspnEvent) -> Option<(&EspnCompetitor, &EspnCompetitor)> {
    let competition = event.competitions.first()?;
    let home = competition
        .competitors
        .iter()
        .find(|side| side.home_away == "home")?;
    let away = competition
        .competitors
        .iter()
        .find(|side| side.home_away == "away")?;
    Some((away, home))
}

fn espn_event_images(event: &EspnEvent) -> Vec<SportsImage> {
    event_sides(event)
        .into_iter()
        .flat_map(<[&EspnCompetitor; 2]>::from)
        .filter_map(|competitor| {
            espn_logo_image(&competitor.team.logos, &competitor.team.display_name)
        })
        .collect()
}

fn espn_league_images(leagues: &[EspnLeague], source: &Source) -> Vec<SportsImage> {
    leagues
        .first()
        .and_then(|league| espn_logo_image(&league.logos, &source.label))
        .into_iter()
        .collect()
}

fn espn_logo_image(logos: &[EspnLogo], alt: &str) -> Option<SportsImage> {
    let logo = logos
        .iter()
        .find(|logo| logo.rel.iter().any(|rel| rel == "default"))
        .or_else(|| logos.first())?;
    Some(SportsImage {
        url: provider_image_url(&logo.href)?,
        alt: alt.trim().to_owned(),
    })
}

fn espn_athlete_image(competitor: &EspnCompetitor) -> Option<SportsImage> {
    let id = competitor.id.trim();
    if id.is_empty() || !id.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    Some(SportsImage {
        url: format!("https://a.espncdn.com/i/headshots/mma/players/full/{id}.png"),
        alt: athlete_name(competitor).to_owned(),
    })
}

fn provider_http_url(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.into())
}

fn provider_image_url(value: &str) -> Option<String> {
    let mut url = Url::parse(value).ok()?;
    if url.scheme() == "http" {
        url.set_scheme("https").ok()?;
    }
    (url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some("a.espncdn.com" | "static.lolesports.com")
        ))
    .then(|| url.into())
}

fn event_url(source: &Source, event: &EspnEvent) -> Option<String> {
    for wanted in ["web", "canonical", "mobile"] {
        if let Some(url) = event.links.iter().find_map(|link| {
            link.rel
                .iter()
                .any(|rel| rel == wanted)
                .then(|| provider_http_url(&link.href))
                .flatten()
        }) {
            return Some(url);
        }
    }
    event
        .reference
        .as_deref()
        .and_then(provider_http_url)
        .or_else(|| provider_http_url(&source.url))
}

fn fixture_identity(provider: &str, id: &str, url: &str) -> String {
    if id.trim().is_empty() {
        format!("{provider}:url:{url}")
    } else {
        format!("{provider}:id:{}", id.trim())
    }
}

fn competition_name(season: Option<&EspnSeason>, source: &Source) -> String {
    season
        .map(|season| season.display_name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| source.label.clone())
}

fn espn_scoreboard_updates(
    client: &HttpClient,
    source: &Source,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Result<Vec<SportsUpdate>> {
    let endpoint = espn_range_endpoint(&source.url, now, options);
    let body = client.get_with_headers(&endpoint, &[("user-agent", ESPN_USER_AGENT)])?;
    espn_scoreboard_updates_from_body(source, &body, now, options)
}

fn espn_scoreboard_updates_from_body(
    source: &Source,
    body: &[u8],
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Result<Vec<SportsUpdate>> {
    let schedule: EspnSchedule =
        serde_json::from_slice(body).context("parse ESPN scoreboard JSON")?;
    let league_images = espn_league_images(&schedule.leagues, source);
    let mut updates = Vec::new();
    for event in &schedule.events {
        let Some(url) = espn_fightcenter_url(event) else {
            continue;
        };
        let event_update =
            espn_scoreboard_event_update(source, event, &url, &league_images, now, options);
        if event_update
            .as_ref()
            .is_some_and(|update| update.status == SPORTS_STATUS_UPCOMING)
        {
            updates.extend(event_update);
            continue;
        }
        let usable_bouts = event
            .competitions
            .iter()
            .filter_map(espn_scoreboard_bout)
            .collect::<Vec<_>>();
        if usable_bouts.is_empty() {
            updates.extend(event_update);
            continue;
        }
        for (competition, competitors) in usable_bouts {
            if let Some(update) =
                espn_bout_update(source, event, competition, &competitors, &url, now, options)
            {
                updates.push(update);
            }
        }
    }
    Ok(updates)
}

fn espn_scoreboard_bout(
    competition: &EspnCompetition,
) -> Option<(&EspnCompetition, Vec<&EspnCompetitor>)> {
    if competition.id.trim().is_empty()
        || competition.competitors.len() != 2
        || competition
            .competitors
            .iter()
            .any(|competitor| competitor.kind != "athlete" || athlete_name(competitor).is_empty())
    {
        return None;
    }
    let mut competitors = competition.competitors.iter().collect::<Vec<_>>();
    competitors.sort_by_key(|competitor| competitor.order.unwrap_or(i64::MAX));
    Some((competition, competitors))
}

fn espn_bout_update(
    source: &Source,
    event: &EspnEvent,
    competition: &EspnCompetition,
    competitors: &[&EspnCompetitor],
    url: &str,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Option<SportsUpdate> {
    let kickoff = parse_kickoff(if competition.date.trim().is_empty() {
        &event.date
    } else {
        &competition.date
    })?;
    let completed = competition.status.kind.completed || event.status.kind.completed;
    let status = update_status(kickoff, completed, now, options)?;
    let first = *competitors.first()?;
    let second = *competitors.get(1)?;
    Some(SportsUpdate {
        fixture_identity: fixture_identity("espn_scoreboard", &competition.id, url),
        source_key: source.key.clone(),
        source_label: source.label.clone(),
        status: status.to_owned(),
        title: espn_bout_title(first, second, status),
        competition: espn_bout_competition(event, competition, status),
        url: url.to_owned(),
        starts_at: kickoff,
        images: competitors
            .iter()
            .filter_map(|competitor| espn_athlete_image(competitor))
            .collect(),
    })
}

fn espn_scoreboard_event_update(
    source: &Source,
    event: &EspnEvent,
    url: &str,
    images: &[SportsImage],
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Option<SportsUpdate> {
    let kickoff = parse_kickoff(&event.date)?;
    let status = update_status(kickoff, event_completed(event), now, options)?;
    let title = if event.name.trim().is_empty() {
        source.label.clone()
    } else {
        event.name.trim().to_owned()
    };
    Some(SportsUpdate {
        fixture_identity: fixture_identity("espn_scoreboard", &event.id, url),
        source_key: source.key.clone(),
        source_label: source.label.clone(),
        status: status.to_owned(),
        title,
        competition: source.label.clone(),
        url: url.to_owned(),
        starts_at: kickoff,
        images: images.to_vec(),
    })
}

fn athlete_name(competitor: &EspnCompetitor) -> &str {
    if competitor.athlete.display_name.trim().is_empty() {
        competitor.athlete.full_name.trim()
    } else {
        competitor.athlete.display_name.trim()
    }
}

fn athlete_record(competitor: &EspnCompetitor) -> Option<&str> {
    competitor
        .records
        .iter()
        .find(|record| {
            (record.kind == "total" || record.name == "overall")
                && !record.summary.trim().is_empty()
        })
        .or_else(|| {
            competitor
                .records
                .iter()
                .find(|record| !record.summary.trim().is_empty())
        })
        .map(|record| record.summary.trim())
}

fn athlete_with_record(competitor: &EspnCompetitor) -> String {
    let name = athlete_name(competitor);
    athlete_record(competitor)
        .map_or_else(|| name.to_owned(), |record| format!("{name} ({record})"))
}

fn espn_bout_title(first: &EspnCompetitor, second: &EspnCompetitor, status: &str) -> String {
    if status == SPORTS_STATUS_FINAL && first.winner != second.winner {
        let (winner, loser) = if first.winner {
            (first, second)
        } else {
            (second, first)
        };
        return format!(
            "{} defeated {}",
            athlete_with_record(winner),
            athlete_with_record(loser)
        );
    }
    format!(
        "{} vs {}",
        athlete_with_record(first),
        athlete_with_record(second)
    )
}

fn espn_bout_competition(event: &EspnEvent, competition: &EspnCompetition, status: &str) -> String {
    let mut parts = [event.name.trim(), competition.kind.abbreviation.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if status == SPORTS_STATUS_FINAL {
        match (
            competition.status.period,
            competition.status.display_clock.trim(),
        ) {
            (period, clock) if period > 0 && !clock.is_empty() => {
                parts.push(format!("round {period} at {clock}"));
            }
            (period, _) if period > 0 => parts.push(format!("round {period}")),
            (_, clock) if !clock.is_empty() => parts.push(format!("ended at {clock}")),
            _ => {}
        }
    }
    parts.join(", ")
}

fn espn_fightcenter_url(event: &EspnEvent) -> Option<String> {
    event
        .links
        .iter()
        .find(|link| {
            ["event", "summary", "desktop"]
                .iter()
                .any(|wanted| link.rel.iter().any(|rel| rel == wanted))
                && is_espn_fightcenter_url(&link.href)
        })
        .map(|link| link.href.clone())
}

fn is_espn_fightcenter_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str() == Some("www.espn.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.path().contains("/mma/fightcenter/")
    })
}

fn espn_range_endpoint(endpoint: &str, now: DateTime<Utc>, options: &SportsOptions) -> String {
    let start = now
        .checked_sub_signed(options.post_game_window)
        .unwrap_or(now)
        .format("%Y%m%d");
    let horizon = now
        .checked_add_signed(options.pre_game_window)
        .unwrap_or(now)
        .format("%Y%m%d");
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    format!("{endpoint}{separator}dates={start}-{horizon}")
}

#[cfg(test)]
fn riot_updates(
    source: &Source,
    body: &[u8],
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Result<Vec<SportsUpdate>> {
    let response = parse_riot_schedule(body)?;
    Ok(riot_updates_for_events(
        source,
        riot_schedule_events(&response)?,
        now,
        options,
    ))
}

fn riot_fetch_updates(
    client: &HttpClient,
    source: &Source,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Result<Vec<SportsUpdate>> {
    let headers = [("x-api-key", source.api_key.as_str())];
    let body = client.get_with_headers(&source.url, &headers)?;
    let response = parse_riot_schedule(&body)?;
    let events = riot_schedule_events(&response)?;
    let eligible = events
        .iter()
        .filter(|event| riot_event_update(source, event, now, options).is_some())
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return Ok(Vec::new());
    }
    if source.schedule_filter != SCHEDULE_FILTER_STANDINGS_TOP_TWO {
        return Ok(riot_updates_for_events(source, events, now, options));
    }

    let tournaments_url =
        riot_gateway_endpoint(&source.url, "getTournamentsForLeague", "leagueId", None)?;
    let tournament_body = client.get_with_headers(&tournaments_url, &headers)?;
    let tournament_response: RiotTournamentsResponse =
        serde_json::from_slice(&tournament_body).context("parse Riot tournaments JSON")?;
    let tournament_ids = riot_overlapping_tournaments(&tournament_response, now, options);
    if tournament_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut teams = Vec::new();
    let mut found_rankings = false;
    for tournament_id in tournament_ids {
        let standings_url = riot_gateway_endpoint(
            &source.url,
            "getStandings",
            "tournamentId",
            Some(&tournament_id),
        )?;
        let standings_body = client.get_with_headers(&standings_url, &headers)?;
        let standings: RiotStandingsResponse =
            serde_json::from_slice(&standings_body).context("parse Riot standings JSON")?;
        if let Some(rankings) = riot_first_rankings(&standings) {
            found_rankings = true;
            teams.extend(riot_top_two_rankings(rankings));
        }
    }
    let selection = if found_rankings {
        riot_team_selection(teams)
    } else {
        riot_schedule_record_selection(events)
    };
    if selection.codes.is_empty() && selection.names.is_empty() {
        return Ok(Vec::new());
    }
    Ok(eligible
        .into_iter()
        .filter(|event| riot_event_matches_selection(event, &selection))
        .filter_map(|event| riot_event_update(source, event, now, options))
        .collect())
}

fn parse_riot_schedule(body: &[u8]) -> Result<RiotScheduleResponse> {
    serde_json::from_slice(body).context("parse Riot schedule JSON")
}

fn riot_schedule_events(response: &RiotScheduleResponse) -> Result<&[RiotEvent]> {
    response
        .data
        .as_ref()
        .and_then(|data| data.schedule.as_ref())
        .map(|schedule| schedule.events.as_slice())
        .context("Riot schedule JSON has no schedule events")
}

fn riot_updates_for_events(
    source: &Source,
    events: &[RiotEvent],
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Vec<SportsUpdate> {
    events
        .iter()
        .filter_map(|event| riot_event_update(source, event, now, options))
        .collect()
}

fn riot_gateway_endpoint(
    configured: &str,
    action: &str,
    parameter_name: &str,
    parameter_value: Option<&str>,
) -> Result<String> {
    let mut url = Url::parse(configured).context("parse Riot schedule URL")?;
    let query = url
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let league_ids = query
        .iter()
        .filter(|(name, _value)| name == "leagueId")
        .map(|(_name, value)| value.trim())
        .collect::<Vec<_>>();
    let [league_id] = league_ids.as_slice() else {
        bail!("Riot schedule URL must contain exactly one leagueId");
    };
    if league_id.is_empty() || league_id.contains(',') {
        bail!("Riot schedule URL must contain exactly one leagueId");
    }
    let marker = "/persisted/gw/";
    let Some((prefix, _configured_action)) = url.path().split_once(marker) else {
        bail!("Riot schedule URL must use a persisted gateway endpoint");
    };
    let path = format!("{prefix}{marker}{action}");
    url.set_path(&path);
    url.set_fragment(None);
    url.set_query(None);
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(language) = query
            .iter()
            .find(|(name, value)| name == "hl" && !value.trim().is_empty())
            .map(|(_name, value)| value.as_str())
        {
            pairs.append_pair("hl", language);
        }
        pairs.append_pair(parameter_name, parameter_value.unwrap_or(league_id));
    }
    Ok(url.into())
}

fn riot_overlapping_tournaments(
    response: &RiotTournamentsResponse,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Vec<String> {
    let window_start = now
        .checked_sub_signed(options.post_game_window)
        .unwrap_or(now)
        .date_naive();
    let window_end = now
        .checked_add_signed(options.pre_game_window)
        .unwrap_or(now)
        .date_naive();
    response
        .data
        .leagues
        .iter()
        .flat_map(|league| &league.tournaments)
        .filter(|tournament| {
            tournament_overlaps(tournament, window_start, window_end)
                && !tournament.id.trim().is_empty()
        })
        .map(|tournament| tournament.id.clone())
        .collect()
}

fn tournament_overlaps(
    tournament: &RiotTournament,
    window_start: NaiveDate,
    window_end: NaiveDate,
) -> bool {
    NaiveDate::parse_from_str(&tournament.start_date, "%Y-%m-%d")
        .ok()
        .zip(NaiveDate::parse_from_str(&tournament.end_date, "%Y-%m-%d").ok())
        .is_some_and(|(start, end)| start <= window_end && end >= window_start)
}

fn riot_first_rankings(response: &RiotStandingsResponse) -> Option<&[RiotRanking]> {
    response
        .data
        .standings
        .iter()
        .flat_map(|standing| &standing.stages)
        .flat_map(|stage| &stage.sections)
        .find(|section| !section.rankings.is_empty())
        .map(|section| section.rankings.as_slice())
}

fn riot_top_two_rankings(rankings: &[RiotRanking]) -> Vec<RiotRankedTeam> {
    let mut rankings = rankings.iter().collect::<Vec<_>>();
    rankings.sort_by_key(|ranking| ranking.ordinal);
    let mut identities = BTreeSet::new();
    let mut teams = Vec::new();
    let mut boundary = None;
    for ranking in rankings {
        if boundary.is_some_and(|boundary| boundary != ranking.ordinal) {
            break;
        }
        for team in &ranking.teams {
            let code = normalized_team(&team.code);
            let name = normalized_team(&team.name);
            let identity = if code.is_empty() { &name } else { &code };
            if !identity.is_empty() && identities.insert(identity.clone()) {
                teams.push(team.clone());
            }
        }
        if identities.len() >= 2 {
            boundary = Some(ranking.ordinal);
        }
    }
    teams
}

fn riot_team_selection(teams: Vec<RiotRankedTeam>) -> RiotTeamSelection {
    let mut selection = RiotTeamSelection::default();
    for team in teams {
        let code = normalized_team(&team.code);
        let name = normalized_team(&team.name);
        if !code.is_empty() {
            selection.codes.insert(code);
        }
        if !name.is_empty() {
            selection.names.insert(name);
        }
    }
    selection
}

fn riot_schedule_record_selection(events: &[RiotEvent]) -> RiotTeamSelection {
    let mut latest = BTreeMap::<String, (DateTime<Utc>, RiotTeam)>::new();
    for event in events {
        let Some(kickoff) = parse_kickoff(&event.start_time) else {
            continue;
        };
        let Some(match_up) = &event.match_up else {
            continue;
        };
        for team in &match_up.teams {
            let code = normalized_team(&team.code);
            let name = normalized_team(&team.name);
            if (code.is_empty() && name.is_empty())
                || code == "tbd"
                || name == "tbd"
                || (team.record.wins == 0 && team.record.losses == 0)
            {
                continue;
            }
            let identity = if code.is_empty() { name } else { code };
            if latest
                .get(&identity)
                .is_none_or(|(seen_at, _team)| kickoff > *seen_at)
            {
                latest.insert(identity, (kickoff, team.clone()));
            }
        }
    }
    let mut teams = latest
        .into_values()
        .map(|(_kickoff, team)| team)
        .collect::<Vec<_>>();
    teams.sort_by_key(|team| (Reverse(team.record.wins), team.record.losses));
    let mut ranked = Vec::new();
    let mut boundary = None;
    for team in teams {
        let record = (team.record.wins, team.record.losses);
        if ranked.len() >= 2 && boundary.is_some_and(|boundary| boundary != record) {
            break;
        }
        ranked.push(RiotRankedTeam {
            code: team.code,
            name: team.name,
        });
        boundary = Some(record);
    }
    riot_team_selection(ranked)
}

fn riot_event_matches_selection(event: &RiotEvent, selection: &RiotTeamSelection) -> bool {
    event.match_up.as_ref().is_some_and(|match_up| {
        match_up.teams.len() == 2
            && match_up.teams.iter().all(|team| {
                let name = normalized_team(&team.name);
                !name.is_empty() && name != "tbd"
            })
            && match_up.teams.iter().any(|team| {
                let code = normalized_team(&team.code);
                if !code.is_empty() && selection.codes.contains(&code) {
                    return true;
                }
                selection.names.contains(&normalized_team(&team.name))
            })
    })
}

fn normalized_team(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn riot_event_update(
    source: &Source,
    event: &RiotEvent,
    now: DateTime<Utc>,
    options: &SportsOptions,
) -> Option<SportsUpdate> {
    let kickoff = parse_kickoff(&event.start_time)?;
    let status = update_status(
        kickoff,
        event.state.eq_ignore_ascii_case("completed"),
        now,
        options,
    )?;
    let match_up = event.match_up.as_ref()?;
    let mut teams = match_up.teams.iter().filter(|team| {
        let name = normalized_team(&team.name);
        !name.is_empty() && name != "tbd"
    });
    let (first, second) = (teams.next()?, teams.next()?);
    let title = riot_title(first, second, status);
    let competition = event
        .league
        .as_ref()
        .map(|league| league.name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| source.label.clone());
    // Riot's API exposes no stable per-match page; the public schedule lists it.
    let url = "https://lolesports.com/schedule".to_owned();
    Some(SportsUpdate {
        fixture_identity: fixture_identity(
            "riot",
            match_up.id.as_str(),
            &format!("{url}#{}", match_up.id),
        ),
        source_key: source.key.clone(),
        source_label: source.label.clone(),
        status: status.to_owned(),
        title,
        competition,
        url,
        starts_at: kickoff,
        images: <[&RiotTeam; 2]>::from((first, second))
            .into_iter()
            .filter_map(riot_team_image)
            .collect(),
    })
}

fn riot_team_image(team: &RiotTeam) -> Option<SportsImage> {
    Some(SportsImage {
        url: provider_image_url(&team.image)?,
        alt: team.name.trim().to_owned(),
    })
}

fn riot_title(first: &RiotTeam, second: &RiotTeam, status: &str) -> String {
    if status == SPORTS_STATUS_FINAL {
        let scores = first
            .result
            .as_ref()
            .and_then(|result| result.game_wins)
            .zip(second.result.as_ref().and_then(|result| result.game_wins));
        if let Some((first_score, second_score)) = scores {
            return format!(
                "{} {first_score} - {second_score} {}",
                first.name, second.name
            );
        }
        let first_won = first
            .result
            .as_ref()
            .is_some_and(|result| result.outcome.eq_ignore_ascii_case("win"));
        let second_won = second
            .result
            .as_ref()
            .is_some_and(|result| result.outcome.eq_ignore_ascii_case("win"));
        if first_won != second_won {
            let (winner, loser) = if first_won {
                (&first.name, &second.name)
            } else {
                (&second.name, &first.name)
            };
            return format!("{winner} defeated {loser}");
        }
    }
    format!("{} vs {}", first.name, second.name)
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::panic_in_result_fn,
        reason = "behavior tests return Result for fixture decoding while assertions report contract failures"
    )]

    use super::{
        RiotStandingsResponse, SPORTS_STATUS_FINAL, SPORTS_STATUS_UPCOMING, SportsOptions,
        espn_range_endpoint, espn_scoreboard_updates_from_body, espn_updates, parse_kickoff,
        prepare_sports_updates, provider_http_url, provider_image_url, render_sports_section,
        riot_event_matches_selection, riot_first_rankings, riot_schedule_events,
        riot_team_selection, riot_top_two_rankings, riot_updates, update_status,
    };
    use crate::contract::SportsUpdate;
    use crate::domain::{
        SCHEDULE_FORMAT_ESPN, SCHEDULE_FORMAT_ESPN_SCOREBOARD, SCHEDULE_FORMAT_RIOT, Source,
    };
    use anyhow::{Context, Result};
    use chrono::{DateTime, TimeDelta, Utc};

    fn schedule_source(format: &str) -> Source {
        Source {
            key: "team_alpha".to_owned(),
            label: "Team Alpha".to_owned(),
            kind: crate::domain::SOURCE_KIND_SCHEDULE.to_owned(),
            url: "https://fixture.example/schedule".to_owned(),
            section: "sports".to_owned(),
            threshold: "medium".to_owned(),
            enabled: true,
            schedule_format: format.to_owned(),
            api_key: if format == SCHEDULE_FORMAT_RIOT {
                "public-frontend-key".to_owned()
            } else {
                String::new()
            },
            ..Source::default()
        }
    }

    fn now() -> Result<DateTime<Utc>> {
        let moment = DateTime::parse_from_rfc3339("2026-08-24T15:00:00Z")?;
        Ok(moment.with_timezone(&Utc))
    }

    fn options() -> SportsOptions {
        SportsOptions {
            pre_game_window: TimeDelta::days(7),
            post_game_window: TimeDelta::days(3),
            timezone: chrono_tz::America::New_York,
        }
    }

    #[test]
    fn espn_schedule_emits_upcoming_fixture_and_recent_result() -> Result<()> {
        let body = br#"{"events":[
            {"id":"401","date":"2026-08-30T12:00Z","name":"Team Beta at Team Alpha",
             "competitions":[{"competitors":[
                {"homeAway":"home","team":{"displayName":"Team Alpha","logos":[{"href":"https://a.espncdn.com/alpha.png","rel":["default"]}]}},
                {"homeAway":"away","team":{"displayName":"Team Beta","logos":[{"href":"http://a.espncdn.com/beta.png","rel":["default"]}]}}]}],
             "links":[{"href":"https://fixture.example/events/401","rel":["web"]}]},
            {"id":"400","date":"2026-08-22T12:00Z","name":"Team Gamma at Team Alpha",
             "status":{"type":{"completed":true}},
             "competitions":[{"competitors":[
                {"homeAway":"home","score":"3","winner":true,"team":{"displayName":"Team Alpha"}},
                {"homeAway":"away","score":"1","winner":false,"team":{"displayName":"Team Gamma"}}]}]},
            {"id":"402","date":"2026-09-20T20:00Z","name":"Far fixture"},
            {"id":"399","date":"2026-08-20T20:00Z","name":"Old result",
             "status":{"type":{"completed":true}}}
        ],"season":{"displayName":"Example League"}}"#;
        let updates = espn_updates(
            &schedule_source(SCHEDULE_FORMAT_ESPN),
            body,
            now()?,
            &options(),
        )?;
        assert_eq!(updates.len(), 2, "sports windows selected the wrong events");
        let upcoming = updates
            .iter()
            .find(|update| update.status == SPORTS_STATUS_UPCOMING)
            .context("missing upcoming fixture")?;
        assert_eq!(upcoming.title, "Team Beta vs Team Alpha");
        assert_eq!(upcoming.competition, "Example League");
        assert_eq!(upcoming.fixture_identity, "espn:id:401");
        assert_eq!(
            upcoming
                .images
                .iter()
                .map(|image| image.url.as_str())
                .collect::<Vec<_>>(),
            [
                "https://a.espncdn.com/beta.png",
                "https://a.espncdn.com/alpha.png"
            ]
        );
        let result = updates
            .iter()
            .find(|update| update.status == SPORTS_STATUS_FINAL)
            .context("missing recent result")?;
        assert_eq!(result.title, "Team Gamma 1 - 3 Team Alpha");
        Ok(())
    }

    #[test]
    fn espn_reference_scores_and_teams_fall_back_to_event_name() -> Result<()> {
        let body = br#"{"events":[
            {"id":"core-1","date":"2026-08-23T12:00Z","name":"Fighter A vs Fighter B",
             "status":{"type":{"completed":true}},
             "competitions":[{"competitors":[
                {"homeAway":"home","score":{"$ref":"https://fixture.example/score/home"},"team":{"$ref":"https://fixture.example/team/a"}},
                {"homeAway":"away","score":{"$ref":"https://fixture.example/score/away"},"team":{"$ref":"https://fixture.example/team/b"}}]}]}
        ]}"#;
        let updates = espn_updates(
            &schedule_source(SCHEDULE_FORMAT_ESPN),
            body,
            now()?,
            &options(),
        )?;
        assert_eq!(
            updates.first().map(|update| update.title.as_str()),
            Some("Fighter A vs Fighter B")
        );
        Ok(())
    }

    #[test]
    fn espn_scoreboard_uses_fightcenter_link_and_bout_result() -> Result<()> {
        let body = br#"{"events":[{
            "id":"event-1","date":"2026-08-23T12:00Z","name":"UFC Example",
            "$ref":"https://site.api.espn.com/apis/sports/event-1",
            "links":[
                {"href":"https://site.api.espn.com/apis/sports/event-1","rel":["event"]},
                {"href":"https://www.espn.com/mma/fightcenter/_/id/event-1/league/ufc","rel":["summary"]}
            ],
            "status":{"type":{"completed":true}},
            "competitions":[{
                "id":"bout-42","date":"2026-08-23T12:00Z","type":{"abbreviation":"Welterweight"},
                "status":{"period":2,"displayClock":"3:14","type":{"completed":true}},
                "competitors":[
                    {"id":"202","type":"athlete","order":2,"winner":false,"athlete":{"displayName":"Fighter B"},"records":[{"type":"total","summary":"9-3-0"}]},
                    {"id":"101","type":"athlete","order":1,"winner":true,"athlete":{"displayName":"Fighter A"},"records":[{"type":"total","summary":"12-1-0"}]}
                ]
            }]
        }]}"#;
        let updates = espn_scoreboard_updates_from_body(
            &schedule_source(SCHEDULE_FORMAT_ESPN_SCOREBOARD),
            body,
            now()?,
            &options(),
        )?;
        let update = updates.first().context("missing UFC bout result")?;
        assert_eq!(update.fixture_identity, "espn_scoreboard:id:bout-42");
        assert_eq!(
            update.title,
            "Fighter A (12-1-0) defeated Fighter B (9-3-0)"
        );
        assert_eq!(
            update.competition,
            "UFC Example, Welterweight, round 2 at 3:14"
        );
        assert_eq!(
            update.url,
            "https://www.espn.com/mma/fightcenter/_/id/event-1/league/ufc"
        );
        assert_eq!(
            update
                .images
                .iter()
                .map(|image| image.url.as_str())
                .collect::<Vec<_>>(),
            [
                "https://a.espncdn.com/i/headshots/mma/players/full/101.png",
                "https://a.espncdn.com/i/headshots/mma/players/full/202.png"
            ]
        );
        Ok(())
    }

    #[test]
    fn espn_scoreboard_collapses_upcoming_bouts_into_one_card() -> Result<()> {
        let body = br#"{"events":[{
            "id":"event-2","date":"2026-08-30T12:00Z","name":"UFC Example",
            "links":[{"href":"https://www.espn.com/mma/fightcenter/_/id/event-2/league/ufc","rel":["event"]}],
            "competitions":[
                {"id":"bout-1","competitors":[{"type":"athlete","athlete":{"displayName":"Alpha"}},{"type":"athlete","athlete":{"displayName":"Beta"}}]},
                {"id":"bout-2","competitors":[{"type":"athlete","athlete":{"displayName":"Gamma"}},{"type":"athlete","athlete":{"displayName":"Delta"}}]}
            ]
        }],"leagues":[{"logos":[{"href":"https://a.espncdn.com/ufc.png","rel":["default"]}]}]}"#;
        let updates = espn_scoreboard_updates_from_body(
            &schedule_source(SCHEDULE_FORMAT_ESPN_SCOREBOARD),
            body,
            now()?,
            &options(),
        )?;
        assert_eq!(updates.len(), 1);
        assert_eq!(
            updates.first().map(|update| update.title.as_str()),
            Some("UFC Example")
        );
        assert_eq!(
            updates
                .first()
                .and_then(|update| update.images.first())
                .map(|image| image.url.as_str()),
            Some("https://a.espncdn.com/ufc.png")
        );
        Ok(())
    }

    #[test]
    fn espn_scoreboard_appends_the_configured_date_window() -> Result<()> {
        assert_eq!(
            espn_range_endpoint(
                "https://site.api.espn.com/apis/site/v2/sports/mma/ufc/scoreboard?limit=50",
                now()?,
                &options(),
            ),
            "https://site.api.espn.com/apis/site/v2/sports/mma/ufc/scoreboard?limit=50&dates=20260821-20260831"
        );
        Ok(())
    }

    #[test]
    fn riot_rankings_include_the_full_second_place_tie() -> Result<()> {
        let response: RiotStandingsResponse = serde_json::from_str(
            r#"{"data":{"standings":[{"stages":[{"sections":[{"rankings":[
                {"ordinal":2,"teams":[{"code":"B","name":"Beta"}]},
                {"ordinal":1,"teams":[{"code":"A","name":"Alpha"}]},
                {"ordinal":2,"teams":[{"code":"C","name":"Gamma"}]}
            ]}]}]}]}}"#,
        )?;
        let rankings = riot_first_rankings(&response).context("missing rankings")?;
        let teams = riot_top_two_rankings(rankings);
        assert_eq!(
            teams
                .iter()
                .map(|team| team.code.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "C"]
        );
        Ok(())
    }

    #[test]
    fn riot_schedule_filter_matches_code_then_name() -> Result<()> {
        let body = br#"{"data":{"schedule":{"events":[
            {"startTime":"2026-08-25T10:00:00Z","match":{"id":"code","teams":[{"code":"TOP","name":"Renamed"},{"code":"X","name":"Other"}]}},
            {"startTime":"2026-08-26T10:00:00Z","match":{"id":"name","teams":[{"code":"NEW","name":"Second Team"},{"code":"Y","name":"Other"}]}},
            {"startTime":"2026-08-27T10:00:00Z","match":{"id":"other","teams":[{"code":"NO","name":"Nobody"},{"code":"Z","name":"Other"}]}},
            {"startTime":"2026-08-28T10:00:00Z","match":{"id":"placeholder","teams":[{"code":"TOP","name":"Renamed"},{"code":"TBD","name":"TBD"}]}}
        ]}}}"#;
        let response = super::parse_riot_schedule(body)?;
        let events = riot_schedule_events(&response)?;
        let selection = riot_team_selection(vec![
            super::RiotRankedTeam {
                code: " top ".to_owned(),
                name: "Alpha".to_owned(),
            },
            super::RiotRankedTeam {
                code: String::new(),
                name: " second   team ".to_owned(),
            },
        ]);
        let matching = events
            .iter()
            .filter(|event| riot_event_matches_selection(event, &selection))
            .filter_map(|event| event.match_up.as_ref().map(|match_up| match_up.id.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(matching, ["code", "name"]);
        Ok(())
    }

    #[test]
    fn sports_section_is_separate_deduplicated_and_timezone_aware() -> Result<()> {
        let upcoming = SportsUpdate {
            fixture_identity: "espn:id:401".to_owned(),
            source_key: "team_alpha".to_owned(),
            source_label: "Team Alpha".to_owned(),
            status: SPORTS_STATUS_UPCOMING.to_owned(),
            title: "Team ] Beta vs Team Alpha".to_owned(),
            competition: "Example [League".to_owned(),
            url: "https://fixture.example/events/401?next=>".to_owned(),
            starts_at: DateTime::parse_from_rfc3339("2026-08-30T12:00:00+00:00")?
                .with_timezone(&Utc),
            images: Vec::new(),
        };
        let updates = prepare_sports_updates(vec![upcoming.clone(), upcoming]);
        let section = render_sports_section(&updates, chrono_tz::America::New_York);
        assert_eq!(updates.len(), 1, "duplicate fixture survived");
        assert!(section.starts_with("## Sports\n\n### Upcoming fixtures"));
        assert!(section.contains("Sun Aug 30, 8:00 AM EDT"));
        assert!(section.contains(
            "- [Team \\] Beta vs Team Alpha](<https://fixture.example/events/401?next=%3E>) - Example \\[League"
        ));
        Ok(())
    }

    #[test]
    fn riot_schedule_reports_completed_match_score() -> Result<()> {
        let body = br#"{"data":{"schedule":{"events":[
            {"startTime":"2026-08-23T10:00:00Z","state":"completed","league":{"name":"Example Esports"},
             "match":{"id":"match-1","teams":[
                {"name":"Blue","image":"http://static.lolesports.com/blue.png","result":{"outcome":"win","gameWins":3}},
                {"name":"Red","image":"https://static.lolesports.com/red.png","result":{"outcome":null,"gameWins":1}}]}}
        ]}}}"#;
        let updates = riot_updates(
            &schedule_source(SCHEDULE_FORMAT_RIOT),
            body,
            now()?,
            &options(),
        )?;
        let update = updates
            .iter()
            .find(|update| update.title == "Blue 3 - 1 Red")
            .context("missing Riot match result")?;
        assert_eq!(
            update
                .images
                .iter()
                .map(|image| image.url.as_str())
                .collect::<Vec<_>>(),
            [
                "https://static.lolesports.com/blue.png",
                "https://static.lolesports.com/red.png"
            ]
        );
        Ok(())
    }

    #[test]
    fn configured_window_boundaries_are_inclusive() -> Result<()> {
        let moment = now()?;
        let options = options();
        let upcoming = moment
            .checked_add_signed(TimeDelta::days(7))
            .context("calculate upcoming boundary")?;
        let recent = moment
            .checked_sub_signed(TimeDelta::days(3))
            .context("calculate result boundary")?;
        let old = moment
            .checked_sub_signed(TimeDelta::days(4))
            .context("calculate old result")?;
        assert_eq!(
            update_status(upcoming, false, moment, &options),
            Some(SPORTS_STATUS_UPCOMING)
        );
        assert_eq!(
            update_status(recent, true, moment, &options),
            Some(SPORTS_STATUS_FINAL)
        );
        assert_eq!(update_status(old, true, moment, &options), None);
        Ok(())
    }

    #[test]
    fn provider_links_and_images_reject_active_credentials_and_unexpected_hosts() {
        assert!(provider_http_url("javascript:alert(1)").is_none());
        assert!(provider_http_url("https://user@example.test/event").is_none());
        assert!(provider_image_url("https://127.0.0.1/logo.png").is_none());
        assert!(provider_image_url("https://tracker.example/logo.png").is_none());
        assert!(provider_image_url("https://a.espncdn.com/logo.png").is_some());
        assert!(provider_image_url("http://static.lolesports.com/logo.png").is_some());
    }

    #[test]
    fn kickoff_parser_handles_espn_and_riot_timestamps() {
        assert!(parse_kickoff("2026-03-17T20:00Z").is_some());
        assert!(parse_kickoff("2026-03-17T20:00:00Z").is_some());
        assert!(parse_kickoff("not-a-date").is_none());
    }
}
