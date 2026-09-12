import { expect, test } from "bun:test"

import type {
  AgentSession,
  AgentSessionEvent,
} from "@earendil-works/pi-coding-agent"

import { TurnCollector } from "./pi-events"

type Assistant = Extract<
  AgentSession["messages"][number],
  { role: "assistant" }
>

function assistant(overrides: Partial<Assistant> = {}): AgentSessionEvent {
  return {
    type: "message_end",
    message: {
      role: "assistant",
      provider: "eval-test",
      model: "synthetic",
      api: "openai-completions",
      content: [{ type: "text", text: "Done" }],
      stopReason: "stop",
      timestamp: 0,
      usage: {
        input: 0,
        output: 0,
        cacheRead: 0,
        cacheWrite: 0,
        totalTokens: 0,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
      },
      ...overrides,
    },
  }
}
function collector(): TurnCollector {
  const turn = new TurnCollector({ provider: "eval-test", id: "synthetic" })
  turn.observe({ type: "agent_start" })
  return turn
}
function settle(turn: TurnCollector): void {
  turn.observe({ type: "agent_end", messages: [], willRetry: false })
  turn.observe({ type: "agent_settled" })
}

test("native translation counts failed attempts and rejects unsuccessful final output or changed models", () => {
  const retried = collector()
  retried.observe(assistant({ stopReason: "error" }))
  retried.observe({ type: "agent_end", messages: [], willRetry: true })
  retried.observe({ type: "agent_start" })
  retried.observe(assistant())
  settle(retried)
  expect(retried.finish().assistant_calls).toBe(2)
  const invalid: Partial<Assistant>[] = [
    ...(["length", "error", "aborted", "toolUse"] as const).map(
      (stopReason) => ({ stopReason }),
    ),
    { errorMessage: "native execution failed" },
    { content: [{ type: "text", text: " \n" }] },
    { content: [{ type: "thinking", thinking: "No answer" }] },
  ]
  for (const overrides of invalid) {
    const turn = collector()
    turn.observe(assistant(overrides))
    settle(turn)
    expect(() => turn.finish()).toThrow()
  }
  expect(() => collector().observe(assistant({ model: "other" }))).toThrow(
    "provider/model",
  )
  expect(() => collector().observe(assistant({ provider: "other" }))).toThrow(
    "provider/model",
  )
  expect(() => collector().finish()).toThrow("Unsettled")
})

test("native lifecycle and execution hooks reject orphan, duplicate, mismatched and unsettled calls", () => {
  const declaration = assistant({
    content: [{ type: "toolCall", id: "call", name: "other", arguments: {} }],
  })
  const start: AgentSessionEvent = {
    type: "tool_execution_start",
    toolCallId: "call",
    toolName: "other",
    args: {},
  }
  const end: AgentSessionEvent = {
    type: "tool_execution_end",
    toolCallId: "call",
    toolName: "other",
    result: {},
    isError: false,
  }
  expect(() => collector().observe(start)).toThrow("Orphan")
  expect(() => collector().observe(end)).toThrow("Orphan")
  const turn = collector()
  turn.observe(declaration)
  expect(() => turn.finish()).toThrow("Unsettled tool")
  turn.observe(start)
  expect(() => turn.observe(end)).toThrow(
    "Successful tool result without execution receipt",
  )
  turn.recordExecution("call", { kind: "other", name: "other" })
  expect(() => turn.finish()).toThrow("Unsettled tool")
  expect(() => turn.observe(start)).toThrow("duplicate")
  expect(() => turn.observe({ ...end, toolName: "wrong" })).toThrow("Orphan")
  turn.observe(end)
  expect(() => turn.observe(end)).toThrow("duplicate")
  expect(() => turn.observe(declaration)).toThrow("Duplicate")
  settle(turn)
  expect(() => turn.finish()).toThrow("Unexpected terminal content")
  turn.recordExecution("call", { kind: "other", name: "other" })
  expect(() => turn.finish()).toThrow("Orphan or duplicate tool execution")
})
