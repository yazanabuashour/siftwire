import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import DeliveriesPage from "./components/DeliveriesPage"
import {
  IconFeed,
  IconHome,
  IconMail,
  IconPulse,
  IconShield,
  IconSliders,
} from "./components/icons"
import OutletsPage from "./components/OutletsPage"
import OverviewPage from "./components/OverviewPage"
import RunsPage from "./components/RunsPage"
import SettingsPage from "./components/SettingsPage"
import SourcesPage from "./components/SourcesPage"

const client = new QueryClient()
const NAV_ITEMS = [
  { id: "overview", label: "Overview", icon: IconHome, group: null },
  { id: "sources", label: "Sources", icon: IconFeed, group: "Data" },
  { id: "outlets", label: "Outlets", icon: IconShield, group: "Data" },
  { id: "runs", label: "Runs", icon: IconPulse, group: "Activity" },
  { id: "deliveries", label: "Deliveries", icon: IconMail, group: "Activity" },
  { id: "settings", label: "Settings", icon: IconSliders, group: "System" },
] as const
const NAV_GROUPS = [null, "Data", "Activity", "System"] as const

type Page = (typeof NAV_ITEMS)[number]["id"]
type NavItem = (typeof NAV_ITEMS)[number]

function currentPage(): Page {
  const name = window.location.pathname.replace(/^\//, "") || "overview"
  return NAV_ITEMS.find((item) => item.id === name)?.id ?? "overview"
}

function pagePath(page: Page): string {
  return page === "overview" ? "/" : `/${page}`
}

export default function App() {
  const [page, setPage] = useState<Page>(currentPage)
  useEffect(() => {
    const onPop = (): void => setPage(currentPage())
    window.addEventListener("popstate", onPop)
    return () => window.removeEventListener("popstate", onPop)
  }, [])
  const select = (next: Page): void => {
    setPage(next)
    window.history.pushState({}, "", pagePath(next))
  }

  return (
    <QueryClientProvider client={client}>
      <div className="min-h-screen">
        <Sidebar page={page} onNavigate={select} />
        <MobileNav page={page} onNavigate={select} />
        <main className="min-w-0 md:ml-56">
          <div className="mx-auto max-w-6xl px-4 py-5 sm:px-6 md:px-8 md:py-7">
            {renderPage(page)}
          </div>
        </main>
      </div>
    </QueryClientProvider>
  )
}

function Brand() {
  return (
    <div className="flex items-center gap-2.5">
      <span className="bg-accent text-canvas flex h-7 w-7 items-center justify-center rounded-lg">
        <IconFeed className="h-4 w-4" />
      </span>
      <span className="text-ink text-[15px] font-semibold tracking-tight">
        SiftWire
      </span>
    </div>
  )
}

function Sidebar({
  page,
  onNavigate,
}: {
  page: Page
  onNavigate: (page: Page) => void
}) {
  return (
    <aside className="border-edge bg-sidebar fixed inset-y-0 left-0 hidden w-56 flex-col border-r md:flex">
      <div className="px-4 py-4">
        <Brand />
      </div>
      <nav className="flex-1 overflow-y-auto px-3 pb-4">
        {NAV_GROUPS.map((group, index) => (
          <div key={group ?? "main"} className={index === 0 ? "" : "mt-5"}>
            {group ? (
              <p className="text-muted/70 px-2.5 pb-1.5 text-[10px] font-semibold tracking-widest uppercase">
                {group}
              </p>
            ) : null}
            {NAV_ITEMS.filter((item) => item.group === group).map((item) => (
              <NavLink
                key={item.id}
                item={item}
                active={item.id === page}
                onNavigate={onNavigate}
              />
            ))}
          </div>
        ))}
      </nav>
      <p className="border-edge text-muted border-t px-4 py-3 text-xs">
        Local console
      </p>
    </aside>
  )
}

function MobileNav({
  page,
  onNavigate,
}: {
  page: Page
  onNavigate: (page: Page) => void
}) {
  return (
    <header className="border-edge bg-sidebar border-b md:hidden">
      <div className="px-4 py-3">
        <Brand />
      </div>
      <nav
        className="grid grid-cols-3 gap-1 px-3 pb-2"
        aria-label="Main navigation"
      >
        {NAV_ITEMS.map((item) => (
          <NavLink
            key={item.id}
            item={item}
            active={item.id === page}
            onNavigate={onNavigate}
            compact
          />
        ))}
      </nav>
    </header>
  )
}

function NavLink({
  item,
  active,
  onNavigate,
  compact = false,
}: {
  item: NavItem
  active: boolean
  onNavigate: (page: Page) => void
  compact?: boolean | undefined
}) {
  return (
    <a
      href={pagePath(item.id)}
      onClick={(event) => {
        event.preventDefault()
        onNavigate(item.id)
      }}
      aria-current={active ? "page" : undefined}
      className={`relative flex shrink-0 items-center gap-2 rounded-lg text-sm transition-colors ${
        compact ? "justify-center px-2 py-1.5 text-xs" : "mt-0.5 px-2.5 py-1.5"
      } ${
        active
          ? "bg-accent-dim text-accent before:bg-accent font-medium before:absolute before:top-1/2 before:left-0 before:h-4 before:w-0.5 before:-translate-y-1/2 before:rounded-full"
          : "text-muted hover:bg-raised hover:text-ink"
      }`}
    >
      <item.icon className="h-4 w-4" />
      {item.label}
    </a>
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
