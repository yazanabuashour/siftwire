import type { Source } from "../api-contracts"
import { HandlingFields, PreferenceFields } from "./SourceFields"
import { Field } from "./ui"

export default function SourceOptions({
  draft,
  update,
  editing,
  suggestedKey,
}: {
  draft: Source
  update: (patch: Partial<Source>) => void
  editing: boolean
  suggestedKey: string
}) {
  return (
    <>
      {draft.kind !== "sports_schedule" && (
        <fieldset className="folio-form-group">
          <legend>Selection</legend>
          <div className="folio-form-grid">
            <PreferenceFields draft={draft} update={update} />
          </div>
        </fieldset>
      )}
      <HandlingFields draft={draft} update={update} />
      {editing ? (
        <p className="folio-muted">
          Immutable source key: <code>{draft.key}</code>
        </p>
      ) : (
        <Field
          label="Source key"
          hint="Created from the name unless you enter one."
        >
          <input
            value={draft.key}
            placeholder={suggestedKey}
            onChange={(event) => update({ key: event.target.value })}
          />
        </Field>
      )}
    </>
  )
}
