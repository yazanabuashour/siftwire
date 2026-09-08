import type { ReportingMode } from "./api-contracts"

export const reportingLabels = {
  required: "Always include eligible new items",
  sports: "Recurring sports updates",
  highlights: "Choose highlights",
  major: "Choose only major news",
  observe: "Observe without including",
} satisfies Record<ReportingMode, string>
