import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseQueryResult,
} from "@tanstack/react-query"
import { useState } from "react"

import { deleteSource, fetchConfig, saveSource } from "../api-client"
import type { ConfigResult, Source } from "../api-contracts"
import SourceCollection from "./SourceCollection"
import SourceEditor, { emptySource } from "./SourceEditor"
import { Dialog, PageHeading } from "./ui"

export default function SourcesPage() {
  const [editor, setEditor] = useState<{
    source: Source
    editing: boolean
  } | null>(null)
  const [removing, setRemoving] = useState<Source | null>(null)
  const [notice, setNotice] = useState("")
  const { config, save, remove } = useSources((message) => {
    setNotice(message)
    setEditor(null)
    setRemoving(null)
  })
  const busy = save.isPending || remove.isPending
  const ready = config.isSuccess && !config.data.rejected
  const sources = config.data?.sources ?? []
  const startEditing = (source: Source, editing: boolean): void => {
    save.reset()
    remove.reset()
    setNotice("")
    setEditor({ source, editing })
  }
  return (
    <>
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
    </>
  )
}

function useSources(onSaved: (message: string) => void) {
  const client = useQueryClient()
  const config = useQuery({
    queryKey: ["config"],
    queryFn: async ({ signal }) => {
      const result = await fetchConfig(signal)
      if (result.rejected)
        throw new Error("The runner rejected loading sources.")
      return result
    },
  })
  const save = useMutation({
    mutationFn: async (source: Source) => {
      const result = await saveSource(source)
      if (result.rejected)
        throw new Error("The runner rejected saving this source.")
      return result
    },
    onSuccess: async (_result, source) => {
      await client.invalidateQueries({ queryKey: ["config"] })
      onSaved(`${source.label} saved.`)
    },
  })
  const remove = useMutation({
    mutationFn: async (key: string) => {
      const result = await deleteSource(key)
      if (result.rejected)
        throw new Error(
          result.summary || "The runner rejected removing this source.",
        )
      return result
    },
    onSuccess: async (_result, key) => {
      await client.invalidateQueries({ queryKey: ["config"] })
      onSaved(`${key} removed.`)
    },
  })
  return { config, save, remove }
}

function SourceLoadState({ config }: { config: UseQueryResult<ConfigResult> }) {
  if (config.isPending) return <output>Loading sources…</output>
  if (!config.isError && !config.data?.rejected) return null
  return (
    <div className="folio-error" role="alert">
      <p>{config.error?.message || "The runner rejected loading sources."}</p>
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
      title={`Remove ${source.label}?`}
      onClose={() => {
        if (!busy) onClose()
      }}
    >
      <p>
        This removes {source.key} and its latest-seen state. Future briefs will
        no longer include it. Past briefs will stay unchanged.
      </p>
      {error && (
        <p className="folio-error" role="alert">
          {error.message}
        </p>
      )}
      <div className="folio-form-actions">
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
      </div>
    </Dialog>
  )
}
