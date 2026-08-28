import type { BriefOptionsInput } from "../api-client"
import { Button, inputClass } from "./controls"
import { Card, ErrorNote } from "./shared"

export default function BriefOptionsCard({
  value,
  dirty,
  saving,
  error,
  onChange,
  onSave,
}: {
  value: BriefOptionsInput | null
  dirty: boolean
  saving: boolean
  error: unknown
  onChange: (value: BriefOptionsInput) => void
  onSave: () => void
}) {
  return (
    <Card title="Brief options">
      <form
        className="grid grid-cols-1 gap-4 px-4 py-4 sm:grid-cols-3"
        onSubmit={(event) => {
          event.preventDefault()
          onSave()
        }}
      >
        <NumberField
          label="Max delivery items"
          value={value?.maxDeliveryItems}
          min={1}
          max={25}
          onChange={(maxDeliveryItems) => {
            if (value) onChange({ ...value, maxDeliveryItems })
          }}
        />
        <NumberField
          label="Pre-game days"
          value={value?.sportsPreGameDays}
          min={0}
          onChange={(sportsPreGameDays) => {
            if (value) onChange({ ...value, sportsPreGameDays })
          }}
        />
        <NumberField
          label="Post-game days"
          value={value?.sportsPostGameDays}
          min={0}
          onChange={(sportsPostGameDays) => {
            if (value) onChange({ ...value, sportsPostGameDays })
          }}
        />
        <TimezoneField value={value} onChange={onChange} />
        <div className="sm:col-span-3">
          <Button variant="primary" type="submit" disabled={saving || !dirty}>
            {saving ? "Saving…" : "Save"}
          </Button>
        </div>
      </form>
      {error ? (
        <div className="px-4 pb-3">
          <ErrorNote error={error} />
        </div>
      ) : null}
      <p className="border-edge text-muted border-t px-4 py-2.5 text-xs">
        The sports section repeats fixtures and final results for these windows.
        It does not count toward max delivery items.
      </p>
    </Card>
  )
}

function TimezoneField({
  value,
  onChange,
}: {
  value: BriefOptionsInput | null
  onChange: (value: BriefOptionsInput) => void
}) {
  const detected = browserTimezone()
  return (
    <label className="block sm:col-span-3">
      <span className="text-muted mb-1 block text-xs font-medium">
        Sports time zone
      </span>
      <div className="flex gap-2">
        <input
          type="text"
          required
          className={inputClass}
          placeholder="America/Chicago"
          value={value?.sportsTimezone ?? ""}
          onChange={(event) => {
            if (value) {
              onChange({ ...value, sportsTimezone: event.target.value })
            }
          }}
        />
        <Button
          disabled={!value || !detected}
          onClick={() => {
            if (value && detected) {
              onChange({ ...value, sportsTimezone: detected })
            }
          }}
        >
          Use browser time zone
        </Button>
      </div>
      <span className="text-muted mt-1 block text-xs">
        Use an IANA name such as America/Chicago. The browser button only fills
        the field; Save approves the durable change.
      </span>
    </label>
  )
}

function NumberField({
  label,
  value,
  min,
  max,
  onChange,
}: {
  label: string
  value: number | undefined
  min: number
  max?: number | undefined
  onChange: (value: number) => void
}) {
  return (
    <label className="block">
      <span className="text-muted mb-1 block text-xs font-medium">{label}</span>
      <input
        type="number"
        required
        min={min}
        max={max}
        step={1}
        className={inputClass}
        value={value ?? ""}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  )
}

function browserTimezone(): string | null {
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone
  return timezone.trim() ? timezone : null
}
