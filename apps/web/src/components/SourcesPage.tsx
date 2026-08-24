import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"

import { deleteSource, saveSource } from "../api-client"
import type { Source } from "../api-contracts"
import { Button, inputClass, Modal } from "./controls"
import { IconPlus, IconSearch } from "./icons"
import { ErrorNote, PageHeader, useConfig } from "./shared"
import { SourceEditorModal } from "./SourceEditorModal"
import { SourcesTable } from "./SourcesTable"

const EMPTY_SOURCE: Source = {
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
}

export default function SourcesPage() {
  const config = useConfig()
  const client = useQueryClient()
  const [draft, setDraft] = useState<Source | null>(null)
  const [pendingDelete, setPendingDelete] = useState<Source | null>(null)
  const [query, setQuery] = useState("")
  const save = useMutation({
    mutationFn: (source: Source) => saveSource(source),
    onSuccess: () => {
      setDraft(null)
      void client.invalidateQueries({ queryKey: ["config"] })
    },
  })
  const remove = useMutation({
    mutationFn: (key: string) => deleteSource(key),
    onSuccess: () => {
      setPendingDelete(null)
      void client.invalidateQueries({ queryKey: ["config"] })
    },
  })
  const sources = filteredSources(config.data?.sources ?? [], query)
  const existing = draft
    ? (config.data?.sources.some((source) => source.key === draft.key) ?? false)
    : false

  return (
    <>
      <SourcesScreen
        count={config.data?.sources.length}
        sources={sources}
        query={query}
        pending={config.isPending}
        error={config.isError ? config.error : null}
        mutationError={save.error ?? remove.error ?? null}
        onQuery={setQuery}
        onAdd={() => setDraft({ ...EMPTY_SOURCE })}
        onEdit={setDraft}
        onDelete={setPendingDelete}
      />
      {draft ? (
        <SourceEditorModal
          draft={draft}
          editingExisting={existing}
          busy={save.isPending}
          onClose={() => setDraft(null)}
          onSave={save.mutate}
        />
      ) : null}
      {pendingDelete ? (
        <DeleteSourceModal
          source={pendingDelete}
          busy={remove.isPending}
          onClose={() => setPendingDelete(null)}
          onDelete={() => remove.mutate(pendingDelete.key)}
        />
      ) : null}
    </>
  )
}

function filteredSources(sources: Source[], query: string): Source[] {
  const sorted = [...sources].sort((a, b) => a.key.localeCompare(b.key))
  const needle = query.trim().toLowerCase()
  if (!needle) return sorted
  return sorted.filter((source) =>
    [source.key, source.label, source.section, source.kind]
      .join(" ")
      .toLowerCase()
      .includes(needle),
  )
}

function SourcesScreen({
  count,
  sources,
  query,
  pending,
  error,
  mutationError,
  onQuery,
  onAdd,
  onEdit,
  onDelete,
}: {
  count: number | undefined
  sources: Source[]
  query: string
  pending: boolean
  error: unknown | null
  mutationError: unknown | null
  onQuery: (value: string) => void
  onAdd: () => void
  onEdit: (source: Source) => void
  onDelete: (source: Source) => void
}) {
  return (
    <>
      <PageHeader
        title="Sources"
        subtitle={count === undefined ? undefined : `${count} configured feeds`}
        actions={
          <Button variant="primary" onClick={onAdd}>
            <IconPlus className="h-4 w-4" />
            Add source
          </Button>
        }
      />
      {mutationError ? (
        <div className="mb-4">
          <ErrorNote error={mutationError} />
        </div>
      ) : null}
      <div className="border-edge bg-surface overflow-hidden rounded-xl border">
        <div className="border-edge border-b px-4 py-3">
          <label className="relative block max-w-sm">
            <span className="sr-only">Search sources</span>
            <IconSearch className="text-muted pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2" />
            <input
              value={query}
              onChange={(event) => onQuery(event.target.value)}
              placeholder="Search key, label, section…"
              className={`${inputClass} pl-8`}
            />
          </label>
        </div>
        <SourcesTable
          sources={sources}
          pending={pending}
          error={error}
          filtered={query.trim().length > 0}
          onEdit={onEdit}
          onDelete={onDelete}
        />
      </div>
    </>
  )
}

function DeleteSourceModal({
  source,
  busy,
  onClose,
  onDelete,
}: {
  source: Source
  busy: boolean
  onClose: () => void
  onDelete: () => void
}) {
  return (
    <Modal
      title="Delete source"
      subtitle={source.key}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="danger" disabled={busy} onClick={onDelete}>
            {busy ? "Deleting…" : "Delete source"}
          </Button>
        </>
      }
    >
      <p className="text-muted text-sm">
        This removes <span className="text-ink font-mono">{source.key}</span>{" "}
        and its latest-seen state. Future briefs will no longer include it.
      </p>
    </Modal>
  )
}
