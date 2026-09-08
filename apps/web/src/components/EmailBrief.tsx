import { useEffect, useRef } from "react"

// Keep email CSS inside the frame. Only remote images may load automatically;
// scripts, forms, embedded pages, and external styles stay disabled.
const emailPolicy =
  "default-src 'none'; img-src https: http:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"

export default function EmailBrief({ html }: { html: string }) {
  const frame = useRef<HTMLIFrameElement>(null)

  useEffect(() => {
    const element = frame.current
    if (!element) return
    let pending = 0
    let observer: ResizeObserver | undefined

    function observe() {
      const document = element?.contentDocument
      // Do not wait for the frame's load event: slow logos must not delay layout.
      if (
        !document?.body ||
        document.URL !== "about:srcdoc" ||
        document.readyState === "loading"
      ) {
        pending = requestAnimationFrame(observe)
        return
      }
      for (const link of document.querySelectorAll("a")) {
        link.target = "_blank"
        link.rel = "noopener noreferrer"
        if (!/^https?:\/\//i.test(link.getAttribute("href") ?? ""))
          link.removeAttribute("href")
      }
      const resize = () => {
        if (element)
          element.style.height = `${Math.ceil(document.body.getBoundingClientRect().height)}px`
      }
      observer = new ResizeObserver(resize)
      observer.observe(document.body)
      resize()
    }

    observe()
    return () => {
      cancelAnimationFrame(pending)
      observer?.disconnect()
    }
  }, [html])

  return (
    <iframe
      ref={frame}
      className="folio-email"
      title="Recorded email brief"
      sandbox="allow-same-origin allow-popups allow-popups-to-escape-sandbox"
      referrerPolicy="no-referrer"
      srcDoc={`<!doctype html><meta http-equiv="Content-Security-Policy" content="${emailPolicy}"><meta name="referrer" content="no-referrer"><base target="_blank">${html}`}
    />
  )
}
