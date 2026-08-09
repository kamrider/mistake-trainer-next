import { render, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import ReviewHistoryDetail from './ReviewHistoryDetail.vue'

describe('ReviewHistoryDetail mobile dialog', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('contains keyboard focus inside the modal surface', async () => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() })))
    const user = userEvent.setup()
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'auto'
    const background = document.createElement('button')
    document.body.append(background)
    const view = render(ReviewHistoryDetail, { props: { detail: undefined, loading: false, error: 'Read failed.' } })

    try {
      const dialog = view.container.querySelector<HTMLElement>('.history-detail')!
      const close = view.container.querySelector<HTMLButtonElement>('.history-detail header button')!
      const retry = view.container.querySelector<HTMLButtonElement>('.detail-state button')!
      await waitFor(() => expect(dialog).toHaveAttribute('role', 'dialog'))
      expect(dialog).toHaveAttribute('aria-modal', 'true')
      expect(document.body.style.overflow).toBe('hidden')
      expect(background).toHaveAttribute('inert')
      await waitFor(() => expect(close).toHaveFocus())
      await user.keyboard('{Shift>}{Tab}{/Shift}')
      expect(retry).toHaveFocus()
      await user.tab()
      expect(close).toHaveFocus()

      view.unmount()
      expect(document.body.style.overflow).toBe('auto')
      expect(background).not.toHaveAttribute('inert')
    }
    finally {
      view.unmount()
      background.remove()
      document.body.style.overflow = previousOverflow
    }
  })
})
