# Operate the automation

`siftwire automation` runs one scheduled brief end to end: collect, select with
Pi, prepare, send through Mailgate, and confirm. The launcher and selector prompt
ship inside the binary, so they change only with a release. This guide covers
operation and recovery on the host that runs it. The
[runner contract](runner-contract.md) defines the actions the launcher calls.

The host owns the schedule, the service unit, and its environment, the Mailgate
profile, and failure notifications. SiftWire does not install any of them.

## Requirements

The launcher needs Linux with GNU coreutils, `bash`, `jq`, `flock`, `pi`, and
`mailgate` on `PATH`, and a Mailgate profile named `siftwire`.
It admits a stable SiftWire release that reports `siftwire-runner/v6` with
`prepared-delivery/v1` and `current-news/v1`. The declared contract decides, not
a version floor. These checks run before any durable attempt starts.

The launcher runs `pi -p --approve "<prompt>"` with stock defaults. The plain-text
prompt names the instruction, input, and per-attempt output paths. Selection
requires a successful child exit and a result file that passes SiftWire's
selection validator; stdout and stderr are human diagnostics only. Use a
[replay](#replay-selection) to check selection after a harness upgrade.

## Find durable state

| Item                         | Location                                                                 |
| ---------------------------- | ------------------------------------------------------------------------ |
| Database                     | `SIFTWIRE_DATABASE_PATH`, or the [default](runner-contract.md#storage-selection) |
| State root                   | `${XDG_STATE_HOME:-$HOME/.local/state}/pi-automations/siftwire`          |
| Runs, outbox, and journals   | `runs/`, `outbox/`, `active-job.json`, and `run-brief-*` in the state root |
| Installed prompt, per release | `${XDG_DATA_HOME:-$HOME/.local/share}/siftwire/automation/VERSION/`      |

Keep the database and state root across upgrades. They hold delivery history,
the outbox, and recovery records.

## Override the launcher

Each variable below is optional.

| Variable                                                  | Effect                                                 |
| --------------------------------------------------------- | ------------------------------------------------------ |
| `SIFTWIRE_STATE_DIR`, `SIFTWIRE_RUN_ROOT`                 | Move the state root or the run directories.            |
| `SIFTWIRE_BIN`                                            | Run another binary. The default is the binary started. |
| `SIFTWIRE_RENDER_PROMPT`                                  | Use another selector prompt.                           |
| `MAILGATE_BIN`                                            | Change the email transport command.                    |
| `SIFTWIRE_NOTES_*`                                        | [Publish to Notes](#publish-to-notes).                 |

Pi runs from the prompt's directory with `--approve` for that directory only, so
point `SIFTWIRE_RENDER_PROMPT` at a directory you trust. Do not load the full
SiftWire skill into selection; ordinary research skills stay available.

The embedded prompt holds the selection contract and general evidence rules, and
no reader priorities. To add yours, copy
`crates/siftwire/automation/siftwire-render.md`, keep its output contract and
authority rules, and set `SIFTWIRE_RENDER_PROMPT` to your copy.

The launcher removes Resend credentials before starting SiftWire or Pi.
Mailgate loads the Resend key in its own process.

## Understand what can retry

- **Collection:** `run_brief` is not retry-safe. `run-brief-attempt.json` marks
  an invocation in progress. If a run ends without a usable result, the next run
  refuses to collect until you reconcile that marker.
- **Selection:** No candidates or zero optional capacity skips Pi and records an
  empty selection with its reason. `active-job.json` saves the brief and later the chosen indexes,
  so the next run resumes without collecting. An invalid selection retries up to
  three times in one run, retaining every raw attempt file. A nonzero child exit
  blocks delivery without a retry, even if it wrote a valid selection.
- **Delivery:** a pending outbox record holds the immutable bodies and one
  `siftwire-v2-` idempotency key. Each run finishes pending records first, and
  collection stays blocked until they finish. Accepted email is never resent.
  Attempt counts are diagnostics, not a retry budget.

Unsupported or malformed records stop the run and stay unchanged. The launcher
never resets counters, rewrites keys, or imports old state.

## Publish to Notes

SiftWire can copy each finished brief to a Notes vault. Email stays the delivery
path: publication reads the same saved plan and runs after email has settled, so
a Notes failure cannot block, delay, or change email or its confirmation.

### Enable publication

Publication is off until the service sets these variables. Setting any of them
turns it on, and a missing or invalid value then fails visibly.

| Variable                    | Value                                                                    |
| --------------------------- | ------------------------------------------------------------------------ |
| `SIFTWIRE_NOTES_API_URL`    | Notes API origin. Plain HTTP is accepted only for a loopback address.    |
| `SIFTWIRE_NOTES_PUBLIC_URL` | Origin that readers use, as in `https://notes.example.test`.             |
| `SIFTWIRE_NOTES_TOKEN_FILE` | A regular file with mode 600 that holds one Notes bearer token.          |
| `SIFTWIRE_NOTES_START_DATE` | First reporting date to publish, as `YYYY-MM-DD`. Older briefs are never sent. |

The launcher removes these variables before it starts other processes and passes
the token file's path to `siftwire notes publish` on stdin. The token stays out
of arguments, the environment, logs, and run evidence.

To turn publication off, remove the variables. Published content stays.

### Find an edition

Dates and slots use `America/Chicago` and the brief's start time, so a late
delivery keeps its original edition. A start before noon is `am`; any later start
is `pm`. The email's Morning and Evening labels use the same cutoff in the sports
time zone.

| Item                | Value                                         |
| ------------------- | --------------------------------------------- |
| Page path           | `html/brief-YYYY-MM-DD-am/index.html`, or `pm` |
| Page URL            | `PUBLIC_URL/pages/brief-YYYY-MM-DD-am/`       |
| Later run, same slot | `brief-YYYY-MM-DD-am-r2`, then `-r3`         |
| Index note          | `technology/daily-briefs.md`                  |

The page is the email's HTML, byte for byte. A new path never replaces a published
page. To correct an edition, run a new brief in the same slot.

The index note holds links for the last 14 dates and one JSON manifest. Each slot
links its highest revision. Notes rejects a section over 400 words, so the link
list stays short while the manifest lists every edition and revision. The link
list has no `latest` entry, which keeps an old edition from reading as a current
one.

| Manifest field          | Meaning                                                     |
| ----------------------- | ----------------------------------------------------------- |
| `schemaVersion`         | Version `1`. SiftWire refuses to rewrite any other version. |
| `timezone`              | `America/Chicago`.                                          |
| `updatedAt`             | When SiftWire last wrote the note. Open slots are as of this time. |
| `editions[]`            | `date`, `slot`, `revision`, `editionId`, `producerRunId`, `generatedAt`, `researchAsOf`, `htmlPath`, `url`, and `htmlSha256`. |
| `editions[].generatedAt` | When `run_brief` finished.                                 |
| `editions[].researchAsOf` | When `run_brief` started. It is the collection cutoff, not a claim that a person or model checked anything at that time. |
| `slots[]`               | Every slot without an edition, from the first edition to the current date: `date`, `slot`, and `status`. |

`status` is `not-ready` for a slot of the current date that has not ended, and
`missing` for a slot that ended without an edition. Noon ends `am` and midnight
ends `pm`. A date after the last manifest write is absent, so treat an absent
date as not ready. An empty brief has no HTML and no edition, so its slot reads
`missing` once a later edition updates the note.

### Check publication

Publication and delivery have separate receipts, and neither implies the other.

- **Email:** the outbox record holds Mailgate's receipt.
- **Publication:** the manifest entry is the durable record. Each run also keeps
  `publication.json`, with the Notes commit, and `publication.stderr` in its run
  directory.

The report's `pending` field lists runs that are not published, and `error` gives
the first failure. An unreadable index stops the run before any write, so the
report is empty and the cause is in `publication.stderr`.

### Recover from a failed publish

When publication stays incomplete, the launcher logs `Notes publication is
incomplete` and exits 69, unless delivery already set its own failure status. Exit
69 means that email finished, so do not resend. The next run retries. To map this
status to its own notification mode, match exit status 69.

The manifest decides what is already published, so retries are safe. Each page and
its index entry land in one Notes commit. If a response is lost after that commit,
the retry finds the run in the manifest and writes nothing. To retry now, send the
same four values as JSON:

```bash
printf '%s\n' '{"api_url":"API_URL","public_url":"PUBLIC_URL","token_file":"TOKEN_FILE","start_date":"START_DATE"}' |
  siftwire notes publish
```

If `technology/daily-briefs.md` loses its manifest block, publication stops rather
than overwrite the note. Restore the block from Notes history, then retry.

## Inspect a run

Scheduled runs keep a private directory under `runs/`, on success and failure,
with no automatic cleanup. The launcher prints the path as `retained evidence`.

| File                                                | Contents                                     |
| --------------------------------------------------- | -------------------------------------------- |
| `brief.json`, `siftwire-render.md`                  | Full normalized brief and the prompt used    |
| `selector-input.json`                               | Editorial projection given to the selector             |
| `selection.json`, `selection-receipt.json`          | Chosen indexes and the sanitized receipt     |
| `selection.N.json`, `selector.N.log`, `selector.N.stderr` | Raw evidence for each selection attempt      |
| `prepared.json`, `delivery-KEY.json`, `delivery-KEY.stderr` | Delivery plan and confirmation       |
| `publication.json`, `publication.stderr`            | Notes report, when publication is on         |

The projection keeps candidate order and membership, and it adds each candidate's
`candidate_index`. It drops identifier aliases, fetch statistics, paths, and
transport bodies. The host binds selected indexes to URLs in the full brief.

Optional items share the allowance across all delivery sections. Required items
and sports remain outside it. Prepared bodies separate each populated category;
empty categories do not add headings.

Selector stderr diagnostics warn without blocking delivery. `selection-receipt.json` stays
local and holds counts and identifiers, not story content. Recording errors and
`empty_selection_with_candidates` warn too; there is no minimum story floor.

## Replay selection

Replay runs selection against a saved brief without collecting or delivering:

```bash
siftwire automation --replay BRIEF_JSON NEW_OUTPUT_DIRECTORY
```

Use a run's `brief.json`, not its projection. The output directory must not
exist. Only the selection JSON goes to stdout. To replay unreleased launcher or
prompt changes, use a checkout build such as `target/release/siftwire`.

Replay never invokes the SiftWire runner or Mailgate, and it
creates no state. It uses the current prompt, model, and Pi tools, so it cannot
recreate a past judgment. Pi tools stay available: replay avoids delivery, but it
is not a sandbox.

## Upgrade

Install the runner and skill from one release, and update process consumers with
them.

1. Stop the timer and the service, then prove that no durable work remains:

   ```bash
   set -euo pipefail
   systemctl --user stop siftwire.timer siftwire.service

   state_root="${XDG_STATE_HOME:-$HOME/.local/state}/pi-automations/siftwire"
   test ! -e "$state_root/active-job.json" && test ! -L "$state_root/active-job.json"
   test ! -e "$state_root/run-brief-attempt.json" && test ! -L "$state_root/run-brief-attempt.json"
   pending="$state_root/outbox/pending"
   test ! -L "$pending"
   if test -e "$pending"; then
     test -d "$pending"
     test -z "$(find "$pending" -mindepth 1 -maxdepth 1 -print -quit)"
   fi
   ```

   If a check fails, finish or reconcile that state first. Before replacing the
   runner, preserve a consistent database backup and the installed runner and
   skill. Schema-changing upgrades require that backup for rollback; an older
   runner cannot open a newer schema.

2. Verify and install the runner at the path the unit uses:

   ```bash
   tag="$(gh release view --repo yazanabuashour/siftwire --json tagName --jq .tagName)"
   tmp="$(mktemp -d)"
   trap 'rm -rf "$tmp"' EXIT
   gh release download "$tag" --repo yazanabuashour/siftwire \
     --pattern install.sh --dir "$tmp"
   gh attestation verify "$tmp/install.sh" --repo yazanabuashour/siftwire
   SIFTWIRE_VERSION="$tag" SIFTWIRE_INSTALL_DIR="$HOME/.local/bin" \
     sh "$tmp/install.sh"
   test "$("$HOME/.local/bin/siftwire" --version)" = "siftwire $tag"
   ```

3. Install the skill outside global discovery, in the same shell:

   ```bash
   asset="siftwire_${tag#v}_skill.tar.gz"
   gh release download "$tag" --repo yazanabuashour/siftwire \
     --pattern "$asset" --dir "$tmp"
   gh attestation verify "$tmp/$asset" --repo yazanabuashour/siftwire
   test "$(tar -tzf "$tmp/$asset")" = SKILL.md
   tar -xzf "$tmp/$asset" -C "$tmp" SKILL.md
   install -Dm644 "$tmp/SKILL.md" \
     "$HOME/.local/share/siftwire/skills/siftwire/SKILL.md"
   ```

   Load it explicitly for full runner tasks with `pi --skill PATH`. The selector
   does not use it.

4. Start one run, then re-enable the schedule:

   ```bash
   systemctl --user start siftwire.service
   journalctl --user -u siftwire.service -n 100 --no-pager
   systemctl --user enable --now siftwire.timer
   ```

A release that embeds the launcher replaces the previous launcher and embedded
prompt with step 2. A prompt set through `SIFTWIRE_RENDER_PROMPT` stays in place. An upgrade that crosses `siftwire-runner` needs matching consumers; a
fresh database loses delivery history and latest-seen markers.

### Move an older host onto the embedded launcher

Hosts that still start a separately installed launcher cross one boundary: the old
launcher runs new binaries, but `siftwire automation` cannot start old ones.

1. Install the new binary. Do not change the unit. A timer firing now runs the old
   launcher against the new binary.
2. Set `ExecStart=%h/.local/bin/siftwire automation`. To keep the host's prompt,
   also set `SIFTWIRE_RENDER_PROMPT` to it. Reload systemd and start one run. A pending delivery or saved selection from the old launcher finishes
   here, without a resend, because both launchers share the state root, lock, and
   record schemas.
3. After a clean scheduled run, remove the old launcher. Remove its prompt file
   too, unless `SIFTWIRE_RENDER_PROMPT` uses it.

To roll back before step 3, restore the old `ExecStart` first, then the old binary
if needed. The reverse order fails with exit 2 on `unknown siftwire command`
until the unit changes. After step 3, restore the old files before restoring the
unit; the old launcher refuses to run without its prompt.

## Stop the automation

Stop future runs, then remove the host's unit. Keep the database and state root.
Do not roll back with unresolved active, attempt, or pending state.

```bash
systemctl --user disable --now siftwire.timer
systemctl --user stop siftwire.service
```

## Evaluate sports selection

Structured schedules own fixtures, results, and scores, so keep schedule sources
enabled. If your prompt limits optional sports news to major developments, run
the offline selection evaluation after you change it:

```bash
scripts/eval-sports-selection.sh PROMPT NEW_OUTPUT_DIRECTORY
```

It offers 59 stream promotions, nine routine stories, three major developments,
and two items without demonstrated impact, and it passes only when Pi selects the
three. The embedded prompt does not set that limit, so use your own. The evaluation
uses the real model, so it costs money and its judgment varies. It appends a
fixture-only instruction to a private copy of the prompt, which means it tests
editorial classification, not source verification. It never collects, prepares,
confirms, or sends.
