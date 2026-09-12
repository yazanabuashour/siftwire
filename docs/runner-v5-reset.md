# Upgrade to v0.9.0 with a fresh database

v0.9.0 is a breaking compatibility reset, not an automatic migration from
v0.8.0 or earlier. The runner reports `siftwire-runner/v5`. Use the v0.9.0
runner and agent skill; build the optional console and web assets from the
matching tag or source archive.

## Keep earlier data separate

The runner initializes empty databases and rejects incompatible database
formats. It no longer upgrades schemas, backfills delivery records, converts
retired source settings on read, or infers evidence from old delivery formats.

Before switching an installation:

1. Stop scheduled collection and finish any pending delivery with the old runner.
2. Preserve the old database and its matching released runner for historical
   reads. Do not delete the database or edit its schema version to bypass checks.
3. Select a new external database path with `SIFTWIRE_DATABASE_PATH` or `--db`.
4. Install matching runner, console, web assets, and skill versions together.
5. Inspect and approve current source and publisher configuration before writing
   it through `config`. There is no operational-state import path.

A fresh database has no delivery history or latest-seen state. Recent-delivery
suppression starts empty, and Required feeds follow fresh-source selection.

## Update callers

Require `siftwire-runner/v5` before configuration writes or collection. The
`prepared-delivery/v1`, `sports-updates/v1`, and `current-news/v1` capabilities
remain available.

- Use `--db`, not the removed Go-style `-db` spelling.
- Use `rss` for RSS and Atom feeds. Retired `atom`, Observe/audit,
  `always_report`, and unsupported processing choices have no read adapters.
- Sports appear through `sports_updates` and prepared bodies, not duplicated
  ordinary `must_include` items. They remain outside normal candidate slots.
- Read recent suppression from `suppressed_recent`; `suppressed` describes
  same-run duplicate evidence rather than repeating recent suppression.
- Read `delivery_status`, not the removed `selected` compatibility boolean.
- Prepared items reference recorded evidence directly; old title/URL matching
  does not substitute for references.
- Console source priorities use decimal strings, not JSON numbers. Confirmed
  briefs display saved HTML without legacy Markdown or story-link fallbacks.

See the [runner contract](runner-contract.md), [run history](run-history.md), and
[console guide](console.md) for current behavior. Earlier migration guides and
release reports describe their published releases, not v0.9.0.
