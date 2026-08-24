import type { Source } from "../api-contracts"
import { IconButton } from "./controls"
import { IconPencil, IconTrash } from "./icons"
import { EmptyState, ErrorNote, Pill, Skeleton } from "./shared"

export function SourcesTable({
  sources,
  pending,
  error,
  filtered,
  onEdit,
  onDelete,
}: {
  sources: Source[]
  pending: boolean
  error: unknown | null
  filtered: boolean
  onEdit: (source: Source) => void
  onDelete: (source: Source) => void
}) {
  if (pending) {
    return (
      <div className="space-y-px p-2">
        {[1, 2, 3, 4, 5, 6].map((key) => (
          <Skeleton key={key} className="h-10" />
        ))}
      </div>
    )
  }
  if (error) return <ErrorNote error={error} />
  if (sources.length === 0) {
    return (
      <EmptyState
        title={filtered ? "No sources match" : "No sources configured"}
        hint={
          filtered
            ? "Try a different search."
            : "Add your first feed to start receiving briefs."
        }
      />
    )
  }
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-4xl text-sm">
        <thead>
          <tr className="border-edge text-muted border-b text-left text-[11px] font-semibold tracking-wider uppercase">
            <th className="px-4 py-2.5 font-semibold">Key</th>
            <th className="px-3 py-2.5 font-semibold">Label</th>
            <th className="px-3 py-2.5 font-semibold">Kind</th>
            <th className="px-3 py-2.5 font-semibold">Section</th>
            <th className="px-3 py-2.5 font-semibold">Threshold</th>
            <th className="px-3 py-2.5 font-semibold">Status</th>
            <th className="bg-surface sticky right-0 z-10 px-4 py-2.5 text-right font-semibold">
              Actions
            </th>
          </tr>
        </thead>
        <tbody className="divide-edge divide-y">
          {sources.map((source) => (
            <SourceRow
              key={source.key}
              source={source}
              onEdit={onEdit}
              onDelete={onDelete}
            />
          ))}
        </tbody>
      </table>
    </div>
  )
}

function SourceRow({
  source,
  onEdit,
  onDelete,
}: {
  source: Source
  onEdit: (source: Source) => void
  onDelete: (source: Source) => void
}) {
  const tone =
    source.threshold === "always"
      ? "ok"
      : source.threshold === "audit"
        ? "warn"
        : "muted"
  return (
    <tr className="group hover:bg-raised/60 transition-colors">
      <td className="text-accent px-4 py-2.5 font-mono text-[13px]">
        {source.key}
      </td>
      <td className="text-ink max-w-56 truncate px-3 py-2.5">{source.label}</td>
      <td className="text-muted px-3 py-2.5 font-mono text-xs">
        {source.kind}
      </td>
      <td className="text-muted px-3 py-2.5">{source.section}</td>
      <td className="px-3 py-2.5">
        <Pill tone={tone}>{source.threshold}</Pill>
      </td>
      <td className="px-3 py-2.5">
        <Pill tone={source.enabled ? "ok" : "muted"}>
          {source.enabled ? "enabled" : "disabled"}
        </Pill>
      </td>
      <td className="bg-surface group-hover:bg-raised sticky right-0 z-10 px-4 py-2.5 transition-colors">
        <div className="flex justify-end gap-1 opacity-60 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
          <IconButton
            aria-label={`Edit ${source.key}`}
            onClick={() => onEdit(source)}
          >
            <IconPencil />
          </IconButton>
          <IconButton
            aria-label={`Delete ${source.key}`}
            danger
            onClick={() => onDelete(source)}
          >
            <IconTrash />
          </IconButton>
        </div>
      </td>
    </tr>
  )
}
