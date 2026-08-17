#!/bin/sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
root="$(mktemp -d "${TMPDIR:-/tmp}/siftwire-install-test.XXXXXX")"
trap 'rm -rf "$root"' EXIT HUP INT TERM
release_root="$root/releases"
install_dir="$root/bin"
test_installer="$root/install.sh"
mkdir -p "$release_root" "$install_dir"
sed "s|release_base=\"https://github.com/\${repo}/releases/download\"|release_base=\"file://$release_root\"|g" "$repo_root/install.sh" > "$test_installer"

case "$(uname -s)" in
  Darwin) os=darwin ;;
  Linux) os=linux ;;
  *) echo "unsupported test OS" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=amd64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) echo "unsupported test architecture" >&2; exit 1 ;;
esac

make_release() {
  tag=$1
  reported=$2
  dir="$release_root/$tag"
  asset="siftwire_${tag#v}_${os}_${arch}"
  checksum="siftwire_${tag#v}_checksums.txt"
  mkdir -p "$dir"
  cat > "$dir/$asset" <<EOF
#!/bin/sh
printf '%s\\n' 'siftwire $reported'
EOF
  chmod 755 "$dir/$asset"
  if command -v shasum >/dev/null 2>&1; then
    (cd "$dir" && shasum -a 256 "$asset" > "$checksum")
  else
    (cd "$dir" && sha256sum "$asset" > "$checksum")
  fi
}

run_install() {
  tag=$1
  PATH="$install_dir:$PATH" SIFTWIRE_VERSION="$tag" SIFTWIRE_INSTALL_DIR="$install_dir" sh "$test_installer"
}

make_release v0.2.0 v0.2.0
printf '#!/bin/sh\nprintf "old binary\\n"\n' > "$install_dir/siftwire"
chmod 755 "$install_dir/siftwire"
run_install v0.2.0 >/dev/null
[ "$("$install_dir/siftwire" --version)" = "siftwire v0.2.0" ] || {
  echo "installed binary version mismatch" >&2
  exit 1
}

relative_root="$root/relative"
mkdir -p "$relative_root"
(cd "$relative_root" && SIFTWIRE_VERSION=v0.2.0 SIFTWIRE_INSTALL_DIR=relative-bin sh "$test_installer" >/dev/null)
[ -x "$relative_root/relative-bin/siftwire" ] || {
  echo "relative install directory was not resolved from the caller directory" >&2
  exit 1
}

shadow_dir="$root/shadow"
shadow_target="$root/shadow-target"
mkdir -p "$shadow_dir" "$shadow_target"
printf '#!/bin/sh\nexit 0\n' > "$shadow_dir/siftwire"
chmod 755 "$shadow_dir/siftwire"
printf 'keep\n' > "$shadow_target/siftwire"
if PATH="$shadow_dir:$shadow_target:$PATH" SIFTWIRE_VERSION=v0.2.0 SIFTWIRE_INSTALL_DIR="$shadow_target" sh "$test_installer" >"$root/shadow.out" 2>&1; then
  echo "installer replaced a PATH-shadowed target" >&2
  exit 1
fi
[ "$(cat "$shadow_target/siftwire")" = "keep" ] || {
  echo "PATH shadow failure mutated the target" >&2
  exit 1
}

make_release v0.2.1 wrong
if run_install v0.2.1 >"$root/mismatch.out" 2>&1; then
  echo "installer accepted mismatched binary version" >&2
  exit 1
fi
[ "$("$install_dir/siftwire" --version)" = "siftwire v0.2.0" ] || {
  echo "version mismatch replaced existing binary" >&2
  exit 1
}

make_release v0.2.2 v0.2.2
printf 'tampered\n' >> "$release_root/v0.2.2/siftwire_0.2.2_${os}_${arch}"
if run_install v0.2.2 >"$root/tampered.out" 2>&1; then
  echo "installer accepted tampered binary" >&2
  exit 1
fi

make_release v0.2.3 v0.2.3
line="$(cat "$release_root/v0.2.3/siftwire_0.2.3_checksums.txt")"
printf '%s\n' "$line" >> "$release_root/v0.2.3/siftwire_0.2.3_checksums.txt"
if run_install v0.2.3 >"$root/duplicate.out" 2>&1; then
  echo "installer accepted duplicate checksum entries" >&2
  exit 1
fi

if OPENBRIEF_VERSION=v0.1.8 run_install v0.2.0 >"$root/legacy-env.out" 2>&1; then
  echo "installer accepted OPENBRIEF_VERSION" >&2
  exit 1
fi

test -z "$(find "$install_dir" -name '.siftwire.new.*' -print -quit)"
printf 'installer contract passed\n'
