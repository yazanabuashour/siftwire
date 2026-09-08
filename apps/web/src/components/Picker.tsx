import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react"

import { usePickerPopup } from "./usePickerPopup"

import "./picker.css"

export interface PickerOption {
  id: string
  label: string
  description: string
}

interface PickerProps {
  label: string
  value: PickerOption | undefined
  placeholder: string
  options: PickerOption[]
  search: string
  onSearch: (search: string) => void
  onChoose: (id: string) => void
  onOpenChange: (open: boolean) => void
  help: string
  loading: boolean
  footer?: ReactNode
}

function PickerResults({
  id,
  options,
  activeId,
  onActiveChange,
  onChoose,
}: {
  id: string
  options: PickerOption[]
  activeId: string | undefined
  onActiveChange: (id: string) => void
  onChoose: (id: string) => void
}) {
  const list = useRef<HTMLUListElement>(null)
  useLayoutEffect(() => {
    const container = list.current
    const active = container?.querySelector('[aria-selected="true"]')
    if (!container || !(active instanceof HTMLElement)) return
    if (active.offsetTop < container.scrollTop)
      container.scrollTop = active.offsetTop
    else if (
      active.offsetTop + active.offsetHeight >
      container.scrollTop + container.clientHeight
    )
      container.scrollTop =
        active.offsetTop + active.offsetHeight - container.clientHeight
  })
  return (
    /* oxlint-disable jsx-a11y/prefer-tag-over-role, jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-to-interactive-role -- The combobox owns keyboard focus and option activation through aria-activedescendant. */
    <ul
      className="folio-picker-list"
      id={`${id}-results`}
      role="listbox"
      aria-label="Matching records"
      ref={list}
    >
      {options.map((option) => (
        <li
          className="folio-picker-option"
          id={`${id}-option-${option.id}`}
          role="option"
          aria-selected={activeId === option.id}
          tabIndex={-1}
          key={option.id}
          onMouseDown={(event) => event.preventDefault()}
          onPointerMove={(event) => {
            if (event.pointerType === "mouse") onActiveChange(option.id)
          }}
          onClick={() => onChoose(option.id)}
        >
          <span>{option.label}</span>
          <small>{option.description}</small>
        </li>
      ))}
    </ul>
    /* oxlint-enable jsx-a11y/prefer-tag-over-role, jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-to-interactive-role */
  )
}

function browseOptions(
  event: KeyboardEvent<HTMLInputElement>,
  options: PickerOption[],
  activeId: string | undefined,
  highlight: (id: string | undefined) => void,
  choose: (id: string) => void,
) {
  if (event.nativeEvent.isComposing) return
  if (event.key === "Enter") {
    event.preventDefault()
    if (activeId !== undefined) choose(activeId)
  } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault()
    if (!options.length) return
    const current = options.findIndex(({ id }) => id === activeId)
    const offset = event.key === "ArrowDown" ? 1 : -1
    highlight(options[(current + offset + options.length) % options.length]?.id)
  }
}

function PickerPanel({
  id,
  trigger,
  onClose,
  ...props
}: PickerProps & {
  id: string
  trigger: RefObject<HTMLButtonElement | null>
  onClose: (restoreFocus: boolean) => void
}) {
  const [highlight, setHighlight] = useState(props.value?.id)
  const input = useRef<HTMLInputElement>(null)
  const popup = useRef<HTMLDialogElement>(null)
  const style = usePickerPopup(trigger, popup, onClose)
  const activeId =
    props.options.find(({ id: optionId }) => optionId === highlight)?.id ??
    props.options[0]?.id
  useLayoutEffect(() => {
    if (style.visibility === "visible") input.current?.focus()
  }, [style.visibility])
  function choose(id: string) {
    props.onChoose(id)
    onClose(true)
  }
  return (
    <dialog
      open
      ref={popup}
      id={`${id}-popup`}
      aria-label={`${props.label}: search records`}
      className="folio-picker-popup"
      style={style}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.nativeEvent.isComposing) {
          event.preventDefault()
          event.stopPropagation()
          onClose(true)
        }
      }}
    >
      <div className="folio-picker-input-wrap">
        <label className="folio-sr-only" htmlFor={`${id}-search`}>
          Search records
        </label>
        <input
          ref={input}
          id={`${id}-search`}
          role="combobox"
          type="text"
          placeholder="Date, run ID or summary"
          autoComplete="off"
          spellCheck={false}
          aria-expanded="true"
          aria-autocomplete="list"
          aria-controls={`${id}-results`}
          aria-activedescendant={
            activeId !== undefined ? `${id}-option-${activeId}` : undefined
          }
          aria-describedby={`${id}-search-help`}
          value={props.search}
          onKeyDown={(event) =>
            browseOptions(event, props.options, activeId, setHighlight, choose)
          }
          onChange={(event) => {
            props.onSearch(event.target.value)
            setHighlight(undefined)
          }}
        />
        <p id={`${id}-search-help`} className="folio-muted">
          {props.help} Use arrow keys to browse and Enter to choose.
        </p>
      </div>
      <PickerResults
        id={id}
        options={props.options}
        activeId={activeId}
        onActiveChange={setHighlight}
        onChoose={choose}
      />
      <div className="folio-picker-footer">
        <output aria-live="polite">
          {props.loading
            ? "Loading records…"
            : `${props.options.length} records loaded.`}
        </output>
        {props.footer}
      </div>
    </dialog>
  )
}

// Locally adapted from Salary Atlas's CitySearch. Filtering and order belong to
// the caller; typing and highlighting never change the committed value.
export function Picker(props: PickerProps) {
  const id = useId()
  const [open, setOpen] = useState(false)
  const trigger = useRef<HTMLButtonElement>(null)
  const root = useRef<HTMLDivElement>(null)
  const { onOpenChange, onSearch } = props
  const close = useCallback(
    (restoreFocus: boolean) => {
      setOpen(false)
      onOpenChange(false)
      onSearch("")
      if (restoreFocus) trigger.current?.focus()
    },
    [onOpenChange, onSearch],
  )
  useEffect(() => {
    if (!open) return
    function dismissOutside(event: PointerEvent) {
      if (event.target instanceof Node && !root.current?.contains(event.target))
        close(false)
    }
    document.addEventListener("pointerdown", dismissOutside, true)
    return () =>
      document.removeEventListener("pointerdown", dismissOutside, true)
  }, [open, close])
  function show() {
    setOpen(true)
    onOpenChange(true)
  }
  return (
    <div
      className={`folio-picker${open ? " folio-picker-open" : ""}`}
      ref={root}
      onBlur={(event) => {
        if (
          event.relatedTarget instanceof Node &&
          !event.currentTarget.contains(event.relatedTarget)
        )
          close(false)
      }}
    >
      <span id={`${id}-label`} className="folio-picker-label">
        {props.label}
      </span>
      <button
        type="button"
        className="folio-picker-trigger"
        ref={trigger}
        aria-labelledby={`${id}-label ${id}-value`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? `${id}-popup` : undefined}
        onClick={() => (open ? close(false) : show())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault()
            show()
          }
        }}
      >
        <span id={`${id}-value`}>
          {props.value?.label ?? props.placeholder}
        </span>
        <span aria-hidden="true">⌄</span>
      </button>
      {open && (
        <PickerPanel {...props} id={id} trigger={trigger} onClose={close} />
      )}
    </div>
  )
}
