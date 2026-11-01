# Current-news selection

## Why optional feeds use publication time

All optional `rss` sources, including Atom feeds, use publication-time selection.
The operator chose a rolling 24-hour window for both Major and Highlights
reporting. This is a freshness policy, not a guarantee of finding every unseen
story. There is no source-level mode, article queue, or seen-ID ledger.

The 24-hour value is an explicit operator decision. It matches the existing
confirmed-delivery repeat window. It is not a measured feed-retention guarantee
or a new capacity limit.

## Selection order

For optional RSS, collection follows these steps:

1. Parses the whole fetched feed and keeps original item links. It never calls
   the Google News decoder or applies its five-entry admission limit.
2. Retains publication timestamps in the inclusive interval from collection
   time minus 24 hours through collection time. RSS uses declared publication
   metadata. Atom uses `published`, not `updated` alone. Missing or invalid dates,
   older dates, and future dates have separate visible exclusion counts.
3. Applies configured publisher extraction and policy locally. Block still
   excludes stories. A publisher suffix is feed metadata, not verified article
   provenance.
4. Collapses exact nonempty absolute URLs across sources. Exact normalized
   headlines collapse only within the effective `dedup_group`, which defaults
   to the source key. Relative URL equality matches only within the same source.
   Newer publication wins before the lower `priority_rank` breaks a tie.
5. Suppresses exact normalized headlines from confirmed normal deliveries in the
   preceding 24 hours. A changed headline at a reused URL remains eligible.

The agent chooses consequential stories from the remaining candidates and
recent brief context. The runner does not treat similar headline prefixes as
proof of the same event. Exact headline matches can still miss a substantive
article update with an unchanged headline. Changed headlines can be cosmetic.
Editorial selection must account for both limits and must not claim article-body
verification from feed metadata alone.

Optional collection neither reads nor advances source markers. Existing marker
rows remain intact. An unsent story can return on another collection while it
remains current and present in the upstream feed. That does not create a durable
backlog or guarantee discovery of omitted upstream entries.

## Required sources and saved history

Required RSS retains marker selection, date handling, and configured link
canonicalization. GitHub releases remain Required. Sports retain fixture and
result windows. Switching a source to Required reuses its preserved marker. It
does not reconstruct observations made while optional. Saving a canonicalization
choice on an optional source preserves it for Required handling but does not
activate decoding during optional collection.

Preparation remains network-free and renders the immutable collected selection.
Confirmation still records exact accepted bodies and supports idempotent retry.
Saved plans, messages, HTML, and evidence remain immutable within the current
database format.

Fetch logs store selection evidence without inferring publication windows from
current source settings. Current-news counts say `eligible`, not `new`. Failed
checks report an unknown selection count rather than zero.

## Compatibility and alternatives

Current-news behavior was introduced in `siftwire-runner/v4`. v0.9.0
requires `siftwire-runner/v5` with `current-news/v1` before writes or collection.
See the [compatibility reset](../runner-v5-reset.md).

A per-source mode would preserve optional selection by feed position but add a
setting the operator explicitly declined. A persistent seen ledger would track
past observations and add retention and replay obligations. An aggregator can
improve feed operations, but another RSS endpoint does not establish complete
membership or fix selection by feed position by itself. None is required here.

## Evidence required for acceptance

Safety requires unchanged Required, network, immutable-plan, confirmation, and
historical-evidence contracts. Synthetic process tests in
`crates/siftwire/tests/process_contract/current_news.rs` cover marker preservation,
reordered feeds, unsent eligibility, confirmed suppression, and archive readback.
Engine tests in `crates/siftwire/src/engine/tests.rs` cover date boundaries,
missing dates, and original-link handling.

Capability requires access to current tail entries beyond the old decoder limit,
without claiming exhaustive discovery or semantic event clustering.

User experience requires visible date exclusions and honest candidate labels.
Candidate volume and serialized input bytes must be measured separately from
model tokens and selection latency. A passing feed snapshot is not a measured
speedup or evidence of equivalent publisher coverage. Promotion still requires
repository gates and the approved checkpoint review. This document alone does
not authorize deployment.

## Optional-news admission follow-up

The current-news window establishes recency, not legitimacy. A sports feed can
contain fresh stream promotions that must not become editorial candidates.
Narrow runner-owned admission now rejects promotional livestream headlines and
embedded data/script query loaders, retaining dropped evidence through the
existing suppression and archive surfaces. This protects selection even when a
Google News link conceals its publisher URL; it does not fetch opaque links.

A publisher/domain blacklist alone cannot address the observed class, while a
new resolver, source kind, or reputation framework would add network and policy
contracts. Keep source authority and major-news judgment with the operator and
selector. Generic `high` reporting gates broad team feeds to major developments;
structured schedules remain the source for recurring fixtures and scores.

Safety: no new fetches, protocol fields, schema, or changes to immutable plans.
Capability: synthetic polluted-feed tests require rejected candidates to remain
unselectable while legitimate reports, major news, fixtures, and scores survive.
User experience: skip routine team coverage rather than filling spare slots.
Follow-up: admission is deliberately narrow; evaluate new evasion patterns and
false positives before extending it. The operator's selector evaluation belongs
to its owning automation, not a claim of universal spam detection.
