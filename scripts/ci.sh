#!/usr/bin/env bash
set -Eeuo pipefail

repo_root="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

scripts/check-rust-toolchain.sh
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --workspace
bun install
bunx oxfmt --check .
bun run lint
bun --cwd apps/web typecheck
bun --cwd apps/web test
bun --cwd apps/web build
# SC1007 misreads CDPATH assignment prefixes; SC2016 flags the printed literal $PATH guidance.
shellcheck --exclude=SC1007,SC2016 install.sh scripts/*.sh
scripts/validate-agent-skill.sh skills/siftwire
scripts/test-prooflane-shadow.sh
scripts/test-install.sh
cargo build --locked --release
scripts/test-release-install.sh target/release/siftwire
