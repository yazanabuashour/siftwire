import { useEffect, useId, useRef, type ReactNode } from "react"

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
  return (
    <p className="folio-error" role="alert">
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

export function Dialog({
  title,
  onClose,
  children,
}: {
  title: string
  onClose: () => void
  children: ReactNode
}) {
  const ref = useRef<HTMLDialogElement>(null)
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
        onClose()
      }}
    >
      <header className="folio-dialog-heading">
        <h2 id={titleId}>{title}</h2>
      </header>
      {children}
    </dialog>
  )
}
