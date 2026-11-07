#!/usr/bin/env bash
set -Eeuo pipefail

# No inherited account paths, credentials, tool launchers, or application state.
if [[ ${SIFTWIRE_FIXTURE_ENV:-} != 1 ]]; then
  exec env -i PATH=/usr/bin:/bin SIFTWIRE_FIXTURE_ENV=1 /usr/bin/bash "$0" "$@"
fi

root="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
binary="$(realpath -- "${1:-$root/target/release/siftwire}")"
[[ -x $binary ]] || {
  printf 'Usage: %s [SIFTWIRE_BINARY]\n' "$0" >&2
  exit 2
}
prompt_source="$root/crates/siftwire/automation/siftwire-render.md"
export TEST_RUNNER_PROTOCOL=siftwire-runner/v5
temp="$(mktemp -d)"
waiting_runner=""
cleanup() {
  local status=$?
  trap - EXIT
  if [[ -n $waiting_runner ]]; then
    # Settle the isolated FIFO fixture even when an assertion fails mid-wait.
    kill -TERM -- "-$waiting_runner" 2>/dev/null || true
    wait "$waiting_runner" 2>/dev/null || true
  fi
  rm -rf -- "$temp"
  exit "$status"
}
trap cleanup EXIT
mkdir -p "$temp/bin" "$temp/home" "$temp/state" "$temp/run"
telemetry_probe="$temp/home/.local/share/dotfiles-pi-package/current/node_modules/yazan-pi-setup/runtime-probes/automation-telemetry.ts"
mkdir -p "${telemetry_probe%/*}"
printf '%s\n' '// Fixture observer' >"$telemetry_probe"
runner="$temp/bin/siftwire-automation"
cat >"$runner" <<EOF
#!/usr/bin/env bash
exec "$binary" automation "\$@"
EOF
cat >"$temp/bin/model-role" <<'EOF'
#!/usr/bin/env bash
[[ $* == fast ]] && printf '%s\n' openai/fixture-fast
EOF
chmod +x "$runner" "$temp/bin/model-role"
touch "$temp/database.sqlite" "$temp/contract.json"
printf '%s\n' 'xaat-test-token' >"$temp/axiom-token"
chmod 0600 "$temp/axiom-token"

# Exercise lifecycle, transport, and recovery with the current runner contract.
cat >"$temp/bin/runner-result" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
protocol="$TEST_RUNNER_PROTOCOL"
missing_capability=""
if [[ ${TEST_INVALID_STAGE:-} == "$1" ]]; then
  case "$TEST_INVALID_CONTRACT" in
    legacy) protocol=siftwire-runner/v4 ;;
    unknown) protocol=siftwire-runner/v6 ;;
    missing_capability) missing_capability=prepared-delivery/v1 ;;
    missing_current_news) missing_capability=current-news/v1 ;;
  esac
fi
jq --arg protocol "$protocol" --arg missing_capability "$missing_capability" --arg stage "$1" '
  if has("runner_protocol") then
    .runner_protocol = $protocol
    | .capabilities += ["current-news/v1"]
    | if $stage == "run_brief" then
            .fetch_status = [
              {source_key:"alpha",source_label:"Alpha",status:"ok",items:10,new_items:null,
               current_news:{since:"2026-09-07T12:00:00Z",until:"2026-09-08T12:00:00Z",
                             eligible_items:1,stale_items:2,undated_items:3,future_items:4}},
              {source_key:"beta",source_label:"Beta",status:"ok",items:1,new_items:0}
            ]
      else . end
    | if $stage == "run_brief" then
        if env.TEST_REQUIRED_ITEMS == "omitted" then del(.must_include)
        elif env.TEST_REQUIRED_ITEMS == "null" then .must_include = null
        else . end
        | if env.TEST_LINKLESS == "1" then
            .must_include[0].url = "" | .candidates |= map(.url = "")
          else . end
        | if env.TEST_INVALID_ITEM != null then
            setpath([env.TEST_ITEM_GROUP, 0]; (env.TEST_INVALID_ITEM | fromjson))
          else . end
      else . end
    | .capabilities -= [$missing_capability]
    | if env.TEST_INVALID_STAGE == $stage and env.TEST_INVALID_CONTRACT == "string_capabilities" then
        .capabilities = "prepared-delivery/v10 current-news/v10"
      else . end
  else . end
' | tee "$(if [[ $1 == run_brief ]]; then printf '%s' "${TEST_BRIEF_CAPTURE:-/dev/null}"; else printf /dev/null; fi)"
EOF
chmod +x "$temp/bin/runner-result"

cat >"$temp/bin/prooflane" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
case "$1 $2 $3" in
  'version --format json')
    printf '%s\n' '{"schema_version":"prooflane/version-report/v1","product_version":"test","platform":"test"}'
    ;;
  'dogfood siftwire run')
    [[ -z ${TEST_PROOFLANE_RUN_COUNT:-} ]] || printf 'run\n' >>"$TEST_PROOFLANE_RUN_COUNT"
    if [[ ${TEST_BRIEF_MODE:-brief} == empty_failure ]]; then
      exit 127
    elif [[ ${TEST_BRIEF_MODE:-brief} == multiple ]]; then
      printf '%s\n' \
        '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"run_id":"brief-run-multiple","max_delivery_items":2,"candidate_slots":2}' \
        '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"run_id":"brief-run-multiple","max_delivery_items":2,"candidate_slots":2}'
    elif [[ ${TEST_BRIEF_MODE:-brief} == rejected ]]; then
      printf '%s\n' '{"rejected":true,"rejection_reason":"no enabled sources configured"}'
    elif [[ ${TEST_BRIEF_MODE:-brief} == no_reply ]]; then
      printf '%s\n' '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"run_id":"brief-run-2","max_delivery_items":2,"candidate_slots":2}'
    elif [[ ${TEST_BRIEF_MODE:-brief} == duplicate ]]; then
      printf '%s\n' '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"run_id":"brief-run-duplicate","candidates":[{"title":"a","url":"https://example.com/same"},{"title":"b","url":"https://example.com/same"}],"max_delivery_items":2,"candidate_slots":2}'
    elif [[ ${TEST_BRIEF_MODE:-brief} == invalid_candidates ]]; then
      printf '%s\n' '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"run_id":"brief-run-invalid","candidates":{},"max_delivery_items":2,"candidate_slots":2}'
    else
      jq -n '{
        runner_protocol:"siftwire-runner/v5",capabilities:["prepared-delivery/v1"],rejected:false,
        run_id:"brief-run-1",max_delivery_items:2,candidate_slots:1,
        must_include:[{title:"Required",url:"https://example.com/required",source_key:"required",threshold:"always"}],
        candidates:[
          {title:"Selected",url:"https://news.google.com/rss/articles/OPAQUE-SELECTED",
           candidate_index:999,id:"candidate-id",ids:["alias-id"],aliases:["https://example.com/selected"],run_item_ids:["item-id"],
           summary:"A substantive development",source_key:"alpha",source_label:"Alpha",kind:"rss",
           section:"technology",threshold:"high",priority_rank:0,published_at:"2026-09-08T10:00:00Z",
           outlet:"Example",reporting:{policy:"watch",note:"Check evidence"}},
          {title:"Skipped",url:"https://news.google.com/rss/articles/OPAQUE-SKIPPED"}
        ],
        sports_updates:[{source_key:"sport",source_label:"Sport",status:"final",title:"Team A 2–1 Team B",
          competition:"League",starts_at:"2026-09-08T09:00:00Z",url:"https://example.com/match",
          images:[{url:"https://example.com/logo.png",alt:"Team A"}]}],
        sports_section:"Rendered sports with images and links",
        previous_briefs:[{run_id:"prior-run",delivered_at:"2026-09-08T07:00:00Z",
          message:"[Earlier coverage](https://example.com/prior)\n\nNO_REPLY\nHealth: source unavailable\n"}],
        recent_sent:[{title:"Earlier coverage",url:"https://example.com/prior",sent_at:"2026-09-08T07:00:00Z"}],
        health_footnote:"Health: source unavailable",
        suppressed:[{title:"Dropped bulk",url:"https://news.google.com/rss/articles/OPAQUE-DROPPED"}],
        suppressed_recent:[{title:"Already sent"}],suppressed_policy:[{title:"Policy dropped"}],
        suppressed_unresolved:[{title:"Unresolved"}],fetch_status:[{source_key:"alpha",status:"ok",items:2,new_items:2}],
        paths:{database_path:"/private/transport/path"},summary:"Full runner diagnostics",
        message:"Current transport markdown",text:"Current transport text",html:"<p>Current transport HTML</p>"
      }'
    fi | runner-result run_brief
    printf 'PROOFLANE_METADATA\t{"prooflane":{"run_id":"%s","status":"observed","verdict":"unverified","proof_error":false}}\n' "${TEST_PROOF_RUN_ID:-proof-run-1}" >&2
    [[ ${TEST_BRIEF_EXIT_FAILURE:-0} != 1 ]] || exit 99
    ;;
  'dogfood siftwire delivery')
    request="$(cat)"
    if [[ -n ${TEST_DELIVERY_ALREADY_CLAIMED_ONCE_FILE:-} && ! -e $TEST_DELIVERY_ALREADY_CLAIMED_ONCE_FILE ]]; then
      touch "$TEST_DELIVERY_ALREADY_CLAIMED_ONCE_FILE"
      printf '%s\n' 'adapter context is already claimed' >&2
      exit 1
    fi
    [[ -z ${TEST_DELIVERY_COUNT:-} ]] || printf 'delivery\n' >>"$TEST_DELIVERY_COUNT"
    printf '%s\n' "$request" >"$TEST_DELIVERY_REQUEST"
    if [[ -n ${TEST_DELIVERY_FAIL_FILE:-} ]]; then exit 9; fi
    [[ -z ${TEST_SEALED_FILE:-} ]] || printf 'sealed\n' >"$TEST_SEALED_FILE"
    if [[ -n ${TEST_DELIVERY_CRASH_ONCE_FILE:-} && ! -e $TEST_DELIVERY_CRASH_ONCE_FILE ]]; then
      touch "$TEST_DELIVERY_CRASH_ONCE_FILE"
      exit 99
    fi
    jq -cn --arg run_id "$(jq -r .run_id <<<"$request")" \
      --arg plan_id "$(jq -r .delivery_plan_id <<<"$request")" \
      '{rejected:false,run_id:$run_id,delivery_plan_id:$plan_id,final_answer:"Current brief"}'
    printf 'PROOFLANE_METADATA\t{"prooflane":{"run_id":"%s","status":"sealed","verdict":"unverified","proof_error":false}}\n' "${TEST_PROOF_RUN_ID:-proof-run-1}" >&2
    ;;
  'show --run '*)
    [[ -z ${TEST_SEALED_FILE:-} || -e $TEST_SEALED_FILE ]] || exit 1
    assertion_outcome=pass
    [[ ${TEST_RECOVERY_ASSERTIONS_FAIL:-0} != 1 ]] || assertion_outcome=error
    printf '{"schema_version":"prooflane/receipt/v0","run_id":"%s","state":"unverified","actions":[{"id":"siftwire-record-delivery","status":"succeeded"}],"assertions":[{"id":"runner-protocol-valid","outcome":"%s"},{"id":"local-delivery-recorded","outcome":"%s"}]}\n' \
      "$3" "$assertion_outcome" "$assertion_outcome"
    ;;
  *)
    printf 'unexpected prooflane args: %s\n' "$*" >&2
    exit 1
    ;;
esac
EOF
cat >"$temp/bin/pi" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ ${MISE_QUIET:-} == 1 ]] || { printf 'Pi launcher must keep status output out of JSON stdout\n' >&2; exit 1; }
[[ $PWD == "$HOME/.local/share/siftwire/automation/"* ]]
for argument in "$@"; do
  case "$argument" in
    --no-* | --tools | --exclude-tools | --skill | --system-prompt) exit 1 ;;
  esac
done
[[ " $* " == *' -p --mode json --approve '* ]]
telemetry_probe="$HOME/.local/share/dotfiles-pi-package/current/node_modules/yazan-pi-setup/runtime-probes/automation-telemetry.ts"
extensions=()
previous_argument=""
for argument in "$@"; do
  [[ $previous_argument != --extension ]] || extensions+=("$argument")
  previous_argument="$argument"
done
if [[ ${TEST_TELEMETRY:-} == missing_probe ]]; then
  [[ ! -e $telemetry_probe && ${#extensions[@]} == 0 ]]
else
  [[ -r $telemetry_probe && ${#extensions[@]} == 1 && ${extensions[0]} == "$telemetry_probe" ]]
fi
[[ " $* " == *' --model openai/fixture-fast --thinking medium '* ]]
[[ -z ${PI_RUNTIME_DIGEST+x} ]]
selector_input=""
prompt_snapshot=""
session_dir=""
previous_argument=""
for argument in "$@"; do
  [[ $argument != @*/selector-input.json ]] || selector_input="${argument#@}"
  [[ $argument != @*/siftwire-render.md ]] || prompt_snapshot="${argument#@}"
  [[ $previous_argument != --session-dir ]] || session_dir="$argument"
  previous_argument="$argument"
done
[[ -f $selector_input && $(stat -c %a "$selector_input") == 600 ]]
[[ $(stat -c %a "$(dirname "$selector_input")") == 700 ]]
[[ $(wc -l <"$selector_input") == 1 ]]
run_dir="$(dirname "$selector_input")"
[[ $prompt_snapshot == "$run_dir/siftwire-render.md" ]]
cmp -- "$PWD/siftwire-render.md" "$prompt_snapshot"
[[ $session_dir == "$run_dir/pi-sessions/"* ]]
[[ ${session_dir##*/} =~ ^[1-3]$ ]]
mkdir -p -m 0700 -- "$session_dir"
jq -cn --arg cwd "$PWD" '{type:"session",version:3,id:"fixture",timestamp:"2026-09-08T12:00:00Z",cwd:$cwd}' \
  >"$session_dir/fixture.jsonl"
cp -- "$selector_input" "$TEST_PI_INPUT_CAPTURE"
if [[ ${TEST_REPLAY:-0} == 1 ]]; then
  [[ ! -e $SIFTWIRE_STATE_DIR/active-job.json ]]
else
  jq -e --slurpfile full "$TEST_BRIEF_CAPTURE" '
    .brief == $full[0] and .prooflane_run_id == (env.TEST_PROOF_RUN_ID // "proof-run-1")
  ' "$SIFTWIRE_STATE_DIR/active-job.json" >/dev/null
fi
jq -e --slurpfile full "$TEST_BRIEF_CAPTURE" '
  $full[0] as $brief
  | .input_kind == "editorial_projection"
    and .runner_protocol == $brief.runner_protocol and .capabilities == $brief.capabilities
    and .run_id == $brief.run_id and .candidate_slots == $brief.candidate_slots
    and .candidates == (($brief.candidates // []) | to_entries
      | map(.key as $index | .value | del(.id, .ids, .aliases, .run_item_ids, .candidate_index)
        + {candidate_index:$index}))
    and .must_include == ($brief.must_include // [])
    and .sports_updates == (($brief.sports_updates // []) | map(del(.url, .images)))
    and .previous_briefs == ($brief.previous_briefs // [])
    and .recent_sent == ($brief.recent_sent // [])
    and .health_footnote == ($brief.health_footnote // "")
    and (keys == (["input_kind", "runner_protocol", "capabilities", "run_id", "candidate_slots",
      "candidates", "must_include", "sports_updates", "previous_briefs", "recent_sent", "health_footnote"] | sort))
' "$selector_input" >/dev/null
[[ -z ${TEST_PI_COUNT:-} ]] || printf 'selection\n' >>"$TEST_PI_COUNT"
case "${TEST_TELEMETRY:-present}" in
  missing | missing_probe) ;;
  malformed) printf 'invalid telemetry\n' >"$PI_AUTOMATION_TELEMETRY_PATH" ;;
  incomplete) printf '%s\n' '{"event":"summary","settled":false,"orphanedTools":1}' >"$PI_AUTOMATION_TELEMETRY_PATH" ;;
  *) printf '%s\n' '{"event":"summary","settled":true,"orphanedTools":0,"version":"unrelated-new-version"}' >"$PI_AUTOMATION_TELEMETRY_PATH" ;;
esac
[[ ${TEST_PI_DIAGNOSTIC:-0} != 1 ]] || printf 'fixture observer hook failed\n' >&2
if [[ -n ${TEST_PI_FAIL_ONCE_FILE:-} && ! -e $TEST_PI_FAIL_ONCE_FILE ]]; then
  touch "$TEST_PI_FAIL_ONCE_FILE"
  exit 1
fi
selection='{"candidate_indexes":[0]}'
[[ ${TEST_LINKLESS:-0} != 1 ]] || selection='{"candidate_indexes":[1]}'
case "${TEST_PI_MODE:-select_one}" in
  select_none) selection='{"candidate_indexes":[]}' ;;
  overflow) selection='{"candidate_indexes":[0,1]}' ;;
  duplicate) selection='{"candidate_indexes":[0,0]}' ;;
  negative) selection='{"candidate_indexes":[-1]}' ;;
  fractional) selection='{"candidate_indexes":[0.5]}' ;;
  boolean) selection='{"candidate_indexes":[true]}' ;;
  extra_key) selection='{"candidate_indexes":[0],"send":true}' ;;
  multiple) selection='{"candidate_indexes":[0]} {"candidate_indexes":[]}' ;;
esac
if [[ -n ${TEST_PI_INVALID_ONCE_FILE:-} && ! -e $TEST_PI_INVALID_ONCE_FILE ]]; then
  touch "$TEST_PI_INVALID_ONCE_FILE"
  selection='{"candidate_indexes":[7]}'
fi
printf '%s\n' '{"type":"session","version":999}' '{"type":"agent_start"}'
# Intermediate text and tool results are deliberately not selection objects.
printf '%s\n' \
  '{"type":"message_end","message":{"role":"assistant","stopReason":"toolUse","content":[{"type":"text","text":"Check context first"},{"type":"toolCall","id":"research","name":"ordinary_extension_tool","arguments":{}}]}}' \
  '{"type":"tool_execution_start","toolCallId":"research","toolName":"ordinary_extension_tool"}'
if [[ ${TEST_PI_MODE:-} != dangling_tool ]]; then
  printf '%s\n' '{"type":"tool_execution_end","toolCallId":"research","isError":false}'
fi
printf '%s\n' '{"type":"message_end","message":{"role":"toolResult","toolCallId":"research","content":[{"type":"text","text":"not a selection"}]}}'
# Failed partial calls never execute. Retry must retain any started research tool.
for stop in error aborted; do
  jq -cn --arg stop "$stop" '{type:"message_end",message:{role:"assistant",content:[{type:"toolCall",id:("partial-"+$stop),name:"read",arguments:{}}],stopReason:$stop,errorMessage:"stream failed"}}'
  printf '%s\n' '{"type":"agent_end","messages":[]}' '{"type":"agent_start"}'
done
stop_reason=stop
case "${TEST_PI_MODE:-}" in
  assistant_error) stop_reason=error ;;
  aborted) stop_reason=aborted ;;
  truncated) stop_reason=length ;;
esac
jq -cn --arg text "$selection" --arg stop "$stop_reason" \
  '{type:"message_end",message:{role:"assistant",stopReason:$stop,content:[{type:"thinking",thinking:"private fixture thought"},{type:"text",text:$text}]}}'
notification_mode="${TEST_PI_MODE:-}"
case "$notification_mode" in
  notification-prior-tool) printf '%s\n' '{"type":"tool_execution_start","toolCallId":"late"}' '{"type":"tool_execution_end","toolCallId":"late"}' ;;
  notification-prior-tool-update | notification-prior-tool-end)
    jq -cn --arg type "tool_execution_${notification_mode##*-}" '{type:$type,toolCallId:"late"}' ;;
  notification-prior-user-end) printf '%s\n' '{"type":"message_end","message":{"role":"user","content":"New input"}}' ;;
  notification-prior-unknown | notification-replacement) printf '%s\n' '{"type":"message_end","message":{"role":"custom","customType":"unknown","content":"Display only"}}' ;;
esac
if [[ $notification_mode == notification-replacement ]]; then
  printf '%s\n' '{"type":"turn_start"}' '{"type":"message_start","message":{"role":"assistant"}}'
  jq -cn '{type:"message_end",message:{role:"assistant",stopReason:"stop",content:[{type:"text",text:"{\"candidate_indexes\":[1]}"}]}}'
  printf '%s\n' '{"type":"turn_end"}'
fi
for cycle in 1 2; do
  [[ $notification_mode == notification-* ]] || break
  [[ $notification_mode == notification-unopened ]] || printf '%s\n' '{"type":"turn_start"}'
  role=custom
  custom_type=web-search-content-ready
  [[ $notification_mode != notification-user ]] || role=user
  [[ $notification_mode != notification-unknown ]] || custom_type=unknown
  if [[ $notification_mode != notification-ordinary ]]; then
    notification_events=(message_start message_end)
    [[ $notification_mode != notification-replacement ]] || notification_events+=(message_start message_end)
    for event in "${notification_events[@]}"; do
      [[ $notification_mode != notification-custom-no-start || $event != message_start ]] || continue
      [[ $notification_mode != notification-custom-no-end || $event != message_end ]] || continue
      jq -cn --arg event "$event" --arg role "$role" --arg custom_type "$custom_type" \
        '{type:$event,message:{role:$role,customType:$custom_type,content:"Fixture content available"}}'
    done
  fi
  case "$notification_mode" in
    notification-tool) printf '%s\n' '{"type":"tool_execution_start","toolCallId":"late"}' '{"type":"tool_execution_end","toolCallId":"late"}' ;;
    notification-restart) printf '%s\n' '{"type":"agent_start"}' ;;
  esac
  if [[ $notification_mode == notification-text-start ]]; then
    printf '%s\n' '{"type":"message_start","message":{"role":"assistant","content":[{"type":"text","text":"discarded text"}]}}'
  elif [[ $notification_mode != notification-no-start ]]; then
    printf '%s\n' '{"type":"message_start","message":{"role":"assistant","content":[]}}'
  fi
  printf '%s\n' \
    '{"type":"message_update","assistantMessageEvent":{"type":"thinking_start","contentIndex":0},"usage":{}}' \
    '{"type":"message_update","assistantMessageEvent":{"type":"thinking_end","contentIndex":0,"content":"fixture"},"usage":{}}' \
    '{"type":"message_update","assistantMessageEvent":{"type":"text_start","contentIndex":1},"usage":{}}' \
    '{"type":"message_update","assistantMessageEvent":{"type":"text_end","contentIndex":1,"content":""},"usage":{}}'
  case "$notification_mode" in
    notification-native-text-delta) printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"discarded text"},"usage":{}}' ;;
    notification-native-text-end) printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_end","content":"discarded text"},"usage":{}}' ;;
    notification-native-toolcall-update) printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"toolcall_delta","delta":"{}"},"usage":{}}' ;;
    notification-double-start) printf '%s\n' '{"type":"message_start","message":{"role":"assistant","content":[]}}' ;;
    notification-text-update) printf '%s\n' '{"type":"message_update","message":{"role":"assistant","content":[{"type":"text","text":"discarded text"}]}}' ;;
  esac
  message='{"role":"assistant","stopReason":"stop","content":[{"type":"thinking","thinking":"fixture"},{"type":"text","text":""}]}'
  case "$notification_mode" in
    notification-whitespace) message="$(jq '.content[1].text=" "' <<<"$message")" ;;
    notification-content) message="$(jq '.content += [{type:"unknown"}]' <<<"$message")" ;;
    notification-error | notification-aborted) message="$(jq --arg stop "${notification_mode#notification-}" '.stopReason=$stop | .errorMessage="fixture failure"' <<<"$message")" ;;
  esac
  [[ $notification_mode == notification-no-end ]] || jq -cn --argjson message "$message" '{type:"message_end",message:$message}'
  printf '%s\n' '{"type":"entry_appended","entry":{"type":"custom","customType":"web-search-results"}}'
  if [[ $notification_mode == notification-turn-error ]]; then
    message="$(jq '.stopReason="error" | .errorMessage="fixture turn failure"' <<<"$message")"
  fi
  case "$notification_mode" in
    notification-turn-missing-message) printf '%s\n' '{"type":"turn_end"}' ;;
    notification-turn-null-message) printf '%s\n' '{"type":"turn_end","message":null}' ;;
    notification-no-turn-end) ;;
    *) jq -cn --argjson message "$message" '{type:"turn_end",message:$message}' ;;
  esac
done
case "${TEST_PI_MODE:-}" in
  stale_turn) printf '%s\n' '{"type":"turn_start"}' '{"type":"turn_end"}' ;;
  unfinished_message) printf '%s\n' '{"type":"message_start","message":{"role":"assistant"}}' ;;
  updated_message) printf '%s\n' '{"type":"message_update","message":{"role":"assistant"}}' ;;
esac
printf '%s\n' '{"type":"message_start","message":{"role":"custom","content":"before settlement"}}' \
  '{"type":"message_end","message":{"role":"custom","content":"before settlement"}}' \
  '{"type":"agent_end","messages":[]}'
[[ ${TEST_PI_MODE:-} == unsettled ]] || printf '%s\n' '{"type":"agent_settled"}'
[[ ${TEST_PI_MODE:-} != restarted ]] || printf '%s\n' '{"type":"agent_start"}'
printf '%s\n' '{"type":"message_start","message":{"role":"custom","content":"after settlement"}}' \
  '{"type":"message_end","message":{"role":"custom","content":"after settlement"}}' \
  '{"type":"new_diagnostic","details":"additive event after settlement"}'
# A valid stream is not enough. The host must wait for the actual child exit.
[[ -z ${TEST_PI_WAIT_FIFO:-} ]] || { printf 'ready\n' >"$TEST_PI_READY_FIFO"; read -r _ <"$TEST_PI_WAIT_FIFO"; }
[[ ${TEST_PI_MODE:-} != exit_failure ]] || exit 9
EOF
cat >"$temp/bin/siftwire" <<'EOF'
#!/usr/bin/env bash
if [[ ${1:-} == --version ]]; then
  printf '%s\n' "${TEST_BRIEF_VERSION:-siftwire v0.9.1}"
  exit 0
fi
[[ -z ${TEST_EXPECTED_DATABASE_PATH:-} || ${SIFTWIRE_DATABASE_PATH:-} == "$TEST_EXPECTED_DATABASE_PATH" ]]
request="$(cat)"
case "$(jq -r .action <<<"$request")" in
  validate)
    printf '%s\n' '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"candidate_slots":0,"summary":"valid"}' | runner-result validate
    ;;
  run_brief)
    printf '%s\n' '{"runner_protocol":"siftwire-runner/v5","capabilities":["prepared-delivery/v1"],"rejected":false,"run_id":"direct-run","max_delivery_items":2,"candidate_slots":2}' | runner-result run_brief
    ;;
  prepare_delivery)
    if [[ ${TEST_PI_MODE:-} == notification-replacement || ${TEST_LINKLESS:-0} == 1 ]]; then
      jq -e '.candidate_indexes==[1]' <<<"$request" >/dev/null || exit 1
    elif [[ -n ${TEST_REQUIRED_ITEMS:-} ]]; then
      jq -e '.candidate_indexes==[0]' <<<"$request" >/dev/null || exit 1
    fi
    jq -e --argjson request "$request" '
      .schema_version == "siftwire-active-job/v2"
      and .brief.run_id == $request.run_id
      and .candidate_indexes == $request.candidate_indexes
    ' "${SIFTWIRE_STATE_DIR:-$HOME/.local/state/pi-automations/siftwire}/active-job.json" >/dev/null || exit 1
    if [[ $(jq -r .run_id <<<"$request") == brief-run-1 ]]; then
      jq -e --slurpfile full "$TEST_BRIEF_CAPTURE" '.brief == $full[0]' \
        "$SIFTWIRE_STATE_DIR/active-job.json" >/dev/null || exit 1
    fi
    if [[ -n ${TEST_PREPARE_FAIL_ONCE_FILE:-} && ! -e $TEST_PREPARE_FAIL_ONCE_FILE ]]; then
      touch "$TEST_PREPARE_FAIL_ONCE_FILE"
      exit 9
    fi
    run_id="$(jq -r .run_id <<<"$request")"
    plan_id="plan-$run_id"
    if [[ $run_id == brief-run-1 ]]; then
      jq -cn --arg run_id "$run_id" --arg plan_id "$plan_id" \
        --arg message "${TEST_PREPARED_MESSAGE:-$'MARKDOWN-BODY\n'}" --arg text $'TEXT-WITH-SPORTS\n' \
        --arg html $'<p>HTML-BODY</p>\n' \
        '{runner_protocol:"siftwire-runner/v5",capabilities:["prepared-delivery/v1"],rejected:false,run_id:$run_id,delivery_plan_id:$plan_id,message:$message,text:$text,html:$html}'
    else
      jq -cn --arg run_id "$run_id" --arg plan_id "$plan_id" \
        '{runner_protocol:"siftwire-runner/v5",capabilities:["prepared-delivery/v1"],rejected:false,run_id:$run_id,delivery_plan_id:$plan_id,message:"NO_REPLY",text:"NO_REPLY"}'
    fi | runner-result prepare_delivery
    ;;
  confirm_delivery)
    printf '%s\n' "$request" >"$TEST_DELIVERY_REQUEST"
    jq -cn --arg run_id "$(jq -r .run_id <<<"$request")" \
      --arg plan_id "$(jq -r .delivery_plan_id <<<"$request")" \
      '{rejected:false,run_id:$run_id,delivery_plan_id:$plan_id,final_answer:"Current brief"}'
    ;;
  *) exit 1 ;;
esac
EOF
cat >"$temp/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ $* == *api.axiom.co* && $* == *'--data-binary @-'* ]]
request="$(cat)"
printf '%s\n' "$request" >"$TEST_AXIOM_REQUEST"
jq -c . <<<"$request" >>"$TEST_AXIOM_POSTS"
[[ ${TEST_AXIOM_FAIL:-0} != 1 ]] || exit 22
EOF
cat >"$temp/bin/mailgate" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ $* == validate ]]; then
  profiles='["firesale","siftwire"]'
  [[ ${TEST_MAILGATE_PROFILE_MISSING:-0} != 1 ]] || profiles='["firesale"]'
  jq -cn --argjson profiles "$profiles" \
    '{schema_version:"mailgate/config-validation/v1",provider:"resend",profiles:$profiles}'
  exit 0
fi
[[ $* == 'send --profile siftwire' ]]
[[ ${TEST_NO_MAILGATE:-0} != 1 ]]
[[ -z ${RESEND_API_KEY+x} && -z ${RESENT_API_KEY+x} ]]
request="$(cat)"
jq -e '
	.schema_version == "mailgate/send-request/v1"
	and (.idempotency_key | type == "string" and length > 0)
	and (.subject | type == "string" and length > 0)
	and (.text | type == "string" and length > 0)
	and (.html | type == "string" and length > 0)
	and (keys - ["schema_version", "idempotency_key", "subject", "text", "html"] | length == 0)
' <<<"$request" >/dev/null
printf '%s\n' "$request" >"$TEST_MAILGATE_REQUEST"
if [[ ${TEST_MAILGATE_REJECT_RECEIPT:-0} == 1 ]]; then
  printf '%s\n' '{"schema_version":"mailgate/send-result/v2","submission_state":"rejected"}'
  exit 0
fi
jq -cn \
	'{schema_version:"mailgate/send-result/v2",submission_state:"provider_accepted",delivery_state:"provider_accepted",profile:"siftwire",request_digest:"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",provider:"resend",provider_message_id:"email-1",provider_accepted_at:"2026-08-11T12:00:00Z",replayed:false}'
EOF
chmod +x "$temp/bin/prooflane" "$temp/bin/pi" \
  "$temp/bin/siftwire" "$temp/bin/curl" "$temp/bin/mailgate"

expect_status() {
  local actual expected="$1"
  shift
  set +e
  "$@" >/dev/null 2>&1
  actual=$?
  set -e
  test "$actual" = "$expected"
}

assert_selection_receipt() {
  local run_dir="$1" mode="$2" origin="$3" prooflane_run_id="$4"
  test "$(stat -c %a "$run_dir")" = 700
  test "$(stat -c %a "$run_dir/selection-receipt.json")" = 600
  jq -e --arg automation_run_id "${run_dir##*/}" --arg mode "$mode" --arg origin "$origin" \
    --arg prooflane_run_id "$prooflane_run_id" --slurpfile brief "$run_dir/brief.json" \
    --slurpfile selection "$run_dir/selection.json" '
    .schema_version == "siftwire-selection-receipt/v1"
    and (.observed_at | type == "string" and length > 0)
    and .automation_run_id == $automation_run_id
    and .siftwire_run_id == $brief[0].run_id
    and .prooflane_run_id == $prooflane_run_id
    and .mode == $mode and .selection_origin == $origin
    and .configured_model == (if $origin == "pi" then "openai/fixture-fast" else null end)
    and .candidate_count == (($brief[0].candidates // []) | length)
    and .candidate_slots == $brief[0].candidate_slots
    and .selected_candidate_count == ($selection[0].candidate_indexes | length)
    and .required_count == (($brief[0].must_include // []) | length)
    and .sports_update_count == (($brief[0].sports_updates // []) | length)
    and .empty_selection_with_candidates ==
      (.candidate_count > 0 and .candidate_slots > 0 and .selected_candidate_count == 0)
    and (keys == (["schema_version", "observed_at", "automation_run_id", "siftwire_run_id",
      "prooflane_run_id", "mode", "selection_origin", "configured_model", "candidate_count",
      "candidate_slots", "selected_candidate_count", "required_count", "sports_update_count",
      "empty_selection_with_candidates"] | sort))
  ' "$run_dir/selection-receipt.json" >/dev/null
}

assert_pi_evidence() {
  local run_dir="$1"
  test -s "$run_dir/selector-input.json"
  test -s "$run_dir/pi-events.1.jsonl"
  test -s "$run_dir/pi-sessions/1/fixture.jsonl"
  cmp -- "$prompt_source" "$run_dir/siftwire-render.md"
  jq -e '.type == "session" and .version == 3' "$run_dir/pi-sessions/1/fixture.jsonl" >/dev/null
}

assert_axiom_selection() {
  local run_dir="$1" posts="${2:-$TEST_AXIOM_POSTS}"
  jq -es --slurpfile receipt "$run_dir/selection-receipt.json" '
    [.[][] | select(.["event.name"] == "siftwire.selection")] as $events
    | ($events | length) == 1
      and $events[0]["automation.name"] == "siftwire"
      and $events[0]["service.name"] == "siftwire"
      and $events[0].siftwire == $receipt[0]
  ' "$posts" >/dev/null
}

base_env=(
  HOME="$temp/home" PATH="$temp/bin:$PATH" SIFTWIRE_DATABASE_PATH="$temp/database.sqlite"
  SIFTWIRE_BIN="$temp/bin/siftwire"
  CURL_BIN="$temp/bin/curl"
  MAILGATE_BIN="$temp/bin/mailgate"
  TEST_BRIEF_CAPTURE="$temp/brief-source.json" TEST_PI_INPUT_CAPTURE="$temp/selector-input.json"
)
export TEST_AXIOM_POSTS="$temp/axiom-posts.jsonl"
prooflane_env=(
  PROOFLANE_BIN="$temp/bin/prooflane" PROOFLANE_SIFTWIRE_CONTRACT="$temp/contract.json"
)
axiom_env=(
  PROOFLANE_AXIOM_DATASET=prooflane PROOFLANE_AXIOM_TOKEN_FILE="$temp/axiom-token"
)
brief_run_one_digest="$(printf '%s' brief-run-1 | sha256sum | cut -d' ' -f1)"

mkdir "$temp/no-pi-bin"
for command in bash flock grep jq; do
  ln -s "$(command -v "$command")" "$temp/no-pi-bin/$command"
done
ln -s "$temp/bin/model-role" "$temp/no-pi-bin/model-role"
expect_status 127 env "${base_env[@]}" PATH="$temp/no-pi-bin" \
  SIFTWIRE_STATE_DIR="$temp/state-missing-pi" SIFTWIRE_RUN_ROOT="$temp/run-missing-pi" \
  "$runner"
test ! -e "$temp/state-missing-pi"
test ! -e "$temp/run-missing-pi"

expect_status 1 env "${base_env[@]}" TEST_BRIEF_VERSION='otherapp v9.9.9' \
  SIFTWIRE_STATE_DIR="$temp/state-old-runner" SIFTWIRE_RUN_ROOT="$temp/run-old-runner" \
  "$runner"

expect_status 1 env "${base_env[@]}" TEST_BRIEF_VERSION='siftwire v0.9.0-rc.1' \
  SIFTWIRE_STATE_DIR="$temp/state-prerelease-siftwire" SIFTWIRE_RUN_ROOT="$temp/run-prerelease-siftwire" \
  "$runner"

expect_status 1 env "${base_env[@]}" TEST_MAILGATE_PROFILE_MISSING=1 \
  SIFTWIRE_STATE_DIR="$temp/state-invalid-mailgate" SIFTWIRE_RUN_ROOT="$temp/run-invalid-mailgate" \
  "$runner"

invalid_contracts=(legacy unknown missing_capability string_capabilities missing_current_news)
for stage in validate run_brief prepare_delivery; do
  for invalid in "${invalid_contracts[@]}"; do
    state="$temp/state-contract-$stage-$invalid"
    expected_status=1
    [[ $stage != run_brief ]] || expected_status=70
    expect_status "$expected_status" env "${base_env[@]}" "${prooflane_env[@]}" \
      TEST_INVALID_STAGE="$stage" TEST_INVALID_CONTRACT="$invalid" \
      TEST_PROOFLANE_RUN_COUNT="$state/brief.count" \
      TEST_MAILGATE_REQUEST="$state/mailgate.json" TEST_DELIVERY_REQUEST="$state/delivery.json" \
      SIFTWIRE_STATE_DIR="$state" SIFTWIRE_RUN_ROOT="$state/runs" "$runner"
    test ! -e "$state/mailgate.json"
    test ! -e "$state/delivery.json"
    test -z "$(find "$state/outbox" -name '*.json' -print -quit)"
    case "$stage" in
      validate) test ! -e "$state/brief.count" ;;
      run_brief)
        test -s "$state/run-brief-attempt.json"
        test ! -e "$state/active-job.json"
        ;;
      prepare_delivery)
        jq -e '.candidate_indexes == [0]' "$state/active-job.json" >/dev/null
        ;;
    esac
  done
done

# Empty required items are omitted on the wire; null is also admitted by the host.
for required_items in omitted null; do
  state="$temp/state-required-$required_items"
  env "${base_env[@]}" "${prooflane_env[@]}" TEST_REQUIRED_ITEMS="$required_items" \
    TEST_PI_COUNT="$state/pi.count" TEST_SEALED_FILE="$state/sealed" \
    TEST_MAILGATE_REQUEST="$state/mailgate.json" TEST_DELIVERY_REQUEST="$state/delivery.json" \
    SIFTWIRE_STATE_DIR="$state" SIFTWIRE_RUN_ROOT="$state/runs" "$runner" >/dev/null
  jq -e --arg shape "$required_items" '
    if $shape == "omitted" then has("must_include") | not
    else has("must_include") and .must_include == null end
  ' "$temp/brief-source.json" >/dev/null
  jq -e '.must_include == [] and .candidate_slots == 1' "$temp/selector-input.json" >/dev/null
  test "$(wc -l <"$state/pi.count")" = 1
  jq -e '.text == "TEXT-WITH-SPORTS\n"' "$state/mailgate.json" >/dev/null
  jq -e '.action == "confirm_delivery" and .run_id == "brief-run-1"
    and .delivery_plan_id == "plan-brief-run-1"' "$state/delivery.json" >/dev/null
  test -f "$state/outbox/sent/$brief_run_one_digest.json"
  printf 'no-required selection/delivery: pass (%s, %s)\n' "$TEST_RUNNER_PROTOCOL" "$required_items"
done

# A completed linkless result survives an uncertain child exit without collecting again.
state="$temp/state-linkless"
linkless_env=(
  TEST_LINKLESS=1 TEST_PROOFLANE_RUN_COUNT="$state/brief.count" TEST_PI_COUNT="$state/pi.count"
  TEST_SEALED_FILE="$state/sealed" TEST_MAILGATE_REQUEST="$state/mailgate.json"
  TEST_DELIVERY_REQUEST="$state/delivery.json" SIFTWIRE_STATE_DIR="$state" SIFTWIRE_RUN_ROOT="$state/runs"
)
expect_status 99 env "${base_env[@]}" "${prooflane_env[@]}" "${linkless_env[@]}" TEST_BRIEF_EXIT_FAILURE=1 "$runner"
cp "$state/run-brief-result.json" "$temp/linkless-result.json"
test -s "$state/run-brief-attempt.json"
test ! -e "$state/mailgate.json"
env "${base_env[@]}" "${prooflane_env[@]}" "${linkless_env[@]}" "$runner" >/dev/null
cmp "$temp/linkless-result.json" "$temp/brief-source.json"
jq -e '.must_include[0].url == "" and (.candidates | length == 2 and all(.[]; .url == ""))' \
  "$temp/brief-source.json" >/dev/null
test "$(wc -l <"$state/brief.count")" = 1
test "$(wc -l <"$state/pi.count")" = 1
jq -e '.text == "TEXT-WITH-SPORTS\n" and .html == "<p>HTML-BODY</p>\n"' "$state/mailgate.json" >/dev/null
test -f "$state/outbox/sent/$brief_run_one_digest.json"
test ! -e "$state/run-brief-attempt.json"
printf 'linkless required/candidate recovery and delivery: pass\n'

# Empty string is the only additional URL shape; titles and nonempty URLs stay strict.
for group in must_include candidates; do
  for item in '{"title":"Bad"}' '{"title":"Bad","url":null}' '{"title":"Bad","url":7}' \
    '{"title":"Bad","url":"javascript:alert(1)"}' '{"title":"Bad","url":"https://example.com/a b"}' \
    '{"title":"Bad","url":"https://example.com/\u0001"}' '{"title":"","url":""}' \
    '{"title":7,"url":""}' '{"title":"Bad\nTitle","url":""}'; do
    state="$(mktemp -d "$temp/state-invalid-item.XXXXXX")"
    expect_status 70 env "${base_env[@]}" "${prooflane_env[@]}" \
      TEST_ITEM_GROUP="$group" TEST_INVALID_ITEM="$item" TEST_PI_COUNT="$state/pi.count" \
      TEST_PROOFLANE_RUN_COUNT="$state/brief.count" \
      SIFTWIRE_STATE_DIR="$state" SIFTWIRE_RUN_ROOT="$state/runs" "$runner"
    cp "$state/run-brief-result.json" "$temp/invalid-result.json"
    expect_status 70 env "${base_env[@]}" "${prooflane_env[@]}" \
      TEST_PROOFLANE_RUN_COUNT="$state/brief.count" \
      SIFTWIRE_STATE_DIR="$state" SIFTWIRE_RUN_ROOT="$state/runs" "$runner"
    cmp "$temp/invalid-result.json" "$state/run-brief-result.json"
    test "$(wc -l <"$state/brief.count")" = 1
    test ! -e "$state/pi.count"
    test ! -e "$state/active-job.json"
    test -z "$(find "$state/outbox" -name '*.json' -print -quit)"
  done
done

mkdir -p "$temp/state-old-pending/outbox/pending" "$temp/run-old-pending"
printf '%s\n' '{"schema_version":"foreign-email-outbox/v1","foreign_run_id":"old-run","idempotency_key":"foreign-old"}' \
  >"$temp/state-old-pending/outbox/pending/old.json"
chmod 0600 "$temp/state-old-pending/outbox/pending/old.json"
expect_status 70 env "${base_env[@]}" TEST_NO_MAILGATE=1 \
  SIFTWIRE_STATE_DIR="$temp/state-old-pending" SIFTWIRE_RUN_ROOT="$temp/run-old-pending" \
  "$runner"
test -f "$temp/state-old-pending/outbox/pending/old.json"

mkdir -p "$temp/state-legacy-outbox/outbox/pending" "$temp/run-legacy-outbox"
printf '%s\n' \
  '{"schema_version":"siftwire-email-outbox/v2","siftwire_run_id":"legacy-run","prooflane_run_id":"","delivery_plan_id":"legacy-plan","needs_email":false,"idempotency_key":"siftwire-legacy","request_body":null,"email_id":null,"delivery_recorded_at":"2026-08-11T12:00:00Z"}' \
  >"$temp/state-legacy-outbox/outbox/pending/legacy.json"
chmod 0600 "$temp/state-legacy-outbox/outbox/pending/legacy.json"
cp "$temp/state-legacy-outbox/outbox/pending/legacy.json" "$temp/legacy-outbox.json"
expect_status 70 env "${base_env[@]}" TEST_NO_MAILGATE=1 \
  SIFTWIRE_STATE_DIR="$temp/state-legacy-outbox" SIFTWIRE_RUN_ROOT="$temp/run-legacy-outbox" \
  "$runner"
cmp "$temp/legacy-outbox.json" "$temp/state-legacy-outbox/outbox/pending/legacy.json"
test ! -e "$temp/state-legacy-outbox/outbox/sent/legacy.json"
mv "$temp/state-legacy-outbox/outbox/pending/legacy.json" "$temp/state-legacy-outbox/outbox/pending/.unrecognized"
expect_status 70 env "${base_env[@]}" TEST_NO_MAILGATE=1 \
  SIFTWIRE_STATE_DIR="$temp/state-legacy-outbox" SIFTWIRE_RUN_ROOT="$temp/run-legacy-outbox" \
  "$runner"
cmp "$temp/legacy-outbox.json" "$temp/state-legacy-outbox/outbox/pending/.unrecognized"

mkdir -p "$temp/state-old-active" "$temp/run-old-active"
printf '%s\n' '{"schema_version":"foreign-active-job/v1","prooflane_run_id":"","brief":{}}' \
  >"$temp/state-old-active/active-job.json"
expect_status 70 env "${base_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-old-active" SIFTWIRE_RUN_ROOT="$temp/run-old-active" \
  "$runner"

mkdir -p "$temp/state-old-attempt" "$temp/run-old-attempt"
printf '%s\n' '{"schema_version":"foreign-run-attempt/v1","state":"invoking","attempted_at":"2026-08-11T12:00:00Z"}' \
  >"$temp/state-old-attempt/run-brief-attempt.json"
expect_status 70 env "${base_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-old-attempt" SIFTWIRE_RUN_ROOT="$temp/run-old-attempt" \
  "$runner"

mkdir -p "$temp/state-null-active" "$temp/run-null-active"
printf '%s\n' '{"schema_version":"siftwire-active-job/v2","prooflane_run_id":"","brief":null,"candidate_indexes":null}' \
  >"$temp/state-null-active/active-job.json"
expect_status 70 env "${base_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-null-active" SIFTWIRE_RUN_ROOT="$temp/run-null-active" \
  "$runner"

mkdir -p "$temp/state-invalid-active-brief" "$temp/run-invalid-active-brief"
printf '%s\n' '{"schema_version":"siftwire-active-job/v2","prooflane_run_id":"","brief":{},"candidate_indexes":null}' \
  >"$temp/state-invalid-active-brief/active-job.json"
expect_status 70 env "${base_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-invalid-active-brief" SIFTWIRE_RUN_ROOT="$temp/run-invalid-active-brief" \
  "$runner"

mkdir -p "$temp/state-empty-attempt" "$temp/run-empty-attempt"
empty_attempt_env=(
  SIFTWIRE_STATE_DIR="$temp/state-empty-attempt" SIFTWIRE_RUN_ROOT="$temp/run-empty-attempt"
  TEST_BRIEF_MODE=empty_failure
)
expect_status 127 env "${base_env[@]}" "${prooflane_env[@]}" "${empty_attempt_env[@]}" "$runner"
test -f "$temp/state-empty-attempt/run-brief-result.json" && test ! -s "$temp/state-empty-attempt/run-brief-result.json"
expect_status 70 env "${base_env[@]}" "${prooflane_env[@]}" "${empty_attempt_env[@]}" "$runner"
test -s "$temp/state-empty-attempt/run-brief-attempt.json"
test ! -e "$temp/state-empty-attempt/active-job.json"

mkdir -p "$temp/state-multiple-result" "$temp/run-multiple-result"
expect_status 70 env "${base_env[@]}" "${prooflane_env[@]}" TEST_BRIEF_MODE=multiple \
  SIFTWIRE_STATE_DIR="$temp/state-multiple-result" SIFTWIRE_RUN_ROOT="$temp/run-multiple-result" \
  "$runner"
test ! -e "$temp/state-multiple-result/active-job.json"

cat >"$temp/bin/broken-prooflane" <<'EOF'
#!/usr/bin/env bash
exec definitely-missing-prooflane-runtime "$@"
EOF
chmod +x "$temp/bin/broken-prooflane"
export TEST_DELIVERY_REQUEST="$temp/delivery-broken-prooflane-fallback.json"
env "${base_env[@]}" PROOFLANE_BIN="$temp/bin/broken-prooflane" \
  PROOFLANE_SIFTWIRE_CONTRACT="$temp/contract.json" \
  SIFTWIRE_STATE_DIR="$temp/state-broken-prooflane" SIFTWIRE_RUN_ROOT="$temp/run-broken-prooflane" \
  "$runner" >/dev/null
test ! -e "$temp/state-broken-prooflane/run-brief-attempt.json"
test ! -e "$temp/state-broken-prooflane/active-job.json"
jq -e '.action == "confirm_delivery" and .run_id == "direct-run"' \
  "$temp/delivery-broken-prooflane-fallback.json" >/dev/null

mkdir -p "$temp/state-rejected" "$temp/run-rejected"
export TEST_BRIEF_MODE=rejected
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-rejected" SIFTWIRE_RUN_ROOT="$temp/run-rejected" \
  "$runner"
test ! -e "$temp/state-rejected/run-brief-attempt.json"
test ! -e "$temp/state-rejected/active-job.json"
unset TEST_BRIEF_MODE

mkdir -p "$temp/state-duplicate" "$temp/run-duplicate"
set +e
env "${base_env[@]}" "${prooflane_env[@]}" TEST_BRIEF_MODE=duplicate \
  SIFTWIRE_STATE_DIR="$temp/state-duplicate" SIFTWIRE_RUN_ROOT="$temp/run-duplicate" \
  "$runner" \
  >/dev/null 2>"$temp/duplicate.stderr"
duplicate_status=$?
set -e
test "$duplicate_status" = 70
grep -F 'duplicate candidate URL' "$temp/duplicate.stderr" >/dev/null
if grep -F 'https://example.com/same' "$temp/duplicate.stderr" >/dev/null; then
  printf 'duplicate URL leaked into diagnostics\n' >&2
  exit 1
fi

mkdir -p "$temp/state-invalid-candidates" "$temp/run-invalid-candidates"
set +e
env "${base_env[@]}" "${prooflane_env[@]}" TEST_BRIEF_MODE=invalid_candidates \
  SIFTWIRE_STATE_DIR="$temp/state-invalid-candidates" SIFTWIRE_RUN_ROOT="$temp/run-invalid-candidates" \
  "$runner" \
  >/dev/null 2>"$temp/invalid-candidates.stderr"
invalid_candidates_status=$?
set -e
test "$invalid_candidates_status" = 70
grep -F 'candidates is not an array' "$temp/invalid-candidates.stderr" >/dev/null

export TEST_DELIVERY_REQUEST="$temp/delivery-after-rejection.json"
export TEST_MAILGATE_REQUEST="$temp/mailgate-after-rejection.json"
env "${base_env[@]}" "${prooflane_env[@]}" TEST_PI_MODE=notification-replacement \
  SIFTWIRE_STATE_DIR="$temp/state-rejected" SIFTWIRE_RUN_ROOT="$temp/run-rejected" \
  "$runner" >/dev/null
test -f "$temp/state-rejected/outbox/sent/$brief_run_one_digest.json"

for mode in overflow duplicate negative fractional boolean extra_key multiple \
  assistant_error aborted truncated dangling_tool unsettled restarted stale_turn unfinished_message updated_message exit_failure \
  notification-unopened notification-user notification-unknown notification-ordinary notification-tool notification-restart \
  notification-no-start notification-double-start notification-text-update notification-whitespace notification-content notification-error notification-aborted \
  notification-no-end notification-turn-error notification-no-turn-end \
  notification-custom-no-start notification-custom-no-end notification-turn-missing-message notification-turn-null-message \
  notification-prior-tool notification-prior-tool-update notification-prior-tool-end notification-prior-user-end notification-prior-unknown \
  notification-text-start notification-native-text-delta notification-native-text-end notification-native-toolcall-update; do
  expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" TEST_PI_MODE="$mode" \
    TEST_PI_COUNT="$temp/pi-$mode.count" \
    SIFTWIRE_STATE_DIR="$temp/state-selection-$mode" SIFTWIRE_RUN_ROOT="$temp/run-selection-$mode" "$runner"
  jq -e '.candidate_indexes == null' "$temp/state-selection-$mode/active-job.json" >/dev/null
  test -z "$(find "$temp/state-selection-$mode/outbox" -name '*.json' -print -quit)"
  case "$mode" in
    notification-error | notification-aborted | notification-no-end) expected_attempts=1 ;;
    overflow | duplicate | negative | fractional | boolean | extra_key | multiple | notification-*) expected_attempts=3 ;;
    *) expected_attempts=1 ;;
  esac
  test "$(wc -l <"$temp/pi-$mode.count")" = "$expected_attempts"
done

for telemetry in missing missing_probe malformed incomplete publication_failure stderr_diagnostic; do
  [[ $telemetry != missing_probe ]] || mv "$telemetry_probe" "$temp/telemetry-probe.saved"
  mkdir -p "$temp/state-$telemetry"
  if [[ $telemetry == publication_failure ]]; then
    mkdir "$temp/state-$telemetry/last-pi-telemetry.jsonl"
  fi
  env "${base_env[@]}" "${prooflane_env[@]}" TEST_TELEMETRY="$telemetry" \
    TEST_PI_DIAGNOSTIC="$([[ $telemetry == stderr_diagnostic ]] && printf 1 || printf 0)" \
    SIFTWIRE_STATE_DIR="$temp/state-$telemetry" SIFTWIRE_RUN_ROOT="$temp/run-$telemetry" \
    "$runner" >"$temp/$telemetry.stdout" 2>"$temp/$telemetry.stderr"
  test -f "$temp/state-$telemetry/outbox/sent/$brief_run_one_digest.json"
  test -n "$(find "$temp/run-$telemetry" -name 'pi-events.1.jsonl' -print -quit)"
  [[ $telemetry != missing_probe ]] || mv "$temp/telemetry-probe.saved" "$telemetry_probe"
  if [[ $telemetry == incomplete ]]; then
    if grep -Fq 'Pi telemetry' "$temp/$telemetry.stderr"; then
      printf 'Observer settlement metadata must not control telemetry diagnostics\n' >&2
      exit 1
    fi
  else
    grep -E 'Pi (telemetry|emitted diagnostics)' "$temp/$telemetry.stderr" >/dev/null
  fi
  [[ $telemetry != missing_probe ]] || grep -Fq 'optional Pi telemetry probe is unavailable' "$temp/$telemetry.stderr"
done

# Keep the real fixture child alive after it writes settlement evidence.
mkfifo "$temp/pi-ready" "$temp/pi-release"
setsid env "${base_env[@]}" "${prooflane_env[@]}" \
  TEST_PI_WAIT_FIFO="$temp/pi-release" TEST_PI_READY_FIFO="$temp/pi-ready" \
  SIFTWIRE_STATE_DIR="$temp/state-wait" SIFTWIRE_RUN_ROOT="$temp/run-wait" \
  "$runner" >"$temp/wait.stdout" 2>"$temp/wait.stderr" &
waiting_runner=$!
read -r _ <"$temp/pi-ready"
jq -e '.candidate_indexes == null' "$temp/state-wait/active-job.json" >/dev/null
test -z "$(find "$temp/state-wait/outbox" -name '*.json' -print -quit)"
expect_status 75 env "${base_env[@]}" "${prooflane_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-wait" SIFTWIRE_RUN_ROOT="$temp/run-wait" "$runner"
printf 'release\n' >"$temp/pi-release"
wait "$waiting_runner"
waiting_runner=""
test -f "$temp/state-wait/outbox/sent/$brief_run_one_digest.json"

# An almost-NO_REPLY message still needs Mailgate acceptance before confirmation.
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" \
  TEST_PREPARED_MESSAGE=$'NO_REPLY\n' TEST_MAILGATE_REJECT_RECEIPT=1 \
  TEST_DELIVERY_REQUEST="$temp/unaccepted-delivery.json" TEST_SEALED_FILE="$temp/unaccepted-seal" \
  SIFTWIRE_STATE_DIR="$temp/state-unaccepted" SIFTWIRE_RUN_ROOT="$temp/run-unaccepted" "$runner"
test ! -e "$temp/unaccepted-delivery.json"
jq -e '.needs_email and .email_id == null and .delivery_attempt_count == 0' \
  "$temp/state-unaccepted/outbox/pending/$brief_run_one_digest.json" >/dev/null

mkdir -p "$temp/state-selection-retry" "$temp/run-selection-retry"
export TEST_DELIVERY_REQUEST="$temp/delivery-after-selection-retry.json"
export TEST_MAILGATE_REQUEST="$temp/mailgate-after-selection-retry.json"
env "${base_env[@]}" "${prooflane_env[@]}" TEST_PI_INVALID_ONCE_FILE="$temp/pi-invalid-once" \
  SIFTWIRE_STATE_DIR="$temp/state-selection-retry" SIFTWIRE_RUN_ROOT="$temp/run-selection-retry" \
  "$runner" \
  >/dev/null 2>"$temp/selection-retry.stderr"
grep -F 'invalid candidate selection on attempt 1' "$temp/selection-retry.stderr" >/dev/null

mkdir -p "$temp/state-prepare-resume" "$temp/run-prepare-resume"
export TEST_DELIVERY_REQUEST="$temp/delivery-prepare-resume.json"
export TEST_MAILGATE_REQUEST="$temp/mailgate-prepare-resume.json"
prepare_resume_env=(
  SIFTWIRE_STATE_DIR="$temp/state-prepare-resume" SIFTWIRE_RUN_ROOT="$temp/run-prepare-resume"
  TEST_LINKLESS=1 TEST_PREPARE_FAIL_ONCE_FILE="$temp/prepare-failed-once" TEST_PI_COUNT="$temp/pi-count"
)
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${prepare_resume_env[@]}" \
  "$runner"
jq -e '.schema_version == "siftwire-active-job/v2" and .candidate_indexes == [1]' \
  "$temp/state-prepare-resume/active-job.json" >/dev/null
env "${base_env[@]}" "${prooflane_env[@]}" "${prepare_resume_env[@]}" \
  "$runner" >/dev/null
test "$(wc -l <"$temp/pi-count")" = 1
for receipt in "$temp/run-prepare-resume"/*/selection-receipt.json; do
  origin="$(jq -r .selection_origin "$receipt")"
  assert_selection_receipt "${receipt%/*}" scheduled "$origin" proof-run-1
done
test "$(jq -sr '[.[] | select(.selection_origin == "resumed")] | length' \
  "$temp/run-prepare-resume"/*/selection-receipt.json)" = 1

export TEST_DELIVERY_REQUEST="$temp/delivery.json"
export TEST_MAILGATE_REQUEST="$temp/mailgate.json"
export TEST_AXIOM_REQUEST="$temp/axiom.json"
export TEST_SEALED_FILE="$temp/sealed"
env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" TEST_PI_MODE=notification-preserve \
  SIFTWIRE_STATE_DIR="$temp/state" SIFTWIRE_RUN_ROOT="$temp/run" \
  "$runner" >/dev/null

expected_text=$'TEXT-WITH-SPORTS\n'
expected_html=$'<p>HTML-BODY</p>\n'
jq -e --arg text "$expected_text" --arg html "$expected_html" '
  .schema_version == "mailgate/send-request/v1"
  and (.subject | startswith("SiftWire — "))
  and .text == $text
  and .html == $html
  and ((has("from") or has("to")) | not)
' "$temp/mailgate.json" >/dev/null
jq -e '.action == "confirm_delivery" and .run_id == "brief-run-1" and .delivery_plan_id == "plan-brief-run-1" and (has("message") | not)' "$temp/delivery.json" >/dev/null
jq -e '.[0]["automation.name"] == "siftwire" and .[0].prooflane.run_id == "proof-run-1"' "$temp/axiom.json" >/dev/null
sent_outbox="$temp/state/outbox/sent/$brief_run_one_digest.json"
jq -e '
  .schema_version == "siftwire-email-outbox/v2"
  and .siftwire_run_id == "brief-run-1"
  and .delivery_plan_id == "plan-brief-run-1"
  and (has("message") | not)
  and (.idempotency_key | startswith("siftwire-v2-"))
' "$sent_outbox" >/dev/null

run_dir="$(find "$temp/run" -mindepth 1 -maxdepth 1 -type d -print -quit)"
assert_selection_receipt "$run_dir" scheduled pi proof-run-1
assert_pi_evidence "$run_dir"
assert_axiom_selection "$run_dir"

state="$temp/state-empty-selection"
env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" \
  TEST_PI_MODE=select_none TEST_AXIOM_FAIL=1 TEST_SEALED_FILE="$state/sealed" \
  TEST_AXIOM_REQUEST="$state/axiom.json" TEST_AXIOM_POSTS="$state/axiom-posts.jsonl" \
  SIFTWIRE_STATE_DIR="$state" SIFTWIRE_RUN_ROOT="$state/runs" \
  "$runner" >"$temp/empty-selection.stdout" 2>"$temp/empty-selection.stderr"
empty_run="$(find "$state/runs" -mindepth 1 -maxdepth 1 -type d -print -quit)"
assert_selection_receipt "$empty_run" scheduled pi proof-run-1
assert_pi_evidence "$empty_run"
assert_axiom_selection "$empty_run" "$state/axiom-posts.jsonl"
jq -e '.selected_candidate_count == 0 and .empty_selection_with_candidates' "$empty_run/selection-receipt.json" >/dev/null
grep -F 'no news selected from 2 candidates with 1 slots' "$temp/empty-selection.stderr" >/dev/null
grep -F 'selection receipt recording/export failed' "$temp/empty-selection.stderr" >/dev/null
test -f "$state/outbox/sent/$brief_run_one_digest.json"

cat >"$temp/bin/forbidden-host" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$TEST_FORBIDDEN_HOST_CALLS"
exit 99
EOF
chmod +x "$temp/bin/forbidden-host"
replay_env=(
  "${base_env[@]}" "${axiom_env[@]}" TEST_REPLAY=1 TEST_BRIEF_CAPTURE="$run_dir/brief.json"
  TEST_FORBIDDEN_HOST_CALLS="$temp/forbidden-host-calls" TEST_PI_COUNT="$temp/replay-pi.count"
  SIFTWIRE_STATE_DIR="$temp/replay-live-state" SIFTWIRE_RUN_ROOT="$temp/replay-live-runs"
  PROOFLANE_HOME="$temp/replay-live-prooflane" SIFTWIRE_BIN="$temp/bin/forbidden-host"
  PROOFLANE_BIN="$temp/bin/forbidden-host" MAILGATE_BIN="$temp/bin/forbidden-host" CURL_BIN="$temp/bin/forbidden-host"
)
cp -- "$run_dir/brief.json" "$temp/replay-source.before"
env "${replay_env[@]}" "$runner" --replay "$run_dir/brief.json" "$temp/replay" >"$temp/replay.stdout"
cmp -- "$temp/replay-source.before" "$run_dir/brief.json"
cmp -- "$run_dir/selection.json" "$temp/replay/selection.json"
jq -e '.candidate_indexes == [0] and keys == ["candidate_indexes"]' "$temp/replay.stdout" >/dev/null
assert_selection_receipt "$temp/replay" replay pi ""
assert_pi_evidence "$temp/replay"
test ! -e "$temp/forbidden-host-calls"
test ! -e "$temp/replay-live-state"
test ! -e "$temp/replay-live-runs"
test ! -e "$temp/replay-live-prooflane"
expect_status 1 env "${replay_env[@]}" "$runner" --replay "$run_dir/brief.json" "$temp/replay"
test "$(wc -l <"$temp/replay-pi.count")" = 1
jq '.runner_protocol = "siftwire-runner/v4"' "$run_dir/brief.json" >"$temp/replay-incompatible.json"
expect_status 1 env "${replay_env[@]}" "$runner" --replay "$temp/replay-incompatible.json" "$temp/replay-invalid"
test ! -e "$temp/replay-invalid"

mkdir -p "$temp/state-no-reply" "$temp/run-no-reply"
export TEST_BRIEF_MODE=no_reply TEST_PROOF_RUN_ID=proof-run-2 TEST_NO_MAILGATE=1
export TEST_DELIVERY_REQUEST="$temp/delivery-no-reply.json"
export TEST_SEALED_FILE="$temp/sealed-no-reply"
env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" \
  SIFTWIRE_STATE_DIR="$temp/state-no-reply" SIFTWIRE_RUN_ROOT="$temp/run-no-reply" \
  "$runner" >/dev/null
jq -e '.run_id == "brief-run-2" and .action == "confirm_delivery" and .delivery_plan_id == "plan-brief-run-2"' "$temp/delivery-no-reply.json" >/dev/null
no_candidate_run="$(find "$temp/run-no-reply" -mindepth 1 -maxdepth 1 -type d -print -quit)"
assert_selection_receipt "$no_candidate_run" scheduled no_candidates proof-run-2
jq -e '.candidate_count == 0 and (.empty_selection_with_candidates | not)' "$no_candidate_run/selection-receipt.json" >/dev/null

unset TEST_BRIEF_MODE TEST_NO_MAILGATE
mkdir -p "$temp/state-resume" "$temp/run-resume"
export TEST_PROOF_RUN_ID=proof-run-resume TEST_DELIVERY_REQUEST="$temp/delivery-resume.json"
export TEST_SEALED_FILE="$temp/sealed-resume" TEST_PI_FAIL_ONCE_FILE="$temp/pi-failed-once"
export TEST_PROOFLANE_RUN_COUNT="$temp/prooflane-run-count"
resume_env=(
  SIFTWIRE_STATE_DIR="$temp/state-resume" SIFTWIRE_RUN_ROOT="$temp/run-resume"
)
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" "${resume_env[@]}" \
  "$runner"
jq -e '.schema_version == "siftwire-active-job/v2" and .candidate_indexes == null' "$temp/state-resume/active-job.json" >/dev/null
cp "$temp/selector-input.json" "$temp/selector-before-resume.json"
env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" "${resume_env[@]}" \
  "$runner" >/dev/null
test "$(wc -l <"$temp/prooflane-run-count")" = 1
cmp "$temp/selector-before-resume.json" "$temp/selector-input.json"
test ! -e "$temp/state-resume/active-job.json"

unset TEST_PI_FAIL_ONCE_FILE TEST_PROOFLANE_RUN_COUNT
mkdir -p "$temp/state-sealed" "$temp/run-sealed"
export TEST_PROOF_RUN_ID=proof-run-sealed TEST_DELIVERY_REQUEST="$temp/delivery-sealed.json"
export TEST_SEALED_FILE="$temp/sealed-delivery" TEST_DELIVERY_COUNT="$temp/delivery-count"
export TEST_DELIVERY_CRASH_ONCE_FILE="$temp/delivery-crashed-once"
sealed_env=(
  SIFTWIRE_STATE_DIR="$temp/state-sealed" SIFTWIRE_RUN_ROOT="$temp/run-sealed"
)
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" "${sealed_env[@]}" \
  "$runner"
test -f "$temp/state-sealed/outbox/pending/$brief_run_one_digest.json"
env "${base_env[@]}" "${prooflane_env[@]}" "${axiom_env[@]}" "${sealed_env[@]}" \
  "$runner" >/dev/null
test "$(wc -l <"$temp/delivery-count")" = 1
test -f "$temp/state-sealed/outbox/sent/$brief_run_one_digest.json"

unset TEST_DELIVERY_CRASH_ONCE_FILE TEST_DELIVERY_COUNT TEST_SEALED_FILE
mkdir -p "$temp/state-unverified-recovery" "$temp/run-unverified-recovery"
export TEST_PROOF_RUN_ID=proof-run-unverified TEST_DELIVERY_REQUEST="$temp/delivery-unverified.json"
export TEST_SEALED_FILE="$temp/sealed-unverified" TEST_DELIVERY_COUNT="$temp/delivery-unverified-count"
export TEST_DELIVERY_CRASH_ONCE_FILE="$temp/delivery-unverified-crashed-once" TEST_RECOVERY_ASSERTIONS_FAIL=1
unverified_recovery_env=(
  SIFTWIRE_STATE_DIR="$temp/state-unverified-recovery" SIFTWIRE_RUN_ROOT="$temp/run-unverified-recovery"
)
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${unverified_recovery_env[@]}" \
  "$runner"
env "${base_env[@]}" "${prooflane_env[@]}" "${unverified_recovery_env[@]}" \
  "$runner" >/dev/null
test "$(wc -l <"$temp/delivery-unverified-count")" = 2
test -f "$temp/state-unverified-recovery/outbox/sent/$brief_run_one_digest.json"

unset TEST_DELIVERY_CRASH_ONCE_FILE TEST_DELIVERY_COUNT TEST_SEALED_FILE TEST_RECOVERY_ASSERTIONS_FAIL
mkdir -p "$temp/home/.local/share/siftwire"
touch "$temp/home/.local/share/siftwire/siftwire.sqlite"
export TEST_DELIVERY_REQUEST="$temp/delivery-direct.json"
env HOME="$temp/home" XDG_STATE_HOME="$temp/home/.local/state" XDG_DATA_HOME="$temp/home/.local/share" \
  PATH="$temp/bin:$PATH" SIFTWIRE_BIN="$temp/bin/siftwire" \
  CURL_BIN="$temp/bin/curl" MAILGATE_BIN="$temp/bin/mailgate" TEST_EXPECTED_DATABASE_PATH="$temp/home/.local/share/siftwire/siftwire.sqlite" \
  PROOFLANE_BIN="$temp/bin/missing-prooflane" PROOFLANE_SIFTWIRE_CONTRACT="$temp/missing-contract.json" \
  "$runner" >/dev/null
jq -e '.run_id == "direct-run" and .action == "confirm_delivery" and .delivery_plan_id == "plan-direct-run"' "$temp/delivery-direct.json" >/dev/null
test -f "$temp/home/.local/state/pi-automations/siftwire/outbox/sent/$(printf '%s' direct-run | sha256sum | cut -d' ' -f1).json"

mkdir -p "$temp/state-shadow-fallback" "$temp/run-shadow-fallback"
export TEST_PROOF_RUN_ID=proof-run-fallback TEST_DELIVERY_REQUEST="$temp/delivery-fallback.json"
export TEST_SEALED_FILE="$temp/sealed-fallback" TEST_DELIVERY_FAIL_FILE=1
shadow_fallback_env=(
  SIFTWIRE_STATE_DIR="$temp/state-shadow-fallback" SIFTWIRE_RUN_ROOT="$temp/run-shadow-fallback"
)
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${shadow_fallback_env[@]}" \
  "$runner"
unset TEST_DELIVERY_FAIL_FILE
env "${base_env[@]}" "${prooflane_env[@]}" "${shadow_fallback_env[@]}" \
  PROOFLANE_BIN="$temp/bin/missing-prooflane" \
  "$runner" >/dev/null
fallback_sent="$temp/state-shadow-fallback/outbox/sent/$brief_run_one_digest.json"
jq -e '.delivery_attempt_count == 2 and .delivery_record_source == "runner-result"' "$fallback_sent" >/dev/null

mkdir -p "$temp/state-retry-budget" "$temp/run-retry-budget"
export TEST_PROOF_RUN_ID=proof-run-budget TEST_DELIVERY_REQUEST="$temp/delivery-budget.json"
export TEST_SEALED_FILE="$temp/sealed-budget" TEST_DELIVERY_FAIL_FILE=1 TEST_DELIVERY_COUNT="$temp/delivery-budget-count"
retry_budget_env=(
  SIFTWIRE_STATE_DIR="$temp/state-retry-budget" SIFTWIRE_RUN_ROOT="$temp/run-retry-budget"
)
retry_budget_env+=(TEST_PROOFLANE_RUN_COUNT="$temp/retry-brief-count")
retry_pending="$temp/state-retry-budget/outbox/pending/$brief_run_one_digest.json"
for attempt in 1 2 3; do
  expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${retry_budget_env[@]}" \
    "$runner"
  jq -e --argjson attempt "$attempt" '.delivery_attempt_count == $attempt' "$retry_pending" >/dev/null
  test "$(wc -l <"$temp/retry-brief-count")" = 1
  if [[ $attempt == 1 ]]; then
    cp "$temp/delivery-budget.json" "$temp/retry-request.json"
    jq 'del(.delivery_attempt_count, .last_delivery_attempt_at, .delivery_recorded_at, .delivery_record_source)' \
      "$retry_pending" >"$temp/retry-immutable.json"
    export TEST_MAILGATE_REJECT_RECEIPT=1
  fi
  cmp "$temp/retry-request.json" "$temp/delivery-budget.json"
  jq 'del(.delivery_attempt_count, .last_delivery_attempt_at, .delivery_recorded_at, .delivery_record_source)' \
    "$retry_pending" | cmp "$temp/retry-immutable.json" -
done
unset TEST_DELIVERY_FAIL_FILE
env "${base_env[@]}" "${prooflane_env[@]}" "${retry_budget_env[@]}" "$runner" >/dev/null
retry_sent="$temp/state-retry-budget/outbox/sent/$brief_run_one_digest.json"
jq -e '.delivery_attempt_count == 4 and .delivery_record_source == "runner-result"' "$retry_sent" >/dev/null
cmp "$temp/retry-request.json" "$temp/delivery-budget.json"
jq 'del(.delivery_attempt_count, .last_delivery_attempt_at, .delivery_recorded_at, .delivery_record_source)' \
  "$retry_sent" | cmp "$temp/retry-immutable.json" -
test "$(wc -l <"$temp/delivery-budget-count")" = 4

unset TEST_DELIVERY_COUNT TEST_MAILGATE_REJECT_RECEIPT
mkdir -p "$temp/state-claimed" "$temp/run-claimed"
export TEST_PROOF_RUN_ID=proof-run-claimed TEST_DELIVERY_REQUEST="$temp/delivery-claimed.json"
export TEST_SEALED_FILE="$temp/sealed-claimed" TEST_DELIVERY_COUNT="$temp/delivery-claimed-count"
export TEST_DELIVERY_ALREADY_CLAIMED_ONCE_FILE="$temp/delivery-already-claimed-once"
claimed_env=(
  SIFTWIRE_STATE_DIR="$temp/state-claimed" SIFTWIRE_RUN_ROOT="$temp/run-claimed"
)
expect_status 1 env "${base_env[@]}" "${prooflane_env[@]}" "${claimed_env[@]}" \
  "$runner"
claimed_pending="$temp/state-claimed/outbox/pending/$brief_run_one_digest.json"
jq -e '.delivery_attempt_count == 0 and .last_delivery_attempt_at == null' "$claimed_pending" >/dev/null
env "${base_env[@]}" "${prooflane_env[@]}" "${claimed_env[@]}" \
  "$runner" >/dev/null
test "$(wc -l <"$temp/delivery-claimed-count")" = 1

printf 'siftwire automation test: pass (%s)\n' "$TEST_RUNNER_PROTOCOL"
