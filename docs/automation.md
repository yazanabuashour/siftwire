# Operate the automation

`siftwire automation` runs one scheduled brief end to end: collect, select with
Pi, prepare, send through Mailgate, and confirm. The launcher and selector prompt
ship inside the binary, so they change only with a release. This guide covers
operation and recovery on the host that runs it. The
[runner contract](runner-contract.md) defines the actions the launcher calls.

The host owns the schedule, the service unit, and its environment, the Mailgate
profile, and failure notifications. SiftWire does not install any of them.

## Requirements

The launcher needs Linux with GNU coreutils, `bash`, `jq`, `flock`, `curl`, `pi`,
`model-role`, and `mailgate` on `PATH`, and a Mailgate profile named `siftwire`.
It admits a stable SiftWire release that reports `siftwire-runner/v5` with
`prepared-delivery/v1` and `current-news/v1`. The declared contract decides, not
a version floor. These checks run before any durable attempt starts.

Selection uses the `fast` model role at `medium` thinking. The launcher parses
Pi's JSON event stream and accepts a result only after `agent_settled`. After a Pi
upgrade, check that lifecycle with a [replay](#replay-selection); a version number
does not prove compatibility.

Prooflane is observational. When its runtime is missing or cannot report a
version, the run continues directly through SiftWire without a receipt. Prooflane
verifies local recording; Mailgate's stored receipt owns provider acceptance.

## Find durable state

| Item                         | Location                                                                 |
| ---------------------------- | ------------------------------------------------------------------------ |
| Database                     | `SIFTWIRE_DATABASE_PATH`, or the [default](runner-contract.md#storage-selection) |
| State root                   | `${XDG_STATE_HOME:-$HOME/.local/state}/pi-automations/siftwire`          |
| Runs, outbox, and journals   | `runs/`, `outbox/`, `active-job.json`, and `run-brief-*` in the state root |
| Prooflane ledger             | `prooflane-siftwire/` in the state root                                  |
| Prooflane runtime            | `${XDG_DATA_HOME:-$HOME/.local/share}/pi-automations/siftwire/prooflane/current` |
| Installed prompt, per release | `${XDG_DATA_HOME:-$HOME/.local/share}/siftwire/automation/VERSION/`      |

Keep the database and state root across upgrades. They hold delivery history,
the outbox, recovery records, and both ledgers.

## Override the launcher

Each variable below is optional.

| Variable                                                  | Effect                                                 |
| --------------------------------------------------------- | ------------------------------------------------------ |
| `SIFTWIRE_STATE_DIR`, `SIFTWIRE_RUN_ROOT`                 | Move the state root or the run directories.            |
| `SIFTWIRE_BIN`                                            | Run another binary. The default is the binary started. |
| `SIFTWIRE_RENDER_PROMPT`                                  | Use another selector prompt.                           |
| `SIFTWIRE_PROOFLANE_RUNTIME`, `PROOFLANE_BIN`, `PROOFLANE_SIFTWIRE_CONTRACT`, `PROOFLANE_HOME` | Change Prooflane resolution. |
| `MAILGATE_BIN`, `CURL_BIN`                                | Change the transport commands.                         |
| `PROOFLANE_AXIOM_DATASET`, `PROOFLANE_AXIOM_TOKEN_FILE`   | Export receipts to Axiom. The token file needs mode 600. |

Pi runs from the prompt's directory with `--approve` for that directory only, so
point `SIFTWIRE_RENDER_PROMPT` at a directory you trust. Do not load the full
SiftWire skill into selection; ordinary research skills stay available.

The embedded prompt holds the selection contract and general evidence rules, and
no reader priorities. To add yours, copy
`crates/siftwire/automation/siftwire-render.md`, keep its output contract and
authority rules, and store your copy outside this repository.

The launcher removes Resend and Axiom credentials and the Axiom settings from its
environment before starting SiftWire, Prooflane, or Pi. Mailgate loads the Resend
key in its own process.

## Understand what can retry

- **Collection:** `run_brief` is not retry-safe. `run-brief-attempt.json` marks
  an invocation in progress. If a run ends without a usable result, the next run
  refuses to collect until you reconcile that marker.
- **Selection:** `active-job.json` saves the brief and later the chosen indexes,
  so the next run resumes without collecting. An invalid selection retries up to
  three times in one run. A Pi failure or incomplete lifecycle blocks delivery
  without a retry.
- **Delivery:** a pending outbox record holds the immutable bodies and one
  `siftwire-v2-` idempotency key. Each run finishes pending records first, and
  collection stays blocked until they finish. Accepted email is never resent.
  Attempt counts are diagnostics, not a retry budget.

Unsupported or malformed records stop the run and stay unchanged. The launcher
never resets counters, rewrites keys, or imports old state.

## Recover a stuck delivery

Delivery claims never expire. If a pending delivery reports
`adapter context is already claimed`, confirm that no run is active and reconcile
its delivery result. Then release the claim:

```bash
state_root="${XDG_STATE_HOME:-$HOME/.local/state}/pi-automations/siftwire"
PROOFLANE_HOME="$state_root/prooflane-siftwire" \
  "${XDG_DATA_HOME:-$HOME/.local/share}/pi-automations/siftwire/prooflane/current/bin/prooflane" \
  dogfood siftwire recover-delivery --run PROOFLANE_RUN_ID
```

The next run retries with the same `delivery_plan_id`. If you confirm by hand, use
the same database and plan.

## Inspect a run

Scheduled runs keep a private directory under `runs/`, on success and failure,
with no automatic cleanup. The launcher prints the path as `retained evidence`.

| File                                                | Contents                                     |
| --------------------------------------------------- | -------------------------------------------- |
| `brief.json`, `siftwire-render.md`                  | Full normalized brief and the prompt used    |
| `selector-input.json`                               | Editorial projection given to Pi             |
| `selection.json`, `selection-receipt.json`          | Chosen indexes and the sanitized receipt     |
| `pi-events.N.jsonl`, `pi.N.stderr`, `pi-sessions/N` | Raw evidence for each selection attempt      |
| `pi-telemetry.N.jsonl`                              | Optional Pi telemetry                        |
| `prepared.json`, `delivery-KEY.json`, `delivery-KEY.stderr` | Delivery plan and confirmation       |

Raw evidence can hold sensitive content. Keep it local and do not commit it.

The projection keeps candidate order and membership, and it adds each candidate's
`candidate_index`. It drops identifier aliases, fetch statistics, paths, and
transport bodies. The host binds selected indexes to URLs in the full brief.

Pi diagnostics and missing telemetry warn without blocking delivery. Telemetry
uses the optional probe that the launcher loads from the Pi package, and
`last-pi-telemetry.jsonl` is diagnostic, not a success receipt.

`selection-receipt.json` also goes to Axiom as `siftwire.selection` on scheduled
runs. It holds counts and identifiers: no titles, URLs, bodies, indexes, or
paths. Recording or export errors warn without blocking delivery.
`empty_selection_with_candidates` warns too; there is no minimum story floor.

## Replay selection

Replay runs selection against a saved brief without collecting or delivering:

```bash
siftwire automation --replay BRIEF_JSON NEW_OUTPUT_DIRECTORY
```

Use a run's `brief.json`, not its projection. The output directory must not
exist. Only the selection JSON goes to stdout. To replay unreleased launcher or
prompt changes, use a checkout build such as `target/release/siftwire`.

Replay never invokes the SiftWire runner, Prooflane, Mailgate, or Axiom, and it
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

   If a check fails, finish or reconcile that state first.

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

## Deploy Prooflane

`scripts/deploy-prooflane.sh` builds a standalone Prooflane release and atomically
activates it for this automation. It builds `HEAD` of a clean checkout at
`PROOFLANE_SOURCE_REPO`, by default `$HOME/repo/prooflane`, and it needs Docker.
Set `DOCKER_HOST` when the daemon is rootless.

```bash
SIFTWIRE_PROOFLANE_RUNTIME_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/pi-automations/siftwire/prooflane" \
  scripts/deploy-prooflane.sh
```

The script does not read `XDG_DATA_HOME` itself, so this root keeps it aligned with
the launcher. With a custom `SIFTWIRE_PROOFLANE_RUNTIME`, deploy to its parent.

The script takes the automation's run lock, so it waits for a running brief. An
unchanged commit produces the same release identifier. To roll back, point
`current` at an earlier directory in `releases/`.

## Stop the automation

Stop future runs, then remove the host's unit. Keep the database and state root.
Do not roll back with unresolved active, attempt, or pending state.

```bash
systemctl --user disable --now siftwire.timer
systemctl --user stop siftwire.service
```

For emergency reconciliation by SiftWire run ID, set `PROOFLANE_HOME` to the
preserved `prooflane-siftwire` ledger.

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
