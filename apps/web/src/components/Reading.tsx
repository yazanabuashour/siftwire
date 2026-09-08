import { createContext, useContext, type ReactNode } from "react"
import Markdown from "react-markdown"

import type { RunDetail, RunItem } from "../api-contracts"
import { useRun } from "./run-queries"
import { dateLabel, ErrorNote } from "./ui"

const RunContext = createContext<RunDetail | null>(null)

function StoryLink({
  href,
  children,
}: {
  href?: string | undefined
  children?: ReactNode
}) {
  const detail = useContext(RunContext)
  const story =
    detail?.must_include.find((item) => item.url === href) ??
    detail?.candidates.find((item) => item.url === href)
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

export function SentBrief({ runId }: { runId: string }) {
  const detail = useRun(runId)
  if (detail.isPending)
    return <output className="folio-empty">Loading brief…</output>
  if (detail.isError) return <ErrorNote error={detail.error} />
  return <RecordedBrief detail={detail.data} />
}

export function RecordedBrief({ detail }: { detail: RunDetail }) {
  const evidence = [...detail.must_include, ...detail.candidates]
  if (detail.run.message?.trim()) {
    return (
      <div className="folio-markdown folio-recorded-brief">
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
        const story = evidence.find((entry) => entry.url === item.url)
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
          ["Source", story.source_label],
          ["Source ID", story.source_key],
          ["Type", story.kind.replaceAll("_", " ") || "Not recorded"],
          ["Include when", story.threshold || "Not recorded"],
          ["Priority", story.priority_rank],
          ["Always included", story.always_report ? "Yes" : "No"],
          ["Published", dateLabel(story.published_at, true)],
          ["Publisher", story.outlet || "Not recorded"],
        ].map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
    </details>
  )
}
