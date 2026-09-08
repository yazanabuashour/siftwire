import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"

import { fetchConfig, replaceOutlets } from "../api-client"
import type { ConfigResult, OutletPolicy } from "../api-contracts"
import { Field, PageHeading } from "./ui"

type OutletDraft = OutletPolicy & { aliasText: string }

function drafts(outlets: OutletPolicy[]): OutletDraft[] {
  return outlets.map((outlet) => ({
    ...outlet,
    aliasText: outlet.aliases.join(", "),
  }))
}

function useOutletEditor() {
  const config = useQuery({
    queryKey: ["config"],
    queryFn: ({ signal }) => fetchConfig(signal),
  })
  const client = useQueryClient()
  // A null draft follows refetches; a local draft preserves unsaved edits.
  const [draft, setDraft] = useState<OutletDraft[] | null>(null)
  const configured = drafts(config.data?.outlets ?? [])
  const rows = draft ?? configured
  const dirty = JSON.stringify(rows) !== JSON.stringify(configured)
  const unavailable = !config.data || config.isError || config.data.rejected
  const save = useMutation({
    mutationFn: async (outlets: OutletPolicy[]) => {
      const result = await replaceOutlets(outlets)
      if (result.rejected) {
        throw new Error(result.summary || "Publisher changes were rejected.")
      }
      return result
    },
    onSuccess: async (_result, outlets) => {
      client.setQueryData<ConfigResult>(["config"], (current) =>
        current ? { ...current, outlets } : current,
      )
      setDraft(null)
      await client.invalidateQueries({ queryKey: ["config"] })
    },
  })
  const update = (index: number, patch: Partial<OutletDraft>): void => {
    setDraft(rows.map((row, at) => (at === index ? { ...row, ...patch } : row)))
    save.reset()
  }
  return { config, rows, dirty, unavailable, save, update }
}

export default function OutletsPage() {
  const { config, rows, dirty, unavailable, save, update } = useOutletEditor()
  return (
    <>
      <PageHeading title="Publishers" />
      <p className="folio-policy-guide">
        Allow lets stories through. Watch flags them for review. Block keeps
        them out.
      </p>
      {config.isPending && <output>Loading publishers…</output>}
      {(config.isError || config.data?.rejected) && (
        <div className="folio-error" role="alert">
          <p>
            {config.error?.message ?? "The configuration request was rejected."}
          </p>
          <button
            type="button"
            disabled={config.isFetching}
            onClick={() => void config.refetch()}
          >
            Retry
          </button>
        </div>
      )}
      {config.data && !config.data.rejected && (
        <form
          onSubmit={(event) => {
            event.preventDefault()
            if (unavailable || !dirty || save.isPending) return
            save.mutate(
              rows.map(({ aliasText, ...row }) => ({
                ...row,
                aliases: aliasText
                  .split(",")
                  .map((alias) => alias.trim())
                  .filter(Boolean),
                note: row.note.trim(),
              })),
            )
          }}
        >
          <div className="folio-outlet-list">
            {rows.map((row, index) => (
              <OutletFields
                key={row.name}
                row={row}
                disabled={save.isPending}
                update={(patch) => update(index, patch)}
              />
            ))}
          </div>
          {rows.length === 0 && (
            <p className="folio-empty">No publishers configured.</p>
          )}
          {save.isError && (
            <p className="folio-error" role="alert">
              {save.error.message}
            </p>
          )}
          <div className="folio-save-bar">
            <button
              type="submit"
              className="folio-primary"
              disabled={unavailable || !dirty || save.isPending}
            >
              {save.isPending ? "Saving…" : "Save publishers"}
            </button>
            <output>
              {save.isPending
                ? "Saving publishers…"
                : dirty
                  ? "Unsaved changes. Saving replaces the full publisher list."
                  : save.isSuccess
                    ? "Publishers saved."
                    : ""}
            </output>
          </div>
        </form>
      )}
    </>
  )
}

function OutletFields({
  row,
  disabled,
  update,
}: {
  row: OutletDraft
  disabled: boolean
  update: (patch: Partial<OutletDraft>) => void
}) {
  return (
    <fieldset className="folio-outlet" disabled={disabled}>
      <legend>{row.name}</legend>
      <div className="folio-outlet-controls">
        <Field label="Rule">
          <select
            value={row.policy}
            onChange={(event) => update({ policy: event.target.value })}
          >
            <option value="allow">Allow</option>
            <option value="watch">Watch</option>
            <option value="block">Block</option>
          </select>
        </Field>
        <Field
          label="Other names"
          hint="Separate names or domains with commas."
        >
          <input
            value={row.aliasText}
            onChange={(event) => update({ aliasText: event.target.value })}
          />
        </Field>
        <Field label="Note" wide>
          <input
            value={row.note}
            onChange={(event) => update({ note: event.target.value })}
          />
        </Field>
        <label className="folio-check">
          <input
            type="checkbox"
            checked={row.enabled}
            onChange={(event) => update({ enabled: event.target.checked })}
          />
          Enabled<span className="folio-sr-only"> for {row.name}</span>
        </label>
      </div>
    </fieldset>
  )
}
