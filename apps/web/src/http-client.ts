import { type ZodType, z } from "zod"

const ErrorEnvelopeSchema = z.object({
  error: z.object({ message: z.string() }),
})

const PlainMessageSchema = z.object({ message: z.string() })
const RejectionSchema = z.object({
  rejected: z.literal(true),
  rejection_reason: z.string().optional(),
  summary: z.string().optional(),
})

export class HttpError extends Error {
  readonly status: number

  constructor(status: number, message: string, options?: ErrorOptions) {
    super(message, options)
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
  const abort = () => controller.abort(external?.reason)
  if (external?.aborted) abort()
  else external?.addEventListener("abort", abort, { once: true })
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
    const httpMessage = response.statusText || "request failed"
    let payload: unknown
    try {
      payload = await response.json()
    } catch (cause) {
      if (
        response.ok ||
        controller.signal.aborted ||
        (cause instanceof DOMException && cause.name === "AbortError")
      )
        throw cause
      throw new HttpError(response.status, httpMessage, { cause })
    }
    if (!response.ok) {
      const envelope = ErrorEnvelopeSchema.safeParse(payload)
      const plain = PlainMessageSchema.safeParse(payload)
      const message = envelope.success
        ? envelope.data.error.message
        : plain.success
          ? plain.data.message
          : httpMessage
      throw new HttpError(response.status, message)
    }
    const rejection = RejectionSchema.safeParse(payload)
    if (rejection.success)
      throw new Error(
        rejection.data.rejection_reason ||
          rejection.data.summary ||
          "The runner rejected the request.",
      )
    return options.schema.parse(payload)
  } finally {
    clearTimeout(timer)
    external?.removeEventListener("abort", abort)
  }
}
