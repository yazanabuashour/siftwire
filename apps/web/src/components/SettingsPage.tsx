import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { setOptions } from "../api-client"
import { Button, inputClass } from "./controls"
import { Card, ErrorNote, formatWhen, PageHeader, useConfig } from "./shared"

export default function SettingsPage() {
  const config = useConfig()
  const client = useQueryClient()
  const [maxItems, setMaxItems] = useState<number | null>(null)
  const configuredMax = readMaxItems(
    config.data?.runtime_config["max_delivery_items"],
  )
  useEffect(() => {
    if (config.data && maxItems === null) setMaxItems(configuredMax)
  }, [config.data, configuredMax, maxItems])
  const save = useMutation({
    mutationFn: (value: number) => setOptions(value),
    onSuccess: () => void client.invalidateQueries({ queryKey: ["config"] }),
  })

  return (
    <>
      <PageHeader
        title="Settings"
        subtitle="Brief behavior and runtime state."
      />
      {config.isError ? (
        <ErrorNote error={config.error} />
      ) : (
        <div className="flex max-w-2xl flex-col gap-4">
          <OptionsCard
            value={maxItems}
            dirty={maxItems !== null && maxItems !== configuredMax}
            saving={save.isPending}
            error={save.error}
            onChange={setMaxItems}
            onSave={() => {
              if (maxItems !== null) save.mutate(maxItems)
            }}
          />
          <RuntimeCard values={config.data?.runtime_config ?? {}} />
          <StorageCard
            database={config.data?.paths.database_path}
            directory={config.data?.paths.data_dir}
          />
        </div>
      )}
    </>
  )
}

function readMaxItems(raw: string | undefined): number {
  const value = Number(raw)
  return Number.isFinite(value) && value > 0 ? value : 7
}

function configLabel(key: string): string {
  const words = key.replaceAll("_", " ")
  return words.charAt(0).toUpperCase() + words.slice(1)
}

function OptionsCard({
  value,
  dirty,
  saving,
  error,
  onChange,
  onSave,
}: {
  value: number | null
  dirty: boolean
  saving: boolean
  error: unknown
  onChange: (value: number) => void
  onSave: () => void
}) {
  return (
    <Card title="Brief options">
      <form
        className="flex items-end gap-3 px-4 py-4"
        onSubmit={(event) => {
          event.preventDefault()
          onSave()
        }}
      >
        <label className="block">
          <span className="text-muted mb-1 block text-xs font-medium">
            Max delivery items
          </span>
          <input
            type="number"
            min={1}
            max={25}
            className={`${inputClass} w-24`}
            value={value ?? ""}
            onChange={(event) => onChange(Number(event.target.value))}
          />
        </label>
        <Button variant="primary" type="submit" disabled={saving || !dirty}>
          {saving ? "Saving…" : "Save"}
        </Button>
      </form>
      {error ? (
        <div className="px-4 pb-3">
          <ErrorNote error={error} />
        </div>
      ) : null}
      <p className="border-edge text-muted border-t px-4 py-2.5 text-xs">
        Between 1 and 25. Applied to future brief runs.
      </p>
    </Card>
  )
}

function RuntimeCard({ values }: { values: Record<string, string> }) {
  return (
    <Card title="Runtime configuration">
      <dl className="divide-edge divide-y text-sm">
        {Object.entries(values).map(([key, value]) => (
          <div
            key={key}
            className="flex items-center justify-between gap-4 px-4 py-2.5"
          >
            <dt className="text-muted text-xs" title={key}>
              {configLabel(key)}
            </dt>
            <dd className="text-ink font-mono text-xs">
              {key === "last_check" ? formatWhen(value) : value}
            </dd>
          </div>
        ))}
      </dl>
    </Card>
  )
}

function StorageCard({
  database,
  directory,
}: {
  database: string | undefined
  directory: string | undefined
}) {
  return (
    <Card title="Storage">
      <dl className="divide-edge divide-y text-sm">
        <StorageRow label="Database" value={database} />
        <StorageRow label="Data directory" value={directory} />
      </dl>
    </Card>
  )
}

function StorageRow({
  label,
  value,
}: {
  label: string
  value: string | undefined
}) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-2.5">
      <dt className="text-muted">{label}</dt>
      <dd className="text-ink truncate font-mono text-xs">
        {value ?? "Not available"}
      </dd>
    </div>
  )
}
