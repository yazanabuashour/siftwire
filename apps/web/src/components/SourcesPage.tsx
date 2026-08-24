import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"

import { deleteSource, saveSource } from "../api-client"
import type { Source } from "../api-contracts"
import { ErrorNote, useConfig } from "./shared"

const EMPTY: Source = {
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
  const save = useMutation({
    mutationFn: (source: Source) => saveSource(source),
    onSuccess: () => {
      setDraft(null)
      void client.invalidateQueries({ queryKey: ["config"] })
    },
  })
  const remove = useMutation({
    mutationFn: (key: string) => deleteSource(key),
    onSuccess: () => void client.invalidateQueries({ queryKey: ["config"] }),
  })

  if (config.isPending) return <p className="text-(--muted)">Loading…</p>
  if (config.isError) return <ErrorNote error={config.error} />
  const sources = [...config.data.sources].sort((a, b) =>
    a.key.localeCompare(b.key),
  )

  return (
    <div className="flex flex-col gap-4">
      <button
        type="button"
        onClick={() => setDraft({ ...EMPTY })}
        className="self-start rounded border border-(--border) px-3 py-1 text-sm hover:border-(--accent)"
      >
        Add source
      </button>
      {save.isError ? <ErrorNote error={save.error} /> : null}
      {remove.isError ? <ErrorNote error={remove.error} /> : null}
      <table className="text-sm">
        <thead className="text-(--muted)">
          <tr className="text-left">
            <th className="py-1">Key</th>
            <th>Kind</th>
            <th>Section</th>
            <th>Threshold</th>
            <th>Enabled</th>
            <th>Label</th>
            <th>
              <span className="sr-only">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {sources.map((source) => (
            <tr key={source.key} className="border-t border-(--border)">
              <td className="py-1 font-mono">{source.key}</td>
              <td>{source.kind}</td>
              <td>{source.section}</td>
              <td>{source.threshold}</td>
              <td>{source.enabled ? "yes" : "no"}</td>
              <td className="max-w-60 truncate">{source.label}</td>
              <td className="space-x-2 whitespace-nowrap">
                <button
                  type="button"
                  className="hover:text-(--accent)"
                  onClick={() => setDraft(source)}
                >
                  edit
                </button>
                <button
                  type="button"
                  className="text-red-400 hover:text-red-300"
                  onClick={() => {
                    if (window.confirm(`Delete source ${source.key}?`))
                      remove.mutate(source.key)
                  }}
                >
                  delete
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {draft ? (
        <SourceForm
          draft={draft}
          editingExisting={sources.some((source) => source.key === draft.key)}
          busy={save.isPending}
          onDone={setDraft}
          onSave={(source) => save.mutate(source)}
        />
      ) : null}
    </div>
  )
}

interface SourceFormProps {
  draft: Source
  editingExisting: boolean
  busy: boolean
  onDone: (next: Source | null) => void
  onSave: (source: Source) => void
}

function SourceForm({
  draft,
  editingExisting,
  busy,
  onDone,
  onSave,
}: SourceFormProps) {
  const field =
    (key: keyof Source): ((value: string | boolean | number) => void) =>
    (value) =>
      onDone({ ...draft, [key]: value })
  return (
    <form
      className="flex flex-col gap-2 rounded-lg border border-(--border) bg-(--surface) p-4 text-sm"
      onSubmit={(event) => {
        event.preventDefault()
        onSave(draft)
      }}
    >
      <FieldRows
        draft={draft}
        field={field}
        editingExisting={editingExisting}
      />
      {busy ? <p className="text-(--muted)">Saving…</p> : null}
      <div className="mt-2 flex gap-2">
        <button
          type="submit"
          className="rounded border border-(--accent) px-3 py-1 hover:bg-(--accent) hover:text-black"
        >
          Save
        </button>
        <button
          type="button"
          onClick={() => onDone(null)}
          className="rounded border border-(--border) px-3 py-1"
        >
          Cancel
        </button>
      </div>
    </form>
  )
}

function FieldRows({
  draft,
  field,
  editingExisting,
}: {
  draft: Source
  field: (key: keyof Source) => (value: string | boolean | number) => void
  editingExisting: boolean
}) {
  return (
    <>
      <Row label="Key">
        <input
          className={inputClass}
          value={draft.key}
          onChange={(event) => field("key")(event.target.value)}
          readOnly={editingExisting}
          required
        />
      </Row>
      {editingExisting ? (
        <p className="text-xs text-(--muted)">
          Keys are immutable while editing; delete and re-add to rename.
        </p>
      ) : null}
      <Row label="Label">
        <input
          className={inputClass}
          value={draft.label}
          onChange={(event) => field("label")(event.target.value)}
          required
        />
      </Row>
      <Row label="Kind">
        <select
          className={inputClass}
          value={draft.kind}
          onChange={(event) => field("kind")(event.target.value)}
        >
          <option value="rss">rss</option>
          <option value="atom">atom</option>
          <option value="github_release">github_release</option>
        </select>
      </Row>
      <Row label="URL">
        <input
          className={inputClass}
          value={draft.url}
          onChange={(event) => field("url")(event.target.value)}
        />
      </Row>
      {draft.kind === "github_release" ? (
        <Row label="Repo">
          <input
            className={inputClass}
            value={draft.repo}
            placeholder="owner/name"
            onChange={(event) => field("repo")(event.target.value)}
          />
        </Row>
      ) : null}
      <Row label="Section">
        <input
          className={inputClass}
          value={draft.section}
          onChange={(event) => field("section")(event.target.value)}
          required
        />
      </Row>
      <Row label="Threshold">
        <select
          className={inputClass}
          value={draft.threshold}
          onChange={(event) => field("threshold")(event.target.value)}
        >
          {["always", "medium", "high", "audit"].map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </Row>
      <Row label="Dedup group">
        <input
          className={inputClass}
          value={draft.dedup_group}
          onChange={(event) => field("dedup_group")(event.target.value)}
        />
      </Row>
    </>
  )
}

function Row({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <label className="grid grid-cols-[8rem_1fr] items-center gap-2">
      <span className="text-(--muted)">{label}</span>
      {children}
    </label>
  )
}

const inputClass =
  "w-full rounded border border-(--border) bg-transparent px-2 py-1 focus:border-(--accent) focus:outline-none"
