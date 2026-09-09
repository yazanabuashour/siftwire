import { act } from "react"
import { beforeEach, expect, it, vi } from "vitest"

import App from "./App"
import {
  AppLink,
  navigate,
  routeFor,
  runHref,
  selectRun,
  useNavigation,
} from "./navigation"
import {
  click,
  element,
  flush,
  setupNavigationTest,
} from "./navigation-test-utils"

const render = setupNavigationTest()

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn<typeof fetch>(async (input) => {
      const url = new URL(String(input), window.location.origin)
      if (url.pathname === "/api/v1/config")
        return Response.json({
          runner_protocol: "siftwire-runner/v4",
          capabilities: ["current-news/v1"],
          rejected: false,
          paths: { database_path: "data/config.sqlite", data_dir: "data" },
          runtime_config: {
            max_delivery_items: "7",
            sports_pre_game_days: "7",
            sports_post_game_days: "3",
            sports_timezone: "UTC",
          },
        })
      if (url.pathname.startsWith("/api/v1/runs/"))
        return Response.json({
          run: {
            run_id: decodeURIComponent(
              url.pathname.slice("/api/v1/runs/".length),
            ),
            started_at: "2026-01-01T00:00:00Z",
            delivered_at: "2026-01-01T01:00:00Z",
            dry_run: false,
            status: "ok",
            summary: "Saved run",
          },
          must_include: [],
          candidates: [],
          dropped: [],
          fetch: [],
          sent_items: [],
        })
      throw new Error(`Unexpected request: ${url}`)
    }),
  )
})

function Location() {
  const { page, runId } = useNavigation()
  return (
    <output>
      {page}:{runId ?? "latest"}
    </output>
  )
}

function link(text: string) {
  const found = [...document.querySelectorAll("a")].find(
    (anchor) => anchor.textContent === text,
  )
  if (!found) throw new Error(`Missing link: ${text}`)
  return found
}

it.each([
  ["/", "overview"],
  ["/deliveries", "overview"],
  ["/overview", "overview"],
  ["/runs", "runs"],
  ["/activity/", "runs"],
  ["/outlets", "outlets"],
])("keeps %s as a supported entry point", (path, page) => {
  expect(routeFor(path)).toBe(page)
})

it("keeps selected IDs across Brief, Activity, Sources subnavigation and Settings", async () => {
  vi.spyOn(window, "scrollTo").mockImplementation(() => {})
  window.history.replaceState(
    {},
    "",
    runHref("/deliveries", "older / ? & brief"),
  )
  await render(<App />)
  expect(element("main h1").textContent).toBe("Brief")
  expect(
    document.querySelectorAll('[aria-label="Main navigation"] a'),
  ).toHaveLength(4)
  expect(link("Brief").getAttribute("aria-current")).toBe("page")
  for (const label of [
    "Activity",
    "Sources",
    "Publisher rules",
    "Feeds",
    "Settings",
    "Brief",
  ]) {
    expect(new URL(link(label).href).searchParams.get("run")).toBe(
      "older / ? & brief",
    )
    await click(link(label))
    expect(new URLSearchParams(window.location.search).get("run")).toBe(
      "older / ? & brief",
    )
    if (label === "Sources") {
      expect(link("Feeds").getAttribute("aria-current")).toBe("page")
    }
    if (label === "Publisher rules") {
      expect(link("Sources").getAttribute("aria-current")).toBe("page")
      expect(link("Publisher rules").getAttribute("aria-current")).toBe("page")
    }
  }
  expect(window.location.pathname).toBe("/")
  expect(document.title).toBe("Brief | SiftWire")
})

it("pushes only changed URLs, preserves unrelated query/hash, and responds to real back and forward events", async () => {
  window.history.replaceState({}, "", "/runs?filter=kept#details")
  await render(<Location />)
  const push = vi.spyOn(window.history, "pushState")
  await act(() => selectRun("older/id"))
  expect(element("output").textContent).toBe("runs:older/id")
  expect(window.location.search).toBe("?filter=kept&run=older%2Fid")
  expect(window.location.hash).toBe("#details")
  await act(() => selectRun("older/id"))
  expect(push).toHaveBeenCalledTimes(1)
  await act(() => navigate(runHref("/", "older/id")))
  expect(element("output").textContent).toBe("overview:older/id")
  await act(async () => {
    window.history.back()
    await vi.runAllTimersAsync()
  })
  await flush()
  expect(element("output").textContent).toBe("runs:older/id")
  await act(async () => {
    window.history.forward()
    await vi.runAllTimersAsync()
  })
  await flush()
  expect(element("output").textContent).toBe("overview:older/id")
  await act(() => selectRun(undefined))
  expect(element("output").textContent).toBe("overview:latest")
  expect(runHref("/", "")).toBe("/?run=")
})

it("intercepts ordinary app links but leaves modified clicks, targets, downloads and foreign links to the browser", async () => {
  await render(
    <>
      <AppLink href="/runs?run=exact">Activity</AppLink>
      <AppLink href="/runs?run=target" target="_blank">
        New tab
      </AppLink>
      <AppLink href="/runs?run=file" download>
        Download
      </AppLink>
      <AppLink href="https://example.org/">External</AppLink>
      <AppLink href="/settings" onClick={(event) => event.preventDefault()}>
        Cancelled
      </AppLink>
      <Location />
    </>,
  )
  const push = vi.spyOn(window.history, "pushState")
  // Observe the React handler, then suppress jsdom's unimplemented native navigation.
  const prevented: boolean[] = []
  const preventNative = (event: Event) => {
    prevented.push(event.defaultPrevented)
    event.preventDefault()
  }
  document.addEventListener("click", preventNative)
  try {
    for (const init of [
      { ctrlKey: true },
      { metaKey: true },
      { shiftKey: true },
      { altKey: true },
      { button: 1 },
    ])
      await click(link("Activity"), init)
    for (const label of ["New tab", "Download", "External"])
      await click(link(label))
    expect(prevented).toEqual(Array.from({ length: 8 }, () => false))
    expect(push).not.toHaveBeenCalled()
    await click(link("Cancelled"))
    expect(push).not.toHaveBeenCalled()
    await click(link("Activity"))
    expect(prevented.at(-1)).toBe(true)
    expect(push).toHaveBeenCalledTimes(1)
    expect(element("output").textContent).toBe("runs:exact")
  } finally {
    document.removeEventListener("click", preventNative)
  }
})
