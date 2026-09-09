import type { Source } from "../api-contracts"
import { reportingLabels } from "../reporting"
import {
  reportingLabel,
  reportingPatch,
  sourceType,
  sourceTypePatch,
  type FeedReportingMode,
} from "./source-validation"
import { Field } from "./ui"

export interface SourceFieldsProps {
  draft: Source
  update: (patch: Partial<Source>) => void
}

export function SourceFlags({ draft, update }: SourceFieldsProps) {
  return (
    <div className="folio-checks">
      <label className="folio-check">
        <input
          type="checkbox"
          checked={draft.enabled}
          onChange={(event) => update({ enabled: event.target.checked })}
        />
        Enabled
      </label>
      <span className="folio-muted">Paused sources are not fetched.</span>
    </div>
  )
}

export function IdentityFields({ draft, update }: SourceFieldsProps) {
  return (
    <>
      <Field label="Name">
        <input
          required
          value={draft.label}
          onChange={(event) => update({ label: event.target.value })}
        />
      </Field>
      <Field
        label="Type"
        hint="Changing type resets the sports format, match filter and public API key. Feed accepts RSS and Atom."
      >
        <select
          value={draft.kind === "rss" ? "feed" : draft.kind}
          onChange={(event) => update(sourceTypePatch(event.target.value))}
        >
          {!["rss", "github_release", "sports_schedule"].includes(
            draft.kind,
          ) && (
            <option value={draft.kind} disabled>
              Unsupported saved type: {draft.kind}
            </option>
          )}
          <option value="feed">Feed</option>
          <option value="github_release">GitHub releases</option>
          <option value="sports_schedule">Sports schedule</option>
        </select>
      </Field>
      <Field label="Topic" wide>
        <input
          required
          value={draft.section}
          onChange={(event) => update({ section: event.target.value })}
        />
      </Field>
      {draft.kind === "github_release" ? (
        <Field
          label="Repository"
          wide
          hint={
            draft.url
              ? `Saving uses this repository, not the saved address: ${draft.url}`
              : undefined
          }
        >
          <input
            required
            value={draft.repo}
            placeholder="owner/name"
            onChange={(event) => update({ repo: event.target.value })}
          />
        </Field>
      ) : (
        <Field
          label="Address"
          wide
          hint={
            draft.kind === "sports_schedule"
              ? "Public schedule endpoint. SiftWire adds the date window for ESPN scoreboards."
              : undefined
          }
        >
          <input
            type="url"
            required
            value={draft.url}
            onChange={(event) => update({ url: event.target.value })}
          />
        </Field>
      )}
    </>
  )
}

export function PolicyFields({
  draft,
  update,
  mode,
  onMode,
}: SourceFieldsProps & {
  mode: string | undefined
  onMode: (mode: FeedReportingMode) => void
}) {
  const legacy = !["always", "high", "medium"].includes(draft.threshold)
  if (
    !legacy &&
    (draft.kind === "github_release" || draft.kind === "sports_schedule")
  )
    return (
      <div className="config-reporting">
        <p>
          {reportingLabel(
            draft.kind === "sports_schedule" ? "sports" : "required",
          )}
        </p>
        <p className="folio-muted">
          {draft.kind === "sports_schedule"
            ? "Sports recur within the configured date window. Reporting is fixed for this type."
            : "GitHub releases require eligible new releases. Reporting is fixed for this type."}
        </p>
      </div>
    )
  return (
    <div className="config-reporting">
      <Field
        label="Reporting"
        hint="Required feeds still report only eligible new items."
      >
        <select
          value={legacy ? "legacy" : (mode ?? "")}
          onChange={(event) => {
            const next = event.target.value
            if (
              next !== "required" &&
              next !== "highlights" &&
              next !== "major"
            )
              return
            onMode(next)
            update(reportingPatch(next))
          }}
        >
          {legacy ? (
            <option value="legacy" disabled>
              Saved reporting: {mode ?? "unavailable"} / {draft.threshold}
            </option>
          ) : (
            mode !== "required" &&
            mode !== "highlights" &&
            mode !== "major" && (
              <option value={mode ?? ""} disabled>
                {reportingLabel(mode)}
              </option>
            )
          )}
          <option value="required">{reportingLabels.required}</option>
          <option value="highlights">{reportingLabels.highlights}</option>
          <option value="major">{reportingLabels.major}</option>
        </select>
      </Field>
      {legacy && (
        <p className="folio-muted">
          This saved reporting policy is no longer writable. Choose current
          reporting before saving. Saving preserves the Enabled setting.
        </p>
      )}
    </div>
  )
}

export function PreferenceFields({ draft, update }: SourceFieldsProps) {
  return (
    <>
      <Field
        label="Source preference"
        hint="Lower numbers prefer this source for duplicate stories and sort ordinary items. The default is 0. This is not article importance."
      >
        <input
          type="text"
          required
          value={draft.priority_rank}
          onChange={(event) => update({ priority_rank: event.target.value })}
        />
      </Field>
      {sourceType(draft.kind) === "feed" && (
        <Field
          label="Title-matching group"
          wide
          hint="Restricts fuzzy same-run title/topic matching for optional feeds. Equal URLs can still collapse across groups. Recent-delivery suppression ignores groups and preference. Required items bypass these ordinary filters."
        >
          <input
            value={draft.dedup_group}
            onChange={(event) => update({ dedup_group: event.target.value })}
          />
        </Field>
      )}
    </>
  )
}

export function ScheduleFields({ draft, update }: SourceFieldsProps) {
  if (draft.kind !== "sports_schedule") return null
  return (
    <fieldset className="folio-form-group">
      <legend>Sports schedule</legend>
      <div className="folio-form-grid">
        <Field
          label="Provider / format"
          hint="Changing format resets the match filter and clears the public API key."
        >
          <select
            value={draft.schedule_format}
            onChange={(event) =>
              update({
                schedule_format: event.target.value,
                schedule_filter: "all",
                api_key: "",
              })
            }
          >
            {!["espn", "espn_scoreboard", "riot"].includes(
              draft.schedule_format,
            ) && (
              <option value={draft.schedule_format} disabled>
                Unsupported saved format: {draft.schedule_format || "empty"}
              </option>
            )}
            <option value="espn">ESPN schedule</option>
            <option value="espn_scoreboard">ESPN scoreboard</option>
            <option value="riot">Riot matches</option>
          </select>
        </Field>
        {draft.schedule_format === "riot" && (
          <>
            <Field label="Matches">
              <select
                value={draft.schedule_filter}
                onChange={(event) =>
                  update({ schedule_filter: event.target.value })
                }
              >
                {!["all", "standings_top_two"].includes(
                  draft.schedule_filter,
                ) && (
                  <option value={draft.schedule_filter} disabled>
                    Unsupported saved filter: {draft.schedule_filter}
                  </option>
                )}
                <option value="all">All matches</option>
                <option value="standings_top_two">
                  Top two standings positions
                </option>
              </select>
            </Field>
            <Field
              label="Public API key"
              wide
              hint="The public frontend key used by the League of Legends esports website. Do not enter private credentials."
            >
              <input
                required
                value={draft.api_key}
                onChange={(event) => update({ api_key: event.target.value })}
              />
            </Field>
          </>
        )}
      </div>
    </fieldset>
  )
}
