import { act } from "react"
import { expect, it, vi } from "vitest"

import type { ConfigResult, ReportingMode, Source } from "../api-contracts"
import { reportingLabels } from "../reporting"
import { configQuery } from "./config-query"
import {
  button,
  change,
  choose,
  click,
  client,
  configuration,
  host,
  input,
  pendingResponse,
  render,
  request,
  select,
  setupConfigTest,
  settle,
} from "./config-test-utils"
import { emptySource } from "./source-validation"
import SourceCollection from "./SourceCollection"
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

it.each([
  { threshold: "high", reporting: "major" },
  { threshold: "always", reporting: "required" },
  { threshold: "medium", reporting: "highlights" },
] satisfies {
  threshold: Source["threshold"]
  reporting: ReportingMode
}[])(
  "preserves $threshold, rank and processing fields on unrelated saves",
  ({ threshold, reporting }) => {
    const source = { ...stored, threshold }
    const onSave = editor(source, reporting)
    expect(select("Reporting").value).toBe(reporting)
    expect(select("Type").value).toBe("feed")
    change("Name", "Renamed")
    click("Save source")
    expect(onSave).toHaveBeenCalledWith({ ...source, label: "Renamed" })
    expect(host.textContent).not.toContain("Always included")
  },
)

it("changes raw reporting only on explicit selection and retains the feed choice across type changes", () => {
  const onSave = editor()
  choose("Reporting", "major")
  choose("Type", "sports_schedule")
  expect(host.textContent).toContain(reportingLabels.sports)
  expect(host.textContent).not.toContain("Link resolution")
  expect(select("Provider / format").closest("details")).toBeNull()
  choose("Provider / format", "riot")
  expect(input("Public API key").closest("details")).toBeNull()
  choose("Type", "github_release")
  expect(host.textContent).toContain(reportingLabels.required)
  expect(host.textContent).not.toContain("Provider / format")
  expect(host.textContent).not.toContain("Link resolution")
  choose("Type", "feed")
  expect(select("Reporting").value).toBe("major")
  click("Save source")
  expect(onSave).toHaveBeenCalledWith({
    ...stored,
    kind: "rss",
    threshold: "high",
    schedule_format: "",
    schedule_filter: "all",
    api_key: "",
  })
})

it("preserves a source draft through refetch and server rejection, then merges the normalized saved source", async () => {
  const configured: ConfigResult = {
    ...configuration(),
    sources: [stored],
    source_reporting: { example: "required" },
  }

  client.setQueryData(configQuery.queryKey, configured)
  render(<SourcesPage />)
  click("Edit Example")
  change("Name", "Draft")
  await act(async () =>
    client.setQueryData(configQuery.queryKey, {
      ...configured,
      sources: [{ ...stored, label: "Remote name" }],
    }),
  )
  await settle()
  expect(input("Name").value).toBe("Draft")
  request.mockResolvedValueOnce(
    Response.json({ rejected: true, summary: "Source rejected" }),
  )
  click("Save source")
  await settle()
  expect(input("Name").value).toBe("Draft")
  expect(host.textContent).toContain("Source rejected")
  const normalized = { ...stored, label: "Normalized name" }
  const write = pendingResponse()
  click("Save source")
  await settle()
  expect(button("Close dialog").disabled).toBe(true)
  await act(async () =>
    host
      .querySelector("dialog")
      ?.dispatchEvent(new Event("cancel", { cancelable: true })),
  )
  expect(host.querySelector("dialog")).not.toBeNull()
  expect(request).toHaveBeenLastCalledWith(
    "/api/v1/sources",
    expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ ...stored, label: "Draft" }),
    }),
  )
  const lateRead = pendingResponse()
  let refresh: Promise<void> | undefined
  act(() => {
    refresh = client.refetchQueries(configQuery)
  })
  expect(lateRead.signal.aborted).toBe(false)
  write.respond(
    Response.json({
      ...configuration(),
      outlets: [],
      runtime_config: {},
      sources: [normalized],
      source_reporting: { example: "required" },
    }),
  )
  await settle()
  expect(host.querySelector("dialog")).toBeNull()
  expect(host.textContent).toContain("Normalized name")
  lateRead.respond(Response.json(configured))
  await refresh
  await settle()
  expect(host.querySelector("article h2")?.textContent).toBe("Normalized name")
  expect(client.getQueryData(configQuery.queryKey)).toEqual({
    ...configured,
    sources: [normalized],
  })
  expect(lateRead.signal.aborted).toBe(true)
})

it("pauses without rewriting raw policy and removes the last source using the server's remaining collection", async () => {
  const configured: ConfigResult = {
    ...configuration(),
    sources: [stored],
    source_reporting: { example: "required" },
  }

  client.setQueryData(configQuery.queryKey, configured)
  request.mockResolvedValueOnce(
    Response.json({
      ...configured,
      sources: [{ ...stored, enabled: false }],
    }),
  )
  render(<SourcesPage />)
  const toggle = host.querySelector<HTMLInputElement>('input[type="checkbox"]')
  act(() => toggle?.click())
  await settle()
  expect(request).toHaveBeenCalledWith(
    "/api/v1/sources",
    expect.objectContaining({
      method: "POST",
      body: JSON.stringify({ ...stored, enabled: false }),
    }),
  )
  const write = pendingResponse()
  click("Remove Example")
  click("Remove source")
  await settle()
  expect(request).toHaveBeenLastCalledWith(
    "/api/v1/sources/example",
    expect.objectContaining({ method: "DELETE" }),
  )
  const lateRead = pendingResponse()
  let refresh: Promise<void> | undefined
  act(() => {
    refresh = client.refetchQueries(configQuery)
  })
  expect(lateRead.signal.aborted).toBe(false)
  write.respond(
    Response.json({ ...configuration(), sources: [], source_reporting: {} }),
  )
  await settle()
  expect(host.textContent).toContain("example removed.")
  lateRead.respond(Response.json(configured))
  await refresh
  await settle()
  expect(host.querySelector("dialog")).toBeNull()
  expect(host.querySelectorAll("article")).toHaveLength(0)
  expect(client.getQueryData(configQuery.queryKey)).toEqual(configuration())
  expect(lateRead.signal.aborted).toBe(true)
})

it("searches feeds without altering the collection", () => {
  const sources: Source[] = [
    { ...stored, label: "Zulu" },
    { ...stored, key: "rss", kind: "rss", label: "Alpha" },
    { ...stored, key: "release", kind: "github_release", label: "Release" },
  ]

  const onEdit = vi.fn()
  const onRemove = vi.fn()
  const onToggle = vi.fn()

  render(
    <SourceCollection
      sources={sources}
      reporting={{}}
      busy={false}
      savingKey={undefined}
      onEdit={onEdit}
      onRemove={onRemove}
      onToggle={onToggle}
    />,
  )
  choose("Type", "feed")
  expect(
    [...host.querySelectorAll("article h2")].map((row) => row.textContent),
  ).toEqual(["Alpha", "Zulu"])
  change("Search sources", "Zulu")
  expect(host.querySelectorAll("article")).toHaveLength(1)
  expect(host.querySelector("article h2")?.textContent).toBe("Zulu")
  change("Search sources", "")
  expect(
    [...host.querySelectorAll("article h2")].map((row) => row.textContent),
  ).toEqual(["Alpha", "Zulu"])
  expect(onEdit).not.toHaveBeenCalled()
  expect(onRemove).not.toHaveBeenCalled()
  expect(onToggle).not.toHaveBeenCalled()
})
