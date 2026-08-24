import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { replaceOutlets } from "../api-client"
import type { OutletPolicy } from "../api-contracts"
import { Button, inputClass, Toggle } from "./controls"
import { ErrorNote, PageHeader, Pill, Skeleton, useConfig } from "./shared"

export default function OutletsPage() {
  const config = useConfig()
  const client = useQueryClient()
  const [rows, setRows] = useState<OutletPolicy[] | null>(null)
  useEffect(() => {
    // Do not clobber unsaved edits when the query refetches.
    if (config.data && rows === null) setRows(config.data.outlets)
  }, [config.data, rows])
  const save = useMutation({
    mutationFn: (outlets: OutletPolicy[]) => replaceOutlets(outlets),
    onSuccess: () => void client.invalidateQueries({ queryKey: ["config"] }),
  })
  const update = (index: number, patch: Partial<OutletPolicy>): void =>
    setRows((current) =>
      (current ?? []).map((row, at) =>
        at === index ? { ...row, ...patch } : row,
      ),
    )
  const dirty =
    rows !== null &&
    JSON.stringify(rows) !== JSON.stringify(config.data?.outlets)

  return (
    <>
      <PageHeader
        title="Outlet policies"
        subtitle={
          config.data
            ? `${config.data.outlets.length} policies · saving replaces the full list`
            : undefined
        }
        actions={
          <Button
            variant="primary"
            disabled={rows === null || !dirty || save.isPending}
            onClick={() => {
              if (rows) save.mutate(rows)
            }}
          >
            {save.isPending ? "Saving…" : "Save changes"}
          </Button>
        }
      />
      {save.isError ? (
        <div className="mb-4">
          <ErrorNote error={save.error} />
        </div>
      ) : null}
      {dirty ? (
        <div className="border-warn/30 bg-warn-dim text-warn mb-4 rounded-lg border px-3 py-2 text-xs">
          You have unsaved changes. Saving replaces every policy in one runner
          write.
        </div>
      ) : null}
      <OutletTable
        rows={rows}
        pending={config.isPending}
        error={config.isError ? config.error : null}
        update={update}
      />
      <p className="text-muted mt-3 flex items-center gap-2 text-xs">
        Policy meanings:
        <Pill tone="ok">allow</Pill>
        <Pill tone="error">block</Pill>
        <Pill tone="warn">watch</Pill>
      </p>
    </>
  )
}

function OutletTable({
  rows,
  pending,
  error,
  update,
}: {
  rows: OutletPolicy[] | null
  pending: boolean
  error: unknown | null
  update: (index: number, patch: Partial<OutletPolicy>) => void
}) {
  let content
  if (error) content = <ErrorNote error={error} />
  else if (pending || rows === null) {
    content = (
      <div className="space-y-px p-2">
        {[1, 2, 3, 4, 5].map((key) => (
          <Skeleton key={key} className="h-10" />
        ))}
      </div>
    )
  } else if (rows.length === 0) {
    content = (
      <p className="text-muted px-4 py-10 text-center text-sm">
        No policies configured.
      </p>
    )
  } else {
    content = (
      <div className="overflow-x-auto">
        <table className="w-full min-w-4xl text-sm">
          <OutletHead />
          <tbody className="divide-edge divide-y">
            {rows.map((row, index) => (
              <OutletRow
                key={row.name}
                row={row}
                index={index}
                update={update}
              />
            ))}
          </tbody>
        </table>
      </div>
    )
  }
  return (
    <div className="border-edge bg-surface overflow-hidden rounded-xl border">
      {content}
    </div>
  )
}

function OutletHead() {
  return (
    <thead>
      <tr className="border-edge text-muted border-b text-left text-[11px] font-semibold tracking-wider uppercase">
        <th className="px-4 py-2.5 font-semibold">Name</th>
        <th className="w-64 px-3 py-2.5 font-semibold">Aliases</th>
        <th className="px-3 py-2.5 font-semibold">Policy</th>
        <th className="w-80 px-3 py-2.5 font-semibold">Note</th>
        <th className="px-4 py-2.5 text-right font-semibold">Enabled</th>
      </tr>
    </thead>
  )
}

function OutletRow({
  row,
  index,
  update,
}: {
  row: OutletPolicy
  index: number
  update: (index: number, patch: Partial<OutletPolicy>) => void
}) {
  return (
    <tr className="hover:bg-raised/60 transition-colors">
      <td className="text-ink px-4 py-2.5 font-mono text-[13px]">{row.name}</td>
      <td className="px-3 py-2.5">
        <input
          className={`${inputClass} font-mono text-xs`}
          aria-label={`Aliases for ${row.name}`}
          title={row.aliases.join(", ")}
          value={row.aliases.join(", ")}
          onChange={(event) =>
            update(index, {
              aliases: event.target.value
                .split(",")
                .map((alias) => alias.trim())
                .filter(Boolean),
            })
          }
        />
      </td>
      <td className="px-3 py-2.5">
        <select
          className={`${inputClass} w-28`}
          aria-label={`Policy for ${row.name}`}
          value={row.policy}
          onChange={(event) => update(index, { policy: event.target.value })}
        >
          <option value="allow">allow</option>
          <option value="block">block</option>
          <option value="watch">watch</option>
        </select>
      </td>
      <td className="px-3 py-2.5">
        <input
          className={inputClass}
          aria-label={`Note for ${row.name}`}
          title={row.note}
          value={row.note}
          onChange={(event) => update(index, { note: event.target.value })}
        />
      </td>
      <td className="px-4 py-2.5">
        <div className="flex justify-end">
          <Toggle
            label={`Enabled policy ${row.name}`}
            checked={row.enabled}
            hideLabel
            onChange={(checked) => update(index, { enabled: checked })}
          />
        </div>
      </td>
    </tr>
  )
}
