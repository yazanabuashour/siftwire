import { z } from "zod"

import {
  ConfigResultSchema,
  RunDetailSchema,
  RunsListSchema,
  type ConfigResult,
  type OutletPolicy,
  type RunDetail,
  type RunsList,
  type Source,
} from "./api-contracts"
import { request } from "./http-client"

export function fetchConfig(
  signal?: AbortSignal | undefined,
): Promise<ConfigResult> {
  return request("/config", {
    method: "GET",
    schema: ConfigResultSchema,
    signal,
  })
}

export function saveSource(
  source: Source,
  signal?: AbortSignal | undefined,
): Promise<ConfigResult> {
  return request("/sources", {
    method: "POST",
    body: source,
    schema: ConfigResultSchema,
    signal,
  })
}

export function deleteSource(
  key: string,
  signal?: AbortSignal | undefined,
): Promise<ConfigResult> {
  return request(`/sources/${encodeURIComponent(key)}`, {
    method: "DELETE",
    schema: ConfigResultSchema,
    signal,
  })
}

const OptionsResultSchema = z.object({
  runtime_config: z.record(z.string(), z.string()),
})
const OutletsResultSchema = ConfigResultSchema.pick({
  outlets: true,
  outlet_conflicts: true,
})

export type BriefOptionsInput = {
  maxDeliveryItems: number
  sportsPreGameDays: number
  sportsPostGameDays: number
  sportsTimezone: string
}

export function setOptions(
  options: BriefOptionsInput,
  signal?: AbortSignal | undefined,
): Promise<z.infer<typeof OptionsResultSchema>> {
  return request("/options", {
    method: "PUT",
    body: {
      max_delivery_items: options.maxDeliveryItems,
      sports_pre_game_days: options.sportsPreGameDays,
      sports_post_game_days: options.sportsPostGameDays,
      sports_timezone: options.sportsTimezone,
    },
    schema: OptionsResultSchema,
    signal,
  })
}

export function replaceOutlets(
  outlets: OutletPolicy[],
  signal?: AbortSignal | undefined,
): Promise<z.infer<typeof OutletsResultSchema>> {
  return request("/outlets", {
    method: "PUT",
    body: { outlets },
    schema: OutletsResultSchema,
    signal,
  })
}

export type RunListOptions = {
  limit?: number
  delivered?: boolean
  before?: string
  search?: string
}

export function listRuns(
  options: RunListOptions = {},
  signal?: AbortSignal | undefined,
): Promise<RunsList> {
  const query = new URLSearchParams()
  if (options.limit !== undefined) query.set("limit", String(options.limit))
  if (options.delivered) query.set("delivered", "true")
  if (options.before) query.set("before", options.before)
  if (options.search) query.set("search", options.search)
  return request(`/runs?${query}`, {
    method: "GET",
    schema: RunsListSchema,
    signal,
  })
}

export function getRun(
  runId: string,
  signal?: AbortSignal | undefined,
): Promise<RunDetail> {
  return request(`/runs/${encodeURIComponent(runId)}`, {
    method: "GET",
    schema: RunDetailSchema,
    signal,
  })
}
