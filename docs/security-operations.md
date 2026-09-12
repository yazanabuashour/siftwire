# Security Operations

This runbook defines recurring security work for SiftWire maintainers and complements [SECURITY.md](../SECURITY.md). Do not put private vulnerability details in public issues, pull requests, release notes, or this document.

## Cadence

- Weekly: triage Dependabot pull requests, dependency-review failures, and new vulnerability alerts for Rust crates and GitHub Actions.
- Monthly: review the GitHub Security tab, private vulnerability reporting state, Dependabot alert backlog, and deferred security work.
- Quarterly: rehearse the advisory workflow, refresh the threat model, and confirm that release, automation, and maintainer-isolation assumptions still match the repository.
- Release-bound: review security impact before tagging any release that changes `.github/workflows/release.yml`, `install.sh`, `skills/siftwire/SKILL.md`, SQLite schema behavior, runner JSON behavior, feed fetching, or release verification docs.

## High-Risk Surfaces

- Local SQLite brief configuration, latest-seen state, health warnings, delivery records, and recent sent items.
- Runner JSON operations in `crates/siftwire/src/runner/`, `crates/siftwire/src/engine/`, and `crates/siftwire/src/storage/`, especially configuration replacement, delivery recording, URL handling, and source validation.
- Agent-facing task policy in `skills/siftwire/SKILL.md`, including direct-reject rules and instructions that prevent bypassing the runner.
- Network fetch providers and URL processing for RSS, Atom, GitHub releases, feed redirects, Google News URL resolution, and outlet extraction.
- Install and release pipeline files: `install.sh`, `.github/workflows/release.yml`, `docs/release-verification.md`, `CHANGELOG.md`, and `docs/release-notes`.
- GitHub Actions and repository policy files under `.github`, including token permissions, environment protection, CODEOWNERS, dependency review, and branch protection assumptions.

## Review Workflow

1. Open or update a GitHub issue for public follow-up work. Keep sensitive findings in a private advisory until disclosure is safe.
2. Classify findings using the severity expectations in `SECURITY.md`.
3. Keep exploit details private until a fix or mitigation is available.
4. For dependency updates, prefer the smallest reviewable update that clears the alert and keeps `mise exec -- cargo test --locked` passing.
5. For workflow or release-pipeline changes, verify that token permissions remain job-scoped. Check that untrusted pull requests receive no release, deployment, package, or secret-bearing permissions.
6. For skill or runner policy changes, confirm the public docs, skill contract, tests, and release notes remain aligned.

## Deeper Testing Expectations

- Runner configuration and delivery behavior should have focused validation and idempotency tests before release.
- Storage migrations should include migration tests and explicit compatibility expectations.
- Skill policy changes should run `mise exec -- ./scripts/validate-agent-skill.sh skills/siftwire` and the relevant production agent eval gate before release.
- Release-pipeline changes should run `mise exec -- ./scripts/validate-release-docs.sh <tag>` for the target tag and verify the expected release asset, checksum, SBOM, and attestation behavior.
- Add fuzzing or property-style tests when parsing, normalization, target resolution, or import logic becomes complex enough that table tests no longer cover realistic malformed input.
- Abuse-case tests should be added before introducing remote APIs, hosted services, secrets-backed integrations, self-hosted runners, or broad automation write privileges.

## Advisory Rehearsal

At least quarterly, maintainers should rehearse the private advisory flow without publishing a real advisory:

- Confirm GitHub private vulnerability reporting is enabled and reachable from the repository Security tab.
- Confirm the private fix path, release notes redaction approach, patch-tag process, and release verification steps are still documented.
- Confirm emergency release expectations still match the current artifact set: platform binaries, skill archive, installer, checksums, SBOM, and attestations.
- Record each public gap in a GitHub issue and each sensitive gap in the private advisory.
