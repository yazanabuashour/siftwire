# Maintainer Notes

This repository is public and includes a production `siftwire` runner binary and a single-file Siftwire skill. Keep maintainer docs honest about the actual supported surface.

Keep `skills/siftwire/SKILL.md` thin. Substantial skill growth must first ask
whether the detail belongs in an existing runner action, a new narrow
runner-owned workflow action, compact runner help, or maintainer/eval docs. If
temporary skill text is still needed, explain why runner JSON results,
rejections, and caller judgment are insufficient, and document follow-up work
to remove or replace that text. Do not repair routine brief or configuration UX
by adding long-lived workflow recipes to the skill.

Recurring security operations are tracked in [docs/security-operations.md](security-operations.md). Use that runbook for dependency review cadence, advisory rehearsal, threat-model refreshes, and deeper testing expectations.

## Initial Setup

Install the repository-pinned developer tools:

```bash
mise install
```

Use GitHub issues and pull requests for tracked project work.

## Public Repo Expectations

- Contributors must be able to work from the public Git repository and GitHub project history.
- Policy and workflow files are part of the public contract and should stay reviewable in Git alone.
- Do not document machine-absolute filesystem paths in committed docs.
- Do not assume private infrastructure, deploy secrets, or internal services exist unless they have been added explicitly.
- Do not commit personal source inventories, outlet policy evidence, delivery logs, run history, `.openclaw` content, workspace backups, or local SQLite databases.

## Repository Administration

Current readiness assumptions:

- `main` is the protected default branch.
- Pull requests run only untrusted-safe validation with read-only token scope.
- Pull requests enforce formatting, lint, unit tests, and skill validation.
- GitHub Releases are created from version tags in the `v0.y.z` form.
- Release publication runs in a protected `release` environment with narrowly scoped write permissions.
- Security reports are expected through GitHub private vulnerability reporting.

Current review enforcement nuance:

- The repository currently has a single maintainer account.
- `main` should require pull requests, status checks, conversation resolution, and one approving review, but code-owner review enforcement and admin enforcement may remain off until a second maintainer can satisfy the review requirement.
- Tighten code-owner review enforcement, admin bypass, and maintainer isolation once a second maintainer can satisfy those controls without blocking routine maintenance.

Untrusted pull request policy:

- Pull request workflows must stay fork-safe and use read-only `contents` permission unless a specific trusted workflow boundary justifies more.
- Do not expose release, package, deployment, or private infrastructure secrets to code from untrusted forks.
- Avoid `pull_request_target` for workflows that check out or execute contributor-controlled code.
- Dependency review, policy checks, formatting, linting, and tests are acceptable untrusted PR validation surfaces when they run without secrets.

Maintainer and automation isolation:

- Prefer `GITHUB_TOKEN` with explicit job-scoped permissions over personal access tokens or long-lived bot credentials.
- Use a dedicated low-privilege bot identity only when new automation needs privileges that `GITHUB_TOKEN` cannot safely provide.
- Keep release and deployment writes behind the protected `release` environment.
- Do not use self-hosted runners for untrusted pull requests. Only consider self-hosted runners for trusted branches or tags after documenting isolation, secret exposure, cleanup, and network-access controls.

When changing GitHub settings, keep the repo aligned with:

- [SECURITY.md](../SECURITY.md) for disclosure handling and patch timing.
- [docs/security-operations.md](security-operations.md) for recurring security operations and deeper testing expectations.
- [.github/CODEOWNERS](../.github/CODEOWNERS) for sensitive file ownership.
- [.github/workflows/pull-request.yml](../.github/workflows/pull-request.yml) for fork-safe checks.
- [.github/workflows/release.yml](../.github/workflows/release.yml) for release publication, checksums, SBOMs, and attestations.

## Release Publication

Public releases use annotated semantic version tags in the `v0.y.z` range. The release contract is a tagged release for the `siftwire` binary and the single-file Siftwire skill. Tag a version like `v0.2.0`, push the tag, and let the release workflow:

- validate release notes, formatting, lint, skill validation, and tests before publish
- build binaries with `siftwire --version` set from the tag
- require `docs/release-notes/<tag>.md` and a matching `CHANGELOG.md` entry before publishing
- create or reuse only a draft GitHub Release before assets are attached
- use `docs/release-notes/<tag>.md`, for example `docs/release-notes/v0.2.0.md`, as the GitHub Release body
- keep release-note paragraphs and list items on one source line so GitHub Releases and API clients do not show hard-wrapped prose
- attach four platform binaries, the skill archive, canonical source archive, root installer, SHA256 checksums, and SPDX SBOM
- verify the draft release has the expected asset set before publication
- generate GitHub attestations for the published assets
- publish the draft only after all assets and attestations are ready, then verify the release is latest

The `release` environment should remain protected so only approved maintainers can publish release assets.

Before tagging, add `docs/release-notes/<tag>.md`, update `CHANGELOG.md`, and run `mise exec -- ./scripts/validate-release-docs.sh <tag>` locally. The release workflow runs the same check before publishing and does not fall back to generated GitHub release notes.

For ADR, POC, eval, promotion, and deferred-capability work, report safety,
capability, and UX quality separately. Exact-command or scripted eval rows prove
capability only. If routine success depends on exact JSON, command choreography,
or skill-only recipes, classify the gap as Siftwire workflow ceremony and
compare runner-owned surface candidates before expanding
`skills/siftwire/SKILL.md`.

GitHub release immutability is enabled. Treat published tags and assets as immutable; fix bad artifacts with a new patch release instead of replacing an existing release.
