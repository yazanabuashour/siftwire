import { skipToken, useQuery } from "@tanstack/react-query"

import { getRun, listRuns } from "../api-client"

export function useRecentRuns() {
  return useQuery({
    queryKey: ["runs"],
    queryFn: ({ signal }) => listRuns(100, signal),
  })
}

export function useRun(runId: string | undefined) {
  return useQuery({
    queryKey: ["run", runId],
    queryFn: runId ? ({ signal }) => getRun(runId, signal) : skipToken,
  })
}
