import { useState } from "react"

import type { Source } from "../api-contracts"
import { Button, Field, inputClass, Modal, Toggle } from "./controls"

export function SourceEditorModal({
  draft,
  editingExisting,
  busy,
  onClose,
  onSave,
}: {
  draft: Source
  editingExisting: boolean
  busy: boolean
  onClose: () => void
  onSave: (source: Source) => void
}) {
  const [form, setForm] = useState<Source>(draft)
  const update = (patch: Partial<Source>): void =>
    setForm({ ...form, ...patch })
  return (
    <Modal
      title={editingExisting ? "Edit source" : "Add source"}
      subtitle="Changes apply on the next brief run."
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            type="submit"
            formId="source-form"
            disabled={busy}
          >
            {busy ? "Saving…" : "Save changes"}
          </Button>
        </>
      }
    >
      <form
        id="source-form"
        className="grid grid-cols-2 gap-x-4 gap-y-3.5"
        onSubmit={(event) => {
          event.preventDefault()
          onSave(form)
        }}
      >
        <IdentityFields
          form={form}
          immutableKey={editingExisting}
          update={update}
        />
        <PolicyFields form={form} update={update} />
      </form>
    </Modal>
  )
}

function IdentityFields({
  form,
  immutableKey,
  update,
}: {
  form: Source
  immutableKey: boolean
  update: (patch: Partial<Source>) => void
}) {
  return (
    <>
      <div className="col-span-2">
        <Field
          label="Key"
          hint="Immutable identifier used in run logs and dedup records."
        >
          <input
            className={`${inputClass} font-mono`}
            value={form.key}
            onChange={(event) => update({ key: event.target.value })}
            readOnly={immutableKey}
            required
          />
        </Field>
      </div>
      <Field label="Label">
        <input
          className={inputClass}
          value={form.label}
          onChange={(event) => update({ label: event.target.value })}
          required
        />
      </Field>
      <Field label="Kind">
        <select
          className={inputClass}
          value={form.kind}
          onChange={(event) => update({ kind: event.target.value })}
        >
          <option value="rss">rss</option>
          <option value="atom">atom</option>
          <option value="github_release">github_release</option>
        </select>
      </Field>
      <div className="col-span-2">
        <Field
          label="URL"
          hint={
            form.kind === "github_release"
              ? "Optional. SiftWire derives it from the repository when empty."
              : undefined
          }
        >
          <input
            className={`${inputClass} font-mono text-[13px]`}
            value={form.url}
            onChange={(event) => update({ url: event.target.value })}
            placeholder="https://example.test/feed.xml"
          />
        </Field>
      </div>
      {form.kind === "github_release" ? (
        <div className="col-span-2">
          <Field label="Repository">
            <input
              className={`${inputClass} font-mono`}
              value={form.repo}
              placeholder="owner/name"
              onChange={(event) => update({ repo: event.target.value })}
            />
          </Field>
        </div>
      ) : null}
    </>
  )
}

function PolicyFields({
  form,
  update,
}: {
  form: Source
  update: (patch: Partial<Source>) => void
}) {
  return (
    <>
      <Field label="Section">
        <input
          className={inputClass}
          value={form.section}
          onChange={(event) => update({ section: event.target.value })}
          required
        />
      </Field>
      <Field label="Threshold">
        <select
          className={inputClass}
          value={form.threshold}
          onChange={(event) => update({ threshold: event.target.value })}
        >
          {["always", "medium", "high", "audit"].map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </Field>
      <Field label="Dedup group">
        <input
          className={inputClass}
          value={form.dedup_group}
          onChange={(event) => update({ dedup_group: event.target.value })}
        />
      </Field>
      <Field label="Priority rank">
        <input
          type="number"
          className={inputClass}
          value={form.priority_rank}
          onChange={(event) =>
            update({ priority_rank: Number(event.target.value) })
          }
        />
      </Field>
      <div className="border-edge col-span-2 mt-1 flex flex-col gap-3 border-t pt-3.5">
        <Toggle
          label="Enabled"
          checked={form.enabled}
          onChange={(checked) => update({ enabled: checked })}
        />
        <Toggle
          label="Always report"
          checked={form.always_report}
          onChange={(checked) => update({ always_report: checked })}
        />
      </div>
    </>
  )
}
