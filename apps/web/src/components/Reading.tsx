import { createContext, useContext, type ReactNode } from "react"
import Markdown from "react-markdown"

import type { RunDetail, RunItem } from "../api-contracts"
import { reportingLabel } from "../reporting"
import EmailBrief from "./EmailBrief"
import { dateLabel } from "./ui"

const RunContext = createContext<RunDetail | null>(null)

function linkEvidence(detail: RunDetail, url: string, title?: string) {
  const matches = [...detail.must_include, ...detail.candidates].filter(
    (item) => item.url === url,
  )
  const exact = matches.filter((item) => item.title === title)
  return exact.length === 1
    ? exact[0]
    : matches.length === 1
      ? matches[0]
      : undefined
}

function StoryLink({
  href,
  children,
}: {
  href?: string | undefined
  children?: ReactNode
}) {
  const detail = useContext(RunContext)
  const story = detail && href ? linkEvidence(detail, href) : undefined
  return (
    <>
      {story && (
        <span className="folio-story-source">
          {story.outlet || story.source_label}
        </span>
      )}
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        className={
          story?.kind === "github_release" ? "folio-release" : undefined
        }
      >
        {children}
      </a>
    </>
  )
}

function ImageLink({
  src,
  alt,
}: {
  src?: string | undefined
  alt?: string | undefined
}) {
  return (
    <a href={src} target="_blank" rel="noreferrer">
      {alt || "Image"}
    </a>
  )
}

const markdownComponents = { a: StoryLink, img: ImageLink }

export function RecordedBrief({ detail }: { detail: RunDetail }) {
  if (detail.delivery_html?.trim())
    return <EmailBrief key={detail.run.run_id} html={detail.delivery_html} />
  if (detail.run.message?.trim()) {
    return (
      <div className="folio-markdown folio-recorded-brief">
        <p className="folio-muted">
          No saved email HTML for this brief. Showing the recorded text.
        </p>
        <RunContext.Provider value={detail}>
          <Markdown components={markdownComponents}>
            {detail.run.message}
          </Markdown>
        </RunContext.Provider>
      </div>
    )
  }
  if (!detail.sent_items.length)
    return <p className="folio-empty">No message or sent stories recorded.</p>
  return (
    <div className="folio-brief">
      <p className="folio-muted">
        The full message was not recorded. These are the saved story links.
      </p>
      {detail.sent_items.map((item) => {
        const story = linkEvidence(detail, item.url, item.title)
        return (
          <article className="folio-story" key={item.url}>
            {story && (
              <p className="folio-story-source">
                {story.outlet || story.source_label}
              </p>
            )}
            <h2>
              <a href={item.url} target="_blank" rel="noreferrer">
                {item.title}
              </a>
            </h2>
          </article>
        )
      })}
    </div>
  )
}

export function StoryReceipt({ story }: { story: RunItem }) {
  return (
    <details>
      <summary>Details</summary>
      <dl className="folio-receipt">
        {[
          ["Source", story.source_label || "Label not recorded"],
          ["Source ID", story.source_key],
          ["Type", story.kind.replaceAll("_", " ") || "Not recorded"],
          [
            "Reporting",
            story.reporting ? reportingLabel(story.reporting) : "Not recorded",
          ],
          ["Source preference", story.priority_rank],
          ["Published", dateLabel(story.published_at, true)],
          ["Publisher", story.outlet || "Not recorded"],
        ].map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <p className="folio-muted">
        Source preference affects duplicate selection and ordering, not article
        importance. These settings belong to this recorded run.
      </p>
    </details>
  )
}
