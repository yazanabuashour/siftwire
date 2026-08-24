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

const DeletionResultSchema = z.object({
  rejected: z.boolean(),
  summary: z.string(),
})

export function deleteSource(
  key: string,
  signal?: AbortSignal | undefined,
): Promise<{ rejected: boolean; summary: string }> {
  return request(`/sources/${encodeURIComponent(key)}`, {
    method: "DELETE",
    schema: DeletionResultSchema,
    signal,
  })
}

const AckSchema = z.object({
  rejected: z.boolean(),
  summary: z.string().optional().default(""),
})

type Ack = z.infer<typeof AckSchema>

export function setOptions(
  maxDeliveryItems: number,
  signal?: AbortSignal | undefined,
): Promise<Ack> {
  return request("/options", {
    method: "PUT",
    body: { max_delivery_items: maxDeliveryItems },
    schema: AckSchema,
    signal,
  })
}

export function replaceOutlets(
  outlets: OutletPolicy[],
  signal?: AbortSignal | undefined,
): Promise<Ack> {
  return request("/outlets", {
    method: "PUT",
    body: { outlets },
    schema: AckSchema,
    signal,
  })
}

export function listRuns(
  limit: number,
  signal?: AbortSignal | undefined,
): Promise<RunsList> {
  return request(`/runs?limit=${limit}`, {
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
