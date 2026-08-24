import { useQuery } from "@tanstack/react-query"
import type { ReactNode } from "react"

import { fetchConfig } from "../api-client"
import { IconX } from "./icons"

export function useConfig() {
  return useQuery({
    queryKey: ["config"],
    queryFn: ({ signal }) => fetchConfig(signal),
  })
}

export function PageHeader({
  title,
  subtitle,
  actions,
}: {
  title: string
  subtitle?: string | undefined
  actions?: ReactNode | undefined
}) {
  return (
    <div className="mb-6 flex items-start justify-between gap-4">
      <div>
        <h1 className="text-ink text-xl font-semibold">{title}</h1>
        {subtitle ? <p className="text-muted mt-0.5">{subtitle}</p> : null}
      </div>
      {actions ? (
        <div className="flex shrink-0 items-center gap-2">{actions}</div>
      ) : null}
    </div>
  )
}

export function Card({
  title,
  children,
  className = "",
}: {
  title?: string | undefined
  children: ReactNode
  className?: string | undefined
}) {
  return (
    <section
      className={`border-edge bg-surface rounded-xl border ${className}`}
    >
      {title ? (
        <header className="border-edge border-b px-4 py-3">
          <h2 className="text-ink text-sm font-semibold">{title}</h2>
        </header>
      ) : null}
      {children}
    </section>
  )
}

export function StatTile({
  label,
  value,
  hint,
}: {
  label: string
  value: ReactNode
  hint?: ReactNode | undefined
}) {
  return (
    <div className="border-edge bg-surface rounded-xl border px-4 py-3.5">
      <p className="text-muted text-[11px] font-semibold tracking-wider uppercase">
        {label}
      </p>
      <p className="text-ink mt-1 text-xl font-semibold sm:text-2xl">{value}</p>
      {hint ? <p className="text-muted mt-0.5 text-xs">{hint}</p> : null}
    </div>
  )
}

type PillTone = "ok" | "error" | "warn" | "muted"

const pillTones = {
  ok: "bg-accent-dim text-accent",
  error: "bg-danger-dim text-danger",
  warn: "bg-warn-dim text-warn",
  muted: "bg-raised text-muted",
} satisfies Record<PillTone, string>

export function Pill({
  tone,
  children,
}: {
  tone: PillTone
  children: ReactNode
}) {
  const dot =
    tone === "ok"
      ? "bg-accent"
      : tone === "error"
        ? "bg-danger"
        : tone === "warn"
          ? "bg-warn"
          : "bg-muted"
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium whitespace-nowrap ${pillTones[tone]}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${dot}`} />
      {children}
    </span>
  )
}

export function Skeleton({
  className = "",
}: {
  className?: string | undefined
}) {
  return <div className={`bg-raised animate-pulse rounded-lg ${className}`} />
}

export function ErrorNote({ error }: { error: unknown }) {
  const message = error instanceof Error ? error.message : String(error)
  return (
    <div className="border-danger/30 bg-danger-dim text-danger flex items-center gap-2 rounded-lg border px-3 py-2 text-sm">
      <IconX className="h-4 w-4 shrink-0" />
      {message}
    </div>
  )
}

export function EmptyState({
  title,
  hint,
}: {
  title: string
  hint?: string | undefined
}) {
  return (
    <div className="flex flex-col items-center gap-1 px-4 py-12 text-center">
      <p className="text-muted text-sm font-medium">{title}</p>
      {hint ? <p className="text-muted/70 text-xs">{hint}</p> : null}
    </div>
  )
}

export function formatWhen(iso: string | null | undefined): string {
  if (!iso) return "Not set"
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  })
}

/** Parses the runner's run summary fields. */
export function parseSummary(
  summary: string,
): { mustInclude: number; candidates: number } | null {
  const must = summary.match(/must_include=(\d+)/)
  const candidates = summary.match(/candidates=(\d+)/)
  if (!must || !candidates) return null
  return { mustInclude: Number(must[1]), candidates: Number(candidates[1]) }
}
