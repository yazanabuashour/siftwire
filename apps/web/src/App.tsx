import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useEffect, useState, type MouseEvent } from "react"

import DeliveriesPage from "./components/DeliveriesPage"
import OutletsPage from "./components/OutletsPage"
import OverviewPage from "./components/OverviewPage"
import RunsPage from "./components/RunsPage"
import SettingsPage from "./components/SettingsPage"
import SourcesPage from "./components/SourcesPage"

const client = new QueryClient()
const pages = [
  { id: "overview", label: "Brief" },
  { id: "sources", label: "Sources" },
  { id: "outlets", label: "Publishers" },
  { id: "runs", label: "History" },
  { id: "deliveries", label: "Sent briefs" },
  { id: "settings", label: "Settings" },
] as const

type Page = (typeof pages)[number]["id"]

function currentPage(): Page {
  const path = window.location.pathname.replace(/^\//, "")
  return pages.find(({ id }) => id === path)?.id ?? "overview"
}

function pagePath(page: Page): string {
  return page === "overview" ? "/" : `/${page}`
}

export default function App() {
  const [page, setPage] = useState(currentPage)
  useEffect(() => {
    const change = (): void => setPage(currentPage())
    window.addEventListener("popstate", change)
    return () => window.removeEventListener("popstate", change)
  }, [])
  useEffect(() => {
    document.title = `${pages.find(({ id }) => id === page)?.label} | SiftWire`
    window.scrollTo(0, 0)
  }, [page])
  const navigate = (event: MouseEvent<HTMLAnchorElement>, next: Page): void => {
    if (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    )
      return
    event.preventDefault()
    if (next !== page) {
      window.history.pushState({}, "", pagePath(next))
      setPage(next)
    }
  }
  return (
    <QueryClientProvider client={client}>
      <div className="folio-app">
        <a
          className="folio-skip"
          href="#folio-content"
          onClick={(event) => {
            event.preventDefault()
            document.getElementById("folio-content")?.focus()
          }}
        >
          Skip to content
        </a>
        <header className="folio-masthead">
          <div className="folio-brand-line">
            <a
              className="folio-brand"
              href="/"
              onClick={(event) => navigate(event, "overview")}
            >
              SiftWire
            </a>
          </div>
          <nav aria-label="Main navigation">
            {pages.map(({ id, label }) => (
              <a
                key={id}
                href={pagePath(id)}
                onClick={(event) => navigate(event, id)}
                aria-current={page === id ? "page" : undefined}
              >
                {label}
              </a>
            ))}
          </nav>
        </header>
        <main id="folio-content" tabIndex={-1} key={page}>
          {page === "overview" && <OverviewPage />}
          {page === "sources" && <SourcesPage />}
          {page === "outlets" && <OutletsPage />}
          {page === "runs" && <RunsPage />}
          {page === "deliveries" && <DeliveriesPage />}
          {page === "settings" && <SettingsPage />}
        </main>
      </div>
    </QueryClientProvider>
  )
}
