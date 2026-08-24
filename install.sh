#!/bin/sh
set -eu

repo="yazanabuashour/siftwire"
default_version="__SIFTWIRE_VERSION__"
release_base="https://github.com/${repo}/releases/download"

fail() {
  printf 'siftwire install: %s\n' "$*" >&2
  exit 1
}

[ "$#" -eq 0 ] || fail "arguments are not supported; use SIFTWIRE_VERSION and SIFTWIRE_INSTALL_DIR"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

detect_os() {
  case "$(uname -s)" in
    Darwin) printf 'darwin' ;;
    Linux) printf 'linux' ;;
    *) fail "unsupported operating system: $(uname -s)" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) printf 'amd64' ;;
    arm64 | aarch64) printf 'arm64' ;;
    *) fail "unsupported CPU architecture: $(uname -m)" ;;
  esac
}

resolve_latest_version() {
  latest_json="$(curl -fsSL "https://api.github.com/repos/${repo}/releases/latest")" ||
    fail "could not resolve latest GitHub Release"
  latest_tag="$(printf '%s\n' "$latest_json" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$latest_tag" ] || fail "could not read latest release tag"
  printf '%s' "$latest_tag"
}

select_version() {
  requested=${SIFTWIRE_VERSION:-$default_version}
  case "$requested" in
    "" | "__SIFTWIRE_VERSION__" | latest) resolve_latest_version ;;
    v*) printf '%s' "$requested" ;;
    *) printf 'v%s' "$requested" ;;
  esac
}

first_writable_path_dir() {
  old_ifs=$IFS
  IFS=:
  for dir in ${PATH:-}; do
    IFS=$old_ifs
    [ -n "$dir" ] || dir=.
    [ "$dir" = . ] && continue
    if [ -d "$dir" ] && [ -w "$dir" ]; then
      printf '%s' "$dir"
      return 0
    fi
    IFS=:
  done
  IFS=$old_ifs
  return 1
}

select_install_dir() {
  if [ -n "${SIFTWIRE_INSTALL_DIR:-}" ]; then
    printf '%s' "$SIFTWIRE_INSTALL_DIR"
  elif dir="$(first_writable_path_dir)"; then
    printf '%s' "$dir"
  else
    [ -n "${HOME:-}" ] || fail "HOME is not set and no writable PATH directory was found"
    printf '%s/.local/bin' "$HOME"
  fi
}

path_contains_dir() {
  needle=$1
  old_ifs=$IFS
  IFS=:
  for dir in ${PATH:-}; do
    IFS=$old_ifs
    [ "$dir" = "$needle" ] && return 0
    IFS=:
  done
  IFS=$old_ifs
  return 1
}

verify_checksum() {
  checksum_file=$1
  asset=$2
  count="$(awk -v file="$asset" '$2 == file { count++ } END { print count + 0 }' "$checksum_file")"
  [ "$count" -eq 1 ] || fail "expected exactly one checksum entry for ${asset}; found ${count}"
  awk -v file="$asset" '$2 == file { print }' "$checksum_file" > "${asset}.sha256"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "${asset}.sha256" >/dev/null || fail "checksum verification failed for ${asset}"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${asset}.sha256" >/dev/null || fail "checksum verification failed for ${asset}"
  else
    fail "missing required command: shasum or sha256sum"
  fi
}

need_cmd curl
need_cmd awk

os=$(detect_os)
arch=$(detect_arch)
tag=$(select_version)
asset_version=${tag#v}
asset="siftwire_${asset_version}_${os}_${arch}"
checksum="siftwire_${asset_version}_checksums.txt"
release_url="${release_base%/}/${tag}"
install_dir=$(select_install_dir)
mkdir -p "$install_dir"
install_dir="$(CDPATH= cd -- "$install_dir" && pwd)"
target="${install_dir}/siftwire"
if path_contains_dir "$install_dir"; then
  active_path="$(command -v siftwire 2>/dev/null || true)"
  [ -z "$active_path" ] || [ "$active_path" = "$target" ] ||
    fail "PATH resolves ${active_path}; refusing to replace shadowed target ${target}"
fi
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/siftwire-install.XXXXXX")"
staged=""

cleanup() {
  rm -rf "$tmp_dir"
  [ -z "$staged" ] || rm -f "$staged"
}
trap cleanup EXIT HUP INT TERM

printf 'Installing SiftWire %s for %s/%s\n' "$tag" "$os" "$arch"
cd "$tmp_dir"
curl -fsSL "${release_url}/${asset}" -o "$asset" || fail "download failed: ${release_url}/${asset}"
curl -fsSL "${release_url}/${checksum}" -o "$checksum" || fail "download failed: ${release_url}/${checksum}"
verify_checksum "$checksum" "$asset"
chmod 755 "$asset"
[ "$(./"$asset" --version)" = "siftwire ${tag}" ] || fail "downloaded binary version does not match ${tag}"

staged="$(mktemp "${install_dir}/.siftwire.new.XXXXXX")" || fail "could not create staging file in ${install_dir}"
cp "$asset" "$staged"
chmod 755 "$staged"
[ "$("$staged" --version)" = "siftwire ${tag}" ] || fail "staged binary version does not match ${tag}"
mv -f "$staged" "$target"

printf 'Runner installed to %s\n' "$target"
if path_contains_dir "$install_dir"; then
  [ "$(siftwire --version 2>/dev/null || true)" = "siftwire ${tag}" ] ||
    fail "installed siftwire did not report ${tag}"
else
  printf 'Add this directory to PATH: export PATH="%s:$PATH"\n' "$install_dir"
fi

printf '%s\n' \
  "Register the matching SiftWire skill before reporting installation complete:" \
  "  https://github.com/${repo}/tree/${tag}/skills/siftwire" \
  "  ${release_url}/siftwire_${asset_version}_skill.tar.gz"
