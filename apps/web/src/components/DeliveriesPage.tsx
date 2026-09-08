import { useState } from "react"

import { SentBrief } from "./Reading"
import { useRecentRuns } from "./run-queries"
import { dateLabel, ErrorNote, Field, PageHeading } from "./ui"

export default function DeliveriesPage() {
  const runs = useRecentRuns()
  const [selected, setSelected] = useState("")
  const delivered =
    runs.data?.runs.filter((run) => run.delivered_at !== null) ?? []
  const active =
    delivered.find((run) => run.run_id === selected) ?? delivered[0]
  return (
    <>
      <PageHeading title="Sent briefs" />
      {runs.isPending ? (
        <output className="folio-empty">Loading briefs…</output>
      ) : runs.isError ? (
        <ErrorNote error={runs.error} />
      ) : active ? (
        <>
          <div className="folio-delivery-picker">
            <Field label="Choose a brief">
              <select
                value={active.run_id}
                onChange={(event) => setSelected(event.target.value)}
              >
                {delivered.map((run) => (
                  <option key={run.run_id} value={run.run_id}>
                    {dateLabel(run.delivered_at ?? run.started_at, true)}
                  </option>
                ))}
              </select>
            </Field>
          </div>
          <SentBrief runId={active.run_id} />
        </>
      ) : (
        <p className="folio-empty">No sent briefs in recent history.</p>
      )}
    </>
  )
}
