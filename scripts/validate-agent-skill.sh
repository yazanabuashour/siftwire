#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

skill_dir=${1:-skills/siftwire}
while [ "$skill_dir" != / ] && [ "${skill_dir%/}" != "$skill_dir" ]; do
  skill_dir=${skill_dir%/}
done
[ -n "$skill_dir" ] || skill_dir=.
[ -d "$skill_dir" ] || fail "skill directory not found: $skill_dir"

skill_file="$skill_dir/SKILL.md"
entry_count="$(find "$skill_dir" ! -path "$skill_dir" -prune -print | awk 'END { print NR + 0 }')"
if [ "$entry_count" -ne 1 ] || [ ! -f "$skill_file" ]; then
  found="$(find "$skill_dir" ! -path "$skill_dir" -prune -exec basename {} \; | LC_ALL=C sort | awk 'BEGIN { separator = "" } { printf "%s%s", separator, $0; separator = ", " }')"
  fail "$skill_dir must contain only SKILL.md; found $found"
fi
[ -r "$skill_file" ] || fail "read $skill_file"

body="$(mktemp "${TMPDIR:-/tmp}/siftwire-skill-body.XXXXXX")"
trap 'rm -f "$body"' EXIT HUP INT TERM
parent_dir="$(basename "$skill_dir")"

awk -v file="$skill_file" -v parent="$parent_dir" '
function trim(value) {
  sub(/^[[:space:]]+/, "", value)
  sub(/[[:space:]]+$/, "", value)
  return value
}
function unquote(value, single_quote) {
  single_quote = sprintf("%c", 39)
  while (substr(value, 1, 1) == "\"" || substr(value, 1, 1) == single_quote) {
    value = substr(value, 2)
  }
  while (substr(value, length(value), 1) == "\"" || substr(value, length(value), 1) == single_quote) {
    value = substr(value, 1, length(value) - 1)
  }
  return value
}
function die(message) {
  failed = 1
  print "error: " message > "/dev/stderr"
  exit 1
}
{
  sub(/\r$/, "")
}
NR == 1 {
  if (trim($0) != "---") {
    die(file " must start with YAML frontmatter delimited by ---")
  }
  next
}
!closed {
  if (trim($0) == "---") {
    if (NR == 2) {
      die(file " frontmatter must contain required fields")
    }
    closed = 1
    next
  }
  if (trim($0) == "") {
    next
  }
  separator = index($0, ":")
  if (separator == 0) {
    die(file " frontmatter line " NR " must use key: value syntax")
  }
  key = trim(substr($0, 1, separator - 1))
  value = unquote(trim(substr($0, separator + 1)))
  if (key == "") {
    die(file " frontmatter line " NR " has an empty key")
  }
  if (key in metadata) {
    die(file " frontmatter field \"" key "\" must not be duplicated")
  }
  metadata[key] = value
  next
}
{
  print
}
END {
  if (failed) {
    exit 1
  }
  if (!closed) {
    die(file " must include a closing --- line for YAML frontmatter")
  }
  if (metadata["name"] == "") {
    die(file " frontmatter must define a non-empty name")
  }
  if (length(metadata["name"]) > 64) {
    die(file " name must be 64 characters or fewer")
  }
  if (metadata["name"] !~ /^[a-z0-9]+(-[a-z0-9]+)*$/) {
    die(file " name must use lowercase letters, numbers, and single hyphens only")
  }
  if (metadata["name"] != parent) {
    die(file " name must match the parent directory (\"" parent "\")")
  }
  required[1] = "description"
  required[2] = "license"
  required[3] = "compatibility"
  for (index_value = 1; index_value <= 3; index_value++) {
    field = required[index_value]
    if (metadata[field] == "") {
      die(file " frontmatter must define a non-empty " field)
    }
  }
  if (length(metadata["description"]) > 1024) {
    die(file " description must be 1024 characters or fewer")
  }
  if (length(metadata["compatibility"]) > 500) {
    die(file " compatibility must be 500 characters or fewer")
  }
}
' "$skill_file" > "$body"

required_contract='# SiftWire
## Production Boundary
## Source Intake
## Config Tasks
## Brief Tasks
siftwire config
siftwire brief
SIFTWIRE_DATABASE_PATH
approval
SQLite directly
delivery history
latest-seen state
run state
NO_REPLY
siftwire-runner/v5
current-news/v1
prepared-delivery/v1'
printf '%s\n' "$required_contract" | while IFS= read -r required; do
  grep -Fq "$required" "$body" || fail "$skill_file missing required runner contract \"$required\""
done

for action in init inspect_config replace_sources upsert_source delete_source replace_outlet_policies set_brief_options validate run_brief prepare_delivery confirm_delivery; do
  required="\"action\":\"$action\""
  grep -Fq "$required" "$body" || fail "$skill_file missing required runner contract \"$required\""
done

links="$(grep -Eo '\[[^]]+\]\([^)]+\)' "$body" || true)"
printf '%s\n' "$links" | while IFS= read -r link; do
  [ -n "$link" ] || continue
  target="$(printf '%s\n' "$link" | sed 's/.*](//; s/)$//')"
  check_target="$(printf '%s\n' "$target" | sed 's/^[<>]*//; s/[<>]*$//')"
  case "$check_target" in
    ''|'#'*|/*|[A-Za-z]:*|[A-Za-z][A-Za-z0-9+.-]*:*) continue ;;
  esac
  if printf '%s\n' "$target" | awk -F/ '
    {
      depth = 0
      escaped = 0
      for (field = 1; field <= NF; field++) {
        if ($field == "" || $field == ".") {
          continue
        }
        if ($field == "..") {
          if (depth == 0) {
            escaped = 1
          } else {
            depth--
          }
        } else {
          depth++
        }
      }
      exit !escaped
    }
  '; then
    fail "$skill_file link target \"$target\" escapes the skill directory"
  fi
  [ -e "$skill_dir/$target" ] || fail "$skill_file link target \"$target\" is not installed with the skill"
done

for forbidden in \
  '"action":"record_delivery"' \
  SIFTWIRE_DATA_DIR \
  SIFTWIRE_EVAL_ALLOW_FILE_URLS \
  'go run ./cmd/siftwire' \
  brief-fetch.ts \
  BRIEF_PAYWALL_POLICY \
  BRIEF_SOURCES \
  'workspace backups, private run logs, or legacy brief scripts' \
  'recovery/import from private historical artifacts' \
  'recover, infer, or import private source inventory' \
  /Volumes/ \
  /Users/; do
  if grep -Fq "$forbidden" "$skill_file"; then
    fail "$skill_file contains forbidden product guidance \"$forbidden\""
  fi
done

printf 'validated %s\n' "$skill_dir"
