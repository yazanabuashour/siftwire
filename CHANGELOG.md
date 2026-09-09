# Changelog

Public release notes are mirrored in `docs/release-notes/`.

## Releases

- [v0.8.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.8.0)
  collects optional RSS stories published in the past 24 hours, keeps original
  links, and reports publication-window counts. It removes legacy configuration
  and delivery writes, requires runner protocol v4, and documents the console
  with a synthetic example brief. Read the
  [upgrade guide](docs/runner-v4-migration.md) before updating consumers.
- [v0.6.1](https://github.com/yazanabuashour/siftwire/releases/tag/v0.6.1)
  keeps console pages at a stable width, bounds edit dialogs, exposes routine
  configuration directly, and revalidates HTML after upgrades.
- [v0.6.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.6.0)
  simplifies configuration and brief navigation, adds a searchable delivery
  archive, and makes reporting policy and historical evidence explicit without
  rewriting source state or saved emails.
- [v0.3.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.3.0)
  adds per-run selection evidence, operator archive commands, and an optional
  local web console. It removes retired product-name compatibility from path
  resolution and the installer.
- [v0.2.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.2.0)
  introduces the SiftWire name, ports the runner to Rust, and documents the JSON
  process contract without changing the SQLite schema.
- [v0.1.8](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.8)
  adds delivery history and a ready-to-send final answer to `record_delivery`.
- [v0.1.7](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.7)
  preserves Markdown links in previous brief bodies and strengthens history
  evaluation coverage.
- [v0.1.6](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.6)
  adds previous-brief context to runner output and separates runner, selection,
  and storage modules.
- [v0.1.5](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.5)
  improves canonicalization fallback, memoizes repeated Google News resolution,
  and backs off after resolver timeouts or rate limits.
- [v0.1.4](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.4)
  adds the SQLite-backed `max_delivery_items` option and delivery-limit checks.
- [v0.1.3](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.3)
  bounds canonicalization work, preserves feed order during parallel resolution,
  and applies publisher policy to extracted RSS outlets.
- [v0.1.2](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.2)
  adds an approved draft-and-apply workflow for legacy configuration migration.
- [v0.1.1](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.1)
  updates the skill name and runner-bypass refusal policy.
- [v0.1.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.1.0)
  is the first public release of the runner and skill.
