import { describe, expect, test } from "vitest"

import { ConfigResultSchema, RunDetailSchema } from "./api-contracts"

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
  })
})
