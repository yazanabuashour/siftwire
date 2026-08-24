import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { replaceOutlets } from "../api-client"
import type { OutletPolicy } from "../api-contracts"
import { ErrorNote, useConfig } from "./shared"

export default function OutletsPage() {
  const config = useConfig()
  const client = useQueryClient()
  const [rows, setRows] = useState<OutletPolicy[] | null>(null)

  useEffect(() => {
    // Hydrate once per server state; local edits are never clobbered.
    if (config.data && rows === null) {
      setRows(config.data.outlets)
    }
  }, [config.data, rows])

  const save = useMutation({
    mutationFn: (outlets: OutletPolicy[]) => replaceOutlets(outlets),
    onSuccess: () => void client.invalidateQueries({ queryKey: ["config"] }),
  })

  if (config.isPending) return <p className="text-(--muted)">Loading…</p>
  if (config.isError) return <ErrorNote error={config.error} />

  const update = (index: number, patch: Partial<OutletPolicy>): void =>
    setRows((current) =>
      (current ?? []).map((row, at) =>
        at === index ? { ...row, ...patch } : row,
      ),
    )

  return (
    <div className="flex flex-col gap-4 text-sm">
      <p className="text-(--muted)">
        Saving replaces the full outlet policy list in one runner write.
      </p>
      <table className="text-sm">
        <thead className="text-(--muted)">
          <tr className="text-left">
            <th className="py-1">Name</th>
            <th>Aliases (comma separated)</th>
            <th>Policy</th>
            <th>Note</th>
            <th>Enabled</th>
          </tr>
        </thead>
        <tbody>
          {(rows ?? []).map((row, index) => (
            <OutletRow
              key={row.name}
              row={row}
              onChange={(patch) => update(index, patch)}
            />
          ))}
        </tbody>
      </table>
      {save.isError ? <ErrorNote error={save.error} /> : null}
      {save.isPending ? <p className="text-(--muted)">Saving…</p> : null}
      <button
        type="button"
        disabled={rows === null || save.isPending}
        onClick={() => {
          if (rows !== null) save.mutate(rows)
        }}
        className="self-start rounded border border-(--accent) px-3 py-1 hover:bg-(--accent) hover:text-black disabled:opacity-50"
      >
        Save policies
      </button>
    </div>
  )
}

function OutletRow({
  row,
  onChange,
}: {
  row: OutletPolicy
  onChange: (patch: Partial<OutletPolicy>) => void
}) {
  return (
    <tr className="border-t border-(--border)">
      <td className="py-1 font-mono">{row.name}</td>
      <td>
        <input
          className={inputClass}
          aria-label={`Aliases for ${row.name}`}
          value={row.aliases.join(", ")}
          onChange={(event) =>
            onChange({
              aliases: event.target.value
                .split(",")
                .map((alias) => alias.trim())
                .filter(Boolean),
            })
          }
        />
      </td>
      <td>
        <select
          className={inputClass}
          aria-label={`Policy for ${row.name}`}
          value={row.policy}
          onChange={(event) => onChange({ policy: event.target.value })}
        >
          <option value="allow">allow</option>
          <option value="block">block</option>
          <option value="watch">watch</option>
        </select>
      </td>
      <td>
        <input
          className={inputClass}
          aria-label={`Note for ${row.name}`}
          value={row.note}
          onChange={(event) => onChange({ note: event.target.value })}
        />
      </td>
      <td>
        <input
          type="checkbox"
          aria-label={`Enabled policy ${row.name}`}
          checked={row.enabled}
          onChange={(event) => onChange({ enabled: event.target.checked })}
        />
      </td>
    </tr>
  )
}

const inputClass =
  "w-full rounded border border-(--border) bg-transparent px-2 py-1 focus:border-(--accent) focus:outline-none"
