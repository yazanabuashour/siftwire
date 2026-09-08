import { useState } from "react"

import type { RunDetail } from "../api-contracts"
import { AppLink, runHref, useNavigation } from "../navigation"
import { useSelectedRun } from "./run-queries"
import { Candidates, Dropped, FetchStatuses } from "./RunEvidence"
import { RunPicker } from "./RunPicker"
import { dateLabel, ErrorNote, PageHeading } from "./ui"

const tabs = ["candidates", "dropped", "fetch"] as const
type Tab = (typeof tabs)[number]

export default function RunsPage() {
  const { runId } = useNavigation()
  const { archive, activeId, detail: result } = useSelectedRun(runId, false)
  const [tab, setTab] = useState<Tab>("candidates")
  const detail = result.data
  return (
    <>
      <PageHeading
        title="Activity"
        action={
          detail?.run.delivered_at && (
            <AppLink href={runHref("/", detail.run.run_id)}>
              Read saved brief
            </AppLink>
          )
        }
      />
      <div className="folio-runs-layout">
        <RunPicker delivered={false} selected={detail?.run} runId={runId} />
        {activeId === undefined ? (
          archive.isPending ? (
            <output className="folio-empty">Loading activity…</output>
          ) : archive.isError ? (
            <ErrorNote error={archive.error} />
          ) : (
            <p className="folio-empty">No recorded runs yet.</p>
          )
        ) : result.isPending ? (
          <output className="folio-empty">Loading run…</output>
        ) : result.isError ? (
          <ErrorNote error={result.error} />
        ) : detail ? (
          <section className="folio-evidence" aria-label="Run activity">
            <RunHeader detail={detail} />
            <EvidenceTabs detail={detail} tab={tab} onSelect={setTab} />
            <div
              role="tabpanel"
              id="folio-evidence-panel"
              aria-labelledby={`folio-tab-${tab}`}
              tabIndex={0}
              className="folio-evidence-body"
              key={`${activeId}-${tab}`}
            >
              {tab === "candidates" && <Candidates detail={detail} />}
              {tab === "dropped" && <Dropped detail={detail} />}
              {tab === "fetch" && <FetchStatuses detail={detail} />}
            </div>
          </section>
        ) : (
          <p className="folio-empty">No history yet.</p>
        )}
      </div>
    </>
  )
}

function RunHeader({ detail }: { detail: RunDetail }) {
  return (
    <header className="folio-evidence-heading">
      <p>
        {detail.run.dry_run
          ? "Preview only. Nothing sent."
          : detail.run.delivered_at
            ? "Sent"
            : `Not sent · ${detail.run.status}`}
      </p>
      <details>
        <summary>Run details</summary>
        <dl className="folio-receipt">
          <div>
            <dt>Run ID</dt>
            <dd>{detail.run.run_id}</dd>
          </div>
          <div>
            <dt>Summary</dt>
            <dd>{detail.run.summary}</dd>
          </div>
          <div>
            <dt>Status</dt>
            <dd>{detail.run.status}</dd>
          </div>
          <div>
            <dt>Started</dt>
            <dd>{dateLabel(detail.run.started_at, true)}</dd>
          </div>
          <div>
            <dt>Finished</dt>
            <dd>
              {detail.run.finished_at
                ? dateLabel(detail.run.finished_at, true)
                : "Not recorded"}
            </dd>
          </div>
          <div>
            <dt>Delivered</dt>
            <dd>
              {detail.run.delivered_at
                ? dateLabel(detail.run.delivered_at, true)
                : "Not delivered"}
            </dd>
          </div>
        </dl>
      </details>
    </header>
  )
}

function EvidenceTabs({
  detail,
  tab,
  onSelect,
}: {
  detail: RunDetail
  tab: Tab
  onSelect: (tab: Tab) => void
}) {
  return (
    <div className="folio-tabs" role="tablist" aria-label="Story history">
      {tabs.map((name, index) => (
        <button
          type="button"
          role="tab"
          id={`folio-tab-${name}`}
          aria-controls="folio-evidence-panel"
          aria-selected={tab === name}
          tabIndex={tab === name ? 0 : -1}
          key={name}
          onClick={() => onSelect(name)}
          onKeyDown={(event) => {
            let next: Tab | undefined
            if (event.key === "ArrowRight")
              next = tabs[(index + 1) % tabs.length]
            if (event.key === "ArrowLeft")
              next = tabs[(index + tabs.length - 1) % tabs.length]
            if (event.key === "Home") next = tabs[0]
            if (event.key === "End") next = tabs[tabs.length - 1]
            if (next) {
              event.preventDefault()
              onSelect(next)
              document.getElementById(`folio-tab-${next}`)?.focus()
            }
          }}
        >
          {
            {
              candidates: "Stories",
              dropped: "Skipped",
              fetch: "Source checks",
            }[name]
          }{" "}
          <span>
            {detail[name].length +
              (name === "candidates" ? detail.must_include.length : 0)}
          </span>
        </button>
      ))}
    </div>
  )
}
