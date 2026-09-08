import { useState } from "react"

import type { Source } from "../api-contracts"
import { normalizeDraft, sourceError, suggestKey } from "./source-validation"
import { IdentityFields, SourceFlags } from "./SourceFields"
import SourceOptions from "./SourceOptions"
import { Dialog } from "./ui"

export { emptySource } from "./source-validation"

export default function SourceEditor({
  source,
  sources,
  editing,
  busy,
  saveError,
  onSave,
  onClose,
}: {
  source: Source
  sources: Source[]
  editing: boolean
  busy: boolean
  saveError: Error | null
  onSave: (source: Source) => void
  onClose: () => void
}) {
  const [draft, setDraft] = useState(() => structuredClone(source))
  const [rank, setRank] = useState(String(source.priority_rank))
  const [error, setError] = useState("")
  const [more, setMore] = useState(false)
  const suggestedKey = suggestKey(draft.label, sources)
  const update = (patch: Partial<Source>): void => {
    setDraft((current) => ({ ...current, ...patch }))
    setError("")
  }
  const save = (): void => {
    if (busy) return
    const next = normalizeDraft(
      draft,
      rank,
      editing ? source.key : draft.key.trim().toLowerCase() || suggestedKey,
    )
    const problem = sourceError(next, sources, editing ? source.key : null)
    setError(problem)
    if (problem) setMore(true)
    else onSave(next)
  }
  return (
    <Dialog
      title={editing ? `Edit ${source.label}` : "Add a source"}
      onClose={() => {
        if (!busy) onClose()
      }}
    >
      <form
        onInvalidCapture={(event) => {
          const disclosure =
            event.target instanceof HTMLElement
              ? event.target.closest("details")
              : null
          if (disclosure) {
            disclosure.open = true
            setMore(true)
          }
        }}
        onSubmit={(event) => {
          event.preventDefault()
          save()
        }}
      >
        <fieldset disabled={busy}>
          <div className="folio-form-grid">
            <IdentityFields draft={draft} update={update} />
          </div>
          <SourceOptions
            draft={draft}
            update={update}
            rank={rank}
            onRank={(value) => {
              setRank(value)
              setError("")
            }}
            editing={editing}
            suggestedKey={suggestedKey}
            open={more}
            onToggle={setMore}
          />
          <SourceFlags draft={draft} update={update} />
          {(error || saveError) && (
            <p className="folio-error" role="alert">
              {error || saveError?.message}
            </p>
          )}
          <div className="folio-form-actions">
            <button className="folio-primary" type="submit">
              {busy ? "Saving…" : "Save source"}
            </button>
            <button type="button" onClick={onClose}>
              Cancel
            </button>
          </div>
        </fieldset>
      </form>
    </Dialog>
  )
}
