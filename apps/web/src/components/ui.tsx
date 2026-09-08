import {
  useEffect,
  useId,
  useRef,
  type PointerEvent,
  type ReactNode,
} from "react"

export function dateLabel(value: string, withTime = false): string {
  if (!value || Number.isNaN(Date.parse(value))) return value || "Not recorded"
  const options: Intl.DateTimeFormatOptions = {
    dateStyle: "long",
    timeZone: "UTC",
  }
  if (withTime) options.timeStyle = "short"
  return (
    new Intl.DateTimeFormat("en-GB", options).format(new Date(value)) +
    (withTime ? " UTC" : "")
  )
}

export function ErrorNote({ error }: { error: Error }) {
  const ref = useRef<HTMLParagraphElement>(null)
  useEffect(() => {
    const message = ref.current
    const body = message?.closest(".folio-dialog-body")
    // Fixed Save actions must reveal repeated failures, not just new messages.
    if (message && body instanceof HTMLElement)
      body.scrollTop +=
        message.getBoundingClientRect().top - body.getBoundingClientRect().top
  }, [error])
  return (
    <p ref={ref} className="folio-error" role="alert">
      {error.message}
    </p>
  )
}

export function PageHeading({
  title,
  children,
  action,
}: {
  title: string
  children?: ReactNode
  action?: ReactNode
}) {
  return (
    <header className="folio-page-heading">
      <div>
        <h1>{title}</h1>
        {children && <p>{children}</p>}
      </div>
      {action}
    </header>
  )
}

export function Field({
  label,
  hint,
  children,
  wide = false,
}: {
  label: string
  hint?: string | undefined
  children: ReactNode
  wide?: boolean
}) {
  return (
    <label className={`folio-field${wide ? " folio-wide" : ""}`}>
      <span>{label}</span>
      {children}
      {hint && <small>{hint}</small>}
    </label>
  )
}

function isBackdrop(event: PointerEvent<HTMLDialogElement>): boolean {
  if (event.target !== event.currentTarget) return false
  const bounds = event.currentTarget.getBoundingClientRect()
  return (
    event.clientX < bounds.left ||
    event.clientX > bounds.right ||
    event.clientY < bounds.top ||
    event.clientY > bounds.bottom
  )
}

export function Dialog({
  title,
  onClose,
  children,
  actions,
  busy = false,
}: {
  title: string
  onClose: () => void
  children: ReactNode
  actions: ReactNode
  busy?: boolean
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const pressedBackdrop = useRef(false)
  const titleId = useId()
  useEffect(() => {
    const dialog = ref.current
    const opener = document.activeElement
    dialog?.showModal()
    return () => {
      dialog?.close()
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus()
      else
        document
          .querySelector<HTMLButtonElement>("#folio-content button")
          ?.focus()
    }
  }, [])
  return (
    <dialog
      ref={ref}
      className="folio-dialog"
      aria-labelledby={titleId}
      onCancel={(event) => {
        event.preventDefault()
        if (!busy) onClose()
      }}
      onPointerDown={(event) => {
        pressedBackdrop.current = isBackdrop(event)
      }}
      onPointerUp={(event) => {
        if (
          !busy &&
          event.button === 0 &&
          pressedBackdrop.current &&
          isBackdrop(event)
        )
          onClose()
        pressedBackdrop.current = false
      }}
    >
      <header className="folio-dialog-heading">
        <h2 id={titleId}>{title}</h2>
        <button
          type="button"
          aria-label="Close dialog"
          disabled={busy}
          onClick={onClose}
        >
          Close
        </button>
      </header>
      <div className="folio-dialog-body">{children}</div>
      <footer className="folio-dialog-actions">{actions}</footer>
    </dialog>
  )
}
