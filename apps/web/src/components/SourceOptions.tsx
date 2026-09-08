import type { Source } from "../api-contracts"
import { HandlingFields, PreferenceFields } from "./SourceFields"
import { Field } from "./ui"

export default function SourceOptions({
  draft,
  update,
  editing,
  suggestedKey,
  open,
  onToggle,
}: {
  draft: Source
  update: (patch: Partial<Source>) => void
  editing: boolean
  suggestedKey: string
  open: boolean
  onToggle: (open: boolean) => void
}) {
  return (
    <details
      className="folio-advanced"
      open={open}
      onToggle={(event) => onToggle(event.currentTarget.open)}
    >
      <summary>Advanced source options</summary>
      <div className="folio-form-grid">
        {editing ? (
          <p className="folio-wide folio-muted">
            Immutable source key: <code>{draft.key}</code>
          </p>
        ) : (
          <Field
            label="Source key"
            wide
            hint="Created from the name unless you enter one."
          >
            <input
              value={draft.key}
              placeholder={suggestedKey}
              onChange={(event) => update({ key: event.target.value })}
            />
          </Field>
        )}
        <PreferenceFields draft={draft} update={update} />
      </div>
      <HandlingFields draft={draft} update={update} />
    </details>
  )
}
