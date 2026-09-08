import { act, useState } from "react"
import { describe, expect, it, vi } from "vitest"

import {
  click,
  element,
  flush,
  key,
  setupNavigationTest,
  typeSearch,
} from "../navigation-test-utils"
import { Picker, type PickerOption } from "./Picker"

const render = setupNavigationTest()
const records: PickerOption[] = [
  { id: "new", label: "Newest", description: "New summary" },
  { id: "middle", label: "Middle", description: "Middle summary" },
  { id: "old", label: "Oldest", description: "Old summary" },
]

function Example({ choose }: { choose?: (id: string) => void }) {
  const [search, setSearch] = useState("")
  const [value, setValue] = useState(records[1])
  const [open, setOpen] = useState(false)
  return (
    <div className="folio-app">
      <Picker
        label="Choose a run"
        value={value}
        placeholder="Latest"
        options={records.filter((record) =>
          record.label.toLowerCase().includes(search.toLowerCase()),
        )}
        search={search}
        onSearch={setSearch}
        onChoose={(id) => {
          choose?.(id)
          setValue(records.find((record) => record.id === id))
        }}
        onOpenChange={setOpen}
        help="Search records."
        loading={false}
        footer={<button type="button">Load older</button>}
      />
      <output data-open>{String(open)}</output>
      <button type="button" id="outside">
        Outside
      </button>
    </div>
  )
}

const trigger = () => element(".folio-picker-trigger")
const active = () => element('[role="option"][aria-selected="true"]')

// Adapted from Salary Atlas's CitySearch interactions, without its catalog or ranking.
describe("record picker commit and cancellation", () => {
  it("keeps caller order, searches without committing, ignores composition, and returns focus after choosing", async () => {
    const choose = vi.fn()
    await render(<Example choose={choose} />)
    await click(trigger())
    expect(document.activeElement).toBe(element('[role="combobox"]'))
    expect(
      [...document.querySelectorAll('[role="option"]')].map(
        (option) => option.textContent,
      ),
    ).toEqual([
      "NewestNew summary",
      "MiddleMiddle summary",
      "OldestOld summary",
    ])
    expect(active().textContent).toContain("Middle")
    await key("ArrowDown")
    expect(active().textContent).toContain("Oldest")
    await key("ArrowDown")
    expect(active().textContent).toContain("Newest")
    await key("ArrowUp")
    expect(active().textContent).toContain("Oldest")
    await typeSearch("New")
    expect(choose).not.toHaveBeenCalled()
    expect(trigger().textContent).toContain("Middle")
    expect(document.querySelectorAll('[role="option"]')).toHaveLength(1)
    await key("Enter", true)
    await key("Escape", true)
    expect(document.querySelector("dialog")).not.toBeNull()
    expect(choose).not.toHaveBeenCalled()
    await key("Enter")
    expect(choose).toHaveBeenCalledExactlyOnceWith("new")
    expect(document.querySelector("dialog")).toBeNull()
    expect(document.activeElement).toBe(trigger())
    expect(trigger().textContent).toContain("Newest")
  })

  it("handles no matches, cancels with Escape, and opens a fresh search with arrows", async () => {
    const choose = vi.fn()
    await render(<Example choose={choose} />)
    await click(trigger())
    await typeSearch("missing")
    expect(
      element('[role="combobox"]').hasAttribute("aria-activedescendant"),
    ).toBe(false)
    await key("ArrowDown")
    await key("Enter")
    expect(choose).not.toHaveBeenCalled()
    expect(document.querySelector("dialog")).not.toBeNull()
    await key("Escape")
    expect(document.activeElement).toBe(trigger())
    await act(() => {
      trigger().dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "ArrowDown",
          bubbles: true,
          cancelable: true,
        }),
      )
    })
    expect(element('[role="combobox"]').getAttribute("value")).toBe("")
    expect(document.querySelectorAll('[role="option"]')).toHaveLength(3)
    await click(element('[role="option"]'))
    expect(choose).toHaveBeenCalledExactlyOnceWith("new")
    expect(document.activeElement).toBe(trigger())
  })

  it("allows focus inside the footer, dismisses on focus leaving or outside taps without committing", async () => {
    const choose = vi.fn()
    await render(<Example choose={choose} />)
    await click(trigger())
    await typeSearch("New")
    await act(() => element(".folio-picker-footer button").focus())
    expect(document.querySelector("dialog")).not.toBeNull()
    // Focus transfer models Tab leaving the popup; jsdom does not perform native tab traversal.
    await act(() => element("#outside").focus())
    expect(document.querySelector("dialog")).toBeNull()
    expect(document.activeElement).toBe(element("#outside"))
    await click(trigger())
    expect(element('[role="combobox"]').getAttribute("value")).toBe("")
    await act(() => {
      element("#outside").dispatchEvent(
        new Event("pointerdown", { bubbles: true }),
      )
    })
    expect(document.querySelector("dialog")).toBeNull()
    expect(choose).not.toHaveBeenCalled()
    expect(trigger().textContent).toContain("Middle")
  })
})

describe("record picker viewport behavior", () => {
  it("keeps the active option in view without scrolling the page", async () => {
    await render(<Example />)
    await click(trigger())
    const list = element('[role="listbox"]')
    Object.defineProperty(list, "clientHeight", {
      configurable: true,
      value: 40,
    })
    document.querySelectorAll('[role="option"]').forEach((option, index) => {
      Object.defineProperty(option, "offsetTop", {
        configurable: true,
        value: index * 40,
      })
      Object.defineProperty(option, "offsetHeight", {
        configurable: true,
        value: 40,
      })
    })
    await key("ArrowDown")
    expect(list.scrollTop).toBe(80)
    await key("ArrowDown")
    expect(list.scrollTop).toBe(0)
    expect(document.activeElement).toBe(element('[role="combobox"]'))
  })

  it("repositions within the visual viewport, ignores list scrolling, and closes when the trigger leaves view", async () => {
    const viewport = Object.assign(new EventTarget(), {
      offsetTop: 0,
      offsetLeft: 0,
      width: 320,
      height: 600,
    })
    vi.stubGlobal("visualViewport", viewport)
    const bounds = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect")
    bounds.mockReturnValue(new DOMRect(280, 500, 300, 44))
    await render(<Example />)
    await click(trigger())
    const popup = element("dialog")
    expect(popup.style.transform).toBe("translateY(-100%)")
    expect(
      Number.parseFloat(popup.style.left) +
        Number.parseFloat(popup.style.width),
    ).toBeLessThanOrEqual(viewport.width)
    bounds.mockReturnValue(new DOMRect(20, 30, 280, 44))
    await act(() => {
      viewport.dispatchEvent(new Event("resize"))
    })
    expect(popup.style.transform).toBe("")
    expect(
      Number.parseFloat(popup.style.top) +
        Number.parseFloat(popup.style.height),
    ).toBeLessThanOrEqual(viewport.height)
    const top = popup.style.top
    bounds.mockReturnValue(new DOMRect(20, 1000, 280, 44))
    await act(() => {
      element('[role="listbox"]').dispatchEvent(
        new Event("scroll", { bubbles: true }),
      )
    })
    expect(popup.style.top).toBe(top)
    await act(() => {
      window.dispatchEvent(new Event("scroll"))
    })
    await flush()
    expect(document.querySelector("dialog")).toBeNull()
    expect(element("[data-open]").textContent).toBe("false")
  })
})
