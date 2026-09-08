import { useState, type FormEvent } from "react"

import type { ReportingMode, Source } from "../api-contracts"
import {
  sourceError,
  sourceType,
  suggestKey,
  type FeedReportingMode,
} from "./source-validation"
import {
  IdentityFields,
  PolicyFields,
  ScheduleFields,
  SourceFlags,
} from "./SourceFields"
import SourceOptions from "./SourceOptions"
import { Dialog } from "./ui"

export { emptySource } from "./source-validation"

type SourceEditorProps = {
  source: Source
  sources: Source[]
  editing: boolean
  reporting: ReportingMode | undefined
  busy: boolean
  saveError: Error | null
  onSave: (source: Source) => void
  onClose: () => void
}

function revealInvalidField(event: FormEvent<HTMLFormElement>): boolean {
  const disclosure =
    event.target instanceof HTMLElement ? event.target.closest("details") : null
  if (!disclosure) return false
  disclosure.open = true
  return true
}

export default function SourceEditor({
  source,
  sources,
  editing,
  reporting,
  busy,
  saveError,
  onSave,
  onClose,
}: SourceEditorProps) {
  const [draft, setDraft] = useState(() => structuredClone(source))
  // Type changes do not change the raw feed policy or the user's selection.
  const [feedMode, setFeedMode] = useState<FeedReportingMode | undefined>(
    !editing
      ? "highlights"
      : sourceType(source.kind) === "feed" && reporting !== "sports"
        ? reporting
        : undefined,
  )
  const [error, setError] = useState("")
  const [more, setMore] = useState(false)
  const suggestedKey = suggestKey(draft.label, sources)
  const update = (patch: Partial<Source>): void => {
    setDraft((current) => ({ ...current, ...patch }))
    setError("")
  }
  const save = (): void => {
    if (busy) return
    const next = {
      ...draft,
      key: editing
        ? source.key
        : draft.key.trim().toLowerCase() || suggestedKey,
    }
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
        className="config-source-editor"
        onInvalidCapture={(event) => {
          if (revealInvalidField(event)) setMore(true)
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
          <ScheduleFields draft={draft} update={update} />
          <PolicyFields
            draft={draft}
            update={update}
            mode={feedMode}
            onMode={setFeedMode}
          />
          <SourceFlags draft={draft} update={update} />
          <SourceOptions
            draft={draft}
            update={update}
            editing={editing}
            suggestedKey={suggestedKey}
            open={more}
            onToggle={setMore}
          />
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
