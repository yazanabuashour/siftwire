#!/usr/bin/env bash
set -Eeuo pipefail

# The artifact digest is part of the release id, and umask changes the digest.
umask 077

prooflane_source="${PROOFLANE_SOURCE_REPO:-$HOME/repo/prooflane}"
runtime_root="${SIFTWIRE_PROOFLANE_RUNTIME_ROOT:-$HOME/.local/share/pi-automations/siftwire/prooflane}"
state_root="${SIFTWIRE_STATE_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/pi-automations/siftwire}"

for command in flock git jq; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'siftwire Prooflane deploy: missing command: %s\n' "$command" >&2
    exit 127
  }
done

git -C "$prooflane_source" rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
  printf 'siftwire Prooflane deploy: not a Git repository: %s\n' "$prooflane_source" >&2
  exit 1
}
[[ -z $(git -C "$prooflane_source" status --porcelain --untracked-files=all) ]] || {
  printf 'siftwire Prooflane deploy: Prooflane worktree must be clean\n' >&2
  exit 1
}

mkdir -p -- "$runtime_root/releases" "$state_root"
exec 9>>"$state_root/run.lock"
flock 9

build_root="$(mktemp -d "$runtime_root/.deploy.XXXXXX")"
trap 'rm -rf -- "$build_root"' EXIT

commit="$(git -C "$prooflane_source" rev-parse HEAD)"
extracted="$build_root/extracted"
artifact_digest="$("$prooflane_source/scripts/prepare-release.sh" "$prooflane_source" "$commit" "$extracted")"
release_id="${commit}_${artifact_digest}"
release="$runtime_root/releases/$release_id"

if [[ ! -d $release ]]; then
  [[ -x $extracted/bin/prooflane && -r $extracted/examples/siftwire/prooflane.json ]] || {
    printf 'siftwire Prooflane deploy: release is incomplete\n' >&2
    exit 1
  }
  "$extracted/bin/prooflane" version --format json | jq -e '
    .schema_version == "prooflane/version-report/v1"
    and (.product_version | type == "string" and length > 0)
    and .platform == "linux-x86_64-glibc"
  ' >/dev/null
  mv -T -- "$extracted" "$release"
fi

[[ -x $release/bin/prooflane && -r $release/examples/siftwire/prooflane.json ]] || {
  printf 'siftwire Prooflane deploy: installed release is incomplete: %s\n' "$release" >&2
  exit 1
}
"$release/bin/prooflane" version --format json | jq -e '.schema_version == "prooflane/version-report/v1"' >/dev/null

link="$runtime_root/current"
temp_link="$runtime_root/.current.$$"
ln -s "releases/$release_id" "$temp_link"
mv -Tf -- "$temp_link" "$link"
printf 'siftwire Prooflane deploy: activated %s\n' "$release"
