import { act } from "react"
import { expect, it, vi } from "vitest"

import type { RunSummary, RunsList } from "../api-contracts"
import { navigate, runHref } from "../navigation"
import {
  button,
  click,
  detail,
  element,
  flush,
  key,
  setupRunNavigationTest,
  summary,
  typeSearch,
} from "../navigation-test-utils"

const { getRun, listRuns, open } = setupRunNavigationTest()

it.each(["/deliveries", "/runs"])(
  "loads explicit off-page IDs directly on %s, with exact links between saved HTML and evidence",
  async (path) => {
    await open(runHref(path, "archived / id"))
    expect(listRuns).not.toHaveBeenCalled()
    expect(getRun).toHaveBeenCalledWith(
      "archived / id",
      expect.any(AbortSignal),
    )
    const destination = path === "/runs" ? "/" : "/runs"
    const exactLink = element(".folio-page-heading a")
    expect(exactLink.getAttribute("href")).toBe(
      runHref(destination, "archived / id"),
    )
    await click(exactLink)
    await flush()
    expect(window.location.pathname).toBe(destination)
    expect(new URLSearchParams(window.location.search).get("run")).toBe(
      "archived / id",
    )
    expect(element(".folio-page-heading a").getAttribute("href")).toBe(
      runHref(destination === "/" ? "/runs" : "/", "archived / id"),
    )
    if (destination === "/")
      expect(element("iframe").getAttribute("srcdoc")).toContain(
        "Saved archived / id",
      )
  },
)

it.each(["/", "/runs"])(
  "shows an unknown explicit ID error on %s without falling back to latest",
  async (path) => {
    vi.mocked(getRun).mockRejectedValue(new Error("Run not found: unknown"))
    await open(runHref(path, "unknown"))
    expect(element('[role="alert"]').textContent).toContain(
      "Run not found: unknown",
    )
    expect(listRuns).not.toHaveBeenCalled()
    expect(getRun).toHaveBeenCalledTimes(1)
    expect(document.querySelector("iframe")).toBeNull()
    await click(element(".folio-picker-trigger"))
    await typeSearch("latest")
    expect(element('[role="alert"]').textContent).toContain("unknown")
    expect(new URLSearchParams(window.location.search).get("run")).toBe(
      "unknown",
    )
    await key("Escape")
    expect(element('[role="alert"]').textContent).toContain("unknown")
  },
)

it("treats an empty explicit ID as an error, not an absent selection", async () => {
  vi.mocked(getRun).mockRejectedValue(new Error("Run ID is empty"))
  await open("/?run=")
  expect(getRun).toHaveBeenCalledWith("", expect.any(AbortSignal))
  expect(listRuns).not.toHaveBeenCalled()
  expect(element('[role="alert"]').textContent).toBe("Run ID is empty")
})

it("asks the server for latest delivery, while Activity retains failed and unsent runs", async () => {
  vi.mocked(listRuns).mockImplementation(async (options) => ({
    runs: [
      summary(
        options?.delivered ? "delayed-delivery" : "new-failure",
        Boolean(options?.delivered),
      ),
    ],
    next_before: null,
  }))
  vi.mocked(getRun).mockImplementation(async (id) =>
    detail(summary(id, id === "delayed-delivery")),
  )
  await open("/")
  expect(listRuns).toHaveBeenCalledWith(
    { delivered: true, search: "" },
    expect.any(AbortSignal),
  )
  expect(element("iframe").getAttribute("srcdoc")).toContain(
    "Saved delayed-delivery",
  )
  expect(element(".folio-page-heading a").getAttribute("href")).toBe(
    "/runs?run=delayed-delivery",
  )
  await act(() => navigate("/runs"))
  await flush()
  await flush()
  expect(listRuns).toHaveBeenCalledWith(
    { delivered: false, search: "" },
    expect.any(AbortSignal),
  )
  expect(element(".folio-evidence-heading").textContent).toContain(
    "Not sent · failed",
  )
  expect(document.querySelector(".folio-page-heading a")).toBeNull()
  await act(() => navigate("/?run=new-failure"))
  await flush()
  expect(document.body.textContent).toContain("This run has no delivered brief")
  expect(element(".folio-page-heading a").getAttribute("href")).toBe(
    "/runs?run=new-failure",
  )
  expect(document.querySelector("iframe")).toBeNull()
})

it("distinguishes empty archives from retrieval errors", async () => {
  vi.mocked(listRuns).mockResolvedValue({ runs: [], next_before: null })
  await open("/")
  expect(document.body.textContent).toContain("No sent briefs yet")
  vi.mocked(listRuns).mockRejectedValue(new Error("Archive unavailable"))
  await act(() => navigate("/runs"))
  await flush()
  expect(element('[role="alert"]').textContent).toContain("Archive unavailable")
  expect(getRun).not.toHaveBeenCalled()
})

it("loads older pages explicitly, searches beyond loaded records with fresh cursors, and commits only the chosen result", async () => {
  vi.mocked(listRuns).mockImplementation(async (options) => {
    if (options?.search)
      return { runs: [summary("archive-match")], next_before: null }
    return options?.before
      ? { runs: [summary("older")], next_before: null }
      : { runs: [summary("newest")], next_before: "page-cursor" }
  })
  await open("/?run=selected")
  await click(element(".folio-picker-trigger"))
  expect(listRuns).toHaveBeenCalledTimes(1)
  expect(document.querySelectorAll('[role="option"]')).toHaveLength(1)
  await click(button("Load older"))
  expect(listRuns).toHaveBeenLastCalledWith(
    { delivered: true, search: "", before: "page-cursor" },
    expect.any(AbortSignal),
  )
  expect(
    [...document.querySelectorAll('[role="option"] small')].map(
      (option) => option.textContent,
    ),
  ).toEqual(["newest · Summary newest", "older · Summary older"])
  for (const search of ["2020-01-01", "% literal_summary"]) {
    await typeSearch(search)
    expect(listRuns).toHaveBeenLastCalledWith(
      { delivered: true, search },
      expect.any(AbortSignal),
    )
    expect(document.querySelectorAll('[role="option"]')).toHaveLength(1)
    expect(window.location.search).toBe("?run=selected")
  }
  await key("Enter")
  expect(window.location.search).toBe("?run=archive-match")
  expect(document.activeElement).toBe(element(".folio-picker-trigger"))
  expect(document.querySelector("dialog")).toBeNull()
  await flush()
  expect(element(".folio-page-heading a").getAttribute("href")).toBe(
    "/runs?run=archive-match",
  )
})

it("aborts superseded searches and ignores their late results", async () => {
  let abandonedSignal: AbortSignal | undefined
  let finishAbandoned:
    | ((value: { runs: RunSummary[]; next_before: null }) => void)
    | undefined
  vi.mocked(listRuns).mockImplementation(async (options, signal) => {
    if (options?.search === "abandoned") {
      abandonedSignal = signal
      return new Promise<RunsList>((resolve) => {
        finishAbandoned = resolve
      })
    }
    return { runs: [summary(options?.search || "newest")], next_before: null }
  })
  await open("/runs?run=selected")
  await click(element(".folio-picker-trigger"))
  await typeSearch("abandoned")
  expect(abandonedSignal?.aborted).toBe(false)
  await typeSearch("current")
  expect(abandonedSignal?.aborted).toBe(true)
  await act(() =>
    finishAbandoned?.({ runs: [summary("stale")], next_before: null }),
  )
  await flush()
  expect(element('[role="option"]').textContent).toContain("current")
  expect(document.querySelector('[role="option"]')?.textContent).not.toContain(
    "stale",
  )
  expect(window.location.search).toBe("?run=selected")
})

it("cancels an unneeded archive request when the picker closes", async () => {
  let archiveSignal: AbortSignal | undefined
  vi.mocked(listRuns).mockImplementation(async (_, signal) => {
    archiveSignal = signal
    return new Promise((_, reject) => {
      signal?.addEventListener("abort", () => reject(signal.reason), {
        once: true,
      })
    })
  })
  await open("/runs?run=selected")
  await click(element(".folio-picker-trigger"))
  expect(archiveSignal?.aborted).toBe(false)
  await key("Escape")
  expect(archiveSignal?.aborted).toBe(true)
  expect(document.querySelector("dialog")).toBeNull()
})

it("retains loaded records on an older-page failure and retries that cursor", async () => {
  vi.mocked(listRuns)
    .mockResolvedValueOnce({
      runs: [summary("newest")],
      next_before: "cursor",
    })
    .mockRejectedValueOnce(new Error("Older page unavailable"))
    .mockResolvedValueOnce({ runs: [summary("oldest")], next_before: null })
  await open("/runs?run=selected")
  await click(element(".folio-picker-trigger"))
  await click(button("Load older"))
  expect(element('[role="alert"]').textContent).toBe("Older page unavailable")
  expect(document.querySelectorAll('[role="option"]')).toHaveLength(1)
  await click(button("Retry"))
  expect(listRuns).toHaveBeenLastCalledWith(
    { delivered: false, search: "", before: "cursor" },
    expect.any(AbortSignal),
  )
  expect(document.querySelectorAll('[role="option"]')).toHaveLength(2)
  expect(document.querySelector('[role="alert"]')).toBeNull()
})

it("passes cancellation to direct record requests when selection changes", async () => {
  let signal: AbortSignal | undefined
  vi.mocked(getRun).mockImplementation(async (id, requestSignal) => {
    if (id === "waiting") {
      signal = requestSignal
      return new Promise((_, reject) => {
        requestSignal?.addEventListener(
          "abort",
          () => reject(requestSignal.reason),
          { once: true },
        )
      })
    }
    return detail(summary(id))
  })
  await open("/?run=waiting")
  expect(signal?.aborted).toBe(false)
  await act(() => navigate("/?run=ready"))
  await flush()
  expect(signal?.aborted).toBe(true)
  expect(element("iframe").getAttribute("srcdoc")).toContain("Saved ready")
})
