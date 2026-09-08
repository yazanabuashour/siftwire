import { useId, useState } from "react"

import type { ConfigResult, OutletPolicy } from "../api-contracts"
import { useOutletDraft, useOutletEditor } from "./config-outlets"
import { Dialog, ErrorNote, Field, PageHeading } from "./ui"

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
            onChange={model.updateRow}
          />
          {save.isError && <ErrorNote error={save.error} />}
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
    <Dialog
      title="Clear all publisher rules?"
      onClose={onClose}
      actions={
        <>
          <button type="button" onClick={onClose}>
            Keep draft
          </button>
          <button type="button" className="folio-danger" onClick={onClear}>
            Clear all rules
          </button>
        </>
      }
    >
      <p>
        Saving this empty list removes every publisher rule, including all
        blocks. No publisher will be blocked by this list.
      </p>
    </Dialog>
  )
}

function PublisherRows({
  rows,
  conflicts,
  busy,
  onEdit,
  onChange,
}: {
  rows: OutletPolicy[]
  conflicts: ConfigResult["outlet_conflicts"]
  busy: boolean
  onEdit: (index: number, row: OutletPolicy) => void
  onChange: (
    index: number,
    patch: Partial<Pick<OutletPolicy, "policy" | "enabled">>,
  ) => void
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
            <select
              className="config-outlet-rule"
              aria-label={`Rule for ${row.name}`}
              disabled={busy}
              value={row.policy}
              onChange={(event) =>
                onChange(index, { policy: event.target.value })
              }
            >
              {!["allow", "watch", "block"].includes(row.policy) && (
                <option value={row.policy}>{row.policy}</option>
              )}
              <option value="allow">Allow</option>
              <option value="watch">Watch</option>
              <option value="block">Block</option>
            </select>
            <label className="folio-check">
              <input
                type="checkbox"
                checked={row.enabled}
                disabled={busy}
                onChange={(event) =>
                  onChange(index, { enabled: event.target.checked })
                }
              />
              Active<span className="folio-sr-only"> for {row.name}</span>
            </label>
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
  const formId = useId()
  const { draft, error, update, validate } = useOutletDraft(row, rows, index)
  return (
    <Dialog
      title={index === null ? "Add publisher" : "Edit publisher"}
      onClose={onClose}
      actions={
        <>
          <button type="submit" form={formId} className="folio-primary">
            Apply to draft
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
        </>
      }
    >
      <form
        id={formId}
        onSubmit={(event) => {
          event.preventDefault()
          const next = validate()
          if (next) onApply(next)
        }}
      >
        <div className="folio-form-grid">
          <Field label="Publisher name" wide>
            <input
              required
              value={draft.name}
              onChange={(event) => update({ name: event.target.value })}
            />
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
        </div>
        {error && <ErrorNote error={error} />}
        <p className="folio-muted">
          Changes stay local until you choose Save publishers.
        </p>
      </form>
    </Dialog>
  )
}
