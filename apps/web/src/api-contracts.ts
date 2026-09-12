import { z } from "zod"

// The runner stores priority_rank as i64. Only decimal strings cross the
// browser boundary; JavaScript numbers cannot represent the full range.
export const PriorityRankSchema = z.string().refine((value) => {
  const digits = value.startsWith("-") ? value.slice(1) : value
  if (!digits || /[^0-9]/.test(digits)) return false
  const rank = BigInt(value)
  return rank >= -9223372036854775808n && rank <= 9223372036854775807n
}, "Priority must be a whole number from -9223372036854775808 to 9223372036854775807.")

export const ReportingModeSchema = z.enum([
  "required",
  "sports",
  "major",
  "highlights",
])
export type ReportingMode = z.infer<typeof ReportingModeSchema>

export const SourceSchema = z.object({
  key: z.string(),
  label: z.string(),
  kind: z.enum(["rss", "github_release", "sports_schedule"]),
  url: z.string().optional().default(""),
  repo: z.string().optional().default(""),
  section: z.string(),
  threshold: z.enum(["always", "high", "medium"]),
  enabled: z.boolean(),
  url_canonicalization: z
    .enum(["", "none", "google_news_article_url"])
    .optional()
    .default(""),
  outlet_extraction: z
    .enum(["", "none", "title_suffix"])
    .optional()
    .default(""),
  dedup_group: z.string().optional().default(""),
  priority_rank: PriorityRankSchema,
  schedule_format: z
    .enum(["", "espn", "espn_scoreboard", "riot"])
    .optional()
    .default(""),
  schedule_filter: z
    .enum(["all", "standings_top_two"])
    .optional()
    .default("all"),
  api_key: z.string().optional().default(""),
})
export type Source = z.infer<typeof SourceSchema>

export const OutletPolicySchema = z.object({
  name: z.string(),
  aliases: z.array(z.string()).optional().default([]),
  policy: z.enum(["allow", "watch", "block"]),
  note: z.string().optional().default(""),
  enabled: z.boolean(),
})
export type OutletPolicy = z.infer<typeof OutletPolicySchema>

const StringMapSchema = z.record(z.string(), z.string())

export const ConfigResultSchema = z.object({
  runner_protocol: z.literal("siftwire-runner/v5"),
  capabilities: z
    .array(z.string())
    .refine(
      (capabilities) => capabilities.includes("current-news/v1"),
      "The runner must support current-news/v1.",
    ),
  rejected: z.boolean(),
  paths: z.object({ data_dir: z.string(), database_path: z.string() }),
  runtime_config: StringMapSchema.optional().default({}),
  sources: z.array(SourceSchema).optional().default([]),
  outlets: z.array(OutletPolicySchema).optional().default([]),
  source_reporting: z
    .record(z.string(), ReportingModeSchema)
    .optional()
    .default({}),
})
export type ConfigResult = z.infer<typeof ConfigResultSchema>

const RunSummarySchema = z.object({
  run_id: z.string(),
  started_at: z.string(),
  finished_at: z.string().nullable(),
  dry_run: z.boolean(),
  status: z.string(),
  summary: z.string(),
  delivered_at: z.string().nullable(),
  message: z.string().nullable(),
})
export type RunSummary = z.infer<typeof RunSummarySchema>

export const RunItemSchema = z.object({
  id: z.string(),
  source_key: z.string(),
  source_label: z.string(),
  kind: SourceSchema.shape.kind,
  priority_rank: PriorityRankSchema,
  published_at: z.string(),
  outlet: z.string(),
  title: z.string(),
  url: z.string(),
  reporting: ReportingModeSchema,
  delivery_status: z.enum(["sent", "not_selected", "not_delivered", "unknown"]),
})
export type RunItem = z.infer<typeof RunItemSchema>

const DroppedSchema = z.object({
  id: z.string(),
  disposition: z.enum(["retained", "dropped"]),
  source_key: z.string(),
  source_label: z.string(),
  title: z.string(),
  url: z.string(),
  reason: z.string(),
  detail: z.unknown(),
})

const CurrentNewsSchema = z.object({
  since: z.string(),
  until: z.string(),
  eligible_items: z.number(),
  stale_items: z.number(),
  undated_items: z.number(),
  future_items: z.number(),
})

const FetchStatusSchema = z.object({
  source_key: z.string(),
  source_label: z.string(),
  status: z.string(),
  error: z.string().optional().default(""),
  items: z.number(),
  new_items: z.number().nullable(),
  current_news: CurrentNewsSchema.optional(),
})

export const RunsListSchema = z.object({
  runs: z.array(RunSummarySchema),
  next_before: z.string().nullable(),
})
export type RunsList = z.infer<typeof RunsListSchema>

export const RunDetailSchema = z.object({
  run: RunSummarySchema,
  delivery_html: z.string().nullable(),
  must_include: z.array(RunItemSchema),
  candidates: z.array(RunItemSchema),
  dropped: z.array(DroppedSchema),
  annotations: z.array(DroppedSchema),
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
