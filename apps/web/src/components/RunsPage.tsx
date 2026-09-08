import { useState } from "react"

import type { RunDetail } from "../api-contracts"
import { useRecentRuns, useRun } from "./run-queries"
import { Candidates, Dropped, Fetch } from "./RunEvidence"
import { dateLabel, ErrorNote, Field, PageHeading } from "./ui"

const tabs = ["candidates", "dropped", "fetch"] as const
type Tab = (typeof tabs)[number]

export default function RunsPage() {
  const history = useRecentRuns()
  const runs = history.data?.runs ?? []
  const [selected, setSelected] = useState("")
  const [tab, setTab] = useState<Tab>("candidates")
  const active = runs.find((run) => run.run_id === selected) ?? runs[0]
  const result = useRun(active?.run_id)
  const detail = result.data
  if (history.isPending)
    return (
      <>
        <PageHeading title="History" />
        <output className="folio-empty">Loading history…</output>
      </>
    )
  if (history.isError)
    return (
      <>
        <PageHeading title="History" />
        <ErrorNote error={history.error} />
      </>
    )
  if (!runs.length)
    return (
      <>
        <PageHeading title="History" />
        <p className="folio-empty">No runs in recent history.</p>
      </>
    )
  return (
    <>
      <PageHeading title="History" />
      <div className="folio-runs-layout">
        <Field label="Choose a brief">
          <select
            value={active?.run_id ?? ""}
            onChange={(event) => setSelected(event.target.value)}
          >
            {runs.map((run) => (
              <option key={run.run_id} value={run.run_id}>
                {dateLabel(run.started_at, true)} ·{" "}
                {run.dry_run
                  ? "Preview only"
                  : run.delivered_at
                    ? "Sent"
                    : run.status === "ok"
                      ? "Not sent"
                      : run.status}
              </option>
            ))}
          </select>
        </Field>
        {result.isPending ? (
          <output className="folio-empty">Loading run…</output>
        ) : result.isError ? (
          <ErrorNote error={result.error} />
        ) : detail ? (
          <section className="folio-evidence" aria-label="Brief history">
            <RunHeader detail={detail} />
            <EvidenceTabs detail={detail} tab={tab} onSelect={setTab} />
            <div
              role="tabpanel"
              id="folio-evidence-panel"
              aria-labelledby={`folio-tab-${tab}`}
              tabIndex={0}
              className="folio-evidence-body"
              key={`${selected}-${tab}`}
            >
              {tab === "candidates" && <Candidates detail={detail} />}
              {tab === "dropped" && <Dropped detail={detail} />}
              {tab === "fetch" && <Fetch detail={detail} />}
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
