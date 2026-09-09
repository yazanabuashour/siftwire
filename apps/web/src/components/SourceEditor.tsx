import { useId, useState } from "react"

import type { Source } from "../api-contracts"
import { sourceError, sourceType, suggestKey } from "./source-validation"
import {
  IdentityFields,
  PolicyFields,
  ScheduleFields,
  SourceFlags,
} from "./SourceFields"
import SourceOptions from "./SourceOptions"
import { Dialog, ErrorNote } from "./ui"

export { emptySource } from "./source-validation"

type SourceEditorProps = {
  source: Source
  sources: Source[]
  editing: boolean
  reporting: string | undefined
  busy: boolean
  saveError: Error | null
  onSave: (source: Source) => void
  onClose: () => void
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
  const formId = useId()
  const [draft, setDraft] = useState(() => structuredClone(source))
  // Type changes do not change the raw feed policy or the user's selection.
  const [feedMode, setFeedMode] = useState<string | undefined>(
    !editing
      ? "highlights"
      : sourceType(source.kind) === "feed" && reporting !== "sports"
        ? reporting
        : undefined,
  )
  const [error, setError] = useState<Error | null>(null)
  const displayedError = error ?? saveError
  const suggestedKey = suggestKey(draft.label, sources)
  const update = (patch: Partial<Source>): void => {
    setDraft((current) => ({ ...current, ...patch }))
    setError(null)
  }
  const save = (): void => {
    if (busy) return
    const next = {
      ...draft,
      url: draft.kind === "github_release" ? "" : draft.url,
      key: editing
        ? source.key
        : draft.key.trim().toLowerCase() || suggestedKey,
    }
    const problem = sourceError(next, sources, editing ? source.key : null)
    setError(problem ? new Error(problem) : null)
    if (!problem) onSave(next)
  }
  return (
    <Dialog
      title={editing ? "Edit source" : "Add source"}
      onClose={onClose}
      busy={busy}
      actions={
        <button
          className="folio-primary"
          type="submit"
          form={formId}
          disabled={busy}
        >
          {busy ? "Saving…" : "Save source"}
        </button>
      }
    >
      <form
        id={formId}
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
          />
          {displayedError && <ErrorNote error={displayedError} />}
        </fieldset>
      </form>
    </Dialog>
  )
}
