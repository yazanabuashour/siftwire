import { useState } from "react"

import type { Source } from "../api-contracts"
import { reportingLabel, sourceType } from "./source-validation"
import { Field } from "./ui"

interface CollectionActions {
  busy: boolean
  savingKey: string | undefined
  onEdit: (source: Source) => void
  onRemove: (source: Source) => void
  onToggle: (source: Source, enabled: boolean) => void
}

export default function SourceCollection({
  sources,
  reporting,
  busy,
  savingKey,
  onEdit,
  onRemove,
  onToggle,
}: CollectionActions & {
  sources: Source[]
  reporting: Record<string, string>
}) {
  const [query, setQuery] = useState("")
  const [status, setStatus] = useState("all")
  const [kind, setKind] = useState("all")
  const needle = query.trim().toLowerCase()
  const visible = [...sources]
    .sort(
      (a, b) => a.label.localeCompare(b.label) || a.key.localeCompare(b.key),
    )
    .filter(
      (source) =>
        [
          source.label,
          source.key,
          source.url,
          source.repo,
          source.section,
          sourceType(source.kind),
        ]
          .join(" ")
          .toLowerCase()
          .includes(needle) &&
        (status === "all" || source.enabled === (status === "enabled")) &&
        (kind === "all" || sourceType(source.kind) === kind),
    )
  return (
    <>
      <div className="folio-source-filters">
        <Field label="Search sources">
          <input
            type="search"
            placeholder="Name, topic, or address"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </Field>
        <Field label="Status">
          <select
            value={status}
            onChange={(event) => setStatus(event.target.value)}
          >
            <option value="all">All sources</option>
            <option value="enabled">Enabled</option>
            <option value="disabled">Disabled</option>
          </select>
        </Field>
        <Field label="Type">
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value)}
          >
            <option value="all">All types</option>
            <option value="feed">Feed</option>
            <option value="github_release">GitHub releases</option>
            <option value="sports_schedule">Sports schedule</option>
          </select>
        </Field>
      </div>
      <p className="folio-list-note">
        {visible.length} of {sources.length} sources, sorted by name
      </p>
      <div className="folio-source-list">
        {visible.map((source) => (
          <SourceRow
            key={source.key}
            source={source}
            reporting={reporting[source.key]}
            busy={busy}
            savingKey={savingKey}
            onEdit={onEdit}
            onRemove={onRemove}
            onToggle={onToggle}
          />
        ))}
        {visible.length === 0 && (
          <EmptySources
            filtered={sources.length > 0}
            onClear={() => {
              setQuery("")
              setStatus("all")
              setKind("all")
            }}
          />
        )}
      </div>
    </>
  )
}

function EmptySources({
  filtered,
  onClear,
}: {
  filtered: boolean
  onClear: () => void
}) {
  return (
    <div className="folio-empty">
      <h2>{filtered ? "No sources found" : "No sources configured"}</h2>
      <p>
        {filtered
          ? "Try a different name, or clear the filters."
          : "Add a source to start your collection."}
      </p>
      {filtered && (
        <button type="button" onClick={onClear}>
          Clear filters
        </button>
      )}
    </div>
  )
}

function SourceRow({
  source,
  reporting,
  busy,
  savingKey,
  onEdit,
  onRemove,
  onToggle,
}: CollectionActions & {
  source: Source
  reporting: string | undefined
}) {
  return (
    <article className="folio-source-row">
      <div className="folio-source-identity">
        <h2>{source.label}</h2>
        <p className="config-source-summary">
          {source.section} · {reportingLabel(reporting)}
          {!source.enabled && " · Paused"}
        </p>
        <p className="folio-source-address" title={source.repo || source.url}>
          {source.repo || source.url}
        </p>
      </div>
      <div className="folio-source-actions">
        <label className="folio-check">
          <input
            type="checkbox"
            disabled={busy}
            checked={source.enabled}
            onChange={(event) => onToggle(source, event.target.checked)}
          />
          {savingKey === source.key ? "Saving…" : "Enabled"}
          <span className="folio-sr-only"> for {source.label}</span>
        </label>
        <div className="folio-button-pair">
          <button
            type="button"
            disabled={busy}
            aria-label={`Edit ${source.label}`}
            onClick={() => onEdit(source)}
          >
            Edit
          </button>
          <button
            type="button"
            className="folio-quiet-danger"
            disabled={busy}
            aria-label={`Remove ${source.label}`}
            onClick={() => onRemove(source)}
          >
            Remove
          </button>
        </div>
      </div>
    </article>
  )
}
