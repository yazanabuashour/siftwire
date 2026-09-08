import {
  useLayoutEffect,
  useState,
  type CSSProperties,
  type RefObject,
} from "react"

// Adapted locally from Salary Atlas's useCityPopup. Available viewport space,
// not an option count or a fixed popup height, bounds the archive list.
export function usePickerPopup(
  trigger: RefObject<HTMLButtonElement | null>,
  popup: RefObject<HTMLDialogElement | null>,
  onClose: (restoreFocus: boolean) => void,
) {
  const [style, setStyle] = useState<CSSProperties>({ visibility: "hidden" })
  useLayoutEffect(() => {
    const viewport = window.visualViewport
    function position(event?: Event) {
      if (
        event?.target instanceof Node &&
        popup.current?.contains(event.target)
      )
        return
      const rect = trigger.current?.getBoundingClientRect()
      if (!rect || !popup.current) return
      const top = viewport?.offsetTop ?? 0
      const left = viewport?.offsetLeft ?? 0
      const width = viewport?.width ?? window.innerWidth
      const height = viewport?.height ?? window.innerHeight
      if (rect.bottom <= top || rect.top >= top + height) {
        onClose(false)
        return
      }
      const gap = Number.parseFloat(getComputedStyle(popup.current).rowGap) || 0
      const above = Math.max(0, rect.top - top - gap)
      const below = Math.max(0, top + height - rect.bottom - gap)
      const opensAbove = above > below
      const popupWidth = Math.max(0, Math.min(rect.width, width - gap * 2))
      setStyle({
        left: Math.max(
          left + gap,
          Math.min(rect.left, left + width - popupWidth - gap),
        ),
        top: opensAbove ? rect.top - gap : rect.bottom + gap,
        width: popupWidth,
        height: opensAbove ? above : below,
        transform: opensAbove ? "translateY(-100%)" : undefined,
        visibility: "visible",
      })
    }
    position()
    window.addEventListener("resize", position)
    window.addEventListener("scroll", position, true)
    viewport?.addEventListener("resize", position)
    viewport?.addEventListener("scroll", position)
    return () => {
      window.removeEventListener("resize", position)
      window.removeEventListener("scroll", position, true)
      viewport?.removeEventListener("resize", position)
      viewport?.removeEventListener("scroll", position)
    }
  }, [onClose, popup, trigger])
  return style
}
