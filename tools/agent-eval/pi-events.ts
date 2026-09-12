import type {
  AgentSession,
  AgentSessionEvent,
} from "@earendil-works/pi-coding-agent"

import type { Action, TurnResult } from "./contract"

export class LifecycleError extends Error {}

export class TurnCollector {
  private readonly actions: Action[] = []
  private readonly calls = new Map<
    string,
    { name: string; state: "declared" | "started" | "ended"; executed: boolean }
  >()
  private executionError: LifecycleError | undefined
  private assistantCalls = 0
  private lastMessage: AgentSession["messages"][number] | undefined
  private active = false
  private ended = false
  private settled = false

  private readonly model: { provider: string; id: string }

  constructor(model: { provider: string; id: string }) {
    this.model = model
  }

  observe(event: AgentSessionEvent): void {
    switch (event.type) {
      case "agent_start":
        if (this.active || this.settled)
          throw new LifecycleError("Duplicate agent start")
        this.active = true
        break
      case "agent_end":
        if (!this.active) throw new LifecycleError("Orphan agent end")
        this.assertCallsEnded()
        this.active = false
        this.ended = true
        break
      case "agent_settled":
        if (this.active || !this.ended || this.settled)
          throw new LifecycleError("Invalid agent settlement")
        this.settled = true
        break
      case "message_end": {
        if (!this.active) throw new LifecycleError("Message outside agent run")
        this.lastMessage = event.message
        if (event.message.role !== "assistant") break
        this.assistantCalls += 1
        if (
          event.message.provider !== this.model.provider ||
          event.message.model !== this.model.id
        ) {
          throw new LifecycleError("Assistant provider/model changed")
        }
        // Failed attempts may contain partial tool calls that Pi never executes.
        if (
          event.message.stopReason === "error" ||
          event.message.stopReason === "aborted"
        )
          break
        for (const part of event.message.content) {
          if (part.type !== "toolCall") continue
          if (this.calls.has(part.id))
            throw new LifecycleError("Duplicate tool call")
          this.calls.set(part.id, {
            name: part.name,
            state: "declared",
            executed: false,
          })
        }
        break
      }
      case "tool_execution_start": {
        const call = this.calls.get(event.toolCallId)
        if (
          !this.active ||
          call?.name !== event.toolName ||
          call.state !== "declared"
        ) {
          throw new LifecycleError("Orphan or duplicate tool start")
        }
        call.state = "started"
        break
      }
      case "tool_execution_end": {
        const call = this.calls.get(event.toolCallId)
        if (
          !this.active ||
          call?.name !== event.toolName ||
          call.state !== "started"
        ) {
          throw new LifecycleError("Orphan or duplicate tool end")
        }
        if (!event.isError && !call.executed)
          throw new LifecycleError(
            "Successful tool result without execution receipt",
          )
        call.state = "ended"
        break
      }
    }
  }

  // Native starts also describe calls rejected before execution. Only tool
  // execute hooks establish action receipts, including executions that fail.
  recordExecution(id: string, action: Action): void {
    const call = this.calls.get(id)
    const name =
      action.kind === "command"
        ? "bash"
        : action.kind === "read"
          ? "read"
          : action.name
    if (
      !this.active ||
      call?.name !== name ||
      call.state !== "started" ||
      call.executed
    ) {
      // The SDK catches tool exceptions. Retain this failure until finish()
      // rather than allowing the agent to recover and hide a recording fault.
      this.executionError ??= new LifecycleError(
        "Orphan or duplicate tool execution",
      )
      return
    }
    call.executed = true
    this.actions.push(action)
  }

  private assertCallsEnded(): void {
    if ([...this.calls.values()].some((call) => call.state !== "ended")) {
      throw new LifecycleError("Unsettled tool calls")
    }
  }

  finish(): TurnResult {
    if (this.executionError) throw this.executionError
    this.assertCallsEnded()
    if (!this.settled || this.active)
      throw new LifecycleError("Unsettled agent lifecycle")
    const message = this.lastMessage
    if (
      message?.role !== "assistant" ||
      message.stopReason !== "stop" ||
      message.errorMessage !== undefined
    ) {
      throw new LifecycleError("Unsuccessful final assistant response")
    }
    let finalMessage = ""
    for (const part of message.content) {
      if (part.type === "text") finalMessage += part.text
      else if (part.type !== "thinking")
        throw new LifecycleError("Unexpected terminal content")
    }
    if (!finalMessage.trim())
      throw new LifecycleError("Empty final assistant response")
    return {
      final_message: finalMessage,
      assistant_calls: this.assistantCalls,
      actions: this.actions,
    }
  }
}
