import { computed, onBeforeUnmount, reactive } from 'vue'

export type CaptureDropTarget = {
  kind: 'new-card' | 'card' | 'unassigned'
  draftId: string | null
}

export type CapturePointerDrop = CaptureDropTarget & {
  itemId: string
}

export function useCapturePointerDrag(onDrop: (drop: CapturePointerDrop) => void) {
  const drag = reactive({
    itemId: '',
    pointerId: -1,
    startX: 0,
    startY: 0,
    x: 0,
    y: 0,
    active: false,
  })
  let source: HTMLElement | undefined
  let suppressClick = false

  const style = computed(() => ({ transform: `translate3d(${drag.x + 14}px, ${drag.y + 14}px, 0)` }))

  function cleanup() {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', finish)
    window.removeEventListener('pointercancel', cancel)
    window.removeEventListener('keydown', keydown)
    if (source && typeof source.hasPointerCapture === 'function' && source.hasPointerCapture(drag.pointerId)) {
      source.releasePointerCapture(drag.pointerId)
    }
    source = undefined
    drag.itemId = ''
    drag.pointerId = -1
    drag.active = false
  }

  function move(event: PointerEvent) {
    if (event.pointerId !== drag.pointerId) return
    drag.x = event.clientX
    drag.y = event.clientY
    if (!drag.active && Math.hypot(drag.x - drag.startX, drag.y - drag.startY) >= 6) {
      drag.active = true
      suppressClick = true
      document.documentElement.classList.add('capture-pointer-dragging')
    }
    if (drag.active) event.preventDefault()
  }

  function targetAt(x: number, y: number): CaptureDropTarget | undefined {
    const element = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-capture-drop]')
    if (!element) return undefined
    const kind = element.dataset.captureDrop
    if (kind !== 'new-card' && kind !== 'card' && kind !== 'unassigned') return undefined
    return { kind, draftId: kind === 'card' ? element.dataset.draftId ?? null : null }
  }

  function finish(event: PointerEvent) {
    if (event.pointerId !== drag.pointerId) return
    const itemId = drag.itemId
    const target = drag.active ? targetAt(event.clientX, event.clientY) : undefined
    document.documentElement.classList.remove('capture-pointer-dragging')
    cleanup()
    if (itemId && target) onDrop({ itemId, ...target })
  }

  function cancel() {
    document.documentElement.classList.remove('capture-pointer-dragging')
    cleanup()
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return
    event.preventDefault()
    cancel()
  }

  function start(itemId: string, event: PointerEvent) {
    if (event.button !== 0 || drag.itemId) return
    source = event.currentTarget instanceof HTMLElement ? event.currentTarget : undefined
    drag.itemId = itemId
    drag.pointerId = event.pointerId
    drag.startX = drag.x = event.clientX
    drag.startY = drag.y = event.clientY
    source?.setPointerCapture?.(event.pointerId)
    window.addEventListener('pointermove', move, { passive: false })
    window.addEventListener('pointerup', finish)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', keydown)
  }

  function consumeSuppressedClick() {
    const value = suppressClick
    suppressClick = false
    return value
  }

  onBeforeUnmount(cancel)

  return { drag, style, start, cancel, consumeSuppressedClick }
}
