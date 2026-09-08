import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"

import { replaceOutlets } from "../api-client"
import type { OutletPolicy } from "../api-contracts"
import {
  applyConfigResult,
  configQuery,
  mergeOutletsResult,
  useConfig,
} from "./config-query"

const emptyOutlet: OutletPolicy = {
  name: "",
  aliases: [],
  policy: "allow",
  note: "",
  enabled: true,
}

export function useOutletEditor() {
  const config = useConfig()
  const client = useQueryClient()
  // A null draft follows refetches; a local draft preserves the collection.
  const [draft, setDraft] = useState<OutletPolicy[] | null>(null)
  const [editor, setEditor] = useState<{
    index: number | null
    row: OutletPolicy
    hadDraft: boolean
  } | null>(null)
  const [notice, setNotice] = useState("")
  const configured = config.data?.outlets ?? []
  const rows = draft ?? configured
  const dirty = JSON.stringify(rows) !== JSON.stringify(configured)
  const save = useMutation({
    mutationFn: (outlets: OutletPolicy[]) => replaceOutlets(outlets),
    onMutate: () => client.cancelQueries(configQuery),
    onSuccess: async (result) => {
      await applyConfigResult(client, (current) =>
        mergeOutletsResult(current, result),
      )
      setDraft(null)
      setNotice("Publishers saved.")
    },
  })
  const update = (next: OutletPolicy[]): void => {
    setDraft(next)
    setNotice("")
    save.reset()
  }
  const updateRow = (
    index: number,
    patch: Partial<Pick<OutletPolicy, "policy" | "enabled">>,
  ): void => {
    if (save.isPending || editor) return
    update(rows.map((row, at) => (at === index ? { ...row, ...patch } : row)))
  }
  const startEditing = (index: number | null, row = emptyOutlet): void => {
    update(rows)
    setEditor({ index, row, hadDraft: draft !== null })
  }
  const closeEditor = (): void => {
    if (!editor?.hadDraft) setDraft(null)
    setEditor(null)
  }
  const apply = (row: OutletPolicy): void => {
    if (!editor) return
    update(
      editor.index === null
        ? [...rows, row]
        : rows.map((entry, index) => (index === editor.index ? row : entry)),
    )
    setEditor(null)
  }
  const remove = (): void => {
    if (!editor || editor.index === null) return
    update(rows.filter((_row, index) => index !== editor.index))
    setEditor(null)
  }
  const discard = (): void => {
    setDraft(null)
    save.reset()
    setNotice("Publisher changes discarded.")
  }
  return {
    config,
    rows,
    dirty,
    save,
    discard,
    notice,
    hasDraft: draft !== null,
    editor,
    startEditing,
    updateRow,
    closeEditor,
    apply,
    remove,
  }
}

export function useOutletDraft(
  row: OutletPolicy,
  rows: OutletPolicy[],
  index: number | null,
) {
  const [draft, setDraft] = useState(() => outletDraft(row))
  const [error, setError] = useState<Error | null>(null)
  const update = (patch: Partial<OutletDraft>): void => {
    setDraft((current) => ({ ...current, ...patch }))
    setError(null)
  }
  const validate = (): OutletPolicy | null => {
    const next = outletFromDraft(draft)
    const problem = outletError(next, rows, index)
    setError(problem ? new Error(problem) : null)
    return problem ? null : next
  }
  return { draft, error, update, validate }
}

export type OutletDraft = OutletPolicy & { aliasText: string }

export function outletDraft(outlet: OutletPolicy): OutletDraft {
  return { ...outlet, aliasText: outlet.aliases.join(", ") }
}

export function outletFromDraft({
  aliasText,
  ...outlet
}: OutletDraft): OutletPolicy {
  return {
    ...outlet,
    // Editing another field must not rewrite aliases or optional notes.
    aliases:
      aliasText === outlet.aliases.join(", ")
        ? outlet.aliases
        : aliasText
            .split(",")
            .map((alias) => alias.trim())
            .filter(Boolean),
  }
}

export function outletError(
  outlet: OutletPolicy,
  rows: OutletPolicy[],
  index: number | null,
): string {
  if (!outlet.name.trim()) return "Enter a publisher name."
  if (
    rows.some(
      (row, at) =>
        at !== index &&
        row.name.trim().toLowerCase() === outlet.name.trim().toLowerCase(),
    )
  )
    return "That publisher name already exists. Edit its rule instead."
  return ""
}
