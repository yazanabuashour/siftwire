import { useEffect, useId, type ReactNode } from "react"

import { IconX } from "./icons"

type ButtonVariant = "primary" | "secondary" | "danger"

const buttonVariants = {
  primary: "bg-accent text-canvas hover:bg-emerald-300",
  secondary:
    "border border-edge-strong text-ink hover:border-accent hover:text-accent",
  danger: "bg-danger-dim text-danger hover:bg-danger hover:text-canvas",
} satisfies Record<ButtonVariant, string>

export function Button({
  variant = "secondary",
  children,
  onClick,
  type = "button",
  disabled,
  formId,
}: {
  variant?: ButtonVariant | undefined
  children: ReactNode
  onClick?: (() => void) | undefined
  type?: "button" | "submit" | undefined
  disabled?: boolean | undefined
  formId?: string | undefined
}) {
  return (
    <button
      type={type}
      form={formId}
      onClick={onClick}
      disabled={disabled}
      className={`focus-visible:ring-accent inline-flex h-8 items-center gap-1.5 rounded-lg px-3 text-sm font-medium transition-colors focus-visible:ring-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50 ${buttonVariants[variant]}`}
    >
      {children}
    </button>
  )
}

export function IconButton(props: {
  "aria-label": string
  onClick?: (() => void) | undefined
  danger?: boolean | undefined
  children: ReactNode
}) {
  const label = props["aria-label"]
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={props.onClick}
      className={`focus-visible:ring-accent rounded-md p-1.5 transition-colors focus-visible:ring-2 focus-visible:outline-none ${
        props.danger
          ? "text-muted hover:bg-danger-dim hover:text-danger"
          : "text-muted hover:bg-raised hover:text-accent"
      }`}
    >
      {props.children}
    </button>
  )
}

export function Modal({
  title,
  subtitle,
  onClose,
  children,
  footer,
}: {
  title: ReactNode
  subtitle?: string | undefined
  onClose: () => void
  children: ReactNode
  footer?: ReactNode | undefined
}) {
  const titleId = useId()
  useEffect(() => {
    const previousOverflow = document.body.style.overflow
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape") onClose()
    }
    document.body.style.overflow = "hidden"
    window.addEventListener("keydown", onKey)
    return () => {
      document.body.style.overflow = previousOverflow
      window.removeEventListener("keydown", onKey)
    }
  }, [onClose])

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto p-4 sm:items-center">
      <button
        type="button"
        aria-label="Close modal"
        className="fixed inset-0 cursor-default bg-black/60"
        onClick={onClose}
      />
      <dialog
        open
        aria-labelledby={titleId}
        className="border-edge bg-surface text-ink relative my-8 w-full max-w-xl rounded-xl border p-0 shadow-2xl"
      >
        <header className="border-edge flex items-start justify-between gap-4 border-b px-5 py-4">
          <div>
            <h2 id={titleId} className="text-ink font-semibold">
              {title}
            </h2>
            {subtitle ? (
              <p className="text-muted mt-0.5 text-xs">{subtitle}</p>
            ) : null}
          </div>
          <button
            type="button"
            aria-label="Close"
            onClick={onClose}
            className="text-muted hover:bg-raised hover:text-ink focus-visible:ring-accent rounded-md p-1 focus-visible:ring-2 focus-visible:outline-none"
          >
            <IconX />
          </button>
        </header>
        <div className="px-5 py-4">{children}</div>
        {footer ? (
          <footer className="border-edge bg-canvas/40 flex items-center justify-end gap-2 rounded-b-xl border-t px-5 py-3.5">
            {footer}
          </footer>
        ) : null}
      </dialog>
    </div>
  )
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string | undefined
  children: ReactNode
}) {
  return (
    <label className="block">
      <span className="text-muted mb-1 block text-xs font-medium">{label}</span>
      {children}
      {hint ? (
        <span className="text-muted mt-1 block text-xs">{hint}</span>
      ) : null}
    </label>
  )
}

export const inputClass =
  "h-8 w-full rounded-lg border border-edge bg-canvas px-2.5 text-sm text-ink placeholder:text-muted/60 focus:border-accent focus:outline-none disabled:opacity-50"

export function Toggle({
  label,
  checked,
  onChange,
  hideLabel = false,
}: {
  label: string
  checked: boolean
  onChange: (checked: boolean) => void
  hideLabel?: boolean | undefined
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className="focus-visible:ring-accent inline-flex items-center gap-2.5 focus-visible:ring-2 focus-visible:outline-none"
    >
      <span
        className={`relative h-5 w-9 rounded-full transition-colors ${checked ? "bg-accent" : "bg-raised border-edge-strong border"}`}
      >
        <span
          className={`absolute top-0.5 h-4 w-4 rounded-full bg-white transition-all ${checked ? "left-4.5" : "left-0.5"}`}
        />
      </span>
      {hideLabel ? null : <span className="text-ink text-sm">{label}</span>}
    </button>
  )
}
