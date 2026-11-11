# Changelog

Public release notes are mirrored in `docs/release-notes/`.

## Unreleased

## v0.12.0

- Remove Prooflane: the launcher calls SiftWire directly and no longer exports to Axiom. Runner protocol v5 and the database schema are unchanged.
- Run selection with Pi's default model and thinking instead of `model-role`; run records drop `configured_model`.
- Shorten the embedded selector prompt and production skill.

## v0.11.0

- Add optional publication of each finished brief to a Notes vault with `siftwire notes publish`. It is off until configured and does not change email, confirmation, runner protocol v5, or the database schema. See `docs/automation.md`.
- Ship the scheduled automation launcher and selector prompt inside the binary, started with `siftwire automation`. Runner protocol v5 and the database schema are unchanged. See `docs/automation.md`.
- Embed a neutral selector prompt. Hosts keep their own priorities in a private prompt set through `SIFTWIRE_RENDER_PROMPT`.

## v0.10.1

- Collect upcoming ESPN soccer fixtures alongside results and link supported ESPN schedules to public game pages.
- Keep current source failures and empty optional-news feeds visible in coverage notices; label upcoming MMA times as event starts.
- Fix image-free sports rows, adopt the responsive email header, and remove duplicate publisher suffixes from HTML headlines.
- Expose publisher feed summaries for selection research without changing prepared-delivery prose, runner protocol v5, or the database schema.

## v0.10.0

- Remove the optional web console, frontend, UI-only dependencies, installer, and build checks. Configuration inspection and edits remain available through the installed JSON runner; operator archive commands and prepared email rendering are unchanged.

## v0.9.3

- Reject promotional sports livestream headlines and embedded query loaders before optional selection, recording exclusions in existing evidence. Keep legitimate match reporting and structured fixtures/results available.

- Distinguish config and preparation runtime failures from validation refusals. Report unreadable confirmation history instead of silently omitting it; same-plan confirmation retries remain safe after a committed write.
- Serialize durable collections so overlapping runs cannot reuse or regress Required markers. Unix locks the database inode; Windows locks a canonical-path sidecar without blocking SQLite. Interrupted collection is still not safe to retry automatically.
- Verify earlier answers in multi-turn evaluation, not just the last turn.
- Put ESPN scoreboard dates in the URL query rather than its fragment, replacing conflicting endpoint dates with the configured sports window.
- Correct fractional-second ordering for recent delivery history and repeat suppression.
- Reject non-object JSON requests before opening storage. Nullable request fields, runner protocol v5, and the database schema remain unchanged.
- Require literal `printf` payloads in evaluator pipelines and flag file redirections, outer-shell expansions, and quote breakouts in wrapped commands as hygiene failures. Evaluation remains an after-the-fact receipt check, not a sandbox.
- Apply the console runner deadline to stdin writes and output collection, and accept bracketed IPv6 Host headers with numeric ports.

## Releases

- [v0.12.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.12.0)
  removes Prooflane and runs selection with Pi's default model.
- [v0.11.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.11.0)
  ships the scheduled automation launcher inside the binary, embeds a neutral
  selector prompt, and adds optional Notes publication. See the
  [release notes](docs/release-notes/v0.11.0.md).

- [v0.10.1](https://github.com/yazanabuashour/siftwire/releases/tag/v0.10.1)
  improves fixture collection, email layout, research context, and source-coverage
  visibility without rewriting saved plans. See the
  [release notes](docs/release-notes/v0.10.1.md).

- [v0.10.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.10.0)
  removes the optional web console and keeps configuration inspection agent-first
  through the installed runner. Existing v0.9.x databases remain compatible.
  See the [release notes](docs/release-notes/v0.10.0.md).

- [v0.9.3](https://github.com/yazanabuashour/siftwire/releases/tag/v0.9.3)
  rejects promotional livestream candidates and embedded query loaders, documents
  major-only sports news selection, and includes the correctness fixes above.
  See the [release notes](docs/release-notes/v0.9.3.md).

- [v0.9.2](https://github.com/yazanabuashour/siftwire/releases/tag/v0.9.2)
  shares email presentation through the standalone `email-ui` crate while
  preserving the existing brief layout, supported links, and immutable delivery
  bodies. No separate renderer installation or database reset is required.
  See the [release notes](docs/release-notes/v0.9.2.md).
- [v0.9.1](https://github.com/yazanabuashour/siftwire/releases/tag/v0.9.1)
  makes agent evaluation harness-neutral through an executable JSON contract,
  adds a Pi implementation and model-free smoke adapter, and records actual
  tool executions separately from native lifecycle events. The production
  runner, skill, database schema, and delivery contract remain unchanged.
  See the [release notes](docs/release-notes/v0.9.1.md).
- [v0.9.0](https://github.com/yazanabuashour/siftwire/releases/tag/v0.9.0)
  requires runner protocol v5, matching consumers, and a fresh database instead
  of migrating old configuration or operational state. It deepens source
  outcomes, plan-based confirmation, and saved-plan replay without changing
  supported source-selection semantics or adding agent calls. The optional
  console supports explicitly configured hostnames behind trusted proxies.
  Read the [compatibility reset](docs/runner-v5-reset.md) and
  [release notes](docs/release-notes/v0.9.0.md) before upgrading.
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
