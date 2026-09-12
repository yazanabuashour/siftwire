import { expect, it, vi } from "vitest"

import type { Source } from "../api-contracts"
import {
  change,
  choose,
  click,
  host,
  render,
  select,
  setupConfigTest,
} from "./config-test-utils"
import { emptySource } from "./source-validation"
import SourceEditor from "./SourceEditor"

setupConfigTest()

it.each([
  { threshold: "high", reporting: "major" },
  { threshold: "medium", reporting: "highlights" },
] as const)(
  "retains saved resolution while showing effective current-news behavior for $reporting feeds",
  ({ threshold, reporting }) => {
    const source: Source = {
      ...emptySource,
      key: "example",
      label: "Example",
      section: "engineering",
      url: "https://example.test/feed",
      threshold,
      url_canonicalization: "google_news_article_url",
    }
    const onSave = vi.fn()
    render(
      <SourceEditor
        source={source}
        sources={[source]}
        editing
        reporting={reporting}
        busy={false}
        saveError={null}
        onClose={vi.fn()}
        onSave={onSave}
      />,
    )
    expect(onSave).not.toHaveBeenCalled()
    expect(select("Link resolution").disabled).toBe(true)
    expect(select("Link resolution").value).toBe("google_news_article_url")
    expect(host.textContent).toContain("rolling 24-hour publication window")
    expect(host.textContent).toContain("Google News decoding is not active")
    expect(host.textContent).not.toContain("If link resolution fails")
    change("Name", "Renamed optional feed")
    click("Save source")
    expect(onSave).toHaveBeenLastCalledWith({
      ...source,
      label: "Renamed optional feed",
    })
    choose("Reporting", "required")
    expect(select("Link resolution").disabled).toBe(false)
    expect(select("Link resolution").value).toBe("google_news_article_url")
    expect(host.textContent).toContain("If link resolution fails")
    expect(host.textContent).not.toContain("rolling 24-hour")
    choose("Reporting", reporting)
    click("Save source")
    expect(onSave).toHaveBeenLastCalledWith({
      ...source,
      label: "Renamed optional feed",
    })
  },
)
