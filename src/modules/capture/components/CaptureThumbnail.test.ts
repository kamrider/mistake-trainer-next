import { fireEvent, render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CaptureItemSummary } from '../../../shared/api/bindings'
import CaptureThumbnail from './CaptureThumbnail.vue'

const item: CaptureItemSummary = {
  id: 'item-1',
  sourceName: '第一题.png',
  sourceSequence: 0,
  mediaType: 'image/png',
  byteLength: 100,
  width: 1200,
  height: 900,
  stagedRole: 'question',
  draftId: null,
  role: null,
  position: null,
  cropDerivationId: null,
  cropSourceItemId: null,
}

let intersectionCallback: IntersectionObserverCallback

beforeEach(() => {
  vi.stubGlobal('IntersectionObserver', class {
    constructor(callback: IntersectionObserverCallback) {
      intersectionCallback = callback
    }

    observe() {}
    disconnect() {}
  })
})

describe('CaptureThumbnail preview lifecycle', () => {
  it('uses a native pressed button for selection and blocks disabled activation', async () => {
    const user = userEvent.setup()
    const view = render(CaptureThumbnail, {
      props: { item, dataUrl: undefined, active: false },
    })

    const selection = screen.getByRole('button', { name: item.sourceName })
    const article = screen.getByRole('article')
    expect(article).not.toHaveAttribute('tabindex')
    expect(article).not.toHaveAttribute('role', 'button')
    expect(selection).toHaveAttribute('aria-pressed', 'false')

    await user.click(selection)
    selection.focus()
    await user.keyboard('{Enter}')
    await user.keyboard(' ')
    expect(view.emitted('activate')).toEqual([
      ['item-1'],
      ['item-1'],
      ['item-1'],
    ])
    const pointerStartCount = view.emitted('pointerStart')?.length ?? 0

    await view.rerender({ active: true })
    expect(selection).toHaveAttribute('aria-pressed', 'true')
    await view.rerender({ disabled: true })
    expect(selection).toBeDisabled()
    await user.click(selection)
    await fireEvent.pointerDown(selection)
    expect(view.emitted('activate')).toHaveLength(3)
    expect(view.emitted('pointerStart')).toHaveLength(pointerStartCount)
  })

  it('keeps crop and remove as isolated sibling actions', async () => {
    const user = userEvent.setup()
    const view = render(CaptureThumbnail, {
      props: {
        item,
        dataUrl: undefined,
        cropable: true,
        removable: true,
      },
    })

    const cropButton = screen.getByRole('button', { name: `裁剪 ${item.sourceName}` })
    expect(cropButton).toHaveAttribute('data-crop-item-id', item.id)
    await user.click(cropButton)
    await user.click(screen.getByRole('button', { name: `删除 ${item.sourceName}` }))

    expect(view.emitted('crop')).toEqual([['item-1']])
    expect(view.emitted('remove')).toEqual([['item-1']])
    expect(view.emitted('activate')).toBeUndefined()
    expect(view.emitted('pointerStart')).toBeUndefined()

    await view.rerender({
      item: { ...item, cropDerivationId: 'crop-1', cropSourceItemId: 'source-1' },
    })
    const resultControl = screen.getByRole('button', { name: '恢复裁剪前原图' })
    expect(resultControl).not.toHaveAttribute('data-crop-item-id')
    expect(resultControl).toHaveAttribute('data-crop-result-item-id', item.id)
  })

  it('reloads a visible thumbnail after the bounded preview cache evicts it', async () => {
    const view = render(CaptureThumbnail, {
      props: { item, dataUrl: undefined },
    })

    intersectionCallback(
      [{ isIntersecting: true } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    )
    expect(view.emitted('preview')).toEqual([['item-1']])

    await view.rerender({ dataUrl: 'data:image/png;base64,loaded' })
    await view.rerender({ dataUrl: undefined })

    expect(view.emitted('preview')).toEqual([['item-1'], ['item-1']])
  })

  it('retries a failed preview only after the item leaves and re-enters the preload margin', () => {
    const view = render(CaptureThumbnail, {
      props: { item, dataUrl: undefined },
    })

    intersectionCallback(
      [{ isIntersecting: true } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    )
    intersectionCallback(
      [{ isIntersecting: false } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    )
    intersectionCallback(
      [{ isIntersecting: true } as IntersectionObserverEntry],
      {} as IntersectionObserver,
    )

    expect(view.emitted('preview')).toEqual([['item-1'], ['item-1']])
  })
})
