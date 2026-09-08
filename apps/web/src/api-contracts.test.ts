import { afterEach, describe, expect, test, vi } from "vitest"

import { fetchConfig, saveSource } from "./api-client"
import {
  ConfigResultSchema,
  PriorityRankSchema,
  RunDetailSchema,
} from "./api-contracts"
import { normalizeDraft, sourceError } from "./components/source-validation"

const configFixture = {
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
  test("fills optional collections and normalizes defaults", () => {
    const decoded = ConfigResultSchema.parse(configFixture)
    expect(decoded.sources[0]?.url_canonicalization).toBe("")
    expect(decoded.sources[0]?.priority_rank).toBe("0")
  })
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
          title: "Story",
          url: "https://fixture.test/story",
          selected: true,
          priority_rank: "9223372036854775807",
        },
      ],
      dropped: [
        {
          source_key: "fixture",
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
  })
})

afterEach(() => vi.unstubAllGlobals())

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
      const draft = normalizeDraft(
        { ...source, label: "Renamed source", enabled: false },
        source.key,
      )
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
