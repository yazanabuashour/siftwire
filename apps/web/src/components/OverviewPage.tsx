import { SentBrief } from "./Reading"
import { useRecentRuns } from "./run-queries"
import { dateLabel, ErrorNote, PageHeading } from "./ui"

export default function OverviewPage() {
  const runs = useRecentRuns()
  const latest = runs.data?.runs.find((run) => run.delivered_at !== null)
  return (
    <>
      <PageHeading title="Latest brief">
        {latest && dateLabel(latest.delivered_at ?? latest.started_at, true)}
      </PageHeading>
      {runs.isPending ? (
        <output className="folio-empty">Loading briefs…</output>
      ) : runs.isError ? (
        <ErrorNote error={runs.error} />
      ) : latest ? (
        <SentBrief runId={latest.run_id} />
      ) : (
        <p className="folio-empty">
          No sent briefs in recent history.{" "}
          <a href="/sources">Manage sources</a> or{" "}
          <a href="/runs">check history</a>.
        </p>
      )}
    </>
  )
}
