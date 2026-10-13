# Develop SiftWire

## Set up a checkout

Install the tools pinned in `mise.toml` and `rust-toolchain.toml`, then install
frontend and evaluator dependencies. Install ShellCheck separately; `mise` does
not manage it, but the repository gate requires it:

```bash
mise install
mise exec -- bun install --frozen-lockfile
```

Run commands below from the repository root. The
[repository map](AGENTS.md#code-and-architecture) identifies module owners. Follow
[frontend ownership and shared policies](docs/architecture/frontend.md) when
changing requests, schemas, cached data, or views. Update checked-in policy
snapshots with their producer's export command, not by editing generated code.

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

## Develop the console

Use the same disposable database and source-built runner. In one terminal, start
the backend:

```bash
SIFTWIRE_CONSOLE_DATABASE="$run_root/siftwire.sqlite" \
SIFTWIRE_CONSOLE_RUNNER_BIN="$PWD/target/debug/siftwire" \
	mise exec -- cargo run --locked -p siftwire-console
```

In another terminal, run `mise exec -- bun run dev:web` and open
`http://127.0.0.1:5175`. Vite proxies `/api` to the backend at
`http://127.0.0.1:8790`; set `API_PROXY_TARGET` if you use another backend address.
If the backend port is occupied, choose another `SIFTWIRE_CONSOLE_BIND` and a
matching `API_PROXY_TARGET`; do not stop an existing console. For another Vite
port, use `mise exec -- bun run dev:web --port 5176`. See
[console settings](docs/console.md#change-server-settings).

Run `mise exec -- bun run build:console` to build the console and its web assets
without installing them. For installation, follow the
[console guide](docs/console.md#install-and-start-the-console).

## Run checks

Run the full gate before handing off a change:

```bash
mise exec -- ./scripts/ci.sh
```

The gate checks Rust toolchain selection, formatting, lint, workspace tests,
TypeScript types, adapter and frontend tests, the web build, shell scripts, the
agent skill, and release installation. Focused checks during development include
`mise exec -- cargo test --locked -p siftwire`,
`mise exec -- cargo test --locked -p siftwire-console`, and
`mise exec -- bun run test:web`.

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
