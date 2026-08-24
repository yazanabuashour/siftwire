#!/usr/bin/env bash
set -Eeuo pipefail

repo_root="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

install_dir="${SIFTWIRE_CONSOLE_INSTALL_DIR:-$HOME/.local/bin}"
web_dir="${SIFTWIRE_CONSOLE_WEB_INSTALL_DIR:-$HOME/.local/share/siftwire-console/web}"

cargo build --locked --release -p siftwire-console
bun --cwd apps/web build

install -d -m 755 "$install_dir"
install -m 755 target/release/siftwire-console "$install_dir/siftwire-console"

rm -rf -- "$web_dir"
install -d -m 700 "$(dirname -- "$web_dir")"
install -d -m 755 "$web_dir"
cp -r apps/web/dist/. "$web_dir/"

printf 'console installed: %s\n' "$install_dir/siftwire-console"
printf 'web assets installed: %s\n' "$web_dir"
