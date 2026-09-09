import { afterEach, expect, test, vi } from "vitest"
import { z } from "zod"

import { HttpError, request } from "./http-client"

const schema = z.object({ ok: z.boolean() })

afterEach(() => vi.unstubAllGlobals())

test.each([200, 502])(
  "preserves JSON parse diagnostics for HTTP %s, including failed HTTP status",
  async (status) => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>(
        async () =>
          new Response("<html>Not JSON</html>", {
            status,
            statusText: status === 200 ? "OK" : "Bad Gateway",
          }),
      ),
    )
    const result = request("/health", { method: "GET", schema })
    if (status === 200) {
      await expect(result).rejects.toBeInstanceOf(SyntaxError)
    } else {
      await expect(result).rejects.toBeInstanceOf(HttpError)
      await expect(result).rejects.toMatchObject({
        status: 502,
        message: "Bad Gateway",
        cause: expect.any(SyntaxError),
      })
    }
  },
)

test.each([200, 502])(
  "preserves cancellation during HTTP %s body consumption",
  async (status) => {
    const external = new AbortController()
    const cause = new DOMException("Body read cancelled", "AbortError")
    let response: Response | undefined
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>(async (_input, init) => {
        response = new Response(
          new ReadableStream({
            start(controller) {
              init?.signal?.addEventListener(
                "abort",
                () => controller.error(cause),
                { once: true },
              )
            },
          }),
          { status },
        )
        return response
      }),
    )
    const result = request("/health", {
      method: "GET",
      schema,
      signal: external.signal,
    })
    const rejection = expect(result).rejects.toBe(cause)
    await vi.waitFor(() => expect(response?.bodyUsed).toBe(true))
    external.abort(cause)
    await rejection
  },
)

test("retains the console's HTTP status and error envelope message", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn<typeof fetch>(async () =>
      Response.json(
        { error: { message: "source key is required" } },
        { status: 400, statusText: "Bad Request" },
      ),
    ),
  )
  await expect(
    request("/sources/", { method: "DELETE", schema }),
  ).rejects.toMatchObject({ status: 400, message: "source key is required" })
})
