import { useQuery } from "@tanstack/react-query"
import Markdown from "react-markdown"

import { listRuns } from "../api-client"

export default function DeliveriesPage() {
  const runs = useQuery({
    queryKey: ["deliveries"],
    queryFn: ({ signal }) => listRuns(100, signal),
  })
  if (runs.isPending) return <p className="text-(--muted)">Loading…</p>
  if (runs.isError)
    return <p className="text-sm text-red-400">{runs.error.message}</p>
  const delivered = runs.data.runs.filter((run) => run.delivered_at)
  if (delivered.length === 0)
    return <p className="text-(--muted)">No deliveries recorded yet.</p>
  return (
    <div className="flex flex-col gap-4">
      {delivered.map((run) => (
        <article
          key={run.run_id}
          className="rounded-lg border border-(--border) bg-(--surface) p-4"
        >
          <header className="mb-2 flex items-baseline justify-between text-xs text-(--muted)">
            <span>{run.delivered_at}</span>
            <span className="font-mono">{run.run_id}</span>
          </header>
          <div className="prose prose-invert text-sm [&_a]:text-(--accent) [&_li]:my-0.5 [&_ul]:list-disc [&_ul]:pl-5">
            <Markdown>{run.message ?? ""}</Markdown>
          </div>
        </article>
      ))}
    </div>
  )
}
