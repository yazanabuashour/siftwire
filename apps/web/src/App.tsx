import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import DeliveriesPage from "./components/DeliveriesPage"
import OutletsPage from "./components/OutletsPage"
import OverviewPage from "./components/OverviewPage"
import RunsPage from "./components/RunsPage"
import SettingsPage from "./components/SettingsPage"
import SourcesPage from "./components/SourcesPage"

const client = new QueryClient()
const PAGES = [
  "overview",
  "sources",
  "outlets",
  "runs",
  "deliveries",
  "settings",
] as const
type Page = (typeof PAGES)[number]

function currentPage(): Page {
  const name = window.location.pathname.replace(/^\//, "") || "overview"
  return PAGES.find((candidate) => candidate === name) ?? "overview"
}

function navigate(page: Page): void {
  window.history.pushState({}, "", page === "overview" ? "/" : `/${page}`)
}

export default function App() {
  const [page, setPage] = useState<Page>(currentPage)
  useEffect(() => {
    const onPop = (): void => setPage(currentPage())
    window.addEventListener("popstate", onPop)
    return () => window.removeEventListener("popstate", onPop)
  }, [])

  return (
    <QueryClientProvider client={client}>
      <div className="mx-auto flex min-h-screen max-w-5xl flex-col gap-4 p-4">
        <header className="flex items-baseline gap-4 border-b border-(--border) pb-2">
          <h1 className="text-lg font-semibold text-(--accent)">
            Siftwire Console
          </h1>
          <nav className="flex gap-3 text-sm">
            {PAGES.map((name) => (
              <a
                key={name}
                href={`/${name}`}
                onClick={(event) => {
                  event.preventDefault()
                  setPage(name)
                  navigate(name)
                }}
                className="text-(--muted) hover:text-(--accent)"
              >
                {name[0]?.toUpperCase() + name.slice(1)}
              </a>
            ))}
          </nav>
        </header>
        <main>{renderPage(page)}</main>
      </div>
    </QueryClientProvider>
  )
}

function renderPage(page: Page) {
  switch (page) {
    case "sources":
      return <SourcesPage />
    case "outlets":
      return <OutletsPage />
    case "runs":
      return <RunsPage />
    case "deliveries":
      return <DeliveriesPage />
    case "settings":
      return <SettingsPage />
    default:
      return <OverviewPage />
  }
}
