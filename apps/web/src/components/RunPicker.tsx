import { useState } from "react"

import type { RunSummary } from "../api-contracts"
import { AppLink, runHref, selectRun } from "../navigation"
import { Picker, type PickerOption } from "./Picker"
import { useRuns } from "./run-queries"
import { dateLabel, ErrorNote } from "./ui"

function runOption(run: RunSummary, delivered: boolean): PickerOption {
  const state = run.delivered_at
    ? "Sent"
    : run.dry_run
      ? "Preview only"
      : `Not sent · ${run.status}`
  return {
    id: run.run_id,
    label: `${delivered && run.delivered_at ? "Delivered" : "Started"} ${dateLabel(delivered ? (run.delivered_at ?? run.started_at) : run.started_at, true)} · ${state}`,
    description: `${run.run_id} · ${run.summary}`,
  }
}

export function RunPicker({
  delivered,
  selected,
  runId,
}: {
  delivered: boolean
  selected: RunSummary | undefined
  runId: string | undefined
}) {
  const [search, setSearch] = useState("")
  const [open, setOpen] = useState(false)
  const archive = useRuns(delivered, search, open)
  const options =
    archive.data?.pages.flatMap((page) =>
      page.runs.map((run) => runOption(run, delivered)),
    ) ?? []
  return (
    <div className="folio-run-navigation">
      <Picker
        label={delivered ? "Choose a brief" : "Choose a run"}
        value={selected ? runOption(selected, delivered) : undefined}
        placeholder={runId === undefined ? "Latest" : runId}
        options={options}
        search={search}
        onSearch={setSearch}
        onOpenChange={setOpen}
        onChoose={selectRun}
        help="Search the full archive by stored ISO date YYYY-MM-DD, run ID or summary. Dates are not searched in local display format."
        loading={archive.isFetching}
        footer={
          <>
            {archive.isError ? (
              <>
                <ErrorNote error={archive.error} />
                <button
                  type="button"
                  disabled={archive.isFetching}
                  onClick={() => {
                    if (archive.isFetchNextPageError)
                      void archive.fetchNextPage()
                    else void archive.refetch()
                  }}
                >
                  Retry
                </button>
              </>
            ) : !archive.isPending && !options.length ? (
              <p>{search ? "No matching records." : "No records yet."}</p>
            ) : null}
            {archive.hasNextPage && (
              <button
                type="button"
                disabled={archive.isFetching}
                onClick={() => {
                  void archive.fetchNextPage()
                }}
              >
                {archive.isFetchingNextPage
                  ? "Loading…"
                  : search
                    ? "Load more results"
                    : "Load older"}
              </button>
            )}
          </>
        }
      />
      {runId !== undefined && (
        <AppLink href={runHref(delivered ? "/" : "/runs", undefined)}>
          {delivered ? "Latest brief" : "Latest run"}
        </AppLink>
      )}
    </div>
  )
}
