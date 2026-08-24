import { useQuery } from "@tanstack/react-query"
import { useState } from "react"

import { getRun } from "../api-client"
import type { RunDetail } from "../api-contracts"
import { IconChevron } from "./icons"
import { RunSelectionList } from "./RunSelectionList"
import { RunStatusPill } from "./RunStatusPill"
import {
  Card,
  EmptyState,
  ErrorNote,
  formatWhen,
  parseSummary,
  Skeleton,
} from "./shared"

type DetailTab = "candidates" | "dropped" | "fetch"
const TABS: DetailTab[] = ["candidates", "dropped", "fetch"]

export function RunDetailPanel({ runId }: { runId: string }) {
  const detail = useQuery({
    queryKey: ["run", runId],
    queryFn: ({ signal }) => getRun(runId, signal),
  })
  if (detail.isPending) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-24" />
        <Skeleton className="h-64" />
      </div>
    )
  }
  if (detail.isError) return <ErrorNote error={detail.error} />
  return <DetailBody data={detail.data} />
}

function DetailBody({ data }: { data: RunDetail }) {
  const [tab, setTab] = useState<DetailTab>("candidates")
  const summary = parseSummary(data.run.summary)
  const candidateCount = summary?.candidates ?? data.candidates.length
  const mustIncludeCount = summary?.mustInclude ?? data.must_include.length
  const hasSelectionEvidence =
    data.must_include.length > 0 ||
    data.candidates.length > 0 ||
    data.dropped.length > 0
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <Card className="px-4 py-3.5">
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2.5">
            <span className="text-ink truncate font-mono text-sm">
              {data.run.run_id}
            </span>
            <RunStatusPill
              status={data.run.status}
              delivered={data.run.delivered_at !== null}
            />
          </div>
          <span className="text-muted shrink-0 text-xs">
            {formatWhen(data.run.started_at)}
          </span>
        </div>
      </Card>
      <div className="grid grid-cols-3 gap-3 sm:gap-4">
        <BigCount
          label="Delivered"
          value={data.sent_items.length}
          tone="text-accent"
        />
        <BigCount label="Candidates" value={candidateCount} />
        <BigCount
          label="Dropped"
          value={hasSelectionEvidence ? data.dropped.length : "N/A"}
          tone="text-muted"
        />
      </div>
      <Card>
        <nav
          className="border-edge flex gap-1 border-b px-3 pt-3"
          aria-label="Run details"
        >
          {TABS.map((name) => (
            <button
              key={name}
              type="button"
              onClick={() => setTab(name)}
              aria-current={tab === name ? "page" : undefined}
              className={`rounded-t-lg px-3 py-2 text-sm capitalize transition-colors ${
                tab === name
                  ? "border-edge bg-raised text-accent border border-b-transparent"
                  : "text-muted hover:text-ink"
              }`}
            >
              {name}
            </button>
          ))}
        </nav>
        <div className="console-scroll max-h-[calc(100vh-21rem)] overflow-y-auto p-2">
          {tab === "candidates" ? (
            <RunSelectionList
              mustInclude={data.must_include}
              candidates={data.candidates}
              expectedMustInclude={mustIncludeCount}
              expectedCandidates={candidateCount}
            />
          ) : null}
          {tab === "dropped" ? (
            <DroppedList data={data} evidenceRecorded={hasSelectionEvidence} />
          ) : null}
          {tab === "fetch" ? <FetchList data={data} /> : null}
        </div>
      </Card>
    </div>
  )
}

function BigCount({
  label,
  value,
  tone = "text-ink",
}: {
  label: string
  value: number | string
  tone?: string | undefined
}) {
  return (
    <div className="border-edge bg-surface min-w-0 rounded-xl border px-3 py-3.5 sm:px-4">
      <p className={`text-2xl font-semibold ${tone}`}>{value}</p>
      <p className="text-muted truncate text-[10px] font-semibold tracking-wider uppercase sm:text-[11px]">
        {label}
      </p>
    </div>
  )
}

function DroppedList({
  data,
  evidenceRecorded,
}: {
  data: RunDetail
  evidenceRecorded: boolean
}) {
  if (data.dropped.length === 0) {
    return evidenceRecorded ? (
      <EmptyState
        title="Nothing was dropped"
        hint="Every candidate made it through."
      />
    ) : (
      <EmptyState title="Drop evidence was not recorded" />
    )
  }
  return (
    <ul className="divide-edge/60 divide-y">
      {data.dropped.map((drop) => (
        <li
          key={`${drop.source_key}-${drop.url}`}
          className="flex items-center gap-3 px-3 py-2.5"
        >
          <span className="bg-muted h-1 w-1 shrink-0 rounded-full" />
          <div className="min-w-0 flex-1">
            <p className="text-muted truncate text-sm">{drop.title}</p>
            <p className="text-muted/70 truncate font-mono text-xs">
              {drop.source_key} · {drop.reason}
            </p>
          </div>
          <a
            href={drop.url}
            target="_blank"
            rel="noreferrer"
            aria-label={`Open ${drop.title}`}
            className="text-muted hover:text-accent shrink-0 rounded-md p-1"
          >
            <IconChevron className="h-4 w-4 -rotate-45" />
          </a>
        </li>
      ))}
    </ul>
  )
}

function FetchList({ data }: { data: RunDetail }) {
  if (data.fetch.length === 0)
    return <EmptyState title="No fetch log for this run" />
  return (
    <ul className="divide-edge/60 divide-y">
      {data.fetch.map((entry) => (
        <li
          key={entry.source_key}
          className="flex items-center gap-3 px-3 py-2.5"
        >
          <span className="text-ink w-44 shrink-0 truncate font-mono text-xs">
            {entry.source_key}
          </span>
          {entry.status === "ok" ? (
            <span className="text-muted text-xs">
              {entry.items} items · {entry.new_items} new
            </span>
          ) : (
            <span className="text-danger truncate text-xs">{entry.error}</span>
          )}
        </li>
      ))}
    </ul>
  )
}
