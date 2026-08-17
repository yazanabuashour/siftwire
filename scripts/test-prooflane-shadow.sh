#!/bin/sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
root="$(mktemp -d "${TMPDIR:-/tmp}/siftwire-prooflane-shadow.XXXXXX")"
trap 'rm -rf "$root"' EXIT HUP INT TERM
arguments="$root/arguments"
input="$root/input"

cat > "$root/prooflane" <<'SH'
#!/bin/sh
set -eu
printf '%s\n' "$@" > "$TEST_ARGUMENTS"
cat > "$TEST_INPUT"
printf '%s\n' '{"rejected":false,"summary":"observed"}'
SH
cat > "$root/siftwire" <<'SH'
#!/bin/sh
exit 0
SH
chmod 755 "$root/prooflane" "$root/siftwire"

request='{"action":"run_brief","dry_run":false}'
printf '%s\n' "$request" |
  TEST_ARGUMENTS="$arguments" TEST_INPUT="$input" \
  PROOFLANE_BINARY="$root/prooflane" SIFTWIRE_BINARY="$root/siftwire" \
  "$repo_root/scripts/prooflane-shadow-siftwire.sh" run "$root/contract.json" >/dev/null
printf '%s\n' \
  dogfood siftwire run --contract "$root/contract.json" \
  --config-binary "$root/siftwire" --brief-binary "$root/siftwire" |
  cmp - "$arguments"
printf '%s\n' "$request" | cmp - "$input"

request='{"action":"record_delivery","run_id":"runner-run","message":"NO_REPLY"}'
printf '%s\n' "$request" |
  TEST_ARGUMENTS="$arguments" TEST_INPUT="$input" \
  PROOFLANE_BINARY="$root/prooflane" SIFTWIRE_BINARY="$root/siftwire" \
  "$repo_root/scripts/prooflane-shadow-siftwire.sh" delivery prooflane-run >/dev/null
printf '%s\n' \
  dogfood siftwire delivery --run prooflane-run --brief-binary "$root/siftwire" |
  cmp - "$arguments"
printf '%s\n' "$request" | cmp - "$input"

if "$repo_root/scripts/prooflane-shadow-siftwire.sh" invalid value >/dev/null 2>&1; then
  printf 'shadow helper accepted an unknown operation\n' >&2
  exit 1
fi

printf 'Prooflane shadow helper contract passed\n'
