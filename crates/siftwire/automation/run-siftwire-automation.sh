#!/usr/bin/env bash
set -Eeuo pipefail

umask 077

state_root="${SIFTWIRE_STATE_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/pi-automations/siftwire}"
run_root="${SIFTWIRE_RUN_ROOT:-$state_root/runs}"
database_path="${SIFTWIRE_DATABASE_PATH:-${XDG_DATA_HOME:-$HOME/.local/share}/siftwire/siftwire.sqlite}"
siftwire_bin="${SIFTWIRE_BIN:-siftwire}"
notes_api_url="${SIFTWIRE_NOTES_API_URL:-}"
notes_public_url="${SIFTWIRE_NOTES_PUBLIC_URL:-}"
notes_token_file="${SIFTWIRE_NOTES_TOKEN_FILE:-}"
notes_start_date="${SIFTWIRE_NOTES_START_DATE:-}"
render_prompt="${SIFTWIRE_RENDER_PROMPT:?start this launcher through siftwire automation, or set SIFTWIRE_RENDER_PROMPT}"
mailgate_bin="${MAILGATE_BIN:-mailgate}"
pending_dir="$state_root/outbox/pending"
sent_dir="$state_root/outbox/sent"
active_job="$state_root/active-job.json"
attempt_marker="$state_root/run-brief-attempt.json"
attempt_result="$state_root/run-brief-result.json"
attempt_stderr="$state_root/run-brief.stderr"

# Delivery can be complete here, so this status names only an unfinished Notes copy.
notes_incomplete_status=69

# Mailgate loads the Resend key in its own process.
unset RESEND_API_KEY RESENT_API_KEY \
  SIFTWIRE_NOTES_API_URL SIFTWIRE_NOTES_PUBLIC_URL SIFTWIRE_NOTES_TOKEN_FILE SIFTWIRE_NOTES_START_DATE

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    printf 'siftwire automation: missing command: %s\n' "$1" >&2
    exit 127
  }
}

write_atomic() {
  local path="$1" content="$2" temp
  mkdir -p -- "$(dirname -- "$path")"
  temp="$(mktemp "$(dirname -- "$path")/.tmp.XXXXXX")"
  if ! printf '%s\n' "$content" >"$temp" || ! sync -d -- "$temp" ||
    ! mv -f -- "$temp" "$path" || ! sync -f -- "$(dirname -- "$path")"; then
    rm -f -- "$temp"
    return 1
  fi
}

validate_outbox_identity() {
  jq -e '
    .schema_version == "siftwire-email-outbox/v2"
    and (.siftwire_run_id | type == "string" and length > 0)
    and (.delivery_plan_id | type == "string" and length > 0)
    and (.idempotency_key | type == "string" and test("^siftwire-v2-[0-9a-f]{64}$"))
    and (.delivery_attempt_count | type == "number" and floor == . and . >= 0)
    and has("last_delivery_attempt_at")
  ' "$1" >/dev/null
}

validate_attempt_marker() {
  jq -e '
    .schema_version == "siftwire-run-attempt/v1"
    and .state == "invoking"
    and (.attempted_at | type == "string" and length > 0)
  ' "$1" >/dev/null 2>&1
}

require_current_durable_state() {
  local active_validation_error outbox
  local -a pending=()
  if [[ -e $active_job || -L $active_job ]]; then
    if ! [[ -f $active_job && ! -L $active_job ]] ||
      ! jq -e '
        .schema_version == "siftwire-active-job/v2"
        and (.brief | type == "object")
        and ((.candidate_indexes == null) or (.candidate_indexes | type == "array"))
      ' "$active_job" >/dev/null; then
      printf 'siftwire automation: incompatible durable active job: %s\n' "$active_job" >&2
      return 70
    fi
    if ! active_validation_error="$(validate_brief_result <(jq '.brief' "$active_job"))"; then
      printf 'siftwire automation: incompatible durable active job (%s): %s\n' \
        "$active_validation_error" "$active_job" >&2
      return 70
    fi
  fi
  if [[ -e $attempt_marker || -L $attempt_marker ]]; then
    if ! [[ -f $attempt_marker && ! -L $attempt_marker ]] || ! validate_attempt_marker "$attempt_marker"; then
      printf 'siftwire automation: incompatible durable run attempt: %s\n' "$attempt_marker" >&2
      return 70
    fi
  fi
  [[ -d $pending_dir && ! -L $pending_dir ]] || {
    printf 'siftwire automation: malformed pending queue: %s\n' "$pending_dir" >&2
    return 70
  }
  shopt -s nullglob dotglob
  pending=("$pending_dir"/*)
  shopt -u nullglob dotglob
  for outbox in "${pending[@]}"; do
    if ! [[ $outbox == *.json && -f $outbox && ! -L $outbox && $(stat -Lc '%u:%a' -- "$outbox") == "$UID:600" ]] ||
      ! validate_outbox_identity "$outbox"; then
      printf 'siftwire automation: incompatible persisted outbox: %s\n' "$outbox" >&2
      return 70
    fi
  done
}

publish_to_notes() {
  local request
  [[ -n $notes_api_url$notes_public_url$notes_token_file$notes_start_date ]] || return 0
  request="$(jq -cn --arg api_url "$notes_api_url" --arg public_url "$notes_public_url" \
    --arg token_file "$notes_token_file" --arg start_date "$notes_start_date" \
    '{api_url:$api_url,public_url:$public_url,token_file:$token_file,start_date:$start_date}')" || return
  printf '%s\n' "$request" | SIFTWIRE_DATABASE_PATH="$database_path" "$siftwire_bin" notes publish \
    >"$run_dir/publication.json" 2>"$run_dir/publication.stderr"
}

send_email() {
  local idempotency_key mailgate_request outbox="$1" receipt request_body updated
  request_body="$(jq -er '.request_body | select(type == "string" and length > 0)' "$outbox")" || return
  idempotency_key="$(jq -er '.idempotency_key | select(type == "string" and length > 0)' "$outbox")" || return
  if ! jq -e '
		type == "object"
		and (keys - ["subject", "text", "html"] | length == 0)
		and (.subject | type == "string" and length > 0)
		and (.text | type == "string" and length > 0)
		and (.html | type == "string" and length > 0)
	' <<<"$request_body" >/dev/null; then
    printf 'siftwire automation: invalid persisted Mailgate request body: %s\n' "$outbox" >&2
    return 1
  fi
  mailgate_request="$(jq -cn --arg idempotency_key "$idempotency_key" --argjson body "$request_body" \
    '$body + {schema_version:"mailgate/send-request/v1",idempotency_key:$idempotency_key}')" || return
  receipt="$(printf '%s\n' "$mailgate_request" | "$mailgate_bin" send --profile siftwire)" || return
  if ! jq -e '
		.schema_version == "mailgate/send-result/v2"
		and .submission_state == "provider_accepted"
		and (.delivery_state | IN("provider_accepted", "delivery_delayed", "delivered", "bounced", "failed", "suppressed"))
		and .profile == "siftwire"
		and .provider == "resend"
		and (.provider_message_id | type == "string" and length > 0)
		and (.provider_accepted_at | type == "string" and length > 0)
		and (.request_digest | type == "string" and startswith("sha256:"))
		and (.replayed | type == "boolean")
	' <<<"$receipt" >/dev/null; then
    printf 'siftwire automation: Mailgate returned an invalid receipt for %s\n' "$outbox" >&2
    return 1
  fi
  updated="$(jq --argjson receipt "$receipt" \
    '. + {email_id:$receipt.provider_message_id,email_accepted_at:$receipt.provider_accepted_at,mailgate_receipt:$receipt}' "$outbox")" || return
  write_atomic "$outbox" "$updated"
}

mark_delivery_recorded() {
  local outbox="$1" updated
  updated="$(jq --arg recorded_at "$(date --utc +%Y-%m-%dT%H:%M:%SZ)" \
    '. + {delivery_recorded_at:$recorded_at,delivery_record_source:"runner-result"}' "$outbox")" || return
  write_atomic "$outbox" "$updated"
}

confirm_delivery() {
  local delivery_err delivery_key delivery_out siftwire_run_id outbox="$1" request updated
  siftwire_run_id="$(jq -er '.siftwire_run_id' "$outbox")" || return
  request="$(jq -cn --arg run_id "$siftwire_run_id" --arg plan_id "$(jq -r '.delivery_plan_id' "$outbox")" \
    '{action:"confirm_delivery",run_id:$run_id,delivery_plan_id:$plan_id}')" || return
  updated="$(jq --arg attempted_at "$(date --utc +%Y-%m-%dT%H:%M:%SZ)" \
    '.delivery_attempt_count += 1 | .last_delivery_attempt_at = $attempted_at' \
    "$outbox")" || return
  write_atomic "$outbox" "$updated" || return
  delivery_key="$(basename -- "$outbox" .json)"
  delivery_out="$run_dir/delivery-$delivery_key.json"
  delivery_err="$run_dir/delivery-$delivery_key.stderr"
  if ! printf '%s\n' "$request" | SIFTWIRE_DATABASE_PATH="$database_path" \
    "$siftwire_bin" brief >"$delivery_out" 2>"$delivery_err"; then
    printf 'siftwire automation: confirm_delivery failed; retained %s\n' "$outbox" >&2
    return 1
  fi
  jq -e --arg run_id "$siftwire_run_id" --arg plan_id "$(jq -r '.delivery_plan_id' "$outbox")" \
    '.rejected == false and .run_id == $run_id and .delivery_plan_id == $plan_id' \
    "$delivery_out" >/dev/null || {
    printf 'siftwire automation: confirm_delivery returned invalid JSON; retained %s\n' "$outbox" >&2
    return 1
  }
  mark_delivery_recorded "$outbox" || return
}

finish_outbox() {
  local needs_email outbox="$1" sent_path
  [[ -f $outbox && ! -L $outbox && $(stat -Lc '%u:%a' -- "$outbox") == "$UID:600" ]] || return
  validate_outbox_identity "$outbox" || {
    printf 'siftwire automation: incompatible persisted outbox: %s\n' "$outbox" >&2
    return 70
  }
  needs_email="$(jq -er '
    .needs_email as $value
    | if ($value | type) == "boolean" then ($value | tostring) else error("needs_email must be boolean") end
  ' "$outbox")" || return
  if [[ $needs_email == true ]] && ! jq -e '.email_id | type == "string" and length > 0' "$outbox" >/dev/null; then
    send_email "$outbox" || return
  fi
  if ! jq -e '.delivery_recorded_at | type == "string" and length > 0' "$outbox" >/dev/null; then
    confirm_delivery "$outbox" || return
  fi
  sent_path="$sent_dir/$(basename -- "$outbox")"
  [[ ! -e $sent_path && ! -L $sent_path ]] || return
  mv -- "$outbox" "$sent_path" || return
  sync -f -- "$pending_dir" "$sent_dir" || return
  if [[ $needs_email == true ]]; then
    printf 'siftwire automation: email accepted as Resend message %s\n' "$(jq -r '.email_id' "$sent_path")"
  else
    printf 'siftwire automation: recorded NO_REPLY without sending an email\n'
  fi
}

process_outbox() {
  local outbox status=0 result
  local -a pending=()
  shopt -s nullglob dotglob
  pending=("$pending_dir"/*)
  shopt -u nullglob dotglob
  for outbox in "${pending[@]}"; do
    finish_outbox "$outbox" && continue
    result=$?
    ((status != 0)) || status=$result
    printf 'siftwire automation: notification remains pending: %s\n' "$outbox" >&2
  done
  return "$status"
}

validate_prepared_result() {
  local expected_run_id="$2"
  jq -e --arg run_id "$expected_run_id" '
    .runner_protocol == "siftwire-runner/v5"
    and (.capabilities | type == "array")
    and (.capabilities | index("prepared-delivery/v1") != null)
    and (.capabilities | index("current-news/v1") != null)
    and .rejected == false
    and .run_id == $run_id
    and (.delivery_plan_id | type == "string" and length > 0)
    and (.message | type == "string" and length > 0)
    and (.text | type == "string" and length > 0)
    and ((.html // "") | type == "string")
  ' "$1" >/dev/null
}

validate_brief_result() {
  local diagnostic document
  if ! document="$(jq -sc '
    if length == 1 and (.[0] | type) == "object" then .[0]
    else error("result must contain one JSON object")
    end
  ' "$1" 2>/dev/null)"; then
    printf 'result is not one JSON object'
    return 1
  fi
  diagnostic="$(jq -r '
    def representable_item:
      (.title | type == "string" and length > 0 and (test("[[:cntrl:]]") | not))
      and (.url | type == "string" and (. == "" or test("^https?://[^<>[:space:]]+$")) and (test("[[:cntrl:]]") | not));
    def invalid_item_index($items):
      if ($items | type) == "array"
      then [range(0; $items | length) | select(($items[.] | representable_item | not))] | first
      else null
      end;
    def has_duplicate_url($items):
      # Linkless items have no URL identity; candidate indexes distinguish them.
      ($items | type) == "array" and any($items | map(select(.url != "")) | group_by(.url)[]; length > 1);
    first(
      if .runner_protocol != "siftwire-runner/v5" then "runner protocol is missing or incompatible" else empty end,
      if (.capabilities | type != "array") then "capabilities is not an array" else empty end,
      if ((.capabilities // []) | index("prepared-delivery/v1") == null) then "prepared-delivery/v1 capability is missing" else empty end,
      if ((.capabilities // []) | index("current-news/v1") == null) then "current-news/v1 capability is missing" else empty end,
      if .rejected != false then "result was rejected or omitted rejected=false" else empty end,
      if (.run_id | type != "string" or length == 0) then "run_id is missing or invalid" else empty end,
      if ((.must_include // []) | type != "array") then "must_include is not an array" else empty end,
      (invalid_item_index(.must_include) as $index | if $index != null then "invalid must_include item at index \($index)" else empty end),
      if ((.candidates // []) | type != "array") then "candidates is not an array" else empty end,
      (invalid_item_index(.candidates) as $index | if $index != null then "invalid candidate item at index \($index)" else empty end),
      if has_duplicate_url(.candidates) then "duplicate candidate URL" else empty end,
      if (.max_delivery_items | type != "number" or . < 1) then "max_delivery_items is missing or invalid" else empty end,
      (.candidate_slots as $slots | if (($slots | type) != "number" or $slots < 0 or $slots != ($slots | floor) or $slots > .max_delivery_items) then "candidate_slots is missing or invalid" else empty end),
      if ((.health_footnote // "") | type != "string") then "health_footnote is not a string" else empty end
    ) // empty
  ' <<<"$document")" || {
    printf 'result is not valid JSON'
    return 1
  }
  [[ -z $diagnostic ]] || {
    printf '%s' "$diagnostic"
    return 1
  }
}

clear_attempt() {
  rm -f -- "$attempt_result" "$attempt_stderr" || return
  rm -f -- "$attempt_marker" || return
  sync -f -- "$state_root"
}

recover_attempt() {
  local brief_validation_error job_json rejection_reason
  [[ -e $attempt_marker || -L $attempt_marker ]] || return 0
  validate_attempt_marker "$attempt_marker" || {
    printf 'siftwire automation: incompatible durable run attempt: %s\n' "$attempt_marker" >&2
    return 70
  }
  if [[ -f $active_job && ! -L $active_job ]]; then
    clear_attempt
    return
  fi
  [[ -f $attempt_marker && ! -L $attempt_marker && -f $attempt_result && ! -L $attempt_result &&
    -f $attempt_stderr && ! -L $attempt_stderr ]] || {
    printf 'siftwire automation: run_brief has an uncertain durable attempt; refusing an unsafe retry: %s\n' \
      "$attempt_marker" >&2
    return 70
  }
  if rejection_reason="$(jq -er '
    select(.rejected == true)
    | .rejection_reason
    | select(type == "string" and length > 0)
  ' "$attempt_result" 2>/dev/null)"; then
    clear_attempt || return
    printf 'siftwire automation: run_brief rejected the request: %s\n' "$rejection_reason" >&2
    return 1
  fi
  if ! brief_validation_error="$(validate_brief_result "$attempt_result")"; then
    printf 'siftwire automation: run_brief result is not recoverable (%s); refusing an unsafe retry\n' \
      "$brief_validation_error" >&2
    return 70
  fi
  job_json="$(jq -cn --slurpfile brief "$attempt_result" \
    '{schema_version:"siftwire-active-job/v2",brief:$brief[0],candidate_indexes:null}')" || return
  write_atomic "$active_job" "$job_json" || return
  clear_attempt
}

clear_active_job() {
  rm -f -- "$active_job" || return
  sync -f -- "$state_root"
}

require_released_siftwire() {
  local version_output
  version_output="$("$siftwire_bin" --version 2>/dev/null)" || return
  # The runner contract, not a version floor, admits the released binary.
  [[ $version_output =~ ^siftwire[[:space:]]+v[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

require_siftwire_contract() {
  local result
  result="$(printf '%s\n' '{"action":"validate"}' |
    SIFTWIRE_DATABASE_PATH="$database_path" "$siftwire_bin" brief)" || return
  jq -e '
    .runner_protocol == "siftwire-runner/v5"
    and (.capabilities | type == "array")
    and (.capabilities | index("prepared-delivery/v1") != null)
    and (.capabilities | index("current-news/v1") != null)
    and .rejected == false
  ' <<<"$result" >/dev/null
}

require_mailgate_profile() {
  local validation
  validation="$("$mailgate_bin" validate)" || return
  jq -e '
    .schema_version == "mailgate/config-validation/v1"
    and .provider == "resend"
    and (.profiles | type == "array")
    and (.profiles | index("siftwire") != null)
  ' <<<"$validation" >/dev/null
}

validate_selection() {
  jq -e -s --slurpfile brief "$brief_result" '
    . as $all
    | ($brief[0].candidates | length) as $count
    | $brief[0].candidate_slots as $remaining
    | ($all | length == 1)
    and ($all[0] | type == "object"
      and (keys == ["candidate_indexes"])
      and (.candidate_indexes | type == "array")
      and ([.candidate_indexes[]] | length == (unique | length))
      and (.candidate_indexes | length <= $remaining)
      and all(.candidate_indexes[]; type == "number" and . == floor and . >= 0 and . < $count))
  ' "$1" >/dev/null
}

record_selection() {
  local receipt
  receipt="$(jq -cn --slurpfile brief "$brief_result" --slurpfile selection "$selection" \
    --arg observed_at "$(date --utc +%Y-%m-%dT%H:%M:%SZ)" \
    --arg automation_run_id "$(basename -- "$run_dir")" \
    --arg mode "$mode" --arg origin "$selection_origin" '
    $brief[0] as $brief | ($selection[0].candidate_indexes | length) as $selected
    | ($brief.candidates | length) as $count
    | {
        schema_version:"siftwire-selection-receipt/v1",observed_at:$observed_at,
        automation_run_id:$automation_run_id,siftwire_run_id:$brief.run_id,
        mode:$mode,selection_origin:$origin,
        candidate_count:$count,candidate_slots:$brief.candidate_slots,selected_candidate_count:$selected,
        required_count:($brief.must_include | length),sports_update_count:(($brief.sports_updates // []) | length),
        empty_selection_with_candidates:($count > 0 and $brief.candidate_slots > 0 and $selected == 0)
      }
  ')" || return
  write_atomic "$run_dir/selection-receipt.json" "$receipt" || return
  if jq -e '.empty_selection_with_candidates' <<<"$receipt" >/dev/null; then
    printf 'siftwire automation: no news selected from %s candidates with %s slots; evidence: %s\n' \
      "$(jq -r '.candidate_count' <<<"$receipt")" "$(jq -r '.candidate_slots' <<<"$receipt")" "$run_dir" >&2
  fi
}

mode=scheduled
case "${1:-}" in
  --replay)
    [[ $# == 3 ]] || {
      printf 'Usage: siftwire automation --replay BRIEF_JSON NEW_OUTPUT_DIRECTORY\n' >&2
      exit 64
    }
    mode=replay
    replay_brief="$2"
    replay_directory="$3"
    ;;
  "") [[ $# == 0 ]] || exit 64 ;;
  *)
    printf 'Usage: siftwire automation [--replay BRIEF_JSON NEW_OUTPUT_DIRECTORY]\n' >&2
    exit 64
    ;;
esac

for command in jq pi realpath; do
  require_command "$command"
done
[[ -r $render_prompt ]] || {
  printf 'siftwire automation: render prompt is not readable: %s\n' "$render_prompt" >&2
  exit 1
}
render_prompt="$(realpath -- "$render_prompt")"

if [[ $mode == replay ]]; then
  replay_brief="$(realpath -- "$replay_brief")"
  if ! brief_validation_error="$(validate_brief_result "$replay_brief")"; then
    printf 'siftwire automation: replay brief is invalid (%s)\n' "$brief_validation_error" >&2
    exit 1
  fi
  mkdir -- "$replay_directory"
  run_dir="$(realpath -- "$replay_directory")"
  brief_result="$run_dir/brief.json"
  jq '.must_include //= [] | .candidates //= [] | .health_footnote //= ""' "$replay_brief" >"$brief_result"
else
  for command in flock sha256sum stat sync "$siftwire_bin" "$mailgate_bin"; do
    require_command "$command"
  done
  [[ -r $database_path ]] || {
    printf 'siftwire automation: database is not readable: %s\n' "$database_path" >&2
    exit 1
  }
  require_mailgate_profile || {
    printf 'siftwire automation: Mailgate validation failed or profile siftwire is unavailable\n' >&2
    exit 1
  }
  mkdir -p -- "$state_root" "$run_root" "$pending_dir" "$sent_dir"
  exec 9>"$state_root/run.lock"
  if ! flock --nonblock 9; then
    printf 'siftwire automation: another run holds %s\n' "$state_root/run.lock" >&2
    exit 75
  fi
  require_current_durable_state

  run_dir="$(mktemp -d "$run_root/run.XXXXXX")"
  run_dir="$(realpath -- "$run_dir")"
  finish_run() {
    local status=$?
    trap - EXIT
    if ! publish_to_notes; then
      printf 'siftwire automation: Notes publication is incomplete; delivery was not gated and the next run retries it\n' >&2
      ((status != 0)) || status=$notes_incomplete_status
    fi
    printf 'siftwire automation: retained evidence: %s\n' "$run_dir" >&2
    exit "$status"
  }
  trap finish_run EXIT
  require_released_siftwire || {
    printf 'siftwire automation: runner must be a stable SiftWire release\n' >&2
    exit 1
  }
  require_siftwire_contract || {
    printf 'siftwire automation: runner requires siftwire-runner/v5 with prepared-delivery/v1 and current-news/v1\n' >&2
    exit 1
  }
  process_outbox
  recover_attempt

  brief_result="$run_dir/brief.json"
  [[ ! -L $active_job ]] || {
    printf 'siftwire automation: refusing symlink active job: %s\n' "$active_job" >&2
    exit 1
  }
  if [[ ! -e $active_job ]]; then
    rm -f -- "$attempt_result" "$attempt_stderr" || exit
    write_atomic "$attempt_marker" \
      "$(jq -cn --arg attempted_at "$(date --utc +%Y-%m-%dT%H:%M:%SZ)" \
        '{schema_version:"siftwire-run-attempt/v1",state:"invoking",attempted_at:$attempted_at}')"
    set +e
    printf '%s\n' '{"action":"run_brief","dry_run":false}' |
      SIFTWIRE_DATABASE_PATH="$database_path" "$siftwire_bin" brief >"$attempt_result" 2>"$attempt_stderr"
    run_brief_status=$?
    set -e
    if ((run_brief_status != 0)); then
      printf 'siftwire automation: run_brief failed with status %s; durable attempt requires reconciliation: %s\n' \
        "$run_brief_status" "$attempt_marker" >&2
      exit "$run_brief_status"
    fi
    recover_attempt
  fi
  jq -e '
  .schema_version == "siftwire-active-job/v2"
  and (.brief | type == "object")
  and ((.candidate_indexes == null) or (.candidate_indexes | type == "array"))
' "$active_job" >/dev/null || {
    printf 'siftwire automation: active job is invalid: %s\n' "$active_job" >&2
    exit 1
  }
  jq '.brief | .must_include //= [] | .candidates //= [] | .health_footnote //= ""' \
    "$active_job" >"$brief_result"
  if ! brief_validation_error="$(validate_brief_result "$brief_result")"; then
    printf 'siftwire automation: active run_brief result is invalid (%s): %s\n' \
      "$brief_validation_error" "$active_job" >&2
    exit 1
  fi
fi

cp -- "$render_prompt" "$run_dir/siftwire-render.md"
selection="$run_dir/selection.json"
if [[ $mode == scheduled ]] && jq -e '.candidate_indexes | type == "array"' "$active_job" >/dev/null; then
  selection_origin=resumed
  jq '{candidate_indexes}' "$active_job" >"$selection"
elif [[ $(jq '.candidates | length' "$brief_result") == 0 ]]; then
  selection_origin=no_candidates
  write_atomic "$selection" '{"candidate_indexes":[]}'
else
  selection_origin=pi
  selector_input="$run_dir/selector-input.json"
  jq -c '
    def editorial_item:
      with_entries(select(.key | IN("title", "summary", "url", "source_key", "source_label",
        "kind", "section", "threshold", "priority_rank", "published_at", "outlet", "reporting")));
    with_entries(select(.key | IN("runner_protocol", "capabilities", "run_id", "candidate_slots",
      "must_include", "candidates", "sports_updates", "previous_briefs", "recent_sent", "health_footnote")))
    | .input_kind = "editorial_projection"
    | .must_include |= map(editorial_item)
    | .candidates |= (to_entries | map(.key as $index | .value | editorial_item + {candidate_index:$index}))
    | if has("sports_updates") then .sports_updates |= map(
        with_entries(select(.key | IN("source_key", "source_label", "status", "title", "competition", "starts_at"))))
      else . end
    | if has("recent_sent") then .recent_sent |= map(
        with_entries(select(.key | IN("title", "url", "sent_at"))))
      else . end
  ' "$brief_result" >"$selector_input"
  # Model output shape is not deterministic, so retry invalid selections and
  # keep every raw attempt for diagnostics.
  for try in 1 2 3; do
    attempt_file="$run_dir/selection.$try.json"
    events_file="$run_dir/pi-events.$try.jsonl"
    # Trust only the prompt's own directory, not an inherited cwd.
    # The foreground child must exit before its event stream can authorize work.
    (
      cd -- "$(dirname -- "$render_prompt")" || exit
      exec env \
        MISE_QUIET=1 \
        pi -p --mode json --approve \
        --name siftwire-automation --session-dir "$run_dir/pi-sessions/$try" \
        "@$run_dir/siftwire-render.md" "@$selector_input" \
        'Return the candidate selection object for this SiftWire run.'
    ) >"$events_file" 2>"$run_dir/pi.$try.stderr" || {
      printf 'siftwire automation: Pi failed; logs: %s\n' "$run_dir" >&2
      exit 1
    }
    if [[ -s $run_dir/pi.$try.stderr ]]; then
      printf 'siftwire automation: Pi emitted diagnostics; logs: %s\n' "$run_dir" >&2
    fi
    if ! jq -ers '
      def successful_message:
        .role=="assistant" and .stopReason=="stop" and .errorMessage==null
        and (.content|type)=="array"
        and all(.content[]; .type=="thinking" or (.type=="text" and (.text|type)=="string"));
      def message_text: [.content[] | select(.type=="text") | .text] | join("");
      def empty_success: successful_message and (message_text=="");
      def empty_chunk:
        (.stopReason==null or (.stopReason|IN("pending","stop")))
        and (.content=(.content // []) | .stopReason="stop" | empty_success);
      def empty_update:
        if has("message") then .message|empty_chunk
        else .assistantMessageEvent
          | if (.type|IN("thinking_start","text_start")) then true
            elif .type=="thinking_delta" then (.delta|type)=="string"
            elif .type=="thinking_end" then (.content|type)=="string"
            elif .type=="text_delta" then .delta==""
            elif .type=="text_end" then .content==""
            else false end
        end;
      # Only the stock availability notification may carry a prior result
      # through one opened, successful, exactly empty assistant turn.
      def notification_event($event):
        # Display-only unknown input leaves the result valid, but not reusable.
        if ($event.type|IN("message_start","message_update","message_end"))
            and $event.message.role=="custom" and $event.message.customType!="web-search-content-ready"
          then .notification_blocked=true
        elif $event.type=="message_end" and ($event.message|successful_message and (message_text|test("\\S")))
          then .notification_blocked=false
        else . end
        | if .notification==null or ($event.type|IN("turn_start","turn_end","entry_appended")) then .
        elif $event.type=="message_start"
            and $event.message.role=="custom" and $event.message.customType=="web-search-content-ready"
            and (.notification.phase|IN("awaiting","notified")) then .notification.phase="notification_started"
        elif $event.type=="message_end"
            and $event.message.role=="custom" and $event.message.customType=="web-search-content-ready"
            and .notification.phase=="notification_started" then .notification.phase="notified"
        elif $event.type=="message_start" and $event.message.role=="assistant"
            and .notification.phase=="notified" and ($event.message|empty_chunk) then .notification.phase="responding"
        elif $event.type=="message_update" and .notification.phase=="responding"
            and ($event|empty_update) then .
        elif $event.type=="message_end" and .notification.phase=="responding"
            and ($event.message|empty_success) then .notification.phase="completed"
        else .notification=null end;
      reduce .[] as $event (
        {started:false, ended:false, settled:false, pending:[], message:null, turnOpen:false, notification:null, notification_blocked:false};
        notification_event($event)
        | if ($event | type) != "object" then error("event must be an object")
        elif $event.type == "agent_start" then
          .started = true | .ended = false | .settled = false | .message = null
        elif $event.type == "turn_start" then
          .notification=(if (.turnOpen|not) and (.pending|length)==0 and (.notification_blocked|not)
              and (.message|successful_message and (message_text|test("\\S")))
            then {phase:"awaiting",prior:.message} else null end)
          | .turnOpen=true | .ended=false | .settled=false | .message=null
        elif $event.type == "turn_end" then
          if .turnOpen and .notification.phase=="completed"
              and (.pending|length)==0 and ($event.message|empty_success) then .message=.notification.prior
          elif .notification!=null then .message=null else . end
          | .notification=null | .turnOpen=false | .ended=false | .settled=false
        # Out-of-turn custom diagnostics do not replace the active result.
        elif ($event.type | IN("message_start", "message_update", "message_end")) and $event.message.role == "custom" then .
        elif ($event.type | IN("message_start", "message_update")) then
          .ended = false | .settled = false | .message = null
        elif $event.type == "message_end" then
          .ended = false | .settled = false | .message = null
          | if $event.message.role == "assistant" then
              .message = $event.message
              | .pending += [$event.message | select(.stopReason != "error" and .stopReason != "aborted")
                | .content[] | select(.type == "toolCall") | .id]
              | .pending |= unique
            else . end
        elif $event.type == "tool_execution_start" then
          .message = null | .ended = false | .settled = false
          | .pending = ((.pending + [$event.toolCallId]) | unique)
        elif $event.type == "tool_execution_end" then
          .message = null | .ended = false | .settled = false | .pending -= [$event.toolCallId]
        elif $event.type == "tool_execution_update" then
          .message = null | .ended = false | .settled = false
        elif $event.type == "agent_end" then .ended = true | .settled = false
        elif $event.type == "agent_settled" then .settled = .ended
        else . end
      )
      | if .started and .ended and .settled and (.pending | length == 0)
          and .message.role == "assistant" and .message.stopReason == "stop" and .message.errorMessage == null
          and (.message.content | type == "array")
          and all(.message.content[]; .type != "toolCall")
          and any(.message.content[]; .type == "text" and (.text | type == "string"))
        then [.message.content[] | select(.type == "text") | .text] | join("")
        else error("Pi did not finish with a successful settled assistant result") end
    ' "$events_file" >"$attempt_file"; then
      printf 'siftwire automation: Pi lifecycle or final assistant result failed; logs: %s\n' "$run_dir" >&2
      exit 1
    fi
    if validate_selection "$attempt_file"; then
      mv -- "$attempt_file" "$selection"
      break
    fi
    printf 'siftwire automation: Pi returned an invalid candidate selection on attempt %s\n' "$try" >&2
  done
  [[ -e $selection ]] || {
    printf 'siftwire automation: Pi returned an invalid candidate selection after 3 attempts; logs: %s\n' "$run_dir" >&2
    exit 1
  }
fi
validate_selection "$selection" || {
  printf 'siftwire automation: invalid persisted candidate selection; evidence: %s\n' "$run_dir" >&2
  exit 1
}
record_selection || printf 'siftwire automation: selection receipt recording failed; evidence: %s\n' "$run_dir" >&2
if [[ $mode == replay ]]; then
  jq -c . "$selection"
  printf 'siftwire automation: replay complete; evidence: %s\n' "$run_dir" >&2
  exit 0
fi
if jq -e '.candidate_indexes == null' "$active_job" >/dev/null; then
  updated_active_job="$(jq --slurpfile selection "$selection" \
    '.candidate_indexes = $selection[0].candidate_indexes' "$active_job")" || exit
  write_atomic "$active_job" "$updated_active_job" || exit
fi

prepared="$run_dir/prepared.json"
prepare_request="$(jq -cn --slurpfile brief "$brief_result" --slurpfile selection "$selection" \
  '{action:"prepare_delivery",run_id:$brief[0].run_id,candidate_indexes:$selection[0].candidate_indexes}')" || exit
if ! printf '%s\n' "$prepare_request" | SIFTWIRE_DATABASE_PATH="$database_path" \
  "$siftwire_bin" brief >"$prepared"; then
  printf 'siftwire automation: prepare_delivery failed; retained %s\n' "$active_job" >&2
  exit 1
fi
siftwire_run_id="$(jq -er '.run_id' "$brief_result")" || exit
validate_prepared_result "$prepared" "$siftwire_run_id" || {
  printf 'siftwire automation: prepare_delivery returned invalid JSON; retained %s\n' "$active_job" >&2
  exit 1
}
outbox_digest="$(printf '%s' "$siftwire_run_id" | sha256sum | cut -d' ' -f1)"
outbox="$pending_dir/$outbox_digest.json"
sent_outbox="$sent_dir/$outbox_digest.json"
if [[ -f $sent_outbox && ! -L $sent_outbox ]]; then
  clear_active_job || exit
  printf 'siftwire automation: recovered completed SiftWire run %s\n' "$siftwire_run_id"
  exit 0
fi
[[ ! -e $outbox && ! -L $outbox && ! -e $sent_outbox && ! -L $sent_outbox ]] || {
  printf 'siftwire automation: unexpected outbox state for run %s\n' "$siftwire_run_id" >&2
  exit 1
}
outbox_json="$(jq -cn --slurpfile prepared "$prepared" --arg siftwire_run_id "$siftwire_run_id" \
  --arg idempotency_key "siftwire-v2-$outbox_digest" \
  --arg subject "SiftWire — $(date '+%Y-%m-%d %H:%M %Z')" '
  $prepared[0] as $plan
  | ($plan.message != "NO_REPLY") as $needs_email
  | {
      schema_version:"siftwire-email-outbox/v2",
      siftwire_run_id:$siftwire_run_id,
      delivery_plan_id:$plan.delivery_plan_id,
      needs_email:$needs_email,
      idempotency_key:$idempotency_key,
      request_body:(if $needs_email then {subject:$subject,text:$plan.text,html:($plan.html // "")} | tojson else null end),
      email_id:null,
      delivery_attempt_count:0,
      last_delivery_attempt_at:null,
      delivery_recorded_at:null,
      delivery_record_source:null
    }
  ')"
write_atomic "$outbox" "$outbox_json"
clear_active_job || exit
process_outbox
printf 'siftwire automation: complete for SiftWire run %s\n' "$siftwire_run_id"
