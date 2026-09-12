#!/usr/bin/env bash
set -Eeuo pipefail

repo_root="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

scripts/check-rust-toolchain.sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --workspace
bun install --frozen-lockfile
bunx oxfmt --check .
bun run lint
bun run typecheck
bun test tools/agent-eval
bun --cwd apps/web test
bun --cwd apps/web build
# SC1007 misreads CDPATH assignment prefixes; SC2016 flags the printed literal $PATH guidance.
shellcheck --exclude=SC1007,SC2016 install.sh scripts/*.sh tools/agent-eval/pi tools/agent-eval/stub
scripts/validate-agent-skill.sh skills/siftwire
scripts/test-install.sh
cargo build --locked --release
scripts/test-release-install.sh target/release/siftwire
