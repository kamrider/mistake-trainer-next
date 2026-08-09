import { nextTick } from 'vue'

export interface ModalReturnFocusCapture {
  contextId: string
  targetId: string
  element: HTMLButtonElement | undefined
}

interface ModalReturnFocusRequest extends ModalReturnFocusCapture {
  generation: number
}

interface ModalReturnFocusOptions {
  currentContextId: () => string | undefined
  isModalOpen: () => boolean
  findFallback: (targetId: string) => HTMLButtonElement | undefined
  afterRender?: () => Promise<void>
}

export interface ModalReturnFocusController {
  capture: (input: ModalReturnFocusCapture) => void
  clear: () => void
  restore: (findSuccessor?: () => HTMLButtonElement | undefined) => Promise<boolean>
}

function canReceiveFocus(button: HTMLButtonElement | undefined) {
  return Boolean(button?.isConnected && !button.disabled)
}

export function createModalReturnFocusController(
  options: ModalReturnFocusOptions,
): ModalReturnFocusController {
  let generation = 0
  let request: ModalReturnFocusRequest | undefined

  function capture(input: ModalReturnFocusCapture) {
    generation += 1
    request = { ...input, generation }
  }

  function clear() {
    generation += 1
    request = undefined
  }

  async function restore(findSuccessor?: () => HTMLButtonElement | undefined) {
    const pending = request
    if (!pending) return false
    await (options.afterRender?.() ?? nextTick())
    if (request !== pending || generation !== pending.generation) return false
    if (options.currentContextId() !== pending.contextId || options.isModalOpen()) {
      clear()
      return false
    }

    request = undefined
    const successor = findSuccessor?.()
    const fallback = options.findFallback(pending.targetId)
    const target = canReceiveFocus(successor)
      ? successor
      : canReceiveFocus(pending.element)
        ? pending.element
        : canReceiveFocus(fallback)
          ? fallback
          : undefined
    target?.focus()
    return document.activeElement === target
  }

  return { capture, clear, restore }
}
