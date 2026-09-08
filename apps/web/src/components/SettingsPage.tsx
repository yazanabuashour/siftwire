import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"

import { fetchConfig, setOptions, type BriefOptionsInput } from "../api-client"
import type { ConfigResult } from "../api-contracts"
import { Field, PageHeading } from "./ui"

type SettingsDraft = {
  max_delivery_items: string
  sports_pre_game_days: string
  sports_post_game_days: string
  sports_timezone: string
}

function settingsDraft(values: Record<string, string>): SettingsDraft {
  // Defaults and the story limit follow crates/siftwire/src/storage/mod.rs.
  return {
    max_delivery_items: values["max_delivery_items"] ?? "7",
    sports_pre_game_days: values["sports_pre_game_days"] ?? "7",
    sports_post_game_days: values["sports_post_game_days"] ?? "3",
    sports_timezone: values["sports_timezone"] ?? "America/Chicago",
  }
}

function settingsError(draft: SettingsDraft): string {
  const stories = Number(draft.max_delivery_items)
  if (!Number.isInteger(stories) || stories < 1 || stories > 25) {
    return "Stories per brief must be a whole number from 1 to 25."
  }
  for (const raw of [draft.sports_pre_game_days, draft.sports_post_game_days]) {
    if (!raw.trim() || !Number.isSafeInteger(Number(raw)) || Number(raw) < 0) {
      return "Sports windows must be whole numbers of zero or more days."
    }
  }
  // The runner owns date-range checks and its supported time-zone database.
  return draft.sports_timezone.trim() ? "" : "Enter a sports time zone."
}

function useSettingsEditor() {
  const config = useQuery({
    queryKey: ["config"],
    queryFn: ({ signal }) => fetchConfig(signal),
  })
  const client = useQueryClient()
  const [draft, setDraft] = useState<SettingsDraft | null>(null)
  const [error, setError] = useState("")
  const configured = settingsDraft(config.data?.runtime_config ?? {})
  // Follow refetches until the user starts editing.
  const values = draft ?? configured
  const dirty = JSON.stringify(values) !== JSON.stringify(configured)
  const unavailable = !config.data || config.isError || config.data.rejected
  const save = useMutation({
    mutationFn: async (next: BriefOptionsInput) => {
      const result = await setOptions(next)
      if (result.rejected) {
        throw new Error(result.summary || "Settings changes were rejected.")
      }
      return result
    },
    onSuccess: async (_result, next) => {
      client.setQueryData<ConfigResult>(["config"], (current) =>
        current
          ? {
              ...current,
              runtime_config: {
                ...current.runtime_config,
                max_delivery_items: String(next.maxDeliveryItems),
                sports_pre_game_days: String(next.sportsPreGameDays),
                sports_post_game_days: String(next.sportsPostGameDays),
                sports_timezone: next.sportsTimezone,
              },
            }
          : current,
      )
      setDraft(null)
      await client.invalidateQueries({ queryKey: ["config"] })
    },
  })
  const update = (key: keyof SettingsDraft, value: string): void => {
    setDraft({ ...values, [key]: value })
    setError("")
    save.reset()
  }
  const submit = (): void => {
    if (unavailable || !dirty || save.isPending) return
    const problem = settingsError(values)
    setError(problem)
    if (problem) return
    save.mutate({
      maxDeliveryItems: Number(values.max_delivery_items),
      sportsPreGameDays: Number(values.sports_pre_game_days),
      sportsPostGameDays: Number(values.sports_post_game_days),
      sportsTimezone: values.sports_timezone.trim(),
    })
  }
  return { config, values, dirty, unavailable, save, update, error, submit }
}

export default function SettingsPage() {
  const { config, values, dirty, unavailable, save, update, error, submit } =
    useSettingsEditor()
  return (
    <>
      <PageHeading title="Settings" />
      {config.isPending && <output>Loading settings…</output>}
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
        <div className="folio-settings-layout">
          <form
            className="folio-settings-form"
            onSubmit={(event) => {
              event.preventDefault()
              submit()
            }}
          >
            <SettingsFields
              draft={values}
              disabled={save.isPending}
              update={update}
            />
            <TechnicalDetails config={config.data} />
            {(error || save.isError) && (
              <p className="folio-error" role="alert">
                {error || save.error?.message}
              </p>
            )}
            <div className="folio-save-bar">
              <button
                className="folio-primary"
                type="submit"
                disabled={unavailable || !dirty || save.isPending}
              >
                {save.isPending ? "Saving…" : "Save settings"}
              </button>
              <output>
                {save.isPending
                  ? "Saving settings…"
                  : dirty
                    ? "Unsaved changes"
                    : save.isSuccess
                      ? "Settings saved."
                      : ""}
              </output>
            </div>
          </form>
        </div>
      )}
    </>
  )
}

function SettingsFields({
  draft,
  disabled,
  update,
}: {
  draft: SettingsDraft
  disabled: boolean
  update: (key: keyof SettingsDraft, value: string) => void
}) {
  return (
    <>
      <fieldset className="folio-form-group" disabled={disabled}>
        <legend>Brief</legend>
        <Field
          label="Stories per brief"
          hint="Preferred size, from 1 to 25. Always-included stories and sports can exceed it."
        >
          <input
            type="number"
            min="1"
            max="25"
            step="1"
            required
            value={draft.max_delivery_items}
            onChange={(event) =>
              update("max_delivery_items", event.target.value)
            }
          />
        </Field>
      </fieldset>
      <fieldset className="folio-form-group" disabled={disabled}>
        <legend>Sports</legend>
        <div className="folio-form-grid">
          <Field label="Days before a game">
            <input
              type="number"
              min="0"
              step="1"
              required
              value={draft.sports_pre_game_days}
              onChange={(event) =>
                update("sports_pre_game_days", event.target.value)
              }
            />
          </Field>
          <Field label="Days after a game">
            <input
              type="number"
              min="0"
              step="1"
              required
              value={draft.sports_post_game_days}
              onChange={(event) =>
                update("sports_post_game_days", event.target.value)
              }
            />
          </Field>
          <Field
            label="Sports time zone"
            wide
            hint="For example, America/Chicago or UTC."
          >
            <input
              required
              value={draft.sports_timezone}
              onChange={(event) =>
                update("sports_timezone", event.target.value)
              }
            />
          </Field>
        </div>
        <button
          type="button"
          onClick={() =>
            update(
              "sports_timezone",
              Intl.DateTimeFormat().resolvedOptions().timeZone,
            )
          }
        >
          Use browser time zone
        </button>
      </fieldset>
    </>
  )
}

function TechnicalDetails({ config }: { config: ConfigResult }) {
  const editable = settingsDraft(config.runtime_config)
  return (
    <details className="folio-advanced">
      <summary>Technical details</summary>
      <p>Read-only configuration and storage paths.</p>
      <dl className="folio-receipt">
        {Object.entries(config.runtime_config)
          .filter(([key]) => !Object.hasOwn(editable, key))
          .map(([key, value]) => (
            <div key={key}>
              <dt>{key.replaceAll("_", " ")}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        <div>
          <dt>Database</dt>
          <dd>{config.paths.database_path}</dd>
        </div>
        <div>
          <dt>Data directory</dt>
          <dd>{config.paths.data_dir}</dd>
        </div>
      </dl>
    </details>
  )
}
