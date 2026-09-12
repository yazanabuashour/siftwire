# Upgrade to runner protocol v4

Historical guide for v0.8.0 only. This checkout instead requires the
[v5 compatibility reset](runner-v5-reset.md).

Use this guide to upgrade to v0.8.0, which uses `siftwire-runner/v4` and
`current-news/v1`. Optional RSS sources now select stories by publication time
within a rolling 24-hour window. Required feeds, GitHub releases, sports, and
prepared-delivery recovery retain their existing handling.

If upgrading from v0.6.1 or another v2 release, also apply the
[v3 source and delivery changes](runner-v3-migration.md). No intermediate runner
installation is required.

## Prepare the upgrade

1. Stop new collection work before replacing installed artifacts.
2. Reconcile any active transport or confirmation. Do not collect a replacement
   run or resend mail merely to upgrade.
3. Keep a consistent database backup and the matching old runner and skill.
   Keep backups outside the repository.
4. Update the runner, skill, and process consumers together. If you use the
   console, update it and its web assets too.

No source field or configuration rewrite is required for the v4 change. Optional
source markers remain saved but unused. A later switch to Required reuses the
retained marker, not observations made while the source was optional. Saved
Google News canonicalization remains configurable, but only Required feeds
apply it. Optional feeds keep original links.

Existing prepared plans retain their exact bodies. An identical preparation
retry can recover a plan before confirmation, and confirmation remains
retry-safe with the same plan ID. Neither action reselects or refetches stories.

## Update protocol checks and fetch displays

Require `siftwire-runner/v4`, `prepared-delivery/v1`, and `current-news/v1` for
the current agent skill. If a transitional host supports v2, v3, or v4, test and
accept each version explicitly. Require `current-news/v1` for v4. Do not accept
unknown future protocols.

Handle these fetch fields in both brief results and archive displays:

- Treat `fetch_status[].new_items` and archived `fetch[].new_items` as nullable.
- For optional RSS, display `current_news` publication bounds and date counts.
  Label `eligible_items` as eligible, not new or first seen. Stale, undated, and
  future counts describe excluded publication dates, not editorial rejections.
- Show an unavailable selection count for failed new checks, not zero.
- Preserve the meaning of old numeric counts, including old failure placeholders.
  Do not infer historical selection modes from today's source settings.

The runner adds `fetch_log.selection_json` without rewriting old fetch rows or
source state. New nullable counts and current-news statistics live there. When
that evidence declares a count unavailable, the legacy non-null `new_item_count`
is not an observation.

## Verify before resuming collection

Check that configuration inspection returns the expected protocol and
capabilities. Test your consumer's nullable counts and current-news labels with
synthetic data. Compare saved plans, confirmed bodies, and old archive records
against the backup without collecting from production sources.

For checkout validation, run the [development checks](../CONTRIBUTING.md) and
[production-agent scenarios](evals/agent-production.md). The
[current-news decision](architecture/current-news.md) defines the behavior and
its limits.

## Avoid an unsafe downgrade

Do not blindly downgrade after v4 collection. Older readers ignore the new
selection evidence and can mislabel current-news counts. Reconcile pending work
and assess retained v4 history before restoring an older runtime. Never replace
current history or delivery state with a pre-upgrade backup just to roll back
binaries.
