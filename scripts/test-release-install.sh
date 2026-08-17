#!/bin/sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

binary=${1:-target/release/siftwire}
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
root="$(mktemp -d "${TMPDIR:-/tmp}/siftwire-release-install.XXXXXX")"
trap 'rm -rf "$root"' EXIT HUP INT TERM

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

[ -n "$version" ] || fail 'Cargo package version is missing'
[ -x "$binary" ] || fail "release binary is not executable: $binary"
[ "$("$binary" --version)" = "siftwire v$version" ] || fail 'release binary version does not match Cargo.toml'

case "$(uname -s)" in
  Darwin) os=darwin ;;
  Linux) os=linux ;;
  *) fail 'unsupported release test operating system' ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=amd64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) fail 'unsupported release test architecture' ;;
esac

release_root="$root/releases"
tag="v$version"
asset="siftwire_${version}_${os}_${arch}"
checksum="siftwire_${version}_checksums.txt"
release="$release_root/$tag"
home="$root/home"
install_dir="$home/.local/bin"
test_installer="$root/install.sh"
mkdir -p "$release" "$home"
cp "$binary" "$release/$asset"
if command -v shasum >/dev/null 2>&1; then
  (cd "$release" && shasum -a 256 "$asset" > "$checksum")
else
  (cd "$release" && sha256sum "$asset" > "$checksum")
fi
sed "s|release_base=\"https://github.com/\${repo}/releases/download\"|release_base=\"file://$release_root\"|g" install.sh > "$test_installer"

HOME="$home" SIFTWIRE_VERSION="$tag" SIFTWIRE_INSTALL_DIR="$install_dir" sh "$test_installer" >/dev/null
installed="$(env -i HOME="$home" PATH="$install_dir:/usr/bin:/bin" siftwire --version)"
[ "$installed" = "siftwire $tag" ] || fail 'clean environment could not execute the installed release'

printf 'real release binary clean-install test passed\n'
