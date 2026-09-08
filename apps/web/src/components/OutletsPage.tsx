import { useState } from "react"

import type { ConfigResult, OutletPolicy } from "../api-contracts"
import { useOutletDraft, useOutletEditor } from "./config-outlets"
import { Dialog, Field, PageHeading } from "./ui"

import "./config.css"

export default function OutletsPage() {
  const model = useOutletEditor()
  const { config, rows, save, editor, startEditing } = model
  const [confirmClear, setConfirmClear] = useState(false)
  return (
    <section className="config-page">
      <PageHeading
        title="Publishers"
        action={
          <button
            type="button"
            disabled={!config.data || save.isPending}
            onClick={() => startEditing(null)}
          >
            Add publisher
          </button>
        }
      />
      <p className="folio-policy-guide">
        Allow lets stories through. Watch allows them and adds a policy
        annotation; it does not create a review queue. Block keeps them out.
        Notes are optional rationale, not rules.
      </p>
      {config.isPending && <output>Loading publishers…</output>}
      {config.isError && (
        <div className="folio-error" role="alert">
          <p>{config.error.message}</p>
          <button
            type="button"
            disabled={config.isFetching}
            onClick={() => void config.refetch()}
          >
            Retry
          </button>
        </div>
      )}
      {config.data && (
        <>
          <PublisherRows
            rows={rows}
            conflicts={config.data.outlet_conflicts}
            busy={save.isPending}
            onEdit={startEditing}
          />
          {save.isError && (
            <p className="folio-error" role="alert">
              {save.error.message}
            </p>
          )}
          <div className="folio-save-bar">
            <button
              type="button"
              className="folio-primary"
              disabled={!model.dirty || save.isPending}
              onClick={() => {
                if (rows.length === 0) setConfirmClear(true)
                else save.mutate(rows)
              }}
            >
              {save.isPending ? "Saving…" : "Save publishers"}
            </button>
            <button
              type="button"
              disabled={!model.hasDraft || save.isPending}
              onClick={model.discard}
            >
              Discard changes
            </button>
            <output>
              {save.isPending
                ? "Saving publishers…"
                : model.dirty
                  ? "Unsaved changes. Saving replaces the full publisher list."
                  : model.notice}
            </output>
          </div>
        </>
      )}
      {editor && (
        <OutletEditor
          row={editor.row}
          rows={rows}
          index={editor.index}
          onClose={model.closeEditor}
          onApply={model.apply}
          onRemove={editor.index === null ? undefined : model.remove}
        />
      )}
      {confirmClear && (
        <ClearPublishersDialog
          onClose={() => setConfirmClear(false)}
          onClear={() => {
            setConfirmClear(false)
            save.mutate(rows)
          }}
        />
      )}
    </section>
  )
}

function ClearPublishersDialog({
  onClose,
  onClear,
}: {
  onClose: () => void
  onClear: () => void
}) {
  return (
    <Dialog title="Clear all publisher rules?" onClose={onClose}>
      <p>
        Saving this empty list removes every publisher rule, including all
        blocks. No publisher will be blocked by this list.
      </p>
      <div className="folio-form-actions">
        <button type="button" onClick={onClose}>
          Keep draft
        </button>
        <button type="button" className="folio-danger" onClick={onClear}>
          Clear all rules
        </button>
      </div>
    </Dialog>
  )
}

function PublisherRows({
  rows,
  conflicts,
  busy,
  onEdit,
}: {
  rows: OutletPolicy[]
  conflicts: ConfigResult["outlet_conflicts"]
  busy: boolean
  onEdit: (index: number, row: OutletPolicy) => void
}) {
  return (
    <>
      {conflicts.length > 0 && (
        <div className="folio-error" role="alert">
          <p>
            Saved publisher rules have overlapping matches. Resolve these in the
            draft before saving.
          </p>
          <ul>
            {conflicts.map((conflict) => (
              <li key={conflict.matcher}>
                <code>{conflict.matcher}</code>: {conflict.names.join(", ")}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="config-outlet-list">
        {rows.map((row, index) => (
          <article className="config-outlet-row" key={row.name}>
            <h2>{row.name}</h2>
            <span>
              {row.policy === "watch"
                ? "Watch · allowed + annotated"
                : row.policy === "block"
                  ? "Block"
                  : row.policy === "allow"
                    ? "Allow"
                    : row.policy}
            </span>
            <span className="folio-muted">
              {row.enabled ? "Active" : "Inactive"}
            </span>
            <button
              type="button"
              disabled={busy}
              aria-label={`Edit ${row.name}`}
              onClick={() => onEdit(index, row)}
            >
              Edit
            </button>
          </article>
        ))}
      </div>
      {rows.length === 0 && (
        <p className="folio-empty">
          No publisher rules. No publisher will be blocked by this list.
        </p>
      )}
    </>
  )
}

interface OutletEditorProps {
  row: OutletPolicy
  rows: OutletPolicy[]
  index: number | null
  onClose: () => void
  onApply: (row: OutletPolicy) => void
  onRemove: (() => void) | undefined
}

function OutletEditor({
  row,
  rows,
  index,
  onClose,
  onApply,
  onRemove,
}: OutletEditorProps) {
  const { draft, error, update, validate } = useOutletDraft(row, rows, index)
  return (
    <Dialog
      title={index === null ? "Add publisher" : `Edit ${row.name}`}
      onClose={onClose}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault()
          const next = validate()
          if (next) onApply(next)
        }}
      >
        <div className="folio-form-grid">
          <Field label="Publisher name">
            <input
              required
              value={draft.name}
              onChange={(event) => update({ name: event.target.value })}
            />
          </Field>
          <Field label="Rule">
            <select
              value={draft.policy}
              onChange={(event) => update({ policy: event.target.value })}
            >
              <option value="allow">Allow</option>
              <option value="watch">Watch, allowed + annotated</option>
              <option value="block">Block</option>
            </select>
          </Field>
          <Field
            label="Aliases"
            wide
            hint="Other names or domains, separated by commas. Enabled rules must not overlap."
          >
            <input
              value={draft.aliasText}
              onChange={(event) => update({ aliasText: event.target.value })}
            />
          </Field>
          <Field
            label="Note, optional"
            wide
            hint="Rationale only. The runner does not interpret this text as a rule."
          >
            <input
              value={draft.note}
              onChange={(event) => update({ note: event.target.value })}
            />
          </Field>
          <label className="folio-check">
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(event) => update({ enabled: event.target.checked })}
            />
            Active
          </label>
        </div>
        {error && (
          <p className="folio-error" role="alert">
            {error}
          </p>
        )}
        <p className="folio-muted">
          Changes stay local until you choose Save publishers.
        </p>
        <div className="folio-form-actions">
          <button type="submit" className="folio-primary">
            Apply to draft
          </button>
          <button type="button" onClick={onClose}>
            Cancel
          </button>
          {onRemove && (
            <button
              type="button"
              className="folio-quiet-danger"
              onClick={onRemove}
            >
              Remove from draft
            </button>
          )}
        </div>
      </form>
    </Dialog>
  )
}
