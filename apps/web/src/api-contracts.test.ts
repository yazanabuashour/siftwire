import { afterEach, describe, expect, test, vi } from "vitest"

import {
  fetchConfig,
  listRuns,
  replaceOutlets,
  saveSource,
  setOptions,
} from "./api-client"
import {
  ConfigResultSchema,
  PriorityRankSchema,
  RunDetailSchema,
} from "./api-contracts"
import { sourceError } from "./components/source-validation"

const configFixture = {
  runner_protocol: "siftwire-runner/v3",
  rejected: false,
  paths: { data_dir: "/tmp", database_path: "/tmp/db.sqlite" },
  runtime_config: { max_delivery_items: "7" },
  sources: [
    {
      key: "fixture",
      label: "Fixture",
      kind: "rss",
      url: "https://fixture.test/feed",
      repo: "",
      section: "technology",
      threshold: "medium",
      enabled: true,
    },
  ],
  outlets: [],
}

describe("ConfigResultSchema", () => {
  test("accepts v3 metadata and extra result fields while normalizing defaults", () => {
    const decoded = ConfigResultSchema.parse({
      ...configFixture,
      future_metadata: { detail: "Not consumed by this console" },
    })
    expect(decoded.runner_protocol).toBe("siftwire-runner/v3")
    expect(decoded.sources[0]?.url_canonicalization).toBe("")
    expect(decoded.sources[0]?.priority_rank).toBe("0")
    expect(decoded.sources[0]).not.toHaveProperty("always_report")
  })
})

test("keeps legacy and unknown source options readable without exposing the removed write flag", () => {
  const decoded = ConfigResultSchema.parse({
    ...configFixture,
    sources: [
      {
        ...configFixture.sources[0],
        kind: "future_feed",
        threshold: "audit",
        enabled: false,
        url_canonicalization: "old_resolver",
        outlet_extraction: "old_extractor",
        always_report: true,
      },
    ],
    source_reporting: { fixture: "future_reporting" },
  })
  expect(decoded.sources[0]).toMatchObject({
    kind: "future_feed",
    threshold: "audit",
    enabled: false,
    url_canonicalization: "old_resolver",
    outlet_extraction: "old_extractor",
  })
  expect(decoded.sources[0]).not.toHaveProperty("always_report")
  expect(decoded.source_reporting["fixture"]).toBe("future_reporting")
})

describe("RunDetailSchema", () => {
  test("decodes a complete run detail", () => {
    const fixture = {
      run: {
        run_id: "abc",
        started_at: "2026-08-24T12:00:00Z",
        finished_at: null,
        dry_run: false,
        status: "ok",
        summary: "must_include=1 candidates=0",
        delivered_at: null,
        message: null,
      },
      must_include: [],
      candidates: [
        {
          source_key: "fixture",
          source_label: "Fixture",
          id: "story-item",
          title: "Story",
          url: "https://fixture.test/story",
          selected: true,
          priority_rank: "9223372036854775807",
        },
      ],
      dropped: [
        {
          source_key: "fixture",
          id: "old-item",
          title: "Old",
          url: "https://fixture.test/old",
          reason: "recently_sent",
          detail: {},
        },
      ],
      fetch: [{ source_key: "fixture", status: "ok" }],
      sent_items: [
        {
          title: "Story",
          url: "https://fixture.test/story",
          sent_at: "2026-08-24T12:01:00Z",
        },
      ],
    }
    const decoded = RunDetailSchema.parse(fixture)
    expect(decoded.fetch[0]?.items).toBe(0)
    expect(decoded.candidates[0]?.priority_rank).toBe("9223372036854775807")
    expect(decoded.candidates[0]?.delivery_status).toBe("unknown")
    expect(decoded.candidates[0]?.reporting).toBeNull()
    expect(decoded.annotations).toEqual([])
  })
})

afterEach(() => vi.unstubAllGlobals())

describe("runner response ownership", () => {
  test.each([undefined, "siftwire-runner/v2", "siftwire-runner/v4"])(
    "rejects config metadata %s before exposing legacy sources or accepting partial mutations",
    async (protocol) => {
      const fetch = vi.fn(() =>
        Promise.resolve(
          Response.json({
            ...configFixture,
            runner_protocol: protocol,
            sources: [
              {
                ...configFixture.sources[0],
                threshold: "high",
                always_report: true,
              },
            ],
          }),
        ),
      )
      vi.stubGlobal("fetch", fetch)
      await expect(fetchConfig()).rejects.toThrow("runner_protocol")
      await expect(replaceOutlets([])).rejects.toThrow("runner_protocol")
      await expect(
        setOptions({
          maxDeliveryItems: 7,
          sportsPreGameDays: 7,
          sportsPostGameDays: 3,
          sportsTimezone: "UTC",
        }),
      ).rejects.toThrow("runner_protocol")
      expect(fetch).toHaveBeenCalledTimes(3)
    },
  )

  test("rejects runner refusals before a page can treat them as saved", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() =>
        Promise.resolve(
          new Response(
            JSON.stringify({
              rejected: true,
              rejection_reason: "Conflicting publisher matcher: example.test",
            }),
          ),
        ),
      ),
    )
    await expect(replaceOutlets([])).rejects.toThrow(
      "Conflicting publisher matcher: example.test",
    )
  })

  test("encodes archive search and cursor separately and preserves continuation", async () => {
    const fetch = vi.fn(() =>
      Promise.resolve(
        new Response(JSON.stringify({ runs: [], next_before: "older-run" })),
      ),
    )
    vi.stubGlobal("fetch", fetch)
    expect(
      await listRuns({
        delivered: true,
        before: "run&1",
        search: "2026-09 + release",
      }),
    ).toEqual({ runs: [], next_before: "older-run" })
    expect(fetch).toHaveBeenCalledWith(
      "/api/v1/runs?delivered=true&before=run%261&search=2026-09+%2B+release",
      expect.objectContaining({ method: "GET" }),
    )
  })
})

describe("lossless priorities", () => {
  test.each([
    "-9223372036854775808",
    "9223372036854775807",
    "-9007199254740993",
    "9007199254740993",
    "0",
  ])(
    "keeps %s exact through config, editing, and the save request",
    async (rank) => {
      const payload = {
        ...configFixture,
        sources: [{ ...configFixture.sources[0], priority_rank: rank }],
      }
      const fetch = vi.fn(() =>
        Promise.resolve(new Response(JSON.stringify(payload))),
      )
      vi.stubGlobal("fetch", fetch)
      const config = await fetchConfig()
      const source = config.sources[0]
      if (!source) throw new Error("Missing fixture source")
      const draft = { ...source, label: "Renamed source", enabled: false }
      expect(sourceError(draft, config.sources, source.key)).toBe("")
      expect(draft.priority_rank).toBe(rank)
      await saveSource(draft)
      expect(fetch).toHaveBeenLastCalledWith(
        "/api/v1/sources",
        expect.objectContaining({
          body: JSON.stringify(draft),
        }),
      )
      expect(JSON.parse(JSON.stringify(draft)).priority_rank).toBe(rank)
    },
  )

  test.each([
    "",
    "1.5",
    "1e3",
    "+1",
    " 1",
    "1\n",
    "1\r",
    "9223372036854775808",
    "-9223372036854775809",
    0,
    9007199254740992,
  ])("rejects malformed, out-of-range, or numeric priority %s", (value) =>
    expect(PriorityRankSchema.safeParse(value).success).toBe(false),
  )
})
