import { useQuery } from "@tanstack/react-query"
import Markdown from "react-markdown"

import { listRuns } from "../api-client"
import {
  Card,
  EmptyState,
  ErrorNote,
  formatWhen,
  PageHeader,
  Skeleton,
} from "./shared"

export default function DeliveriesPage() {
  const runs = useQuery({
    queryKey: ["deliveries"],
    queryFn: ({ signal }) => listRuns(100, signal),
  })

  if (runs.isPending) {
    return (
      <>
        <PageHeader
          title="Deliveries"
          subtitle="Every brief that shipped, newest first."
        />
        <div className="space-y-4">
          {[1, 2, 3].map((key) => (
            <Skeleton key={key} className="h-36" />
          ))}
        </div>
      </>
    )
  }
  if (runs.isError) return <ErrorNote error={runs.error} />

  const delivered = runs.data.runs.filter((run) => run.delivered_at !== null)
  return (
    <>
      <PageHeader
        title="Deliveries"
        subtitle={
          delivered.length > 0
            ? `${delivered.length} briefs shipped, newest first.`
            : undefined
        }
      />
      {delivered.length === 0 ? (
        <Card>
          <EmptyState
            title="No deliveries recorded yet"
            hint="Briefs appear here after the first scheduled run delivers."
          />
        </Card>
      ) : (
        <div className="flex flex-col gap-4">
          {delivered.map((run) => (
            <Card key={run.run_id} className="overflow-hidden">
              <header className="border-edge flex items-center justify-between gap-3 border-b px-4 py-3">
                <div className="flex items-center gap-2.5">
                  <span className="text-ink text-sm font-medium">
                    {formatWhen(run.delivered_at)}
                  </span>
                  <span className="bg-raised text-muted rounded-full px-2 py-0.5 font-mono text-[11px]">
                    {run.run_id.slice(0, 8)}
                  </span>
                </div>
                <span className="text-muted text-xs">
                  {run.message?.match(/^-/gm)?.length ?? 0} stories
                </span>
              </header>
              <div className="[&_a]:text-accent px-4 py-3 text-sm [&_a:hover]:underline [&_li]:my-1 [&_p]:my-2 [&_ul]:list-disc [&_ul]:space-y-1 [&_ul]:pl-5">
                <Markdown>{run.message ?? ""}</Markdown>
              </div>
            </Card>
          ))}
        </div>
      )}
    </>
  )
}
