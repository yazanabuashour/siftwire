import type { RunDetail, RunItem } from "../api-contracts"
import { reportingLabel } from "../reporting"
import EmailBrief from "./EmailBrief"
import { dateLabel } from "./ui"

export function RecordedBrief({ detail }: { detail: RunDetail }) {
  if (detail.delivery_html?.trim())
    return <EmailBrief key={detail.run.run_id} html={detail.delivery_html} />
  return <p className="folio-empty">No saved email HTML for this brief.</p>
}

export function StoryReceipt({ story }: { story: RunItem }) {
  return (
    <details>
      <summary>Details</summary>
      <dl className="folio-receipt">
        {[
          ["Source", story.source_label || "Label not recorded"],
          ["Source ID", story.source_key],
          ["Type", story.kind.replaceAll("_", " ") || "Not recorded"],
          ["Reporting", reportingLabel(story.reporting)],
          ["Source preference", story.priority_rank],
          ["Published", dateLabel(story.published_at, true)],
          ["Publisher", story.outlet || "Not recorded"],
        ].map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <p className="folio-muted">
        Source preference affects duplicate selection and ordering, not article
        importance. These settings belong to this recorded run.
      </p>
    </details>
  )
}
