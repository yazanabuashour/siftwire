import type { Source } from "../api-contracts"

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
  priority_rank: 0,
  always_report: false,
  schedule_format: "",
  schedule_filter: "all",
  api_key: "",
}

export function normalizeDraft(
  draft: Source,
  rank: string,
  key: string,
): Source {
  return {
    ...draft,
    key,
    label: draft.label.trim(),
    section: draft.section.trim().toLowerCase(),
    url: draft.url.trim(),
    repo: draft.repo.trim(),
    dedup_group: draft.dedup_group.trim().toLowerCase(),
    api_key: draft.api_key.trim(),
    priority_rank: rank.trim() ? Number(rank) : NaN,
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
  if (!source.label || !source.section) return "Enter a name and topic."
  if (
    sources.some(
      (entry) =>
        entry.key.toLowerCase() === source.key && entry.key !== originalKey,
    )
  )
    return "That source key already exists. Choose a different key."
  // The runner validates its i64 range after JSON decoding. Do not substitute
  // JavaScript's narrower safe-integer range for that contract.
  if (!Number.isInteger(source.priority_rank))
    return "Priority must be a whole number."
  if (source.kind === "github_release" && !source.repo && !source.url)
    return "Enter a repository or a release URL."
  if (
    source.kind === "github_release" &&
    source.repo &&
    !/^[a-zA-Z0-9][a-zA-Z0-9-]*\/[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(source.repo)
  )
    return "Enter a repository as owner/name."
  if (source.schedule_format === "riot" && !source.api_key)
    return "Enter the public frontend API key used by the League of Legends esports website."
  if (source.kind !== "github_release" || source.url) {
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
