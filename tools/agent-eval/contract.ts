import * as NodePath from "node:path"

import { z } from "zod"

export const protocol = "siftwire-agent-eval/v1"
const nonblank = z.string().refine((text) => text.trim().length > 0)
const absolutePath = z
  .string()
  .refine(NodePath.isAbsolute)
  .refine((path) => !path.includes("\0"))

export const requestSchema = z.strictObject({
  protocol: z.literal(protocol),
  workspace: absolutePath,
  skill_path: absolutePath,
  artifact_dir: absolutePath,
  prompts: z.array(z.string().min(1)).min(1),
  tool_env: z.record(z.string(), z.string()),
})

export const actionSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("command"), command: nonblank }),
  z.strictObject({ kind: z.literal("read"), path: nonblank }),
  z.strictObject({ kind: z.literal("other"), name: nonblank }),
])

export const responseSchema = z.strictObject({
  protocol: z.literal(protocol),
  runtime: z.strictObject({
    adapter: nonblank,
    model: nonblank.nullable(),
    reasoning_effort: nonblank.nullable(),
  }),
  turns: z
    .array(
      z.strictObject({
        final_message: nonblank,
        assistant_calls: z.number().int().nonnegative().nullable(),
        actions: z.array(actionSchema),
      }),
    )
    .min(1),
})

export type EvalRequest = z.infer<typeof requestSchema>
export type EvalResponse = z.infer<typeof responseSchema>
export type Action = z.infer<typeof actionSchema>
export type TurnResult = EvalResponse["turns"][number]
