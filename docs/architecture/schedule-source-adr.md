# ADR: Schedule sources for fixture awareness

## Status

Accepted. Fixture terminology lives in the repository glossary (`CONTEXT.md`).
The [v5 compatibility reset](../runner-v5-reset.md) supersedes the earlier
ordinary-item sports projection; only structured sports updates remain.

## Context

Briefs cover news but scheduled sport needs different repetition rules. A normal
item should disappear after delivery. A fixture should remain visible while it
is approaching, and a final result should remain visible long enough to survive
a missed brief.

The first schedule implementation treated fixtures as must-include items during
the 24 hours before kickoff. It used Central time and never parsed completed
events. That design gave little notice and made fixtures consume the normal
item limit.

On 2026-08-25, the operator requested seven days of recurring pre-game notice,
three days of recurring post-game results, and a sports section outside the
normal item limit. This request is the receipt for those defaults. Both windows
and the time zone need durable configuration because brief cadence and operator
location can differ.

Structured schedule data comes from ESPN's keyless public API and Riot's LoL
esports API. Riot requires the public frontend key, which Riot can rotate
without notice. ESPN endpoints are undocumented and changeable.

## Decision

Keep `sports_schedule` as a source kind beside `rss` and `github_release`.
The `rss` kind reads RSS and Atom documents. One schedule source tracks one team,
athlete event list, league, or competition. `schedule_format` selects `espn`,
`espn_scoreboard`, or `riot`. Protocol v3 retires the unused ESPN Core parser;
see `docs/runner-v3-migration.md` for the source migration boundary.

A schedule fetch returns recurring sports updates separately from normal brief
items:

- Upcoming fixtures appear during the `sports_pre_game_days` window before
  kickoff. The default is 7 days.
- Completed results appear from kickoff through `sports_post_game_days` after
  kickoff. The default is 3 days.
- `sports_timezone` controls rendered kickoff times. It stores an IANA time zone
  and defaults to `America/Chicago` to preserve existing behavior.

The runner returns `sports_section` and structured `sports_updates` for
inspection. Its `prepare_delivery` action owns final composition: it places
sports after normal brief bullets and before health, keeps sports outside
`max_delivery_items`, and persists one immutable delivery plan.

Sports updates do not appear in `must_include` or consume candidate slots.
Prepared plans retain their sports entries separately from ordinary collection
rows; those sports entries have empty `run_item_ids`.

ESPN and Riot completed states produce result lines. Scores render when the
provider supplies them. A completed event without scores still renders as
final. The post-game window uses kickoff time because neither provider offers a
uniform completion timestamp.

Use ESPN's public UFC scoreboard instead of exposing ESPN Core JSON links. One
upcoming card is enough; completed cards show the individual bout results that
the operator requested. All links go to FightCenter.

Keep Riot standings policy on each league source. `standings_top_two` resolves
the current leaders on every run and includes the full second-place tie. It
fails closed when rankings cannot be verified instead of returning every match.
The runner contract defines the stable parsing and fallback behavior.

Live scores and play-by-play remain out of scope. Off-season gaps are normal,
not errors.

## Consequences

- Sports reminders no longer displace news, blogs, or release items.
- Fixture and result repetition follows time windows rather than latest-seen or
  recent-delivery suppression.
- Provider failures remain source health warnings and never break the rest of a
  brief.
- A Riot key rotation or standings response change degrades only Riot schedule
  coverage.
- Current consumers use `prepared-delivery/v1`; the runner owns sports placement
  and exact rendering. Historical messages keep their recorded representation.
