import { useSyncExternalStore, type ComponentProps } from "react"

const navigationEvent = "siftwire:navigate"

function subscribe(onChange: () => void) {
  window.addEventListener("popstate", onChange)
  window.addEventListener(navigationEvent, onChange)
  return () => {
    window.removeEventListener("popstate", onChange)
    window.removeEventListener(navigationEvent, onChange)
  }
}

export function routeFor(pathname: string) {
  switch (pathname.replace(/\/$/, "")) {
    case "/sources":
      return "sources"
    case "/outlets":
      return "outlets"
    case "/runs":
    case "/activity":
      return "runs"
    case "/settings":
      return "settings"
    default:
      return "overview"
  }
}

export function useNavigation() {
  const href = useSyncExternalStore(subscribe, () => window.location.href)
  const url = new URL(href)
  return {
    page: routeFor(url.pathname),
    runId: url.searchParams.get("run") ?? undefined,
  }
}

export function runHref(path: string, runId: string | undefined): string {
  const search = new URLSearchParams()
  if (runId !== undefined) search.set("run", runId)
  return path + (search.size ? `?${search}` : "")
}

export function navigate(href: string) {
  const url = new URL(href, window.location.href)
  if (url.href === window.location.href) return
  window.history.pushState({}, "", url)
  window.dispatchEvent(new Event(navigationEvent))
}

export function selectRun(runId: string | undefined) {
  const url = new URL(window.location.href)
  if (runId === undefined) url.searchParams.delete("run")
  else url.searchParams.set("run", runId)
  navigate(url.href)
}

// Only authored app links use this handler. Recorded HTML and story links keep
// their browser behavior, including links inside the isolated email frame.
export function AppLink({
  onClick,
  href,
  children,
  ...props
}: ComponentProps<"a">) {
  return (
    <a
      {...props}
      href={href}
      onClick={(event) => {
        onClick?.(event)
        const link = event.currentTarget
        if (
          event.defaultPrevented ||
          event.button !== 0 ||
          event.metaKey ||
          event.ctrlKey ||
          event.shiftKey ||
          event.altKey ||
          (link.target && link.target !== "_self") ||
          link.hasAttribute("download") ||
          !link.hasAttribute("href") ||
          new URL(link.href).origin !== window.location.origin
        )
          return
        event.preventDefault()
        navigate(link.href)
      }}
    >
      {children}
    </a>
  )
}
