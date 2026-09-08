import type { ReportingMode, Source } from "../api-contracts"
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
          value={sourceType(draft.kind)}
          onChange={(event) => update(sourceTypePatch(event.target.value))}
        >
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
      <Field
        label="Address"
        wide
        hint={
          draft.kind === "github_release"
            ? "Use a release address or a repository below."
            : draft.kind === "sports_schedule"
              ? "Public schedule endpoint. SiftWire adds the date window for ESPN scoreboards."
              : undefined
        }
      >
        <input
          type="url"
          required={draft.kind !== "github_release" || !draft.repo.trim()}
          value={draft.url}
          onChange={(event) => update({ url: event.target.value })}
        />
      </Field>
      {draft.kind === "github_release" && (
        <Field label="Repository" wide>
          <input
            required={!draft.url.trim()}
            value={draft.repo}
            placeholder="owner/name"
            onChange={(event) => update({ repo: event.target.value })}
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
  mode: ReportingMode | undefined
  onMode: (mode: FeedReportingMode) => void
}) {
  if (sourceType(draft.kind) !== "feed")
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
        hint="Observe advances latest-seen state without offering items. It does not keep a backlog for later. Required feeds still report only eligible new items."
      >
        <select
          value={mode ?? ""}
          onChange={(event) => {
            const next = event.target.value
            if (
              next !== "required" &&
              next !== "highlights" &&
              next !== "major" &&
              next !== "observe"
            )
              return
            onMode(next)
            update(reportingPatch(next))
          }}
        >
          {!mode && (
            <option value="" disabled>
              Reporting unavailable
            </option>
          )}
          <option value="required">{reportingLabels.required}</option>
          <option value="highlights">{reportingLabels.highlights}</option>
          <option value="major">{reportingLabels.major}</option>
          <option value="observe">{reportingLabels.observe}</option>
        </select>
      </Field>
      {!mode && (
        <p className="folio-muted">
          The runner has not provided a reporting mode. Other edits preserve the
          stored policy; choose a mode only to change it.
        </p>
      )}
    </div>
  )
}

export function PreferenceFields({ draft, update }: SourceFieldsProps) {
  if (draft.kind === "sports_schedule")
    return (
      <p className="folio-wide folio-muted">
        Sports use fixture identity and their own order, not source preference
        or title-matching groups.
      </p>
    )
  return (
    <>
      <Field
        label="Source preference"
        hint="Lower wins duplicate representative choice and orders normal items. This is not article importance. Sports use fixture identity and their own order."
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
            value={draft.schedule_format || "espn"}
            onChange={(event) =>
              update({
                schedule_format: event.target.value,
                schedule_filter: "all",
                api_key: "",
              })
            }
          >
            <option value="espn">ESPN schedule</option>
            <option value="espn_scoreboard">ESPN scoreboard</option>
            <option value="espn_core">ESPN events</option>
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

export function HandlingFields({ draft, update }: SourceFieldsProps) {
  if (sourceType(draft.kind) !== "feed") return null
  return (
    <fieldset className="folio-form-group">
      <legend>Feed processing</legend>
      <p className="folio-muted">
        Link resolution can change URL identity and delivery eligibility.
        Publisher identification supplies the name used by publisher rules.
      </p>
      <p className="folio-muted">
        {draft.url_canonicalization && draft.url_canonicalization !== "none"
          ? draft.outlet_extraction === "url_host"
            ? "If link resolution fails, website-address identification omits the item."
            : "If link resolution fails, the original item remains eligible."
          : "Links stay as supplied by the feed."}
      </p>
      <div className="folio-form-grid">
        <Field label="Link resolution">
          <select
            value={draft.url_canonicalization || "none"}
            onChange={(event) =>
              update({ url_canonicalization: event.target.value })
            }
          >
            <option value="none">None</option>
            <option value="feedburner_redirect">Follow FeedBurner links</option>
            <option value="google_news_article_url">
              Use original Google News links
            </option>
          </select>
        </Field>
        <Field label="Publisher identification">
          <select
            value={draft.outlet_extraction || "none"}
            onChange={(event) =>
              update({ outlet_extraction: event.target.value })
            }
          >
            <option value="none">None</option>
            <option value="title_suffix">End of title</option>
            <option value="url_host">Website address</option>
            <option value="rss_source">Feed source</option>
          </select>
        </Field>
      </div>
    </fieldset>
  )
}
