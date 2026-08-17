# Siftwire

Siftwire is a local-first brief runtime for agents. Its supported production
surface is one installed `siftwire` JSON runner plus one single-file skill.
Runtime configuration and mutable state stay in the operator's SQLite database,
not in this repository.

## Install

Tell your agent:

```text
Install Siftwire from https://github.com/yazanabuashour/siftwire.
Complete both steps before reporting success:
1. Install and verify the runner with `siftwire --version`.
2. Register skills/siftwire/SKILL.md with the agent's native skill system.
```

Install the latest runner:

```bash
sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/latest/download/install.sh)"
```

Install a pinned runner:

```bash
SIFTWIRE_VERSION=v0.2.0 sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/download/v0.2.0/install.sh)"
```

Set `SIFTWIRE_INSTALL_DIR` to choose another executable directory. The installer
accepts no arguments; these two environment variables are its configuration.

The skill must come from the same tag as the runner. Register it from
`skills/siftwire/SKILL.md`, the matching repository tag, or the release's
`siftwire_<version>_skill.tar.gz`. Siftwire does not require a particular
agent implementation or skill directory.

## Upgrade

Upgrade the runner with the latest installer, verify it, then re-register the
matching skill:

```bash
sh -c "$(curl -fsSL https://github.com/yazanabuashour/siftwire/releases/latest/download/install.sh)"
siftwire --version
```

## First use

The runner reads exactly one JSON request from stdin and writes one JSON result
to stdout:

```bash
printf '%s\n' '{"action":"init"}' | siftwire config
printf '%s\n' '{"action":"inspect_config"}' | siftwire config
printf '%s\n' '{"action":"run_brief","dry_run":true}' | siftwire brief
```

Configuration actions manage RSS, Atom, and GitHub release sources; generic
feed processing; outlet policies; and brief options. Brief actions validate the
runtime, run a brief, and record the exact delivered message for history and
repeat suppression. See [`skills/siftwire/SKILL.md`](skills/siftwire/SKILL.md)
for the installed agent policy.

## Storage

The default database is
`${XDG_DATA_HOME:-~/.local/share}/siftwire/siftwire.sqlite`. `XDG_DATA_HOME`
is used only when it is absolute. Select another database with:

- `SIFTWIRE_DATABASE_PATH`
- `--db` for explicit datasets and tests

Siftwire does not support a data-directory variable, workspace state, or
repo-local runtime files. This repository must not contain personal source
inventories, outlet policies, delivery logs, `.openclaw` content, workspace
backups, run history, or local SQLite databases.

## Rename from OpenBrief

Siftwire v0.2.0 is a clean command, skill, module, and release-asset rename. It
does not install an `openbrief` command alias or remove an old binary. Remove or
disable the old OpenBrief skill and update scheduled commands after installing
the matching Siftwire runner and skill.

The SQLite schema is unchanged, but Siftwire will not move or silently fork an
existing default database. Select the old file explicitly:

```bash
export SIFTWIRE_DATABASE_PATH="$HOME/.local/share/openbrief/openbrief.sqlite"
siftwire config <<'JSON'
{"action":"inspect_config"}
JSON
```

If the old database used an absolute `XDG_DATA_HOME` or another override, use
its actual path instead. `OPENBRIEF_DATABASE_PATH` remains a deprecated fallback
through v0.2.x; `SIFTWIRE_DATABASE_PATH` is canonical. Conflicting values fail.

To roll back, first capture the exact active path returned by
`inspect_config.paths.database_path`:

```bash
siftwire_database_path="$(
  siftwire config <<'JSON' | jq -er '.paths.database_path'
{"action":"inspect_config"}
JSON
)"
```

Then stop Siftwire jobs, point the old runner at that captured path, and restore
the old command and skill:

```bash
export OPENBRIEF_DATABASE_PATH="$siftwire_database_path"
```

This reuses explicit paths and absolute `XDG_DATA_HOME` paths without guessing.
Do not run both products against the same database during rollback.

## Develop

Install the pinned Rust toolchain and run the repository gate:

```bash
mise install
mise exec -- ./scripts/ci.sh
```

The gate enforces formatting, the repository's strict Rust and Clippy lint
policy, tests, shell checks, skill validation, installer behavior, and a release
build.

Release-document changes also require:

```bash
mise exec -- ./scripts/validate-release-docs.sh <tag>
```

## Documentation

- [`docs/README.md`](docs/README.md): task-oriented documentation index
- [`docs/evals/agent-production.md`](docs/evals/agent-production.md): production
  agent eval protocol
- [`docs/release-verification.md`](docs/release-verification.md): release
  verification
- [`CONTRIBUTING.md`](CONTRIBUTING.md): contribution expectations
- [`SECURITY.md`](SECURITY.md): private vulnerability reporting

Published release assets are immutable. Fix an artifact with a new patch release
rather than replacing an existing tag or asset.
