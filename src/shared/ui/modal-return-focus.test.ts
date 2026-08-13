import { afterEach, describe, expect, it } from 'vitest'
import { createModalReturnFocusController } from './modal-return-focus'

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((finish) => { resolve = finish })
  return { promise, resolve }
}

function addButton(label: string) {
  const button = document.createElement('button')
  button.textContent = label
  document.body.append(button)
  return button
}

afterEach(() => document.body.replaceChildren())

describe('createModalReturnFocusController', () => {
  it('restores an original, replacement fallback, or explicit successor', async () => {
    const contextId = 'batch-1'
    const fallbackTargets = new Map<string, HTMLButtonElement>()
    const controller = createModalReturnFocusController({
      currentContextId: () => contextId,
      isModalOpen: () => false,
      findFallback: targetId => fallbackTargets.get(targetId),
      afterRender: () => Promise.resolve(),
    })
    const original = addButton('original')
    controller.capture({ contextId, targetId: 'item-1', element: original })
    expect(await controller.restore()).toBe(true)
    const replacement = addButton('replacement')
    fallbackTargets.set('item-1', replacement)
    controller.capture({ contextId, targetId: 'item-1', element: original })
    original.remove()
    expect(await controller.restore()).toBe(true)
    expect(replacement).toHaveFocus()
    const successor = addButton('successor')
    controller.capture({ contextId, targetId: 'item-1', element: undefined })
    expect(await controller.restore(() => successor)).toBe(true)
    expect(successor).toHaveFocus()
  })

  it('cancels restore after clear, context change, or another modal opens', async () => {
    let contextId = 'batch-1'
    let modalOpen = false
    let gate = deferred()
    const target = addButton('target')
    const controller = createModalReturnFocusController({
      currentContextId: () => contextId,
      isModalOpen: () => modalOpen,
      findFallback: () => target,
      afterRender: () => gate.promise,
    })
    controller.capture({ contextId, targetId: 'item-1', element: target })
    let restoring = controller.restore()
    controller.clear()
    gate.resolve()
    expect(await restoring).toBe(false)
    gate = deferred()
    controller.capture({ contextId, targetId: 'item-1', element: target })
    restoring = controller.restore()
    contextId = 'batch-2'
    gate.resolve()
    expect(await restoring).toBe(false)
    contextId = 'batch-1'
    gate = deferred()
    controller.capture({ contextId, targetId: 'item-1', element: target })
    restoring = controller.restore()
    modalOpen = true
    gate.resolve()
    expect(await restoring).toBe(false)
  })

  it('lets a newer capture supersede an older pending restore', async () => {
    const contextId = 'batch-1'
    let gate = deferred()
    const older = addButton('older')
    const newer = addButton('newer')
    const controller = createModalReturnFocusController({
      currentContextId: () => contextId,
      isModalOpen: () => false,
      findFallback: id => id === 'new' ? newer : older,
      afterRender: () => gate.promise,
    })
    controller.capture({ contextId, targetId: 'old', element: older })
    const oldRestore = controller.restore()
    controller.capture({ contextId, targetId: 'new', element: newer })
    gate.resolve()
    expect(await oldRestore).toBe(false)
    gate = deferred()
    const newRestore = controller.restore()
    gate.resolve()
    expect(await newRestore).toBe(true)
    expect(newer).toHaveFocus()
  })
})
