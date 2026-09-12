import * as NodeFS from "node:fs"
import * as NodeOS from "node:os"
import * as NodePath from "node:path"
import * as NodeProcess from "node:process"

import { z } from "zod"

import { protocol, type EvalRequest } from "./contract"

const roots: string[] = []
export const poison = "PERSONAL_RESOURCE_MUST_NOT_LOAD"
export const finalText = "  Done exactly.\n\n"
export const failedCommand =
  'printf observed-failure > "$HOME/failed-effect"; exit 9'

export function cleanupFixtures(): void {
  for (const root of roots.splice(0))
    NodeFS.rmSync(root, { recursive: true, force: true })
}

export function write(path: string, content: string): void {
  NodeFS.mkdirSync(NodePath.dirname(path), { recursive: true })
  NodeFS.writeFileSync(path, content)
}

function writeModel(agentDir: string, baseUrl: string): void {
  write(
    NodePath.join(agentDir, "models.json"),
    JSON.stringify({
      providers: {
        "eval-test": {
          baseUrl,
          api: "openai-completions",
          models: [{ id: "synthetic", reasoning: true }],
        },
      },
    }),
  )
}

export function fixture(baseUrl = "http://127.0.0.1:1") {
  const root = NodeFS.mkdtempSync(
    NodePath.join(NodeOS.tmpdir(), "siftwire-pi-test-"),
  )
  roots.push(root)
  const workspace = NodePath.join(root, "workspace")
  const agentDir = NodePath.join(root, "personal-agent")
  const skillPath = NodePath.join(root, "candidate/SKILL.md")
  const artifactDir = NodePath.join(root, "artifacts")
  NodeFS.mkdirSync(workspace)
  NodeFS.mkdirSync(artifactDir, { mode: 0o700 })
  NodeFS.mkdirSync(NodePath.join(root, "tmp"), { mode: 0o700 })
  write(
    skillPath,
    "---\nname: siftwire\ndescription: Synthetic SiftWire skill for evaluation tests.\n---\nUse the runner.\n",
  )
  write(
    NodePath.join(agentDir, "settings.json"),
    JSON.stringify({ defaultProvider: "eval-test", defaultModel: "synthetic" }),
  )
  write(
    NodePath.join(agentDir, "auth.json"),
    JSON.stringify({
      "eval-test": { type: "api_key", key: "synthetic-test-key" },
    }),
  )
  writeModel(agentDir, baseUrl)
  return {
    agentDir,
    input: {
      protocol,
      workspace,
      skill_path: skillPath,
      artifact_dir: artifactDir,
      prompts: ["Hello"],
      tool_env: {
        PATH: "/usr/bin:/bin",
        HOME: workspace,
        EVAL_TOOL: "allowed",
      },
    } satisfies EvalRequest,
  }
}

export function poisonResources(input: EvalRequest, agentDir: string): void {
  for (const dir of [
    NodePath.dirname(input.workspace),
    input.workspace,
    NodePath.join(input.workspace, ".pi"),
    agentDir,
  ]) {
    for (const name of ["AGENTS.md", "SYSTEM.md", "APPEND_SYSTEM.md"])
      write(NodePath.join(dir, name), poison)
    write(
      NodePath.join(dir, "extensions/poison.ts"),
      `throw new Error('${poison}')`,
    )
    write(
      NodePath.join(dir, "settings.json"),
      JSON.stringify({
        defaultProvider: "eval-test",
        defaultModel: "synthetic",
        packages: ["npm:must-not-resolve-siftwire-test"],
        defaultThinkingLevel: "high",
        shellCommandPrefix: `echo ${poison}`,
      }),
    )
    write(
      NodePath.join(dir, ".agents/skills/poison/SKILL.md"),
      `---\nname: poison\ndescription: ${poison}\n---\n`,
    )
  }
}

export function events(input: EvalRequest) {
  return NodeFS.readFileSync(
    NodePath.join(input.artifact_dir, "pi-events.jsonl"),
    "utf8",
  )
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((line) => z.object({ type: z.string() }).parse(JSON.parse(line)))
}

export function startDriver(
  input: EvalRequest,
  agentDir: string,
  modelOverride?: string,
) {
  const output = NodePath.join(input.artifact_dir, "response.json")
  const fd = NodeFS.openSync(output, "w")
  try {
    const child = Bun.spawn([NodePath.join(import.meta.dir, "pi")], {
      cwd: input.workspace,
      env: {
        ...NodeProcess.env,
        PI_CODING_AGENT_DIR: agentDir,
        SIFTWIRE_PI_MODEL: modelOverride,
        TMPDIR: NodePath.join(NodePath.dirname(input.workspace), "tmp"),
      },
      stdin: new Blob([JSON.stringify(input)]),
      stdout: fd,
      stderr: "pipe",
    })
    return { child, output }
  } finally {
    NodeFS.closeSync(fd)
  }
}

type CompletionDelta = {
  role: string
  content?: string
  tool_calls?: {
    index: number
    id: string
    type: string
    function: { name: string; arguments: string }
  }[]
}
export function completion(
  delta: CompletionDelta = { role: "assistant", content: "Done" },
  reason = "stop",
): Response {
  const chunk = {
    id: "synthetic",
    object: "chat.completion.chunk",
    created: 0,
    model: "synthetic",
    choices: [{ index: 0, delta, finish_reason: reason }],
  }
  return new Response(`data: ${JSON.stringify(chunk)}\n\ndata: [DONE]\n\n`, {
    headers: { "Content-Type": "text/event-stream" },
  })
}

const bodySchema = z.object({
  messages: z.array(
    z.object({
      role: z.string(),
      content: z
        .union([
          z.string(),
          z
            .array(z.object({ type: z.literal("text"), text: z.string() }))
            .transform((parts) => parts.map((part) => part.text).join("")),
        ])
        .nullish(),
    }),
  ),
  reasoning_effort: z.string(),
})
export function multiTurnFixture() {
  const requests: z.infer<typeof bodySchema>[] = []
  const { input, agentDir } = fixture()
  input.prompts.push("Follow up")
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    async fetch(request) {
      requests.push(bodySchema.parse(await request.json()))
      if (requests.length === 1) {
        write(NodePath.join(agentDir, "settings.json"), "{}")
        return completion(
          {
            role: "assistant",
            tool_calls: [
              {
                index: 0,
                id: "truncated",
                type: "function",
                function: {
                  name: "bash",
                  arguments: JSON.stringify({
                    command: 'printf should-not-run > "$HOME/truncated-effect"',
                  }),
                },
              },
            ],
          },
          "length",
        )
      }
      if (requests.length === 2) {
        return completion(
          {
            role: "assistant",
            tool_calls: [
              {
                index: 0,
                id: "read-skill",
                type: "function",
                function: {
                  name: "read",
                  arguments: JSON.stringify({ path: input.skill_path }),
                },
              },
              {
                index: 1,
                id: "command",
                type: "function",
                function: {
                  name: "bash",
                  arguments: JSON.stringify({ command: "printf allowed" }),
                },
              },
              {
                index: 2,
                id: "failed-command",
                type: "function",
                function: {
                  name: "bash",
                  arguments: JSON.stringify({ command: failedCommand }),
                },
              },
              {
                index: 3,
                id: "invalid-arguments",
                type: "function",
                function: {
                  name: "bash",
                  arguments: JSON.stringify({
                    missing_command: "not executed",
                  }),
                },
              },
            ],
          },
          "tool_calls",
        )
      }
      return completion({ role: "assistant", content: finalText })
    },
  })
  writeModel(agentDir, `http://127.0.0.1:${server.port}/v1`)
  return { input, agentDir, server, requests }
}
