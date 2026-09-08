import type { Source } from "../api-contracts"
import { Field } from "./ui"

export interface SourceFieldsProps {
  draft: Source
  update: (patch: Partial<Source>) => void
}

export function SourceFlags({ draft, update }: SourceFieldsProps) {
  return (
    <div className="folio-checks">
      {(
        [
          { key: "enabled", label: "Enabled" },
          { key: "always_report", label: "Always included" },
        ] as const
      ).map(({ key, label }) => (
        <label className="folio-check" key={key}>
          <input
            type="checkbox"
            checked={draft[key]}
            onChange={(event) => update({ [key]: event.target.checked })}
          />
          {label}
        </label>
      ))}
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
        hint={
          draft.kind === "sports_schedule"
            ? "Changing type resets the sports options."
            : undefined
        }
      >
        <select
          value={draft.kind}
          onChange={(event) => {
            const kind = event.target.value
            update({
              kind,
              schedule_format: kind === "sports_schedule" ? "espn" : "",
              schedule_filter: "all",
              api_key: "",
            })
          }}
        >
          <option value="rss">RSS feed</option>
          <option value="atom">Atom feed</option>
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

export function PolicyFields({ draft, update }: SourceFieldsProps) {
  return (
    <>
      <Field label="Include when">
        <select
          value={draft.threshold}
          onChange={(event) => update({ threshold: event.target.value })}
        >
          <option value="always">Always</option>
          <option value="medium">Medium importance</option>
          <option value="high">High importance</option>
          <option value="audit">Review only</option>
        </select>
      </Field>
      <Field label="Priority">
        <input
          type="text"
          required
          value={draft.priority_rank}
          onChange={(event) => update({ priority_rank: event.target.value })}
        />
      </Field>
      <Field
        label="Shared duplicate group"
        wide
        hint="Check for duplicates across sources in this group."
      >
        <input
          value={draft.dedup_group}
          onChange={(event) => update({ dedup_group: event.target.value })}
        />
      </Field>
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
          label="Schedule format"
          hint={
            draft.schedule_format === "riot"
              ? "Changing format clears the match filter and API key."
              : undefined
          }
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
  return (
    <div className="folio-form-grid">
      <Field label="Link cleanup">
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
      <Field label="Publisher name from">
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
  )
}
