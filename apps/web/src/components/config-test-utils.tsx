import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, type ReactNode } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, vi } from "vitest"
import { z } from "zod"

import {
  ConfigResultSchema,
  OutletPolicySchema,
  type OutletPolicy,
} from "../api-contracts"
import { configQuery } from "./config-query"

export const request = vi.fn<typeof fetch>()

export function pendingResponse() {
  let respond: ((response: Response) => void) | undefined
  let signal: AbortSignal | undefined
  request.mockImplementationOnce((_url, init) => {
    const requestSignal = init?.signal
    if (!requestSignal) throw new Error("Missing request AbortSignal")
    signal = requestSignal
    return new Promise<Response>((resolve, reject) => {
      const abort = () => reject(requestSignal.reason)
      if (requestSignal.aborted) abort()
      else requestSignal.addEventListener("abort", abort, { once: true })
      respond = (response) => {
        requestSignal.removeEventListener("abort", abort)
        resolve(response)
      }
    })
  })
  return {
    respond(response: Response) {
      if (!respond) throw new Error("Request has not started")
      respond(response)
    },
    get signal() {
      if (!signal) throw new Error("Request has not started")
      return signal
    },
  }
}

export function sentOutlets(): OutletPolicy[] {
  const body = z.string().parse(request.mock.lastCall?.[1]?.body)
  return z
    .object({ outlets: z.array(OutletPolicySchema) })
    .parse(JSON.parse(body)).outlets
}
export const publisher = {
  name: "Example",
  aliases: ["EXAMPLE.TEST"],
  policy: "watch",
  note: "  Keep exact optional note  ",
  enabled: true,
}
export const configuration = () =>
  ConfigResultSchema.parse({
    runner_protocol: "siftwire-runner/v4",
    capabilities: ["current-news/v1"],
    rejected: false,
    paths: { database_path: "data/config.sqlite", data_dir: "data" },
    outlets: [publisher],
    runtime_config: {
      max_delivery_items: "7",
      sports_pre_game_days: "7",
      sports_post_game_days: "3",
      sports_timezone: "UTC",
    },
  })
export let host: HTMLDivElement
export let client: QueryClient
let root: Root

export function setupConfigTest(): void {
  beforeEach(() => {
    vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
    vi.stubGlobal("fetch", request)
    // jsdom does not implement the modal dialog lifecycle.
    Object.defineProperties(HTMLDialogElement.prototype, {
      showModal: {
        configurable: true,
        value(this: HTMLDialogElement) {
          this.open = true
        },
      },
      close: {
        configurable: true,
        value(this: HTMLDialogElement) {
          this.open = false
        },
      },
    })
    client = new QueryClient({
      defaultOptions: {
        queries: { retry: false, staleTime: Infinity },
        mutations: { retry: false },
      },
    })
    client.setQueryData(configQuery.queryKey, configuration())
    host = document.createElement("div")
    document.body.append(host)
    root = createRoot(host)
  })
  afterEach(() => {
    act(() => root.unmount())
    host.remove()
    client.clear()
    Reflect.deleteProperty(HTMLDialogElement.prototype, "showModal")
    Reflect.deleteProperty(HTMLDialogElement.prototype, "close")
    vi.restoreAllMocks()
    vi.resetAllMocks()
    vi.unstubAllGlobals()
  })
}

export function render(node: ReactNode): void {
  act(() =>
    root.render(
      <QueryClientProvider client={client}>{node}</QueryClientProvider>,
    ),
  )
}
export function button(label: string): HTMLButtonElement {
  const found = [...host.querySelectorAll("button")].find(
    (element) =>
      (element.getAttribute("aria-label") ?? element.textContent) === label,
  )
  if (!found) throw new Error(`Missing button ${label}`)
  return found
}
export function click(label: string): void {
  act(() => button(label).click())
}
function field(label: string): HTMLLabelElement {
  const found = [...host.querySelectorAll("label")].find(
    (element) => element.querySelector("span")?.textContent === label,
  )
  if (!found) throw new Error(`Missing field ${label}`)
  return found
}
export function input(label: string): HTMLInputElement {
  const found = field(label).querySelector("input")
  if (!found) throw new Error(`Missing input ${label}`)
  return found
}
export function select(label: string): HTMLSelectElement {
  const found =
    [...host.querySelectorAll("select")].find(
      (element) => element.getAttribute("aria-label") === label,
    ) ?? field(label).querySelector("select")
  if (!found) throw new Error(`Missing select ${label}`)
  return found
}
export function choose(label: string, value: string): void {
  act(() => {
    const element = select(label)
    element.value = value
    element.dispatchEvent(new Event("change", { bubbles: true }))
  })
}
export function change(label: string, value: string): void {
  act(() => {
    const element = input(label)
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set?.call(element, value)
    element.dispatchEvent(new Event("input", { bubbles: true }))
  })
}
export async function settle(): Promise<void> {
  // Flush the query observer's queued notification after a cache update.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}
