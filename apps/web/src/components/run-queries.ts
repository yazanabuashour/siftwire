import { skipToken, useInfiniteQuery, useQuery } from "@tanstack/react-query"

import { getRun, listRuns, type RunListOptions } from "../api-client"

export function useRuns(delivered: boolean, search = "", enabled = true) {
  return useInfiniteQuery({
    queryKey: ["runs", { delivered, search }],
    initialPageParam: null,
    queryFn: ({
      signal,
      pageParam,
    }: {
      signal: AbortSignal
      pageParam: string | null
    }) => {
      const options: RunListOptions = { delivered, search }
      if (pageParam !== null) options.before = pageParam
      return listRuns(options, signal)
    },
    getNextPageParam: (lastPage) => lastPage.next_before ?? undefined,
    enabled,
    // A closed picker must release its request, not just stop future fetches.
    // TanStack keeps shared latest-record requests alive for remaining observers.
    subscribed: enabled,
  })
}

export function useRun(runId: string | undefined) {
  return useQuery({
    queryKey: ["run", runId],
    queryFn:
      runId !== undefined ? ({ signal }) => getRun(runId, signal) : skipToken,
  })
}

export function useSelectedRun(runId: string | undefined, delivered: boolean) {
  // An explicit URL is independent of archive availability, filtering and paging.
  const archive = useRuns(delivered, "", runId === undefined)
  const activeId = runId ?? archive.data?.pages[0]?.runs[0]?.run_id
  const detail = useRun(activeId)
  return { archive, activeId, detail }
}
