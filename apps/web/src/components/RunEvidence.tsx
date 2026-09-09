import type { RunDetail, RunItem } from "../api-contracts"
import { StoryReceipt } from "./Reading"

const deliveryLabels = {
  sent: "Sent",
  sent_as_sports: "Sent in sports section",
  not_selected: "Not selected",
  not_delivered: "No confirmed delivery",
  unknown: "Delivery outcome not recorded",
} satisfies Record<RunItem["delivery_status"], string>

function sourceName(detail: RunDetail, key: string): string {
  return (
    [...detail.must_include, ...detail.candidates].find(
      (item) => item.source_key === key && item.source_label,
    )?.source_label || key
  )
}

export function Candidates({ detail }: { detail: RunDetail }) {
  return (
    <>
      <Annotations detail={detail} />
      {[
        { label: "Required at collection", items: detail.must_include },
        { label: "Optional candidates", items: detail.candidates },
      ].map((group) => (
        <section className="folio-evidence-group" key={group.label}>
          <h2>
            {group.label} <span>({group.items.length})</span>
          </h2>
          {group.items.length === 0 && (
            <p className="folio-muted">None recorded.</p>
          )}
          {group.items.map((story) => (
            <article className="folio-story" key={story.id}>
              <div className="folio-story-meta">
                <span>{story.source_label || story.source_key}</span>
                <span className="folio-pill">
                  {deliveryLabels[story.delivery_status]}
                </span>
              </div>
              <h3>
                <a href={story.url} target="_blank" rel="noreferrer">
                  {story.title}
                </a>
              </h3>
              <StoryReceipt story={story} />
            </article>
          ))}
        </section>
      ))}
    </>
  )
}

function Annotations({ detail }: { detail: RunDetail }) {
  if (!detail.annotations.length) return null
  return (
    <details className="folio-evidence-group">
      <summary>Warnings and annotations ({detail.annotations.length})</summary>
      <p className="folio-muted">
        These are diagnostics, not proof that an item was excluded. Older
        records may not say whether an unresolved link was retained.
      </p>
      {detail.annotations.map((item) => (
        <article className="folio-story" key={item.id}>
          <div className="folio-story-meta">
            <span>
              {item.source_label || sourceName(detail, item.source_key)}
            </span>
            <span>
              {item.reason === "outlet_policy"
                ? "Publisher annotation"
                : item.reason.replaceAll("_", " ")}
            </span>
          </div>
          <h3>{item.title || "Source diagnostic"}</h3>
          <p className="folio-muted">
            {item.disposition === "retained"
              ? "Retained at this step"
              : "Outcome not recorded"}
          </p>
          <details>
            <summary>Recorded diagnostic</summary>
            <pre>{JSON.stringify(item.detail, null, 2)}</pre>
          </details>
        </article>
      ))}
    </details>
  )
}

const exclusionLabels = new Map([
  ["outlet_policy", "Blocked publisher"],
  ["outlet_blocked", "Blocked publisher"],
  ["duplicate", "Duplicate story"],
  ["recently_sent", "Sent recently"],
  ["already_delivered", "Sent recently"],
  ["unresolved", "Unresolved article link"],
])

export function Dropped({ detail }: { detail: RunDetail }) {
  if (!detail.dropped.length)
    return <p className="folio-empty">No exclusions were recorded.</p>
  const groups = new Map<string, RunDetail["dropped"]>()
  for (const item of detail.dropped) {
    const group = groups.get(item.reason) ?? []
    group.push(item)
    groups.set(item.reason, group)
  }
  return (
    <>
      <p className="folio-muted">
        These items were excluded before editorial selection. Unselected
        optional candidates remain in the candidate list.
      </p>
      {[...groups].map(([reason, items]) => (
        <details className="folio-evidence-group" key={reason}>
          <summary>
            {exclusionLabels.get(reason) || reason.replaceAll("_", " ")} (
            {items.length})
          </summary>
          {items.map((item) => (
            <article className="folio-story" key={item.id}>
              <p className="folio-story-source">
                {item.source_label || sourceName(detail, item.source_key)}
              </p>
              <h3>{item.title}</h3>
              <details>
                <summary>Recorded reason</summary>
                <pre>{JSON.stringify(item.detail, null, 2)}</pre>
              </details>
            </article>
          ))}
        </details>
      ))}
    </>
  )
}

export function FetchStatuses({ detail }: { detail: RunDetail }) {
  if (!detail.fetch.length)
    return <p className="folio-empty">No source checks were recorded.</p>
  return (
    <div className="folio-fetch-list">
      {detail.fetch.map((status) => (
        <div className="folio-fetch" key={status.source_key}>
          <div>
            <strong>
              {status.source_label || sourceName(detail, status.source_key)}
            </strong>
            {status.error && <p>{status.error}</p>}
            {status.current_news && (
              <>
                <p className="folio-muted">
                  Publication window: {status.current_news.since} to{" "}
                  {status.current_news.until}
                </p>
                <p className="folio-muted">
                  Excluded: {status.current_news.stale_items} stale ·{" "}
                  {status.current_news.undated_items} undated ·{" "}
                  {status.current_news.future_items} future-dated
                </p>
              </>
            )}
          </div>
          <div>
            <span
              className={`folio-pill ${status.status === "error" || status.status === "failed" ? "folio-pill-warn" : ""}`}
            >
              {status.status.replaceAll("_", " ")}
            </span>
            <span className="folio-muted">
              {status.current_news
                ? `${status.current_news.eligible_items} eligible`
                : status.new_items === null
                  ? "New count unavailable"
                  : `${status.new_items} new`}{" "}
              · {status.items} fetched
            </span>
          </div>
        </div>
      ))}
    </div>
  )
}
