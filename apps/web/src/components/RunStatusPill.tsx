import { Pill } from "./shared"

export function RunStatusPill({
  status,
  delivered,
}: {
  status: string
  delivered: boolean
}) {
  if (status !== "ok") return <Pill tone="error">{status}</Pill>
  return (
    <Pill tone={delivered ? "ok" : "muted"}>
      {delivered ? "delivered" : "no brief"}
    </Pill>
  )
}
