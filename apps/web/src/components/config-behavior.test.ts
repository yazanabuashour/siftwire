import { describe, expect, it } from "vitest"

import {
  ConfigResultSchema,
  type OutletPolicy,
  type Source,
} from "../api-contracts"
import { outletDraft, outletFromDraft } from "./config-outlets"
import {
  mergeOptionsResult,
  mergeOutletsResult,
  mergeSourceResult,
} from "./config-query"
import { emptySource, reportingPatch } from "./source-validation"

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

describe("configuration-owned transformations", () => {
  it("changes only reporting fields when the operator explicitly selects a mode", () => {
    expect(reportingPatch("major")).toEqual({ threshold: "high" })
    expect(reportingPatch("required")).toEqual({ threshold: "always" })
    expect(reportingPatch("highlights")).toEqual({ threshold: "medium" })
    expect(emptySource.threshold).toBe("medium")
    expect(emptySource).not.toHaveProperty("always_report")
  })

  it("preserves optional notes and exact aliases through rename or policy edits", () => {
    expect(
      outletFromDraft({
        ...outletDraft(publisher),
        name: "Renamed",
        policy: "allow",
      }),
    ).toEqual({ ...publisher, name: "Renamed", policy: "allow" })
    expect(outletFromDraft(outletDraft({ ...publisher, note: "" })).note).toBe(
      "",
    )
  })
})

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

  it("merges only each write's returned responsibility", () => {
    const current = config()
    expect(
      mergeOptionsResult(current, {
        runtime_config: { sports_timezone: "Etc/UTC" },
      }),
    ).toEqual({
      ...current,
      runtime_config: { ...current.runtime_config, sports_timezone: "Etc/UTC" },
    })
    const result = {
      outlets: [{ ...publisher, note: "Server normalized" }],
    }
    expect(mergeOutletsResult(current, result)).toEqual({
      ...current,
      ...result,
    })
  })
})
