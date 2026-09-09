import {
  PriorityRankSchema,
  type ReportingMode,
  type Source,
} from "../api-contracts"
export { reportingLabel } from "../reporting"

export const emptySource: Source = {
  key: "",
  label: "",
  kind: "rss",
  url: "",
  repo: "",
  section: "",
  threshold: "medium",
  enabled: true,
  url_canonicalization: "none",
  outlet_extraction: "none",
  dedup_group: "",
  priority_rank: "0",
  schedule_format: "",
  schedule_filter: "all",
  api_key: "",
}

export type FeedReportingMode = Exclude<ReportingMode, "sports" | "observe">

export function reportingPatch(mode: FeedReportingMode): Partial<Source> {
  return {
    threshold: {
      required: "always",
      highlights: "medium",
      major: "high",
    }[mode],
  }
}

export function sourceType(kind: string): string {
  return kind === "rss" || kind === "atom" ? "feed" : kind
}

export function sourceTypePatch(type: string): Partial<Source> {
  return {
    kind: type === "feed" ? "rss" : type,
    schedule_format: type === "sports_schedule" ? "espn" : "",
    schedule_filter: "all",
    api_key: "",
  }
}

export function suggestKey(label: string, sources: Source[]): string {
  const base = label
    .trim()
    .toLowerCase()
    .replaceAll(/[^a-z0-9._-]+/g, "-")
    .replaceAll(/^[^a-z0-9]+|-+$/g, "")
  if (!base) return ""
  let key = base
  for (
    let suffix = 2;
    sources.some((source) => source.key.toLowerCase() === key);
    suffix++
  )
    key = `${base}-${suffix}`
  return key
}

export function sourceError(
  source: Source,
  sources: Source[],
  originalKey: string | null,
): string {
  if (!/^[a-z0-9][a-z0-9._-]*$/.test(source.key))
    return "Start the key with a letter or number. Use lowercase letters, numbers, dots, underscores, or hyphens."
  if (!source.label.trim() || !source.section.trim())
    return "Enter a name and topic."
  if (
    sources.some(
      (entry) =>
        entry.key.toLowerCase() === source.key && entry.key !== originalKey,
    )
  )
    return "That source key already exists. Choose a different key."
  const priority = PriorityRankSchema.safeParse(source.priority_rank)
  if (!priority.success)
    return priority.error.issues[0]?.message ?? "Invalid priority."
  if (!["rss", "github_release", "sports_schedule"].includes(source.kind))
    return "Choose a supported source type."
  if (!["always", "high", "medium"].includes(source.threshold))
    return "Choose current reporting before saving this legacy source."
  if (
    !["", "none", "google_news_article_url"].includes(
      source.url_canonicalization,
    )
  )
    return "Choose a supported link resolution option."
  if (!["", "none", "title_suffix"].includes(source.outlet_extraction))
    return "Choose a supported publisher identification option."
  if (
    source.kind === "sports_schedule" &&
    !["espn", "espn_scoreboard", "riot"].includes(source.schedule_format)
  )
    return "Choose a supported schedule format."
  if (source.kind === "github_release" && source.url)
    return "GitHub releases use a repository, not a custom URL."
  if (!["all", "standings_top_two"].includes(source.schedule_filter))
    return "Choose a supported schedule filter."
  if (
    source.kind === "github_release" &&
    !/^[a-zA-Z0-9][a-zA-Z0-9-]*\/[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(source.repo)
  )
    return "Enter a repository as owner/name."
  if (
    source.kind === "sports_schedule" &&
    source.schedule_format === "riot" &&
    !source.api_key.trim()
  )
    return "Enter the public frontend API key used by the League of Legends esports website."
  if (source.kind !== "github_release") {
    try {
      const url = new URL(source.url)
      if (
        (url.protocol !== "http:" && url.protocol !== "https:") ||
        !url.hostname
      )
        return "Enter a complete http or https URL."
      if (
        url.username ||
        url.password ||
        /^https?:\/\/[^/?#]*@/i.test(source.url)
      )
        return "The address must not include credentials."
      if (
        source.kind === "sports_schedule" &&
        source.schedule_format === "riot" &&
        source.schedule_filter === "standings_top_two"
      ) {
        const leagues = url.searchParams.getAll("leagueId")
        if (
          !url.pathname.includes("/persisted/gw/") ||
          leagues.length !== 1 ||
          !leagues[0]?.trim() ||
          leagues[0].includes(",")
        )
          return "Top-two matches need a Riot persisted Gateway URL with exactly one leagueId."
      }
    } catch {
      return "Enter a complete http or https URL."
    }
  }
  return ""
}
