import { renderToStaticMarkup } from "react-dom/server"
import { expect, it } from "vitest"

import { RunDetailSchema, RunItemSchema } from "../api-contracts"
import { RecordedBrief } from "./Reading"
import { Candidates, Dropped, FetchStatuses } from "./RunEvidence"

function recorded(message: string | null) {
  return RunDetailSchema.parse({
    run: {
      run_id: "recorded-run",
      started_at: "2026-09-07T08:30:00Z",
      finished_at: "2026-09-07T08:31:00Z",
      dry_run: false,
      status: "ok",
      summary: "",
      delivered_at: "2026-09-07T08:31:00Z",
      message,
    },
    delivery_html: null,
    must_include: [],
    candidates: [],
    dropped: [],
    annotations: [],
    fetch: [],
    sent_items: [
      {
        title: "Stored fallback",
        url: "https://example.test/story",
        sent_at: "2026-09-07T08:31:00Z",
      },
    ],
  })
}

it("prefers the saved email in a script-disabled frame over reconstructed content", () => {
  const detail = recorded("Plain text fallback")
  detail.delivery_html =
    '<!doctype html><html><body><img src="https://images.example/club.png" alt=""><p>Saved sports card</p></body></html>'
  const html = renderToStaticMarkup(<RecordedBrief detail={detail} />)
  expect(html).toContain("<iframe")
  expect(html).toContain('title="Recorded email brief"')
  expect(html).toContain("Saved sports card")
  expect(html).toContain("https://images.example/club.png")
  expect(html).toContain(
    'sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox"',
  )
  expect(html).not.toContain("allow-scripts")
  expect(html).not.toContain("Plain text fallback")
  expect(html).not.toContain("Stored fallback")
})

it("reports missing saved HTML without interpreting recorded Markdown", () => {
  const detail = recorded(
    "- [Recorded headline](https://example.test/news)\n\n## Sports\n\nFinal score: 2 to 1.\n\nOne source could not be checked.",
  )
  const html = renderToStaticMarkup(<RecordedBrief detail={detail} />)
  expect(html).toContain("No saved email HTML")
  expect(html).not.toContain("Recorded headline")
  expect(html).not.toContain("Final score: 2 to 1.")
  expect(html).not.toContain("One source could not be checked.")
  expect(html).not.toContain("Stored fallback")
})

it("uses recorded policy and delivery outcomes and separates annotations from exclusions", () => {
  const detail = recorded(null)
  detail.must_include = [
    RunItemSchema.parse({
      id: "sports-item",
      source_key: "fixture",
      source_label: "Fictional schedule",
      kind: "sports_schedule",
      published_at: "",
      outlet: "",
      title: "Upcoming fixture",
      url: "https://example.test/card",
      priority_rank: "0",
      reporting: "sports",
      delivery_status: "sent",
    }),
  ]
  detail.annotations = [
    {
      source_key: "fixture",
      source_label: "",
      id: "warning-item",
      disposition: "retained",
      title: "Retained warning",
      url: "",
      reason: "unresolved",
      detail: { retained: true },
    },
  ]
  const candidates = renderToStaticMarkup(<Candidates detail={detail} />)
  expect(candidates).toContain("Sent")
  expect(candidates).toContain("Recurring sports updates")
  expect(candidates).not.toContain("Not included")
  expect(candidates).not.toContain("Always included")
  expect(candidates).toContain("Retained warning")
  expect(renderToStaticMarkup(<Dropped detail={detail} />)).not.toContain(
    "Retained warning",
  )
})

it("distinguishes current-news eligibility, required-source new counts, and failed checks", () => {
  const currentNews = {
    since: "2026-09-06T08:30:00Z",
    until: "2026-09-07T08:30:00Z",
    eligible_items: 4,
    stale_items: 3,
    undated_items: 2,
    future_items: 1,
  }
  const detail = RunDetailSchema.parse({
    ...recorded(null),
    fetch: [
      {
        source_key: "optional",
        source_label: "Optional feed",
        status: "ok",
        items: 10,
        new_items: null,
        current_news: currentNews,
      },
      {
        source_key: "release",
        source_label: "Releases",
        status: "ok",
        items: 7,
        new_items: 3,
      },
      {
        source_key: "required",
        source_label: "Required feed",
        status: "ok",
        items: 2,
        new_items: 0,
      },
      {
        source_key: "failed",
        source_label: "Failed feed",
        status: "error",
        error: "Feed unavailable",
        items: 0,
        new_items: null,
      },
    ],
  })
  expect(detail.fetch[0]?.current_news).toEqual(currentNews)
  expect(detail.fetch[1]?.current_news).toBeUndefined()
  const html = renderToStaticMarkup(<FetchStatuses detail={detail} />)
  const host = document.createElement("div")
  host.innerHTML = html
  const rows = [...host.querySelectorAll(".folio-fetch")].map(
    (row) => row.textContent,
  )
  expect(rows[0]).toContain("4 eligible · 10 fetched")
  expect(rows[0]).toContain(
    `Publication window: ${currentNews.since} to ${currentNews.until}`,
  )
  expect(rows[0]).toContain("Excluded: 3 stale · 2 undated · 1 future-dated")
  expect(rows[0]).not.toContain(" new")
  expect(rows[1]).toContain("3 new · 7 fetched")
  expect(rows[1]).not.toContain("Publication window")
  expect(rows[2]).toContain("0 new · 2 fetched")
  expect(rows[3]).toContain("Feed unavailable")
  expect(rows[3]).toContain("New count unavailable · 0 fetched")
})
