import { ReportingModeSchema, type ReportingMode } from "./api-contracts"

export const reportingLabels = {
  required: "Always include eligible new items",
  sports: "Recurring sports updates",
  highlights: "Choose highlights",
  major: "Choose only major news",
} satisfies Record<ReportingMode, string>

export function reportingLabel(mode: string | null | undefined): string {
  if (!mode) return "Reporting unavailable"
  const known = ReportingModeSchema.safeParse(mode)
  return known.success
    ? reportingLabels[known.data]
    : `Unknown reporting: ${mode}`
}
