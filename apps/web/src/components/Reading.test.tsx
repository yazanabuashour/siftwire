import { renderToStaticMarkup } from "react-dom/server"
import { expect, it } from "vitest"

import { RunDetailSchema, RunItemSchema } from "../api-contracts"
import { RecordedBrief } from "./Reading"
import { Candidates, Dropped } from "./RunEvidence"

function recorded(message: string | null) {
  return RunDetailSchema.parse({
    run: {
      run_id: "recorded-run",
      started_at: "2026-09-07T08:30:00Z",
      dry_run: false,
      status: "ok",
      summary: "",
      delivered_at: "2026-09-07T08:31:00Z",
      message,
    },
    must_include: [],
    candidates: [],
    dropped: [],
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

it("keeps the complete recorded message, including sports and health notes", () => {
  const detail = recorded(
    "- [Recorded headline](https://example.test/news)\n\n## Sports\n\nFinal score: 2 to 1.\n\nOne source could not be checked.",
  )
  const html = renderToStaticMarkup(<RecordedBrief detail={detail} />)
  expect(html).toContain("No saved email HTML")
  expect(html).toContain("Recorded headline")
  expect(html).toContain("Final score: 2 to 1.")
  expect(html).toContain("One source could not be checked.")
  expect(html).not.toContain("Stored fallback")
})

it("uses saved links only when the message is missing", () => {
  const html = renderToStaticMarkup(<RecordedBrief detail={recorded(null)} />)
  expect(html).toContain("full message was not recorded")
  expect(html).toContain("Stored fallback")
  expect(html).toContain('href="https://example.test/story"')
})

it("does not attach a different fixture's receipt to a shared URL", () => {
  const detail = recorded("[A saved card](https://example.test/card)")
  detail.must_include = ["First fixture", "Second fixture"].map((title) =>
    RunItemSchema.parse({
      id: title,
      source_key: title,
      source_label: `${title} source`,
      title,
      url: "https://example.test/card",
      selected: false,
    }),
  )
  const html = renderToStaticMarkup(<RecordedBrief detail={detail} />)
  expect(html).toContain("A saved card")
  expect(html).not.toContain("First fixture source")
  expect(html).not.toContain("Second fixture source")
})

it("uses recorded policy and delivery outcomes and separates annotations from exclusions", () => {
  const detail = recorded(null)
  detail.must_include = [
    RunItemSchema.parse({
      id: "sports-item",
      source_key: "fixture",
      source_label: "Fictional schedule",
      kind: "sports_schedule",
      title: "Upcoming fixture",
      url: "https://example.test/card",
      threshold: "audit",
      always_report: false,
      reporting: "sports",
      selected: false,
      delivery_status: "sent_as_sports",
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
  expect(candidates).toContain("Sent in sports section")
  expect(candidates).toContain("Recurring sports updates")
  expect(candidates).not.toContain("Not included")
  expect(candidates).not.toContain("Always included")
  expect(candidates).toContain("Retained warning")
  expect(renderToStaticMarkup(<Dropped detail={detail} />)).not.toContain(
    "Retained warning",
  )
})

it("makes recorded images opt-in links rather than remote loads", () => {
  const html = renderToStaticMarkup(
    <RecordedBrief
      detail={recorded("![Chart](https://example.test/chart.png)")}
    />,
  )
  expect(html).not.toContain("<img")
  expect(html).toContain('href="https://example.test/chart.png"')
  expect(html).toContain("Chart")
})
