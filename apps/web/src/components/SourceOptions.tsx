import type { Source } from "../api-contracts"
import { HandlingFields, PolicyFields, ScheduleFields } from "./SourceFields"
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
      <summary>More options</summary>
      <div className="folio-form-grid">
        <Field
          label="Source key"
          wide
          hint={
            editing
              ? "Fixed for this source."
              : "Created from the name unless you enter one."
          }
        >
          <input
            readOnly={editing}
            value={draft.key}
            placeholder={suggestedKey}
            onChange={(event) => update({ key: event.target.value })}
          />
        </Field>
        <PolicyFields draft={draft} update={update} />
      </div>
      <ScheduleFields draft={draft} update={update} />
      <HandlingFields draft={draft} update={update} />
    </details>
  )
}
