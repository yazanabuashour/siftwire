import { type ZodType, z } from "zod"

const ErrorEnvelopeSchema = z.object({
  error: z.object({ message: z.string() }),
})

const PlainMessageSchema = z.object({ message: z.string() })

export class HttpError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

interface RequestOptions<T> {
  method: "GET" | "POST" | "PUT" | "DELETE"
  body?: unknown | undefined
  schema: ZodType<T>
  signal?: AbortSignal | undefined
}

const TIMEOUT_MS = 30_000
const BASE = "/api/v1"

/// One owner per request: creates the abort controller, applies the timeout,
/// and decodes the response through the supplied schema.
export async function request<T>(
  path: string,
  options: RequestOptions<T>,
): Promise<T> {
  const controller = new AbortController()
  const timer = setTimeout(
    () => controller.abort(new DOMException("timeout", "TimeoutError")),
    TIMEOUT_MS,
  )
  const external = options.signal
  if (external) {
    if (external.aborted) controller.abort(external.reason)
    else
      external.addEventListener(
        "abort",
        () => controller.abort(external.reason),
        { once: true },
      )
  }
  try {
    const init: RequestInit = {
      method: options.method,
      signal: controller.signal,
    }
    if (options.body !== undefined) {
      init.headers = { "content-type": "application/json" }
      init.body = JSON.stringify(options.body)
    }
    const response = await fetch(`${BASE}${path}`, init)
    const payload: unknown = await response.json().catch(() => null)
    if (!response.ok) {
      const envelope = ErrorEnvelopeSchema.safeParse(payload)
      const plain = PlainMessageSchema.safeParse(payload)
      const message = envelope.success
        ? envelope.data.error.message
        : plain.success
          ? plain.data.message
          : response.statusText || "request failed"
      throw new HttpError(response.status, message)
    }
    return options.schema.parse(payload)
  } finally {
    clearTimeout(timer)
  }
}
