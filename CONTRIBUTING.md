# Develop SiftWire

## Set up a checkout

Install the tools pinned in `mise.toml` and `rust-toolchain.toml`, then install
evaluator dependencies. Install ShellCheck separately; `mise` does
not manage it, but the repository gate requires it:

```bash
mise install
mise exec -- bun install --frozen-lockfile
```

Run commands below from the repository root. The
[repository map](AGENTS.md#code-and-architecture) identifies module owners.
Evaluator TypeScript uses the checked-in compiler and lint policies under
`tools/`. Update those snapshots with their producer's export command, not by
editing generated code. Each snapshot's README describes the update procedure.

`tsconfig.tooling.json` overrides `skipLibCheck` because the evaluator's Pi,
Google GenAI, and gaxios dependency declarations are incompatible with the
selected Node.js and Bun types. Remove the override once those declarations are
compatible; project-owned tooling still receives strict typechecks and typed lint.

## Exercise the runner without personal state

Use an explicit disposable database for checkout development. Even inspection
and archive reads can initialize storage; omitting `--db` can touch your installed
database. Source-built binaries are for development, not production agent tasks.

```bash
run_root="$(mktemp -d)"
mise exec -- cargo build --locked --bin siftwire
printf '%s\n' '{"action":"inspect_config"}' | target/debug/siftwire config --db "$run_root/siftwire.sqlite"
```

A fresh database has runtime defaults and no sources. Use the
[runner contract](docs/runner-contract.md) for requests, compatibility checks,
rejections, and retry rules. Remove the disposable directory when finished.

## Run checks

Run the full gate before handing off a change:

```bash
mise exec -- ./scripts/ci.sh
```

The gate checks Rust toolchain selection, formatting, lint, workspace tests,
TypeScript types, adapter tests, shell scripts, the agent skill, and release
installation. Focused checks during development include
`mise exec -- cargo test --locked -p siftwire` and
`mise exec -- bun test tools/agent-eval`.

## Keep public artifacts safe

Do not commit personal source inventories, outlet-policy evidence, `.openclaw`
content, workspace backups, delivery logs, run history, or local SQLite databases.
Use repo-relative paths in committed docs and reports. Public contributions must
not depend on private infrastructure, credentials, or internal services.

When a change affects the public runner, skill, release, or security contract,
update the matching docs and release notes. Before tagging a release, run:

```bash
mise exec -- ./scripts/validate-release-docs.sh <tag>
```

Use GitHub issues and pull requests for tracked project work. Follow the
[maintainer guide](docs/maintainers.md) for repository administration and release
publication.
