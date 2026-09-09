import { sourceType } from "./source-validation"
import type { SourceFieldsProps } from "./SourceFields"
import { Field } from "./ui"

export default function SourceHandling({ draft, update }: SourceFieldsProps) {
  const supportedResolution = ["", "none", "google_news_article_url"].includes(
    draft.url_canonicalization,
  )
  const supportedExtraction = ["", "none", "title_suffix"].includes(
    draft.outlet_extraction,
  )
  if (
    sourceType(draft.kind) !== "feed" &&
    supportedResolution &&
    supportedExtraction
  )
    return null
  return (
    <fieldset className="folio-form-group">
      <legend>Feed processing</legend>
      <p className="folio-muted">
        Link resolution can change URL identity and delivery eligibility.
        Publisher identification supplies the name used by publisher rules.
      </p>
      <p className="folio-muted">
        {!supportedResolution || !supportedExtraction
          ? "Choose supported processing options before saving this legacy source."
          : draft.url_canonicalization === "google_news_article_url"
            ? "If link resolution fails, the original item remains eligible."
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
            {!supportedResolution && (
              <option value={draft.url_canonicalization} disabled>
                Unsupported saved resolution: {draft.url_canonicalization}
              </option>
            )}
            <option value="none">None</option>
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
            {!supportedExtraction && (
              <option value={draft.outlet_extraction} disabled>
                Unsupported saved identification: {draft.outlet_extraction}
              </option>
            )}
            <option value="none">None</option>
            <option value="title_suffix">End of title</option>
          </select>
        </Field>
      </div>
    </fieldset>
  )
}
