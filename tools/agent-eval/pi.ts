import * as NodeFS from "node:fs"
import * as NodePath from "node:path"
// oxlint-disable-next-line project/namespace-node-imports -- Bun's namespace facade does not attach listeners to the actual process.
import NodeProcess from "node:process"

import {
  createAgentSession,
  createBashToolDefinition,
  createReadToolDefinition,
  createExtensionRuntime,
  defineTool,
  getAgentDir,
  loadSkills,
  ModelRuntime,
  SessionManager,
  SettingsManager,
  type AgentSession,
  type AgentSessionEvent,
  type ResourceLoader,
} from "@earendil-works/pi-coding-agent"
import { z } from "zod"

import {
  protocol,
  requestSchema,
  responseSchema,
  type Action,
  type EvalRequest,
  type EvalResponse,
} from "./contract"
import { LifecycleError, TurnCollector } from "./pi-events"

class EvalError extends Error {}

export function createResources(
  input: EvalRequest,
  agentDir: string,
): ResourceLoader {
  const skills = loadSkills({
    cwd: input.workspace,
    agentDir,
    skillPaths: [input.skill_path],
    includeDefaults: false,
  })
  if (
    skills.diagnostics.length !== 0 ||
    skills.skills.length !== 1 ||
    skills.skills[0]?.name !== "siftwire" ||
    skills.skills[0]?.filePath !== input.skill_path ||
    skills.skills[0]?.disableModelInvocation
  ) {
    throw new EvalError("Missing or invalid explicit SiftWire skill")
  }
  const extensions = {
    extensions: [],
    errors: [],
    runtime: createExtensionRuntime(),
  }
  return {
    getExtensions: () => extensions,
    getSkills: () => skills,
    getPrompts: () => ({ prompts: [], diagnostics: [] }),
    getThemes: () => ({ themes: [], diagnostics: [] }),
    getAgentsFiles: () => ({ agentsFiles: [] }),
    getSystemPrompt: () => undefined,
    getSystemPromptSource: () => undefined,
    getAppendSystemPrompt: () => [],
    getAppendSystemPromptSources: () => [],
    extendResources: () => {},
    reload: () => Promise.resolve(),
  }
}

type OnExecution = (id: string, action: Action) => void

function createEvalTools(input: EvalRequest, onExecution?: OnExecution) {
  const read = createReadToolDefinition(input.workspace)
  const bash = createBashToolDefinition(input.workspace, {
    shellPath: "/bin/bash",
    exposeSessionEnvironment: false,
    spawnHook: ({ command, cwd }) => ({
      command,
      cwd,
      env: { ...input.tool_env },
    }),
  })
  return [
    defineTool({
      ...read,
      execute: (id, params, signal, onUpdate, ctx) => {
        onExecution?.(id, { kind: "read", path: params.path })
        return read.execute(id, params, signal, onUpdate, ctx)
      },
    }),
    defineTool({
      ...bash,
      execute: (id, params, signal, onUpdate, ctx) => {
        onExecution?.(id, { kind: "command", command: params.command })
        return bash.execute(id, params, signal, onUpdate, ctx)
      },
    }),
  ]
}

export async function createEvalSession(
  input: EvalRequest,
  agentDir: string,
  modelOverride?: string,
  signal = new AbortController().signal,
  onExecution?: OnExecution,
): Promise<AgentSession> {
  let provider: string
  let modelId: string
  if (modelOverride !== undefined) {
    const slash = modelOverride.indexOf("/")
    if (slash <= 0 || slash === modelOverride.length - 1) {
      throw new EvalError("SIFTWIRE_PI_MODEL must be an exact provider/model")
    }
    provider = modelOverride.slice(0, slash)
    modelId = modelOverride.slice(slash + 1)
  } else {
    try {
      // Only model defaults cross this boundary, never personal resource settings.
      const defaults = z
        .object({
          defaultProvider: z.string().min(1),
          defaultModel: z.string().min(1),
        })
        .parse(
          JSON.parse(
            NodeFS.readFileSync(
              NodePath.join(agentDir, "settings.json"),
              "utf8",
            ),
          ),
        )
      provider = defaults.defaultProvider
      modelId = defaults.defaultModel
    } catch {
      throw new EvalError(
        "Pi defaultProvider/defaultModel are missing or invalid",
      )
    }
  }
  signal.throwIfAborted()
  const runtime = await ModelRuntime.create({
    authPath: NodePath.join(agentDir, "auth.json"),
    modelsPath: NodePath.join(agentDir, "models.json"),
    modelsStorePath: NodePath.join(agentDir, "models-store.json"),
    allowModelNetwork: false,
    signal,
  })
  signal.throwIfAborted()
  if (runtime.getError())
    throw new EvalError("Pi model configuration could not be loaded")
  const model = runtime.getModel(provider, modelId)
  if (!model || model.provider !== provider || model.id !== modelId) {
    throw new EvalError("Exact requested provider/model is unavailable")
  }
  const { session } = await createAgentSession({
    cwd: input.workspace,
    agentDir,
    modelRuntime: runtime,
    model,
    thinkingLevel: "medium",
    settingsManager: SettingsManager.inMemory(),
    resourceLoader: createResources(input, agentDir),
    sessionManager: SessionManager.inMemory(input.workspace),
    tools: ["read", "bash"],
    customTools: createEvalTools(input, onExecution),
  })
  if (
    session.thinkingLevel !== "medium" ||
    session.model?.provider !== provider ||
    session.model.id !== modelId
  ) {
    session.dispose()
    throw new EvalError(
      "Pi did not retain the exact model and medium reasoning",
    )
  }
  return session
}

function writeJson(fd: number, value: AgentSessionEvent | EvalResponse): void {
  const line = Buffer.from(`${JSON.stringify(value)}\n`)
  let offset = 0
  while (offset < line.length)
    offset += NodeFS.writeSync(fd, line, offset, line.length - offset)
}

async function runEvaluation(
  input: EvalRequest,
  signal: AbortSignal,
): Promise<EvalResponse> {
  signal.throwIfAborted()
  const log = NodeFS.openSync(
    NodePath.join(input.artifact_dir, "pi-events.jsonl"),
    "wx",
    0o600,
  )
  try {
    let activeCollector: TurnCollector | undefined
    const session = await createEvalSession(
      input,
      getAgentDir(),
      NodeProcess.env["SIFTWIRE_PI_MODEL"],
      signal,
      (id, action) => {
        if (!activeCollector)
          throw new LifecycleError("Tool execution outside a prompt")
        activeCollector.recordExecution(id, action)
      },
    )
    let aborting: Promise<void> | undefined
    const abort = () => {
      aborting ??= session.abort()
    }
    signal.addEventListener("abort", abort, { once: true })
    try {
      const model = session.model
      if (!model) throw new EvalError("Pi session has no model")
      const turns: EvalResponse["turns"] = []
      for (const prompt of input.prompts) {
        signal.throwIfAborted()
        const collector = new TurnCollector(model)
        activeCollector = collector
        let observationError: Error | undefined
        const unsubscribe = session.subscribe((event) => {
          try {
            if (event.type !== "message_update") writeJson(log, event)
            collector.observe(event)
          } catch (error) {
            // Do not interrupt SDK event dispatch; fail after its lifecycle settles.
            observationError ??=
              error instanceof LifecycleError
                ? error
                : new EvalError(
                    "Pi native event recording or lifecycle validation failed",
                  )
          }
        })
        try {
          await session.prompt(prompt, {
            // Cancellation during async preflight must not start a fresh agent run.
            preflightResult: () => signal.throwIfAborted(),
          })
          signal.throwIfAborted()
          if (observationError) throw observationError
          if (
            session.model?.provider !== model.provider ||
            session.model.id !== model.id ||
            session.thinkingLevel !== "medium"
          ) {
            throw new EvalError("Pi model or reasoning changed during scenario")
          }
          turns.push(collector.finish())
        } finally {
          unsubscribe()
          activeCollector = undefined
        }
      }
      return responseSchema.parse({
        protocol,
        runtime: {
          adapter: "pi",
          model: `${model.provider}/${model.id}`,
          reasoning_effort: "medium",
        },
        turns,
      })
    } catch (error) {
      await (aborting ?? session.abort())
      throw error
    } finally {
      signal.removeEventListener("abort", abort)
      session.dispose()
    }
  } finally {
    NodeFS.closeSync(log)
  }
}

async function main(): Promise<number> {
  const controller = new AbortController()
  const abort = () => controller.abort()
  const signals = ["SIGINT", "SIGTERM", "SIGHUP"] satisfies NodeJS.Signals[]
  for (const signal of signals) NodeProcess.on(signal, abort)
  try {
    let input: EvalRequest
    try {
      input = requestSchema.parse(JSON.parse(await Bun.stdin.text()))
    } catch {
      throw new EvalError("Invalid evaluation request")
    }
    const response = await runEvaluation(input, controller.signal)
    controller.signal.throwIfAborted()
    writeJson(1, response)
    return 0
  } catch (error) {
    // SDK/provider errors can include credential command output or response bodies.
    const diagnostic =
      error instanceof EvalError || error instanceof LifecycleError
        ? error.message
        : "Pi evaluation failed; sensitive error details withheld"
    NodeFS.writeSync(2, `${diagnostic}\n`)
    return 1
  } finally {
    for (const signal of signals) NodeProcess.removeListener(signal, abort)
  }
}

if (import.meta.main) NodeProcess.exit(await main())
