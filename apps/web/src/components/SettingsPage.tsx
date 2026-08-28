import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import { setOptions, type BriefOptionsInput } from "../api-client"
import BriefOptionsCard from "./BriefOptionsCard"
import { Card, ErrorNote, formatWhen, PageHeader, useConfig } from "./shared"

export default function SettingsPage() {
  const config = useConfig()
  const client = useQueryClient()
  const values = config.data?.runtime_config
  const configuredMaxItems = readPositiveNumber(
    values?.["max_delivery_items"],
    7,
  )
  const configuredPreGameDays = readNonNegativeNumber(
    values?.["sports_pre_game_days"],
    7,
  )
  const configuredPostGameDays = readNonNegativeNumber(
    values?.["sports_post_game_days"],
    3,
  )
  const configuredTimezone = values?.["sports_timezone"] ?? "America/Chicago"
  const [options, setOptionsState] = useState<BriefOptionsInput | null>(null)
  useEffect(() => {
    if (config.data && options === null) {
      setOptionsState({
        maxDeliveryItems: configuredMaxItems,
        sportsPreGameDays: configuredPreGameDays,
        sportsPostGameDays: configuredPostGameDays,
        sportsTimezone: configuredTimezone,
      })
    }
  }, [
    config.data,
    configuredMaxItems,
    configuredPostGameDays,
    configuredPreGameDays,
    configuredTimezone,
    options,
  ])
  const dirty =
    options !== null &&
    (options.maxDeliveryItems !== configuredMaxItems ||
      options.sportsPreGameDays !== configuredPreGameDays ||
      options.sportsPostGameDays !== configuredPostGameDays ||
      options.sportsTimezone !== configuredTimezone)
  const save = useMutation({
    mutationFn: (next: BriefOptionsInput) => setOptions(next),
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
          <BriefOptionsCard
            value={options}
            dirty={dirty}
            saving={save.isPending}
            error={save.error}
            onChange={setOptionsState}
            onSave={() => {
              if (options) {
                const normalized = {
                  ...options,
                  sportsTimezone: options.sportsTimezone.trim(),
                }
                setOptionsState(normalized)
                save.mutate(normalized)
              }
            }}
          />
          <RuntimeCard values={values ?? {}} />
          <StorageCard
            database={config.data?.paths.database_path}
            directory={config.data?.paths.data_dir}
          />
        </div>
      )}
    </>
  )
}

function readPositiveNumber(raw: string | undefined, fallback: number): number {
  const value = Number(raw)
  return Number.isFinite(value) && value > 0 ? value : fallback
}

function readNonNegativeNumber(
  raw: string | undefined,
  fallback: number,
): number {
  const value = Number(raw)
  return Number.isFinite(value) && value >= 0 ? value : fallback
}

function configLabel(key: string): string {
  const words = key.replaceAll("_", " ")
  return words.charAt(0).toUpperCase() + words.slice(1)
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
