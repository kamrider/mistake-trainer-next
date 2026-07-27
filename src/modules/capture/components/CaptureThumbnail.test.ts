import { render } from '@testing-library/vue'
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
