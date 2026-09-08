import { act } from "react"
import { expect, it, vi } from "vitest"

import { button, host, render, setupConfigTest } from "./config-test-utils"
import { Dialog } from "./ui"

setupConfigTest()

function show(onClose: () => void, busy = false) {
  render(
    <Dialog
      title="Edit source"
      busy={busy}
      onClose={onClose}
      actions={<button type="button">Save</button>}
    >
      <p>Source settings</p>
    </Dialog>,
  )
  const dialog = host.querySelector("dialog")
  if (!dialog) throw new Error("Missing dialog")
  vi.spyOn(dialog, "getBoundingClientRect").mockReturnValue(
    new DOMRect(100, 100, 640, 400),
  )
  return dialog
}

function pointer(
  target: Element,
  type: string,
  x: number,
  y: number,
  mouseButton = 0,
): void {
  act(() =>
    target.dispatchEvent(
      new PointerEvent(type, {
        bubbles: true,
        clientX: x,
        clientY: y,
        button: mouseButton,
      }),
    ),
  )
}

it("dismisses a complete backdrop press, not content, padding, scrollbar or drag gestures", () => {
  const close = vi.fn()
  const dialog = show(close)
  const content = dialog.querySelector("p")
  if (!content) throw new Error("Missing content")
  pointer(content, "pointerdown", 120, 120)
  pointer(content, "pointerup", 120, 120)
  for (const x of [120, 735]) {
    pointer(dialog, "pointerdown", x, 120)
    pointer(dialog, "pointerup", x, 120)
  }
  pointer(content, "pointerdown", 120, 120)
  pointer(dialog, "pointerup", 20, 20)
  pointer(dialog, "pointerdown", 20, 20)
  pointer(dialog, "pointerup", 120, 120)
  pointer(dialog, "pointerdown", 20, 20, 2)
  pointer(dialog, "pointerup", 20, 20, 2)
  expect(close).not.toHaveBeenCalled()
  pointer(dialog, "pointerdown", 20, 20)
  pointer(dialog, "pointerup", 20, 20)
  expect(close).toHaveBeenCalledOnce()
})

it.each(["backdrop", "escape", "close"])(
  "blocks %s dismissal during a write and allows it afterward",
  (method) => {
    const close = vi.fn()
    const dismiss = (dialog: HTMLDialogElement): void => {
      if (method === "backdrop") {
        pointer(dialog, "pointerdown", 20, 20)
        pointer(dialog, "pointerup", 20, 20)
      } else if (method === "escape") {
        act(() =>
          dialog.dispatchEvent(new Event("cancel", { cancelable: true })),
        )
      } else act(() => button("Close dialog").click())
    }
    dismiss(show(close, true))
    expect(button("Close dialog").disabled).toBe(true)
    expect(close).not.toHaveBeenCalled()
    dismiss(show(close))
    expect(close).toHaveBeenCalledOnce()
  },
)
