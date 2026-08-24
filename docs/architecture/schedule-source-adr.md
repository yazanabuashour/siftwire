# ADR: Schedule Sources For Fixture Awareness

## Status

Accepted. Fixture terminology lives in the repository glossary (`CONTEXT.md`).

## Context

Briefs cover news but not upcoming fixtures for followed teams. Live scores and
play-by-play stay with external services; SiftWire's twice-daily cadence cannot
compete there, and per-league live integrations would be high-maintenance.
Fixture lines (who plays whom, when) and morning-after result recaps are
calendar and news facts that fit the brief format.

Structured schedule data comes from ESPN's keyless public API (UEFA Champions
League, NFL, NBA, UFC — UFC via a two-step core-API fetch) and Riot's LoL
esports API, which only works with Riot's public frontend key that Riot can
rotate without notice. ESPN endpoints are undocumented and changeable.
TheSportsDB's free tier offers too little lookahead (one event per call). The
timing requirement — a fixture appears in the two runs before kickoff — needs
schedule awareness (kickoff time relative to run time) that plain feed items
cannot express.

## Decision

Add a `sports_schedule` source kind beside rss, atom, and github_release. One
source per followed competition. Each source names its endpoint URL and a
schedule format that selects the response parser; Riot sources carry Riot's
public frontend key in an optional source API key field.

A fixture becomes a must-include run item when kickoff falls within the next 24
hours, so it appears in every brief run before kickoff; at the 12-hour run
cadence that is usually the two preceding runs. Schedule items bypass
latest-seen dedup, since the window, not dedup, governs repetition, and
cross-source duplicates collapse on the provider fixture identity. Fixture
lines show competition, teams, and kickoff time in Central time. Off-season
gaps are normal, not errors.

Live scores, play-by-play, and structured stakes remain out of scope; preview
and recap articles from team news feeds carry the narrative.

ESPN's edge answers 403 for unrecognized user agents while allowing recognized
scripted-client agents (probed 2026-08-24: `siftwire/0.4` and a browser agent
-> 403; `curl/8.5.0` and `python-requests/2.31.0` -> 200). Schedule fetches
identify as a scripted client for these endpoints.

## Consequences

- The runner gains a third-party JSON fetch surface beyond GitHub's API;
  undocumented endpoint changes surface as source health warnings, never as
  broken briefs.
- A Riot key rotation degrades only LoL fixture coverage, with a health
  footnote.
- Fixtures appear in two consecutive runs by window arithmetic rather than
  dedup state.
- Adding a league is configuration (a source), not code, when its schedule fits
  an implemented format.
