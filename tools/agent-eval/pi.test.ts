import { afterEach, expect, test } from "bun:test"
import * as NodeFS from "node:fs"
import * as NodePath from "node:path"
import * as NodeProcess from "node:process"

import { DEFAULT_MAX_BYTES } from "@earendil-works/pi-coding-agent"
import { z } from "zod"

import { responseSchema } from "./contract"
import {
  cleanupFixtures,
  completion,
  events,
  failedCommand,
  finalText,
  fixture,
  multiTurnFixture,
  poison,
  poisonResources,
  startDriver,
  write,
} from "./pi-fixtures"
import { createEvalSession, createResources } from "./pi.js"

afterEach(cleanupFixtures)

test("only the explicit native skill loads; bash replaces inherited environment and spills privately", async () => {
  const { input, agentDir } = fixture()
  poisonResources(input, agentDir)
  const session = await createEvalSession(input, agentDir)
  const parentEnv = { ...NodeProcess.env }
  const privateTmp = NodePath.join(NodePath.dirname(input.workspace), "tmp")
  const poisonScript = NodePath.join(privateTmp, "poison.sh")
  write(poisonScript, `echo ${poison}\n`)
  Object.assign(NodeProcess.env, {
    TMPDIR: privateTmp,
    EVAL_PARENT_SECRET: "synthetic-parent-secret",
    BASH_ENV: poisonScript,
  })
  try {
    const prompt = session.agent.state.systemPrompt
    expect(prompt).toContain("You are an expert coding assistant")
    expect(prompt).toContain("<name>siftwire</name>")
    expect(prompt).toContain(input.skill_path)
    expect(prompt).not.toContain(poison)
    expect(prompt).not.toContain("inspect PI_*")
    expect(session.thinkingLevel).toBe("medium")
    expect(session.getActiveToolNames()).toEqual(["read", "bash"])
    expect(session.sessionFile).toBeUndefined()
    const bash = session.agent.state.tools.find((tool) => tool.name === "bash")
    if (!bash) throw new Error("missing bash override")
    const result = await bash.execute("test", { command: "/usr/bin/env" })
    const text = result.content
      .flatMap((part) => (part.type === "text" ? [part.text] : []))
      .join("\n")
    expect(text).toContain("EVAL_TOOL=allowed")
    expect(text).toContain(`HOME=${input.workspace}`)
    for (const absent of [
      "EVAL_PARENT_SECRET",
      "BASH_ENV",
      "PI_SESSION",
      "PI_MODEL",
      poison,
    ])
      expect(text).not.toContain(absent)
    const spill = await bash.execute("spill", {
      command: `head -c ${DEFAULT_MAX_BYTES + 1} /dev/zero`,
    })
    const details = z
      .object({
        fullOutputPath: z.string(),
        truncation: z.object({ truncated: z.literal(true) }),
      })
      .parse(spill.details)
    expect(NodePath.dirname(details.fullOutputPath)).toBe(privateTmp)
    expect(NodeFS.statSync(details.fullOutputPath).size).toBe(
      DEFAULT_MAX_BYTES + 1,
    )
  } finally {
    for (const key of ["TMPDIR", "EVAL_PARENT_SECRET", "BASH_ENV"]) {
      if (parentEnv[key] === undefined) delete NodeProcess.env[key]
      else NodeProcess.env[key] = parentEnv[key]
    }
    session.dispose()
  }
  expect(NodeFS.existsSync(NodePath.join(agentDir, "sessions"))).toBe(false)
})

test("invalid skills and non-exact models fail instead of discovering replacements", async () => {
  const { input, agentDir } = fixture()
  for (const override of [
    "eval-test/synthe*",
    "missing/synthetic",
    "eval-test/synthetic:high",
    "synthetic",
  ]) {
    await expect(createEvalSession(input, agentDir, override)).rejects.toThrow()
  }
  write(NodePath.join(agentDir, "settings.json"), "{}")
  await expect(createEvalSession(input, agentDir)).rejects.toThrow(
    "defaultProvider/defaultModel",
  )
  const session = await createEvalSession(
    input,
    agentDir,
    "eval-test/synthetic",
  )
  session.dispose()
  NodeFS.unlinkSync(input.skill_path)
  expect(() => createResources(input, agentDir)).toThrow("Missing or invalid")
  write(input.skill_path, "---\nname: siftwire\n---\nMissing description")
  expect(() => createResources(input, agentDir)).toThrow("Missing or invalid")
})

test.each(["complete", "abort", "error", "later-error"])(
  "response only after complete native scenario lifecycle: %s",
  async (outcome) => {
    const received = Promise.withResolvers<void>()
    const finish = Promise.withResolvers<void>()
    let requests = 0
    const server = Bun.serve({
      port: 0,
      hostname: "127.0.0.1",
      async fetch() {
        requests += 1
        received.resolve()
        await finish.promise
        return outcome === "error" ||
          (outcome === "later-error" && requests === 2)
          ? new Response("Synthetic rejection", { status: 400 })
          : completion()
      },
    })
    const { input, agentDir } = fixture(`http://127.0.0.1:${server.port}/v1`)
    if (outcome === "later-error") input.prompts.push("Follow up")
    const { child, output } = startDriver(input, agentDir)
    try {
      await received.promise
      expect(NodeFS.readFileSync(output, "utf8")).toBe("")
      if (outcome === "abort") NodeProcess.kill(child.pid, "SIGTERM")
      else finish.resolve()
      expect(await child.exited).toBe(outcome === "complete" ? 0 : 1)
      const emitted = events(input)
      expect(emitted.some((event) => event.type === "message_update")).toBe(
        false,
      )
      expect(emitted.at(-1)?.type).toBe("agent_settled")
      expect(
        NodeFS.statSync(NodePath.join(input.artifact_dir, "pi-events.jsonl"))
          .mode & 0o777,
      ).toBe(0o600)
      if (outcome === "complete") {
        expect(
          responseSchema.parse(JSON.parse(NodeFS.readFileSync(output, "utf8"))),
        ).toEqual({
          protocol: input.protocol,
          runtime: {
            adapter: "pi",
            model: "eval-test/synthetic",
            reasoning_effort: "medium",
          },
          turns: [{ final_message: "Done", assistant_calls: 1, actions: [] }],
        })
        expect(await new Response(child.stderr).text()).toBe("")
      } else {
        expect(NodeFS.readFileSync(output, "utf8")).toBe("")
        const diagnostic = await new Response(child.stderr).text()
        expect(diagnostic).not.toBe("")
        expect(diagnostic).not.toContain("Synthetic rejection")
        if (outcome === "error" || outcome === "later-error")
          expect(diagnostic).toBe("Unsuccessful final assistant response\n")
      }
    } finally {
      finish.resolve()
      child.kill()
      await child.exited
      await server.stop(true)
    }
  },
)

test("one in-memory session retains history, ordered actions, exact text and the initially resolved model", async () => {
  const { input, agentDir, server, requests } = multiTurnFixture()
  const { child, output } = startDriver(input, agentDir)
  try {
    expect(await child.exited).toBe(0)
    const response = responseSchema.parse(
      JSON.parse(NodeFS.readFileSync(output, "utf8")),
    )
    expect(response.turns).toEqual([
      {
        final_message: finalText,
        assistant_calls: 3,
        actions: [
          { kind: "read", path: input.skill_path },
          { kind: "command", command: "printf allowed" },
          { kind: "command", command: failedCommand },
        ],
      },
      { final_message: finalText, assistant_calls: 1, actions: [] },
    ])
    expect(requests).toHaveLength(4)
    expect(
      NodeFS.existsSync(NodePath.join(input.workspace, "truncated-effect")),
    ).toBe(false)
    expect(
      NodeFS.readFileSync(
        NodePath.join(input.workspace, "failed-effect"),
        "utf8",
      ),
    ).toBe("observed-failure")
    expect(
      events(input).filter((event) => event.type === "tool_execution_start"),
    ).toHaveLength(5)
    expect(
      requests.every((request) => request.reasoning_effort === "medium"),
    ).toBe(true)
    const messages = requests.at(-1)?.messages ?? []
    expect(
      messages
        .filter((message) => message.role === "user")
        .map((message) => message.content),
    ).toEqual(input.prompts)
    expect(
      messages.filter((message) => message.role === "assistant"),
    ).toHaveLength(3)
    expect(
      messages.some(
        (message) => message.role === "tool" && message.content === "allowed",
      ),
    ).toBe(true)
    expect(messages.some((message) => message.content === finalText)).toBe(true)
    expect(NodeFS.existsSync(NodePath.join(agentDir, "sessions"))).toBe(false)
  } finally {
    child.kill()
    await child.exited
    await server.stop(true)
  }
})
