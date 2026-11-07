#!/usr/bin/env bash
set -Eeuo pipefail

[[ $# == 2 && -r $1 ]] || {
  printf 'Usage: %s PROMPT NEW_OUTPUT_DIRECTORY\n' "$0" >&2
  exit 2
}
prompt="$(realpath -- "$1")"
root="$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output="$(realpath -m -- "$2")"
[[ ! -e $output ]] || {
  printf 'Output already exists: %s\n' "$output" >&2
  exit 2
}
mkdir -m 0700 -- "$output"

# Fictional inputs only; replay has no collection or delivery host path.
jq -n --arg published "$(date -u +%FT%TZ)" '
  def item($title; $threshold):
    {source_key:"team_news",source_label:"Team Alpha news",kind:"rss",section:"sports",
     threshold:$threshold,title:$title,published_at:$published,url:"https://publisher.test/story"};
  ([range(0;59) | item(
      if . % 2 == 0 then "^~[EN~DIRECT] Team Alpha - Team Beta en direct gratuity - Campus Publisher"
      else "【LIVESTREAMS】LiVe]Team Alpha vs Team Beta Live Free Right Now - Campus Publisher" end; "high")]
   + (["Team Alpha 2-0 Team Gamma match recap", "Team Alpha predicted lineup for Saturday",
       "How to watch Team Alpha live on TV", "Team Alpha linked with star in transfer rumor",
       "Team Alpha power rankings: fifth this week", "Team Alpha player discusses training",
       "Team Alpha vs Team Beta odds and picks", "Team Alpha wins routine league game",
       "Team Alpha confirms minor reserve signing"] | map(item(. + " - Wire"; "medium")))
   + (["Team Alpha head coach dismissed after board decision",
       "Team Alpha stripped of championship after major league sanction",
       "Team Alpha star quarterback confirmed out for season with torn ACL",
       "League sues illegal livestream operators", "Broadcaster wins live match rights"]
      | map(item(. + " - Wire"; "high"))))
  | to_entries | map(.key as $index | .value + {url:("https://publisher.test/story-" + ($index|tostring))})
  | {runner_protocol:"siftwire-runner/v5",capabilities:["prepared-delivery/v1","current-news/v1","sports-updates/v1"],
     rejected:false,run_id:"synthetic-sports-editorial",must_include:[],candidates:.,max_delivery_items:7,
     candidate_slots:7,previous_briefs:[],recent_sent:[],health_footnote:"",
     sports_updates:[
       {source_key:"team_schedule",source_label:"Team Alpha",status:"upcoming",title:"Team Beta vs Team Alpha",
        competition:"Example League",starts_at:"2026-10-17T17:00:00Z",url:"https://scores.test/upcoming"},
       {source_key:"team_schedule",source_label:"Team Alpha",status:"final",title:"Team Alpha 2 - 0 Team Gamma",
        competition:"Example League",starts_at:"2026-10-14T19:00:00Z",url:"https://scores.test/final"}]}
' >"$output/brief.json"
cp -- "$prompt" "$output/selector-prompt.md"
printf '\n%s\n' \
  'This is an offline synthetic editorial-classification evaluation. Candidate headlines and sports records are stipulated fictional facts for this evaluation only. Do not fetch the deliberately non-resolving .test URLs. Apply the normal spam, significance, novelty, and selection rules; source verification is not under test.' \
  >>"$output/selector-prompt.md"
SIFTWIRE_RENDER_PROMPT="$output/selector-prompt.md" \
  bash "$root/crates/siftwire/automation/run-siftwire-automation.sh" \
  --replay "$output/brief.json" "$output/replay" >"$output/selection.json"
jq -e '.candidate_indexes | sort == [68,69,70]' "$output/selection.json" >/dev/null || {
  printf 'FAIL: expected only candidates 68, 69, and 70; inspect %s\n' "$output" >&2
  exit 1
}
printf 'PASS: major sports news selected; 59 stream promotions and 11 routine/insufficient stories skipped. Evidence: %s\n' "$output"
