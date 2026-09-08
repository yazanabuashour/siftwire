import type { RunDetail } from "../api-contracts"
import { StoryReceipt } from "./Reading"

function sourceName(detail: RunDetail, key: string): string {
  return (
    [...detail.must_include, ...detail.candidates].find(
      (story) => story.source_key === key,
    )?.source_label ?? key
  )
}

export function Candidates({ detail }: { detail: RunDetail }) {
  return (
    <>
      {[
        { label: "Always included", items: detail.must_include },
        { label: "Considered", items: detail.candidates },
      ].map((group) => (
        <section className="folio-candidate-group" key={group.label}>
          <h3>
            {group.label} <span>{group.items.length}</span>
          </h3>
          {group.items.length === 0 && (
            <p className="folio-empty">
              No {group.label.toLowerCase()} recorded.
            </p>
          )}
          {group.items.map((story) => (
            <article
              className="folio-candidate"
              key={`${story.source_key}-${story.url}`}
            >
              <p className="folio-muted">
                {story.source_label} ·{" "}
                {story.selected ? "Included" : "Not included"}
              </p>
              <h4>
                <a href={story.url} target="_blank" rel="noreferrer">
                  {story.title}
                </a>
              </h4>
              <StoryReceipt story={story} />
            </article>
          ))}
        </section>
      ))}
    </>
  )
}

export function Dropped({ detail }: { detail: RunDetail }) {
  return (
    <>
      {detail.dropped.length === 0 && (
        <p className="folio-empty">No skipped stories.</p>
      )}
      {detail.dropped.map((drop) => (
        <article className="folio-drop" key={`${drop.source_key}-${drop.url}`}>
          <p className="folio-muted">
            {sourceName(detail, drop.source_key)} ·{" "}
            {drop.reason === "outlet_blocked"
              ? "Publisher blocked"
              : drop.reason === "already_delivered"
                ? "Already sent"
                : drop.reason.replaceAll("_", " ")}
          </p>
          <h3>
            <a href={drop.url} target="_blank" rel="noreferrer">
              {drop.title}
            </a>
          </h3>
          <details>
            <summary>Details</summary>
            <p className="folio-muted">Source ID: {drop.source_key}</p>
            <pre>{JSON.stringify(drop.detail, null, 2)}</pre>
          </details>
        </article>
      ))}
    </>
  )
}

export function Fetch({ detail }: { detail: RunDetail }) {
  return (
    <>
      {detail.fetch.length === 0 && (
        <p className="folio-empty">No source checks recorded.</p>
      )}
      <ul className="folio-fetch-list">
        {detail.fetch.map((entry) => (
          <li key={entry.source_key}>
            <div>
              <strong>{sourceName(detail, entry.source_key)}</strong>
              <span className="folio-status">
                {entry.status === "ok" ? "Checked" : entry.status}
              </span>
            </div>
            <p>
              {entry.items} stories, {entry.new_items} new
            </p>
            {entry.error && <p className="folio-error">{entry.error}</p>}
          </li>
        ))}
      </ul>
    </>
  )
}
