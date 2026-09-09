# Changelog

This CHANGELOG summarizes public releases. Full release notes are published with
GitHub Releases and mirrored in `docs/release-notes/`.

## Releases

- [v0.6.1](https://github.com/yazanabuashour/siftwire/releases/tag/v0.6.1)
  keeps console pages at a stable width, bounds edit dialogs, exposes routine
  configuration directly, and revalidates HTML after upgrades.
- [v0.6.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.6.0)
  simplifies configuration and brief navigation, adds a searchable delivery
  archive, and makes reporting policy and historical evidence explicit without
  rewriting source state or saved emails.
- [v0.2.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.2.0)
  presents the SiftWire name, ports the runtime to Rust under strict lint
  rules, documents the JSON process contract, and preserves the SQLite
  schema.
- [v0.1.8](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.8)
  adds `record_delivery` delivery-history output and a ready-to-send final
  answer so agents can return the current brief plus two prior deliveries
  without reconstructing history from large `run_brief` payloads.
- [v0.1.7](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.7)
  fixes previous-brief rendering guidance so recorded previous brief bodies keep
  their Markdown links, strengthens regression coverage for the history eval,
  and refreshes maintainer workflow instructions from the OpenClerk template.
- [v0.1.6](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.6)
  adds previous-brief context to brief runner JSON, splits runner and SQLite
  internals into smaller testable modules, broadens skill validation coverage,
  and records taste-review and candidate-surface decisions for future
  SiftWire workflow work.
- [v0.1.5](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.5)
  improves feed canonicalization fallback behavior, memoizes duplicate Google
  News article resolution within a brief run, and backs off Google News
  resolution after rate limits or timeouts without changing the runner CLI or
  install flow.
- [v0.1.4](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.4)
  adds a SQLite-backed `max_delivery_items` brief option, exposes the resolved
  limit in runner JSON, updates the production skill to honor it, and adds a
  targeted production-agent eval for configured delivery limits.
- [v0.1.3](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.3)
  bounds feed URL canonicalization work, preserves feed item ordering during
  parallel resolution, and makes RSS source outlet extraction effective for
  outlet policy matching without changing the runner CLI or install flow.
- [v0.1.2](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.2)
  allows user-directed legacy automation/config migration through a reviewed
  draft-and-apply SiftWire skill workflow and hardens skill markdown
  validation tests without changing runner behavior.
- [v0.1.1](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.1)
  refines the public SiftWire skill display name and production runner-bypass
  refusal policy without changing runner behavior.
- [v0.1.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.0)
  is the first public release of the local-first SiftWire runner and skill.

## Unreleased

- Narrow the process contract to `siftwire-runner/v3`: prepared-only delivery,
  one RSS/Atom feed kind, and threshold-owned required reporting. Retire observe
  writes, unused provider/extraction options, release URL overrides, source CLI
  shortcuts, and the SiftWire-owned Prooflane wrapper. Preserve historical data.
  See `docs/runner-v3-migration.md` before updating consumers.
- Add immutable prepared delivery plans so the runner, rather than each
  consumer, owns normal-item limits, sports placement, health placement, and
  Markdown, text, and HTML rendering.
- Add explicit runner protocol capabilities and recurring sports fixtures and
  results with configurable windows and time zone.
- Add UFC bout updates from ESPN scoreboards and an optional Riot standings
  filter that keeps matches involving teams in the top two positions, including
  boundary ties.
- Render a compact light email that blends with webmail message canvases and add
  provider team logos, UFC branding, and fighter headshots to sports rows.

## Releases

- [v0.3.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.3.0)
  adds persisted per-run selection evidence, operator `source` and `runs`
  inspection commands, and an optional local web console that drives the runner
  process contract, and removes retired product-name compatibility from path
  resolution and the installer.
