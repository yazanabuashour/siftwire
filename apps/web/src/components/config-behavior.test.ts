import { describe, expect, it } from "vitest"

import {
  ConfigResultSchema,
  type OutletPolicy,
  type Source,
} from "../api-contracts"
import { mergeSourceResult } from "./config-query"
import { emptySource } from "./source-validation"

const stored: Source = {
  ...emptySource,
  key: "example-feed",
  label: "Example feed",
  kind: "rss",
  section: "Engineering",
  url: "https://example.test/feed",
  threshold: "always",
  priority_rank: "-9223372036854775808",
  dedup_group: "Stored group",
  url_canonicalization: "google_news_article_url",
  outlet_extraction: "title_suffix",
  schedule_format: "",
  schedule_filter: "all",
}

const publisher: OutletPolicy = {
  name: "Example publisher",
  aliases: ["EXAMPLE.TEST", " Example News "],
  note: "  Optional rationale  ",
  policy: "watch",
  enabled: true,
}

function config() {
  return ConfigResultSchema.parse({
    runner_protocol: "siftwire-runner/v5",
    capabilities: ["current-news/v1"],
    rejected: false,
    paths: { data_dir: "data", database_path: "data/config.sqlite" },
    runtime_config: {
      max_delivery_items: "7",
      sports_timezone: "UTC",
      unrelated: "keep",
    },
    sources: [stored, { ...stored, key: "other", label: "Other" }],
    source_reporting: { "example-feed": "required", other: "required" },
    outlets: [publisher],
  })
}

describe("normalized configuration mutation merges", () => {
  it("upserts returned sources without erasing unrelated collections or retaining stale reporting", () => {
    const current = config()

    const normalized = {
      ...stored,
      label: "Server normalized",
    }

    const result = {
      ...config(),
      sources: [normalized],
      source_reporting: {},
      outlets: [],
      runtime_config: {},
    }

    const next = mergeSourceResult(current, result, "upsert")
    expect(next).toEqual({
      ...current,
      sources: [normalized, current.sources[1]],
      source_reporting: { other: "required" },
    })
    expect(current.sources[0]).toEqual(stored)
    expect(
      mergeSourceResult(
        current,
        { ...result, source_reporting: { [stored.key]: "major" } },
        "upsert",
      ).source_reporting,
    ).toEqual({ [stored.key]: "major", other: "required" })
  })
})
