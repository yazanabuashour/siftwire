import * as NodeChildProcess from "node:child_process"
import * as NodeFS from "node:fs"

import {
  protocol,
  requestSchema,
  responseSchema,
  type TurnResult,
} from "./contract"

const request = requestSchema.parse(JSON.parse(await Bun.stdin.text()))
if (
  request.prompts.some(
    (prompt) =>
      !prompt.startsWith("Run a normal SiftWire configuration inspection"),
  )
) {
  throw new Error(
    "The deterministic stub only supports configuration inspection",
  )
}
const turns: TurnResult[] = []
for (const _prompt of request.prompts) {
  NodeFS.readFileSync(request.skill_path, "utf8")
  const execution = NodeChildProcess.spawnSync("siftwire", ["config"], {
    input: '{"action":"inspect_config"}\n',
    encoding: "utf8",
    cwd: request.workspace,
    env: request.tool_env,
  })
  if (execution.error) throw execution.error
  if (execution.status !== 0)
    throw new Error(`Stub runner failed: ${execution.stderr}`)
  turns.push({
    final_message: execution.stdout,
    assistant_calls: 0,
    actions: [
      { kind: "read", path: request.skill_path },
      { kind: "command", command: "siftwire config" },
    ],
  })
}
await Bun.stdout.write(
  JSON.stringify(
    responseSchema.parse({
      protocol,
      runtime: {
        adapter: "deterministic-inspection-stub",
        model: null,
        reasoning_effort: null,
      },
      turns,
    }),
  ) + "\n",
)
