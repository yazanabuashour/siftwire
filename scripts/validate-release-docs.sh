#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

[ "$#" -eq 1 ] || fail 'usage: scripts/validate-release-docs.sh <tag>'
tag="$(printf '%s' "$1" | awk '{$1=$1; print}')"
printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || fail "tag must match vMAJOR.MINOR.PATCH: \"$tag\""

brand=Siftwire
repository=siftwire

notes_path="docs/release-notes/$tag.md"
[ -e "$notes_path" ] || fail "$notes_path not found"
[ -f "$notes_path" ] || fail "$notes_path is not a file"
[ -r "$notes_path" ] || fail "read $notes_path"
title="# $brand $tag"

awk -v file="$notes_path" -v title="$title" '
function trim(value) {
  sub(/^[[:space:]]+/, "", value)
  sub(/[[:space:]]+$/, "", value)
  return value
}
{
  sub(/\r$/, "")
  lines++
  trimmed = trim($0)
  if (NR == 1 && trimmed != title) {
    failed = 1
    printf "error: %s must start with \"%s\"\n", file, title > "/dev/stderr"
    exit 1
  }
  if (trimmed == "## Changed") {
    changed = 1
  }
  if (trimmed == "## Verification") {
    verification = 1
  }
}
END {
  if (failed) {
    exit 1
  }
  if (lines == 0) {
    printf "error: %s must start with \"%s\"\n", file, title > "/dev/stderr"
    exit 1
  }
  if (!changed) {
    printf "error: %s must include ## Changed\n", file > "/dev/stderr"
    exit 1
  }
  if (!verification) {
    printf "error: %s must include ## Verification\n", file > "/dev/stderr"
    exit 1
  }
}
' "$notes_path"

awk -v file="$notes_path" '
function trim(value) {
  sub(/^[[:space:]]+/, "", value)
  sub(/[[:space:]]+$/, "", value)
  return value
}
function is_rule(value, marker, character) {
  if (length(value) < 3) {
    return 0
  }
  marker = substr(value, 1, 1)
  if (marker != "-" && marker != "*" && marker != "_") {
    return 0
  }
  for (character = 2; character <= length(value); character++) {
    if (substr(value, character, 1) != marker) {
      return 0
    }
  }
  return 1
}
function is_ordered_item(value) {
  return value ~ /^[0-9]+\. /
}
function is_list_item(line, value) {
  if (line ~ /^[ \t]/) {
    return 0
  }
  value = trim(line)
  return index(value, "- ") == 1 || index(value, "* ") == 1 || index(value, "+ ") == 1 || is_ordered_item(value)
}
function is_boundary(value) {
  return index(value, "#") == 1 || is_rule(value) || index(value, "<!--") == 1
}
function is_plain_prose(line, value) {
  if (trim(line) == "" || line ~ /^[ \t]/) {
    return 0
  }
  value = trim(line)
  if (index(value, "#") == 1 || index(value, "- ") == 1 || index(value, "* ") == 1 ||
      index(value, "+ ") == 1 || index(value, ">") == 1 || index(value, "|") == 1 ||
      index(value, "```") == 1 || index(value, "~~~") == 1 || index(value, "[") == 1 ||
      index(value, "<!--") == 1) {
    return 0
  }
  return !is_ordered_item(value) && !is_rule(value)
}
function die(message) {
  print "error: " message > "/dev/stderr"
  exit 1
}
{
  sub(/\r$/, "")
  line_number = NR
  trimmed = trim($0)
  if (index(trimmed, "```") == 1 || index(trimmed, "~~~") == 1) {
    in_fence = !in_fence
    previous_plain = 0
    previous_list = 0
    next
  }
  if (in_fence || trimmed == "") {
    previous_plain = 0
    previous_list = 0
    next
  }
  if (previous_list != 0 && !is_list_item($0) && !is_boundary(trimmed)) {
    die(file " line " line_number " appears to hard-wrap list item from line " previous_list "; keep release-note list items on one source line")
  }
  if (is_list_item($0)) {
    previous_plain = 0
    previous_list = line_number
    next
  }
  previous_list = 0
  if (!is_plain_prose($0)) {
    previous_plain = 0
    next
  }
  if (previous_plain != 0) {
    die(file " line " line_number " appears to hard-wrap prose from line " previous_plain "; keep release-note prose paragraphs on one source line")
  }
  previous_plain = line_number
}
' "$notes_path"

[ -e CHANGELOG.md ] || fail 'CHANGELOG.md not found'
[ -f CHANGELOG.md ] || fail 'CHANGELOG.md is not a file'
[ -r CHANGELOG.md ] || fail 'read CHANGELOG.md'
release_url="https://github.com/yazanabuashour/$repository/releases/tag/$tag"
grep -Fq "$release_url" CHANGELOG.md || fail "CHANGELOG.md must link to $release_url"

printf 'validated release docs for %s\n' "$tag"
