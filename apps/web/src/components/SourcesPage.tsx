import {
  useMutation,
  useQueryClient,
  type UseQueryResult,
} from "@tanstack/react-query"
import { useState } from "react"

import { deleteSource, saveSource } from "../api-client"
import type { ConfigResult, ReportingMode, Source } from "../api-contracts"
import {
  applyConfigResult,
  configQuery,
  mergeSourceResult,
  useConfig,
} from "./config-query"

import "./config.css"
import SourceCollection from "./SourceCollection"
import SourceEditor, { emptySource } from "./SourceEditor"
import { Dialog, ErrorNote, PageHeading } from "./ui"

export default function SourcesPage() {
  const [editor, setEditor] = useState<{
    source: Source
    editing: boolean
    reporting: ReportingMode | undefined
  } | null>(null)
  const [removing, setRemoving] = useState<Source | null>(null)
  const [notice, setNotice] = useState("")
  const { config, save, remove } = useSources((message) => {
    setNotice(message)
    setEditor(null)
    setRemoving(null)
  })
  const busy = save.isPending || remove.isPending
  const ready = !!config.data
  const sources = config.data?.sources ?? []
  const startEditing = (source: Source, editing: boolean): void => {
    save.reset()
    remove.reset()
    setNotice("")
    setEditor({
      source,
      editing,
      reporting: config.data?.source_reporting[source.key],
    })
  }
  return (
    <section className="config-page">
      <PageHeading
        title="Sources"
        action={
          <button
            className="folio-primary"
            type="button"
            disabled={!ready || busy}
            onClick={() => startEditing({ ...emptySource }, false)}
          >
            Add source
          </button>
        }
      />
      <output className="folio-notice">
        {busy ? "Saving changes…" : notice}
      </output>
      <SourceLoadState config={config} />
      {!editor && save.error && (
        <p className="folio-error" role="alert">
          {save.error.message}
        </p>
      )}
      {ready && (
        <SourceCollection
          sources={sources}
          reporting={config.data?.source_reporting ?? {}}
          busy={busy}
          savingKey={save.isPending ? save.variables.key : undefined}
          onEdit={(source) => startEditing(source, true)}
          onRemove={(source) => {
            remove.reset()
            save.reset()
            setNotice("")
            setRemoving(source)
          }}
          onToggle={(source, enabled) => {
            setNotice("")
            save.mutate({ ...source, enabled })
          }}
        />
      )}
      {editor && (
        <SourceEditor
          source={editor.source}
          sources={sources}
          editing={editor.editing}
          reporting={editor.reporting}
          busy={save.isPending}
          saveError={save.error}
          onClose={() => setEditor(null)}
          onSave={save.mutate}
        />
      )}
      {removing && (
        <DeleteSourceDialog
          source={removing}
          busy={remove.isPending}
          error={remove.error}
          onClose={() => setRemoving(null)}
          onRemove={() => remove.mutate(removing.key)}
        />
      )}
    </section>
  )
}

function useSources(onSaved: (message: string) => void) {
  const client = useQueryClient()
  const config = useConfig()
  const save = useMutation({
    mutationFn: (source: Source) => saveSource(source),
    onMutate: () => client.cancelQueries(configQuery),
    onSuccess: async (result) => {
      await applyConfigResult(client, (current) =>
        mergeSourceResult(current, result, "upsert"),
      )
      onSaved(`${result.sources[0]?.label ?? "Source"} saved.`)
    },
  })
  const remove = useMutation({
    mutationFn: (key: string) => deleteSource(key),
    onMutate: () => client.cancelQueries(configQuery),
    onSuccess: async (result, key) => {
      await applyConfigResult(client, (current) =>
        mergeSourceResult(current, result, "delete"),
      )
      onSaved(`${key} removed.`)
    },
  })
  return { config, save, remove }
}

function SourceLoadState({ config }: { config: UseQueryResult<ConfigResult> }) {
  if (config.isPending) return <output>Loading sources…</output>
  if (!config.isError) return null
  return (
    <div className="folio-error" role="alert">
      <p>{config.error.message}</p>
      <button
        type="button"
        onClick={() => {
          void config.refetch()
        }}
      >
        Retry
      </button>
    </div>
  )
}

function DeleteSourceDialog({
  source,
  busy,
  error,
  onClose,
  onRemove,
}: {
  source: Source
  busy: boolean
  error: Error | null
  onClose: () => void
  onRemove: () => void
}) {
  return (
    <Dialog
      title="Remove source?"
      onClose={onClose}
      busy={busy}
      actions={
        <>
          <button type="button" disabled={busy} onClick={onClose}>
            Keep source
          </button>
          <button
            type="button"
            className="folio-danger"
            disabled={busy}
            onClick={onRemove}
          >
            {busy ? "Removing…" : "Remove source"}
          </button>
        </>
      }
    >
      <p>
        This removes {source.label} and its latest-seen state. Future briefs
        will no longer include it. Past briefs will stay unchanged.
      </p>
      <p className="folio-muted">
        Source key: <code>{source.key}</code>
      </p>
      {error && <ErrorNote error={error} />}
    </Dialog>
  )
}
