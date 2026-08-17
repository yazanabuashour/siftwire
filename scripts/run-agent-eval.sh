#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

cargo run --quiet --locked --bin siftwire-agent-eval -- "$@"
