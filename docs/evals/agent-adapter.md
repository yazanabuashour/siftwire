# Executable agent-eval adapter

SiftWire owns the evaluation request and result, not an agent runtime. An
explicitly selected local executable runs one complete fixed scenario. Runtime,
authentication, model selection, native events, and session infrastructure stay
inside leaf adapters. This development boundary does not change the installed
runner or `skills/siftwire/SKILL.md`.

See the [surface decision](../architecture/agentops-surface-policy.md#the-evaluator-owns-a-vendor-neutral-executable-boundary)
for alternatives and the [production evaluation guide](agent-production.md) for
scenarios, invocation, verification, and promotion evidence.

## Invoke an adapter

From the repository root:

```bash
mise exec -- ./scripts/run-agent-eval.sh run \
	--adapter ./tools/agent-eval/pi --scenario routine-agent-hygiene
```

`--adapter` is required. It accepts an absolute executable path, a relative path
such as `./tools/agent-eval/pi`, or a bare executable name resolved through the
host `PATH`. The evaluator launches that executable directly with no arguments;
it does not interpolate a shell command. Use a wrapper or environment variables
for adapter-specific configuration. Rust has no Pi configuration, Bun, model
flags, native event types, or native session knowledge. An adapter can use any
runtime that implements this contract.

## Exchange one scenario

The evaluator sends one JSON request on stdin, closes stdin, and expects one
JSON result on stdout. Diagnostics belong on stderr. A nonzero exit fails the
scenario, even if stdout contains a result. Extra JSON, native event streams,
unknown fields, malformed values, a different protocol version, or a result
turn count different from the prompt count fail validation.

One request contains the entire fixed scenario, not one turn. The adapter runs
prompts in order and owns continuity internally. This removes a needless
session protocol: the evaluator does not need session IDs, resume operations,
or native session files to run its existing scenarios. Do not add those fields,
a harness registry, or a universal model-only interface without a real caller.

### Request

| Field | Contract |
| --- | --- |
| `protocol` | Exactly `siftwire-agent-eval/v1`. |
| `workspace` | Absolute path to the scenario workspace. |
| `skill_path` | Absolute path to the candidate copied to `.agents/skills/siftwire/SKILL.md` inside that workspace. |
| `artifact_dir` | Absolute path to a private directory for adapter artifacts. |
| `prompts` | Nonempty array of nonempty strings, in scenario order. Rust appends `RUNNER_ONLY_INSTRUCTION` to every prompt. |
| `tool_env` | String-to-string map containing the complete environment for shell tools. |

Runtime paths are absolute; documentation and committed receipts use neutral
placeholders such as `<run-root>` instead of machine-specific paths.

Rust builds the fixture runner once per evaluation run and owns fixture paths,
scenario prompts, the candidate skill copy, requests, reports, temporary
resources, databases, and run-root locking. It launches the adapter with the
supplied workspace as its working directory. Each scenario has separate state.

The adapter host retains provider environment variables, but its `TMPDIR` is
run-local. The adapter must use the supplied workspace and candidate resources
and pass **only `tool_env`** to shell tools, without merging the host environment.
The map contains the run-local runner followed by the system executable search
path, scenario-local `HOME`, `TMPDIR`, and `SIFTWIRE_DATABASE_PATH`, plus
`SIFTWIRE_EVAL_ALLOW_FILE_URLS=1` and `LANG=C.UTF-8`. Provider credentials and
personal configuration paths do not belong in that map.

This is configuration separation, **not an OS sandbox**. A same-account adapter
and its tools retain the caller's filesystem permissions. Adapters are trusted
to truthfully report all actions. Hygiene checks inspect receipts after
execution; they cannot prove that a dishonest adapter omitted nothing.

### Result

All fields below are required; nullable fields use explicit JSON `null` when
unavailable. Objects reject unknown fields.

| Field | Contract |
| --- | --- |
| `protocol` | Exactly `siftwire-agent-eval/v1`. |
| `runtime` | Object with `adapter`, `model`, and `reasoning_effort`. |
| `runtime.adapter` | Nonblank opaque adapter ID. |
| `runtime.model` | Nullable nonblank opaque model ID. |
| `runtime.reasoning_effort` | Nullable nonblank opaque reasoning label. |
| `turns` | Array with exactly the same length and order as `prompts`. |
| `turns[].final_message` | Nonblank final assistant string, preserved exactly, not trimmed or reconstructed. |
| `turns[].assistant_calls` | Nullable nonnegative integer counting actual assistant executions, not streaming deltas. Unknown is `null`, not zero. |
| `turns[].actions` | Complete array of actual action receipts for that turn. |

Each action has exactly one of these shapes; payload strings are nonblank:

```json
{"kind":"command","command":"siftwire config"}
{"kind":"read","path":".agents/skills/siftwire/SKILL.md"}
{"kind":"other","name":"unsupported-tool"}
```

Record every executed command, read, and other tool action, including failed
executions. Do not count proposed calls as executions or silently discard
unsupported actions. Rust counts each action once as a tool execution and each
`command` once as a command execution. It applies the existing command hygiene
gate, permits direct reads only of the candidate skill, and rejects `other`
actions as unexpected tools. Assistant counts aggregate across turns; an
unknown count makes the aggregate unknown. Zero is valid for a deterministic
adapter that makes no assistant calls, not a substitute for missing evidence.

Only emit a complete result after actual native completion of every turn and
adapter lifecycle cleanup. The adapter must validate its native terminal answer,
model evidence where available, and completed tool lifecycle before translating
receipts. A final-only API response cannot claim agent-eval equivalence: an API
adapter must own any necessary tool loop and produce the same complete action
receipts. Rust validates normalized output and owns database, hygiene, and exact
immutable-delivery verification; it does not parse native events.

`tools/agent-eval/contract.ts` owns strict zod decoding for TypeScript adapters.
`crates/siftwire/src/bin/siftwire-agent-eval/types.rs` defines the Rust result
types; `output.rs` applies protocol, count, value, and hygiene checks. Those
implementations own decoding, rather than a vendor SDK schema.

## Use the Pi implementation

Install the locked development dependencies with
`mise exec -- bun install --frozen-lockfile`. The executable
`tools/agent-eval/pi` owns Bun invocation. `tools/agent-eval/pi*.ts` owns Pi SDK
integration and native event parsing; the SDK is pinned in root `package.json`
and `bun.lock`. It is one implementation, not the evaluator's consumer boundary.

The adapter resolves Pi's personal agent directory itself. It reads only
`defaultProvider` and `defaultModel` from `settings.json`, unless
`SIFTWIRE_PI_MODEL` supplies an exact `provider/model` override:

```bash
SIFTWIRE_PI_MODEL='<provider>/<model-id>' mise exec -- \
	./scripts/run-agent-eval.sh run --adapter ./tools/agent-eval/pi \
	--scenario routine-agent-hygiene
```

Missing or malformed defaults without an override fail; there is no fallback.
Selection resolves once per scenario, not globally in Rust. Reasoning is fixed
to `medium`; the adapter rejects a runtime that cannot retain the exact model
and reasoning level. One in-memory session is shared across that scenario's
turns. There is no persisted-session or resume contract.

Native `ModelRuntime` uses the same personal `auth.json`, `models.json`, and
`models-store.json` paths and host provider environment. Credentials are not
copied or symlinked: OAuth refresh keeps native locks on the same credential
file. Model catalog network refresh is disabled; inference still uses the
network. Evaluation settings stay in memory. Explicit resources expose only the
candidate skill, not personal instructions, extensions, skills, prompt templates,
themes, packages, or other settings. Tools are native `read` and
clean-environment bash.

Pi awaits each prompt and verifies authoritative final assistant text,
`stopReason: "stop"`, matching provider/model evidence, and no unfinished tools.
`agent_end` alone is insufficient. Assistant message ends count executions;
action receipts originate in the native tool definitions' `execute` hooks.
Native tool starts alone are not execution evidence: the SDK also emits them
for truncated or invalid calls that it refuses to execute. Such refusals retain
lifecycle validation but produce no action receipt; actual executions that fail
still count. Safe, adapter-owned lifecycle diagnostics reach stderr; arbitrary
SDK/provider errors remain withheld. Raw native events stay under `artifact_dir`
in mode `0600` files, never on protocol stdout. Private artifacts and credentials
must not be committed.

## Smoke-test transport without a model

The deterministic `tools/agent-eval/stub` executable uses the same contract
without importing Pi. It reads the candidate skill and executes a real runner
`inspect_config` request for each supported prompt, returning real read and
command receipts. Its model and reasoning are `null`; `assistant_calls` is zero.
Its Bun implementation is private to the adapter, not a Rust dependency.

```bash
mise exec -- ./scripts/run-agent-eval.sh run \
	--adapter ./tools/agent-eval/stub --scenario routine-agent-hygiene
```

Use this only as a transport smoke test for `routine-agent-hygiene`. It is not a
full-inventory capability evaluation, model substitute, or production adapter.

## Evidence status

The [production evaluation guide](agent-production.md) records current receipts
and separate safety, capability, and user-experience judgments. The
[first Pi partial receipt](../agent-eval-results/siftwire-v0.9.0-pi-sdk-candidate.md)
remains historical evidence of the earlier Pi-specific implementation and its
provider usage-limit interruption. Record new evidence under new report names;
never rewrite history into an adapter receipt. No backward compatibility is
required for the unpublished previous evaluation format.
