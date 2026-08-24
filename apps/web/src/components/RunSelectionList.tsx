import type { RunItem } from "../api-contracts"
import { EmptyState, formatWhen } from "./shared"

export function RunSelectionList({
  mustInclude,
  candidates,
  expectedMustInclude,
  expectedCandidates,
}: {
  mustInclude: RunItem[]
  candidates: RunItem[]
  expectedMustInclude: number
  expectedCandidates: number
}) {
  const items = [
    ...mustInclude.map((item) => ({ item, required: true })),
    ...candidates.map((item) => ({ item, required: false })),
  ]
  const expectedTotal = expectedMustInclude + expectedCandidates
  if (items.length === 0) {
    return (
      <EmptyState
        title={
          expectedTotal > 0
            ? "Selection evidence was not recorded"
            : "No candidates in this run"
        }
        hint={
          expectedTotal > 0
            ? `${expectedMustInclude} must-include · ${expectedCandidates} candidates in the run summary.`
            : undefined
        }
      />
    )
  }
  return (
    <ul className="divide-edge/60 divide-y">
      {items.map(({ item, required }) => (
        <CandidateRow
          key={`${required}-${item.source_key}-${item.url}`}
          item={item}
          required={required}
        />
      ))}
      {mustInclude.length === 0 && expectedMustInclude > 0 ? (
        <MissingEvidence
          count={expectedMustInclude}
          label="must-include items"
        />
      ) : null}
      {candidates.length === 0 && expectedCandidates > 0 ? (
        <MissingEvidence count={expectedCandidates} label="candidates" />
      ) : null}
    </ul>
  )
}

function CandidateRow({
  item,
  required,
}: {
  item: RunItem
  required: boolean
}) {
  return (
    <li className="flex items-center gap-3 px-3 py-2.5">
      <SelectionMark selected={item.selected} />
      <div className="min-w-0 flex-1">
        <a
          href={item.url}
          target="_blank"
          rel="noreferrer"
          className="text-ink hover:text-accent block truncate text-sm"
        >
          {item.title}
        </a>
        <p className="text-muted truncate font-mono text-xs">
          {item.source_key}
          {item.published_at ? ` · ${formatWhen(item.published_at)}` : ""}
        </p>
      </div>
      <span
        className={`hidden shrink-0 text-xs sm:block ${
          required
            ? "text-warn"
            : item.selected
              ? "text-accent font-medium"
              : "text-muted/60"
        }`}
      >
        {required
          ? "must include"
          : item.selected
            ? "delivered"
            : "not selected"}
      </span>
    </li>
  )
}

function MissingEvidence({ count, label }: { count: number; label: string }) {
  return (
    <li className="text-muted bg-raised/40 px-3 py-2 text-xs">
      Evidence was not recorded for {count} {label}.
    </li>
  )
}

function SelectionMark({ selected }: { selected: boolean }) {
  return (
    <span
      className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full ${
        selected ? "bg-accent-dim text-accent" : "bg-raised text-muted"
      }`}
      title={selected ? "Delivered" : "Not selected"}
    >
      {selected ? (
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          className="h-3 w-3"
          aria-hidden="true"
        >
          <path d="M20 6 9 17l-5-5" />
        </svg>
      ) : (
        <span className="bg-muted h-1 w-1 rounded-full" />
      )}
    </span>
  )
}
