import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useEffect } from "react"

import OutletsPage from "./components/OutletsPage"
import OverviewPage from "./components/OverviewPage"
import RunsPage from "./components/RunsPage"
import SettingsPage from "./components/SettingsPage"
import SourcesPage from "./components/SourcesPage"
import { AppLink, runHref, useNavigation } from "./navigation"

import "./styles/navigation.css"

const client = new QueryClient()
const pages = [
  { id: "overview", path: "/", label: "Brief" },
  { id: "sources", path: "/sources", label: "Sources" },
  { id: "runs", path: "/runs", label: "Activity" },
  { id: "settings", path: "/settings", label: "Settings" },
] as const

export default function App() {
  const { page, runId } = useNavigation()
  const section = page === "outlets" ? "sources" : page
  useEffect(() => {
    document.title = `${pages.find(({ id }) => id === section)?.label} | SiftWire`
    window.scrollTo(0, 0)
  }, [page, section])
  return (
    <QueryClientProvider client={client}>
      <div
        className={`folio-app folio-${section === "overview" ? "reader" : "management"}`}
      >
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
            <AppLink className="folio-brand" href={runHref("/", runId)}>
              SiftWire
            </AppLink>
          </div>
          <nav aria-label="Main navigation">
            {pages.map(({ id, path, label }) => (
              <AppLink
                key={id}
                href={runHref(path, runId)}
                aria-current={section === id ? "page" : undefined}
              >
                {label}
              </AppLink>
            ))}
          </nav>
          {section === "sources" && (
            <nav className="folio-source-nav" aria-label="Source management">
              <AppLink
                href={runHref("/sources", runId)}
                aria-current={page === "sources" ? "page" : undefined}
              >
                Feeds
              </AppLink>
              <AppLink
                href={runHref("/outlets", runId)}
                aria-current={page === "outlets" ? "page" : undefined}
              >
                Publisher rules
              </AppLink>
            </nav>
          )}
        </header>
        <main id="folio-content" tabIndex={-1} key={page}>
          {page === "overview" && <OverviewPage />}
          {page === "sources" && <SourcesPage />}
          {page === "outlets" && <OutletsPage />}
          {page === "runs" && <RunsPage />}
          {page === "settings" && <SettingsPage />}
        </main>
      </div>
    </QueryClientProvider>
  )
}
