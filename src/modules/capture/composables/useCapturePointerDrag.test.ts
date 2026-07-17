import { fireEvent, render, screen } from '@testing-library/vue'
import { defineComponent } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import { useCapturePointerDrag } from './useCapturePointerDrag'

function renderDrag() {
  const onDrop = vi.fn()
  const Host = defineComponent({
    setup() {
      return { pointerDrag: useCapturePointerDrag(onDrop) }
    },
    template: `
      <button data-testid="source" @pointerdown="pointerDrag.start('item-1', $event)">source</button>
      <div data-testid="target" data-capture-drop="new-card">target</div>
      <span data-testid="hover">{{ pointerDrag.drag.hoveredTarget?.kind ?? 'none' }}</span>
    `,
  })
  render(Host)
  const target = screen.getByTestId('target')
  Object.defineProperty(document, 'elementFromPoint', {
    configurable: true,
    value: vi.fn(() => target),
  })
  return { onDrop }
}

describe('useCapturePointerDrag', () => {
  it('waits for six pixels and drops through Pointer Events', async () => {
    const { onDrop } = renderDrag()
    const source = screen.getByTestId('source')
    await fireEvent.pointerDown(source, { pointerId: 7, button: 0, clientX: 10, clientY: 10 })
    await fireEvent.pointerMove(window, { pointerId: 7, clientX: 13, clientY: 13 })
    await fireEvent.pointerUp(window, { pointerId: 7, clientX: 13, clientY: 13 })
    expect(onDrop).not.toHaveBeenCalled()

    await fireEvent.pointerDown(source, { pointerId: 8, button: 0, clientX: 10, clientY: 10 })
    await fireEvent.pointerMove(window, { pointerId: 8, clientX: 20, clientY: 10 })
    expect(screen.getByTestId('hover')).toHaveTextContent('new-card')
    await fireEvent.pointerUp(window, { pointerId: 8, clientX: 20, clientY: 10 })
    expect(onDrop).toHaveBeenCalledWith({ itemId: 'item-1', kind: 'new-card', draftId: null })
    expect(screen.getByTestId('hover')).toHaveTextContent('none')
  })

  it('cancels the active drag with Escape', async () => {
    const { onDrop } = renderDrag()
    await fireEvent.pointerDown(screen.getByTestId('source'), { pointerId: 9, button: 0, clientX: 0, clientY: 0 })
    await fireEvent.pointerMove(window, { pointerId: 9, clientX: 12, clientY: 0 })
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.getByTestId('hover')).toHaveTextContent('none')
    await fireEvent.pointerUp(window, { pointerId: 9, clientX: 12, clientY: 0 })
    expect(onDrop).not.toHaveBeenCalled()
  })
})
