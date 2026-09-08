import { renderToStaticMarkup } from "react-dom/server"
import { describe, expect, it } from "vitest"

import { RunDetailSchema } from "../api-contracts"
import { RecordedBrief } from "./Reading"

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

describe("recorded brief", () => {
  it("keeps the complete recorded message, including sports and health notes", () => {
    const detail = recorded(
      "- [Recorded headline](https://example.test/news)\n\n## Sports\n\nFinal score: 2 to 1.\n\nOne source could not be checked.",
    )
    const html = renderToStaticMarkup(<RecordedBrief detail={detail} />)
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
})
