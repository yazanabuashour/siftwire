import { useQuery } from "@tanstack/react-query"
import { useState } from "react"

import { listRuns } from "../api-client"
import type { RunSummary } from "../api-contracts"
import { RunDetailPanel } from "./RunDetailPanel"
import { RunStatusPill } from "./RunStatusPill"
import {
  Card,
  EmptyState,
  ErrorNote,
  formatWhen,
  PageHeader,
  parseSummary,
  Skeleton,
} from "./shared"

export default function RunsPage() {
  const runs = useQuery({
    queryKey: ["runs"],
    queryFn: ({ signal }) => listRuns(50, signal),
  })
  const [selected, setSelected] = useState<string | null>(null)
  const activeId = selected ?? runs.data?.runs[0]?.run_id ?? null

  return (
    <>
      <PageHeader
        title="Runs"
        subtitle="Each scheduled pass, including selected, delivered, and dropped items."
      />
      {runs.isPending ? (
        <RunsSkeleton />
      ) : runs.isError ? (
        <ErrorNote error={runs.error} />
      ) : runs.data.runs.length === 0 ? (
        <Card>
          <EmptyState
            title="No runs yet"
            hint="Runs appear after the first scheduled brief."
          />
        </Card>
      ) : (
        <div className="grid items-start gap-4 xl:grid-cols-[380px_1fr]">
          <RunHistory
            runs={runs.data.runs}
            activeId={activeId}
            onSelect={setSelected}
          />
          {activeId ? <RunDetailPanel runId={activeId} /> : null}
        </div>
      )}
    </>
  )
}

function RunsSkeleton() {
  return (
    <div className="grid gap-4 xl:grid-cols-[380px_1fr]">
      <div className="space-y-px">
        {[1, 2, 3, 4, 5].map((key) => (
          <Skeleton key={key} className="h-14" />
        ))}
      </div>
      <Skeleton className="h-72" />
    </div>
  )
}

function RunHistory({
  runs,
  activeId,
  onSelect,
}: {
  runs: RunSummary[]
  activeId: string | null
  onSelect: (runId: string) => void
}) {
  return (
    <Card className="console-scroll max-h-80 overflow-y-auto xl:sticky xl:top-7 xl:max-h-[calc(100vh-7rem)]">
      <ul className="divide-edge divide-y">
        {runs.map((run) => (
          <li key={run.run_id}>
            <button
              type="button"
              onClick={() => onSelect(run.run_id)}
              aria-current={run.run_id === activeId ? "true" : undefined}
              className={`relative w-full px-4 py-3 text-left transition-colors ${
                run.run_id === activeId ? "bg-accent-dim" : "hover:bg-raised/60"
              }`}
            >
              {run.run_id === activeId ? (
                <span className="bg-accent absolute top-1/2 left-0 h-8 w-0.5 -translate-y-1/2 rounded-full" />
              ) : null}
              <div className="flex items-center justify-between gap-2">
                <span
                  className={`text-sm font-medium ${
                    run.run_id === activeId ? "text-accent" : "text-ink"
                  }`}
                >
                  {formatWhen(run.started_at)}
                </span>
                <RunStatusPill
                  status={run.status}
                  delivered={run.delivered_at !== null}
                />
              </div>
              <RunCounts summary={run.summary} />
            </button>
          </li>
        ))}
      </ul>
    </Card>
  )
}

function RunCounts({ summary }: { summary: string }) {
  const counts = parseSummary(summary)
  if (!counts)
    return <p className="text-muted mt-0.5 truncate text-xs">{summary}</p>
  return (
    <p className="text-muted mt-0.5 text-xs">
      {counts.mustInclude} must-include · {counts.candidates} candidates
    </p>
  )
}
