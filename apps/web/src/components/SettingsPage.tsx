import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { setOptions } from "../api-client"
import { Card, ErrorNote, useConfig } from "./shared"

export default function SettingsPage() {
  const config = useConfig()
  const client = useQueryClient()
  const [maxItems, setMaxItems] = useState<number | null>(null)
  useEffect(() => {
    if (config.data && maxItems === null) {
      const raw = Number(config.data.runtime_config["max_delivery_items"])
      setMaxItems(Number.isFinite(raw) && raw > 0 ? raw : 7)
    }
  }, [config.data, maxItems])
  const save = useMutation({
    mutationFn: (value: number) => setOptions(value),
    onSuccess: () => void client.invalidateQueries({ queryKey: ["config"] }),
  })

  return (
    <div className="flex max-w-xl flex-col gap-4">
      <Card title="Brief options">
        <form
          className="flex items-end gap-2 text-sm"
          onSubmit={(event) => {
            event.preventDefault()
            if (maxItems !== null) save.mutate(maxItems)
          }}
        >
          <label className="flex flex-col gap-1">
            <span className="text-(--muted)">max_delivery_items</span>
            <input
              type="number"
              min={1}
              max={25}
              className="w-24 rounded border border-(--border) bg-transparent px-2 py-1"
              value={maxItems ?? ""}
              onChange={(event) => setMaxItems(Number(event.target.value))}
            />
          </label>
          <button
            type="submit"
            className="rounded border border-(--accent) px-3 py-1 hover:bg-(--accent) hover:text-black"
          >
            Save
          </button>
        </form>
        {save.isPending ? <p className="mt-2 text-(--muted)">Saving…</p> : null}
        {save.isError ? <ErrorNote error={save.error} /> : null}
        <p className="mt-2 text-xs text-(--muted)">
          Between 1 and 25. Applied to future brief runs.
        </p>
      </Card>
      <Card title="Runtime configuration">
        <dl className="text-sm">
          {Object.entries(config.data?.runtime_config ?? {}).map(
            ([key, value]) => (
              <div
                key={key}
                className="flex justify-between gap-4 border-b border-(--border) py-1"
              >
                <dt className="font-mono">{key}</dt>
                <dd className="text-(--muted)">{value}</dd>
              </div>
            ),
          )}
        </dl>
      </Card>
    </div>
  )
}
