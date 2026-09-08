import { queryOptions, useQuery, type QueryClient } from "@tanstack/react-query"

import { fetchConfig } from "../api-client"
import type { ConfigResult } from "../api-contracts"

export const configQuery = queryOptions({
  queryKey: ["config"],
  queryFn: ({ signal }) => fetchConfig(signal),
})

export function useConfig() {
  return useQuery(configQuery)
}

// Reads can start during a write, after onMutate cancelled earlier reads.
export async function applyConfigResult(
  client: QueryClient,
  merge: (current: ConfigResult) => ConfigResult,
): Promise<void> {
  await client.cancelQueries(configQuery)
  client.setQueryData(configQuery.queryKey, (current) =>
    current ? merge(current) : current,
  )
}

// Source upserts return only the normalized source. Other configuration
// collections in that response do not belong to this mutation.
export function mergeSourceResult(
  current: ConfigResult,
  result: ConfigResult,
  operation: "upsert" | "delete",
): ConfigResult {
  if (operation === "delete")
    return {
      ...current,
      sources: result.sources,
      source_reporting: result.source_reporting,
    }
  const sources = new Map(current.sources.map((source) => [source.key, source]))
  const reporting = { ...current.source_reporting }
  for (const source of result.sources) {
    sources.set(source.key, source)
    // Never retain an old explanation when the runner omitted the new one.
    delete reporting[source.key]
  }
  return {
    ...current,
    sources: [...sources.values()],
    source_reporting: { ...reporting, ...result.source_reporting },
  }
}

export function mergeOptionsResult(
  current: ConfigResult,
  result: Pick<ConfigResult, "runtime_config">,
): ConfigResult {
  return {
    ...current,
    runtime_config: { ...current.runtime_config, ...result.runtime_config },
  }
}

export function mergeOutletsResult(
  current: ConfigResult,
  result: Pick<ConfigResult, "outlets" | "outlet_conflicts">,
): ConfigResult {
  return {
    ...current,
    outlets: result.outlets,
    outlet_conflicts: result.outlet_conflicts,
  }
}
