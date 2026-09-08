import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, type ReactNode } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, vi } from "vitest"

import type {
  getRun as getRunRequest,
  listRuns as listRunsRequest,
  RunListOptions,
} from "./api-client"
import { RunDetailSchema, type RunSummary } from "./api-contracts"
import OverviewPage from "./components/OverviewPage"
import RunsPage from "./components/RunsPage"
import { useNavigation } from "./navigation"

export function summary(id: string, delivered = true): RunSummary {
  return {
    run_id: id,
    started_at: "2026-01-01T09:00:00Z",
    finished_at: "2026-01-01T09:01:00Z",
    dry_run: false,
    status: delivered ? "ok" : "failed",
    summary: `Summary ${id}`,
    delivered_at: delivered ? "2026-01-02T09:00:00Z" : null,
    message: null,
  }
}

export function detail(run: RunSummary) {
  return RunDetailSchema.parse({
    run,
    delivery_html: run.delivered_at
      ? `<html><body><p>Saved ${run.run_id}</p></body></html>`
      : null,
    must_include: [],
    candidates: [],
    dropped: [],
    fetch: [],
    sent_items: [],
  })
}

function Pages() {
  const { page } = useNavigation()
  return (
    <div className="folio-app">
      {page === "runs" ? <RunsPage /> : <OverviewPage />}
    </div>
  )
}

export function setupRunNavigationTest() {
  const render = setupNavigationTest()
  const getRun = vi.fn<typeof getRunRequest>()
  const listRuns = vi.fn<typeof listRunsRequest>()
  let client: QueryClient
  beforeEach(() => {
    // Keep the real HTTP client and query hooks; only the network is synthetic.
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>(async (input, init) => {
        const url = new URL(String(input), window.location.origin)
        const signal = init?.signal ?? undefined
        if (url.pathname === "/api/v1/runs") {
          const options: RunListOptions = {
            delivered: url.searchParams.get("delivered") === "true",
            search: url.searchParams.get("search") ?? "",
          }
          const before = url.searchParams.get("before")
          if (before !== null) options.before = before
          return Response.json(await listRuns(options, signal))
        }
        if (url.pathname.startsWith("/api/v1/runs/"))
          return Response.json(
            await getRun(
              decodeURIComponent(url.pathname.slice("/api/v1/runs/".length)),
              signal,
            ),
          )
        throw new Error(`Unexpected request: ${url}`)
      }),
    )
    client = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } },
    })
    listRuns.mockResolvedValue({ runs: [summary("latest")], next_before: null })
    getRun.mockImplementation(async (id) => detail(summary(id)))
  })
  afterEach(() => {
    client.clear()
    vi.resetAllMocks()
  })
  return {
    getRun,
    listRuns,
    open: async (path: string) => {
      window.history.replaceState({}, "", path)
      await render(
        <QueryClientProvider client={client}>
          <Pages />
        </QueryClientProvider>,
      )
      await flush()
    },
  }
}

export function button(text: string) {
  const found = [...document.querySelectorAll("button")].find(
    (item) => item.textContent === text,
  )
  if (!found) throw new Error(`Missing button: ${text}`)
  return found
}

export function setupNavigationTest() {
  let root: Root | undefined
  beforeEach(() => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
    vi.useFakeTimers()
    window.history.replaceState({}, "", "/")
    // jsdom has no layout. These are synthetic visible trigger coordinates.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
      new DOMRect(40, 200, 300, 44),
    )
  })
  afterEach(async () => {
    await act(() => root?.unmount())
    root = undefined
    document.body.replaceChildren()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })
  return async (children: ReactNode) => {
    const host = document.createElement("div")
    document.body.append(host)
    root = createRoot(host)
    await act(() => root?.render(children))
    await flush()
  }
}

export async function flush() {
  // Nested zero-delay query notifications run on the next fake-clock tick.
  await act(() => vi.advanceTimersByTimeAsync(1))
}

export function element(selector: string): HTMLElement {
  const found = document.querySelector(selector)
  if (!(found instanceof HTMLElement))
    throw new Error(`Missing element: ${selector}`)
  return found
}

export async function click(target: HTMLElement, init: MouseEventInit = {}) {
  const event = new MouseEvent("click", {
    bubbles: true,
    cancelable: true,
    ...init,
  })
  await act(() => {
    target.dispatchEvent(event)
  })
  await flush()
  return event
}

export async function key(value: string, isComposing = false) {
  await act(() => {
    element('[role="combobox"]').dispatchEvent(
      new KeyboardEvent("keydown", {
        key: value,
        isComposing,
        bubbles: true,
        cancelable: true,
      }),
    )
  })
  await flush()
}

export async function typeSearch(value: string) {
  await act(() => {
    const input = element('[role="combobox"]')
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set?.call(input, value)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
  await flush()
}
