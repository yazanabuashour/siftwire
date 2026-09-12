import { SourceSchema } from "../api-contracts"
import type { SourceFieldsProps } from "./SourceFields"
import { Field } from "./ui"

export default function SourceHandling({ draft, update }: SourceFieldsProps) {
  const currentNews =
    draft.kind === "rss" && ["high", "medium"].includes(draft.threshold)
  if (draft.kind !== "rss") return null
  return (
    <fieldset className="folio-form-group">
      <legend>Feed processing</legend>
      <p className="folio-muted">
        {currentNews
          ? "Optional feeds use a rolling 24-hour publication window, not newly seen items. Stale, undated, and future-dated items are excluded."
          : "Link resolution can change URL identity and delivery eligibility."}{" "}
        Publisher identification supplies the name used by publisher rules.
      </p>
      {currentNews && (
        <p className="folio-muted">
          Optional feeds keep original feed links; Google News decoding is not
          active. The saved link resolution choice is retained when you save and
          applies only to Required feeds.
        </p>
      )}
      <p className="folio-muted">
        {currentNews
          ? "Publication eligibility does not mean the item is new or will be selected."
          : draft.url_canonicalization === "google_news_article_url"
            ? "If link resolution fails, the original item remains eligible."
            : "Links stay as supplied by the feed."}
      </p>
      <div className="folio-form-grid">
        <Field label="Link resolution">
          <select
            value={draft.url_canonicalization || "none"}
            disabled={currentNews}
            onChange={(event) =>
              update({
                url_canonicalization:
                  SourceSchema.shape.url_canonicalization.parse(
                    event.target.value,
                  ),
              })
            }
          >
            <option value="none">None</option>
            <option value="google_news_article_url">
              Google News decoding
            </option>
          </select>
        </Field>
        <Field label="Publisher identification">
          <select
            value={draft.outlet_extraction || "none"}
            onChange={(event) =>
              update({
                outlet_extraction: SourceSchema.shape.outlet_extraction.parse(
                  event.target.value,
                ),
              })
            }
          >
            <option value="none">None</option>
            <option value="title_suffix">End of title</option>
          </select>
        </Field>
      </div>
    </fieldset>
  )
}
