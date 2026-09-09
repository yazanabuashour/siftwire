import { act } from "react"
import { expect, it, vi } from "vitest"

import type { ReportingMode, Source } from "../api-contracts"
import { reportingLabels } from "../reporting"
import { configQuery } from "./config-query"
import {
  change,
  choose,
  click,
  client,
  configuration,
  host,
  render,
  request,
  select,
  setupConfigTest,
  settle,
} from "./config-test-utils"
import { emptySource } from "./source-validation"
import SourceEditor from "./SourceEditor"
import SourcesPage from "./SourcesPage"

setupConfigTest()

const stored: Source = {
  ...emptySource,
  key: "example",
  kind: "rss",
  label: "Example",
  section: "engineering",
  url: "https://example.test/feed",
  threshold: "always",
  priority_rank: "9223372036854775807",
  dedup_group: "Stored Group",
  url_canonicalization: "google_news_article_url",
  outlet_extraction: "title_suffix",
}

function editor(
  source = stored,
  reporting: ReportingMode | undefined = "required",
) {
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
  return onSave
}

it.each(["audit", "observe"])(
  "requires current reporting for a paused legacy %s source without enabling it",
  (threshold) => {
    const source = { ...stored, threshold, enabled: false }
    const onSave = editor(source, "observe")
    expect(select("Reporting").selectedOptions[0]?.textContent).toContain(
      threshold,
    )
    expect(
      [...select("Reporting").options].flatMap((option) =>
        option.disabled ? [] : [option.value],
      ),
    ).toEqual(["required", "highlights", "major"])
    change("Name", "Renamed")
    click("Save source")
    expect(onSave).not.toHaveBeenCalled()
    expect(host.textContent).toContain("Choose current reporting")
    choose("Reporting", "major")
    click("Save source")
    expect(onSave).toHaveBeenCalledWith({
      ...source,
      label: "Renamed",
      threshold: "high",
      enabled: false,
    })
  },
)

it("opens legacy observe toggles for explicit editing without writing or enabling", async () => {
  const source = { ...stored, threshold: "audit", enabled: false }
  client.setQueryData(configQuery.queryKey, {
    ...configuration(),
    sources: [source],
    source_reporting: { example: "observe" },
  })
  render(<SourcesPage />)
  expect(host.textContent).toContain(reportingLabels.observe)
  act(() =>
    host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.click(),
  )
  await settle()
  expect(host.querySelector("dialog")).not.toBeNull()
  expect(
    host.querySelector<HTMLInputElement>('dialog input[type="checkbox"]')
      ?.checked,
  ).toBe(false)
  expect(request).not.toHaveBeenCalled()
})

it.each(["atom", "future_feed"])(
  "shows saved %s but requires a current source type before saving",
  (kind) => {
    const source = { ...stored, kind, enabled: false }
    const onSave = editor(source)
    expect(select("Type").selectedOptions[0]?.textContent).toContain(kind)
    expect(
      [...select("Type").options].flatMap((option) =>
        option.disabled ? [] : [option.value],
      ),
    ).toEqual(["feed", "github_release", "sports_schedule"])
    click("Save source")
    expect(onSave).not.toHaveBeenCalled()
    choose("Type", "feed")
    click("Save source")
    expect(onSave).toHaveBeenCalledWith({ ...source, kind: "rss" })
  },
)

it("requires a release repository and never writes a custom release address", () => {
  const source = { ...stored, kind: "github_release", repo: "" }
  const onSave = editor(source)
  expect(host.textContent).not.toContain("Address")
  expect(host.textContent).toContain(source.url)
  click("Save source")
  expect(onSave).not.toHaveBeenCalled()
  change("Repository", "owner/name")
  click("Save source")
  expect(onSave).toHaveBeenCalledWith({
    ...source,
    repo: "owner/name",
    url: "",
  })
})

it.each(["rss_source", "url_host"])(
  "shows saved %s processing without offering it as a current choice",
  (outlet_extraction) => {
    const source = {
      ...stored,
      url_canonicalization: "feedburner_redirect",
      outlet_extraction,
    }
    const onSave = editor(source)
    expect(select("Link resolution").selectedOptions[0]?.textContent).toContain(
      "feedburner_redirect",
    )
    expect(
      select("Publisher identification").selectedOptions[0]?.textContent,
    ).toContain(outlet_extraction)
    click("Save source")
    expect(onSave).not.toHaveBeenCalled()
    choose("Link resolution", "none")
    choose("Publisher identification", "none")
    click("Save source")
    expect(onSave).toHaveBeenCalledWith({
      ...source,
      url_canonicalization: "none",
      outlet_extraction: "none",
    })
  },
)

it("requires a current schedule format while displaying the saved unsupported value", () => {
  const source = {
    ...stored,
    kind: "sports_schedule",
    schedule_format: "espn_core",
  }
  const onSave = editor(source, "sports")
  expect(select("Provider / format").selectedOptions[0]?.textContent).toContain(
    "espn_core",
  )
  click("Save source")
  expect(onSave).not.toHaveBeenCalled()
  choose("Provider / format", "espn_scoreboard")
  click("Save source")
  expect(onSave).toHaveBeenCalledWith({
    ...source,
    schedule_format: "espn_scoreboard",
  })
})
