import { AppLink, runHref, useNavigation } from "../navigation"
import { RecordedBrief } from "./Reading"
import { useSelectedRun } from "./run-queries"
import { RunPicker } from "./RunPicker"
import { dateLabel, ErrorNote, PageHeading } from "./ui"

export default function OverviewPage() {
  const { runId } = useNavigation()
  const { archive, activeId, detail: result } = useSelectedRun(runId, true)
  const detail = result.data
  return (
    <>
      <PageHeading
        title="Brief"
        action={
          detail && (
            <AppLink href={runHref("/runs", detail.run.run_id)}>
              View activity
            </AppLink>
          )
        }
      >
        {detail?.run.delivered_at && dateLabel(detail.run.delivered_at, true)}
      </PageHeading>
      <RunPicker delivered selected={detail?.run} runId={runId} />
      {activeId !== undefined ? (
        result.isPending ? (
          <output className="folio-empty">Loading brief…</output>
        ) : result.isError ? (
          <ErrorNote error={result.error} />
        ) : detail?.run.delivered_at ? (
          <RecordedBrief detail={detail} />
        ) : (
          <p className="folio-empty">
            This run has no delivered brief. Its recorded details are available
            in Activity.
          </p>
        )
      ) : archive.isPending ? (
        <output className="folio-empty">Loading briefs…</output>
      ) : archive.isError ? (
        <ErrorNote error={archive.error} />
      ) : (
        <p className="folio-empty">
          No sent briefs yet. <AppLink href="/sources">Manage sources</AppLink>{" "}
          or <AppLink href="/runs">check activity</AppLink>.
        </p>
      )}
    </>
  )
}
