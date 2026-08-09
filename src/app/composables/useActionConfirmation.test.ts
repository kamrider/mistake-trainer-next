import { defineComponent, h } from 'vue'
import { render } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import {
  useActionConfirmation,
  type ActionConfirmationController,
  type ActionConfirmationRequest,
} from './useActionConfirmation'

const request: ActionConfirmationRequest = {
  title: '删除这项内容？',
  description: '确认后会移入回收区。',
  confirmLabel: '删除',
}

function renderHarness() {
  let controller!: ActionConfirmationController
  const view = render(defineComponent({
    setup() {
      controller = useActionConfirmation()
      return () => h('div', controller.current.value?.title ?? 'idle')
    },
  }))
  return { controller, view }
}

describe('useActionConfirmation', () => {
  it('settles the active request only through confirm or cancel', async () => {
    const { controller } = renderHarness()

    const confirmed = controller.ask(request)
    expect(controller.current.value).toEqual(request)
    controller.confirm()
    await expect(confirmed).resolves.toBe(true)
    expect(controller.current.value).toBeUndefined()

    const cancelled = controller.ask(request)
    controller.cancel()
    await expect(cancelled).resolves.toBe(false)
    expect(controller.current.value).toBeUndefined()
  })

  it('does not replace an active decision with a second request', async () => {
    const { controller } = renderHarness()
    const first = controller.ask(request)
    const secondRequest = { ...request, title: '另一个决定？' }

    await expect(controller.ask(secondRequest)).resolves.toBe(false)
    expect(controller.current.value).toEqual(request)

    controller.cancel()
    await expect(first).resolves.toBe(false)
  })

  it('cancels a pending decision when its owner unmounts', async () => {
    const { controller, view } = renderHarness()
    const pending = controller.ask(request)

    view.unmount()

    await expect(pending).resolves.toBe(false)
    expect(controller.current.value).toBeUndefined()
  })
})
