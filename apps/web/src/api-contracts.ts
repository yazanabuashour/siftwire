import { z } from "zod"

export const SourceSchema = z.object({
  key: z.string(),
  label: z.string(),
  kind: z.string(),
  url: z.string().optional().default(""),
  repo: z.string().optional().default(""),
  section: z.string(),
  threshold: z.string(),
  enabled: z.boolean(),
  url_canonicalization: z.string().optional().default(""),
  outlet_extraction: z.string().optional().default(""),
  dedup_group: z.string().optional().default(""),
  priority_rank: z.number().optional().default(0),
  always_report: z.boolean().optional().default(false),
  schedule_format: z.string().optional().default(""),
  api_key: z.string().optional().default(""),
})
export type Source = z.infer<typeof SourceSchema>

export const OutletPolicySchema = z.object({
  name: z.string(),
  aliases: z.array(z.string()).optional().default([]),
  policy: z.string(),
  note: z.string().optional().default(""),
  enabled: z.boolean(),
})
export type OutletPolicy = z.infer<typeof OutletPolicySchema>

const StringMapSchema = z.record(z.string(), z.string())

export const ConfigResultSchema = z.object({
  rejected: z.boolean(),
  paths: z.object({ data_dir: z.string(), database_path: z.string() }),
  runtime_config: StringMapSchema.optional().default({}),
  sources: z.array(SourceSchema).optional().default([]),
  outlets: z.array(OutletPolicySchema).optional().default([]),
})
export type ConfigResult = z.infer<typeof ConfigResultSchema>

const RunSummarySchema = z.object({
  run_id: z.string(),
  started_at: z.string(),
  finished_at: z.string().nullable().optional().default(null),
  dry_run: z.boolean(),
  status: z.string(),
  summary: z.string(),
  delivered_at: z.string().nullable().optional().default(null),
  message: z.string().nullable().optional().default(null),
})
export type RunSummary = z.infer<typeof RunSummarySchema>

const ItemSchema = z.object({
  source_key: z.string(),
  source_label: z.string(),
  kind: z.string().optional().default(""),
  section: z.string().optional().default(""),
  threshold: z.string().optional().default(""),
  priority_rank: z.number().optional().default(0),
  always_report: z.boolean().optional().default(false),
  published_at: z.string().optional().default(""),
  outlet: z.string().optional().default(""),
  title: z.string(),
  url: z.string(),
  selected: z.boolean(),
})
export type RunItem = z.infer<typeof ItemSchema>

const DroppedSchema = z.object({
  source_key: z.string(),
  title: z.string(),
  url: z.string(),
  reason: z.string(),
  detail: z.unknown(),
})

const FetchStatusSchema = z.object({
  source_key: z.string(),
  status: z.string(),
  error: z.string().optional().default(""),
  items: z.number().optional().default(0),
  new_items: z.number().optional().default(0),
})

export const RunsListSchema = z.object({
  runs: z.array(RunSummarySchema),
})
export type RunsList = z.infer<typeof RunsListSchema>

export const RunDetailSchema = z.object({
  run: RunSummarySchema,
  must_include: z.array(ItemSchema),
  candidates: z.array(ItemSchema),
  dropped: z.array(DroppedSchema),
  fetch: z.array(FetchStatusSchema),
  sent_items: z.array(
    z.object({
      title: z.string(),
      url: z.string(),
      sent_at: z.string(),
    }),
  ),
})
export type RunDetail = z.infer<typeof RunDetailSchema>
