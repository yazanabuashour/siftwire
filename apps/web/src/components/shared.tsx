import { useQuery } from "@tanstack/react-query"

import { fetchConfig } from "../api-client"

export function useConfig() {
  return useQuery({
    queryKey: ["config"],
    queryFn: ({ signal }) => fetchConfig(signal),
  })
}

export function Card({
  title,
  children,
}: {
  title: string
  children: React.ReactNode
}) {
  return (
    <section className="rounded-lg border border-(--border) bg-(--surface) p-4">
      <h2 className="mb-2 text-sm font-semibold text-(--muted) uppercase">
        {title}
      </h2>
      {children}
    </section>
  )
}

export function ErrorNote({ error }: { error: unknown }) {
  const message = error instanceof Error ? error.message : String(error)
  return <p className="text-sm text-red-400">{message}</p>
}
