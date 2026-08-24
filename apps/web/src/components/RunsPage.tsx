import { useQuery } from "@tanstack/react-query"
import { useState } from "react"

import { getRun, listRuns } from "../api-client"
import type { RunDetail } from "../api-contracts"

export default function RunsPage() {
  const runs = useQuery({
    queryKey: ["runs"],
    queryFn: ({ signal }) => listRuns(50, signal),
  })
  const [selected, setSelected] = useState<string | null>(null)
  return (
    <div className="flex flex-col gap-4">
      {runs.isPending ? <p className="text-(--muted)">Loading…</p> : null}
      {runs.isError ? (
        <p className="text-sm text-red-400">{runs.error.message}</p>
      ) : null}
      {runs.data ? (
        <table className="text-sm">
          <thead className="text-(--muted)">
            <tr className="text-left">
              <th className="py-1">Started</th>
              <th>Status</th>
              <th>Delivered</th>
              <th>Summary</th>
            </tr>
          </thead>
          <tbody>
            {runs.data.runs.map((run) => (
              <tr
                key={run.run_id}
                className={`cursor-pointer border-t border-(--border) hover:text-(--accent) ${
                  selected === run.run_id ? "text-(--accent)" : ""
                }`}
                onClick={() =>
                  setSelected(run.run_id === selected ? null : run.run_id)
                }
              >
                <td className="py-1 font-mono text-xs">{run.started_at}</td>
                <td>{run.status}</td>
                <td>{run.delivered_at ? "yes" : "no"}</td>
                <td className="max-w-md truncate">{run.summary}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      {selected ? <RunPanel runId={selected} /> : null}
    </div>
  )
}

function RunPanel({ runId }: { runId: string }) {
  const detail = useQuery({
    queryKey: ["run", runId],
    queryFn: ({ signal }) => getRun(runId, signal),
  })
  if (detail.isPending) return <p className="text-(--muted)">Loading run…</p>
  if (detail.isError)
    return <p className="text-sm text-red-400">{detail.error.message}</p>
  return <DetailBody data={detail.data} />
}

function DetailBody({ data }: { data: RunDetail }) {
  const [tab, setTab] = useState<"candidates" | "dropped" | "fetch">(
    "candidates",
  )
  return (
    <section className="rounded-lg border border-(--border) bg-(--surface) p-4 text-sm">
      <h2 className="mb-2 font-semibold">Run {data.run.run_id}</h2>
      <div className="mb-3 flex gap-2">
        {(["candidates", "dropped", "fetch"] as const).map((name) => (
          <button
            key={name}
            type="button"
            onClick={() => setTab(name)}
            className={`rounded border px-2 py-0.5 capitalize ${
              tab === name
                ? "border-(--accent) text-(--accent)"
                : "border-(--border)"
            }`}
          >
            {name}
          </button>
        ))}
      </div>
      {tab === "candidates" ? (
        <>
          <ItemList
            title="Must include"
            items={data.must_include.map(toLine)}
          />
          <ItemList
            title="Candidates (* = delivered)"
            items={data.candidates.map(toLine)}
          />
        </>
      ) : null}
      {tab === "dropped" ? (
        <ul className="flex flex-col gap-1">
          {data.dropped.map((drop) => (
            <li key={`${drop.source_key}-${drop.url}`}>
              <span className="font-mono">{drop.source_key}</span> —{" "}
              {drop.title}
              <span className="text-(--muted)"> ({drop.reason})</span>
            </li>
          ))}
          {data.dropped.length === 0 ? (
            <li className="text-(--muted)">Nothing dropped.</li>
          ) : null}
        </ul>
      ) : null}
      {tab === "fetch" ? (
        <ul className="flex flex-col gap-1">
          {data.fetch.map((entry) => (
            <li key={entry.source_key}>
              <span className="font-mono">{entry.source_key}</span>{" "}
              {entry.status === "ok"
                ? `ok · items ${entry.items ?? 0} · new ${entry.new_items ?? 0}`
                : `error: ${entry.error}`}
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  )
}

function toLine(item: {
  source_key: string
  title: string
  url: string
  selected: boolean
}): string {
  return `${item.selected ? "* " : ""}[${item.source_key}] ${item.title} — ${item.url}`
}

function ItemList({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) {
    return <p className="text-(--muted)">{title}: none</p>
  }
  return (
    <div className="mb-3">
      <h3 className="text-(--muted)">{title}</h3>
      <ul className="list-inside list-disc">
        {items.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
    </div>
  )
}
