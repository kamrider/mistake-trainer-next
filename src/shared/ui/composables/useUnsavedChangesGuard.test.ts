import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { defineComponent, h, nextTick } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import ActionConfirmDialog from '@/shared/ui/components/ActionConfirmDialog.vue'
import {
  type NavigationAttempt,
  useUnsavedChangesGuard,
} from './useUnsavedChangesGuard'

const confirmation = {
  eyebrow: '未保存修改',
  title: '放弃设置修改并离开？',
  description: '未保存的偏好会丢失。',
  cancelLabel: '继续编辑',
  confirmLabel: '放弃修改并离开',
  tone: 'danger' as const,
}

function harness(register = true) {
  let dirty = false
  let busy = false
  let attempt: NavigationAttempt | undefined
  let contextAttempt: NavigationAttempt | undefined
  let controller: ReturnType<typeof useUnsavedChangesGuard> | undefined
  const unregister = vi.fn()
  const unregisterContext = vi.fn()
  const onBusy = vi.fn()
  const registerNavigation = vi.fn((candidate: NavigationAttempt) => {
    attempt = candidate
    return unregister
  })
  const registerContextTransition = vi.fn((candidate: NavigationAttempt) => {
    contextAttempt = candidate
    return unregisterContext
  })
  const Host = defineComponent({
    setup() {
      controller = useUnsavedChangesGuard({
        dirty: () => dirty,
        busy: () => busy,
        onBusy,
        ...(register ? { registerNavigation, registerContextTransition } : {}),
        confirmation,
      })
      return () => controller?.current.value
        ? h(ActionConfirmDialog, {
            request: controller.current.value,
            onCancel: controller.cancel,
            onConfirm: controller.confirm,
          })
        : null
    },
  })
  const view = render(Host)
  return {
    view,
    onBusy,
    unregister,
    unregisterContext,
    registerNavigation,
    registerContextTransition,
    controller: () => controller!,
    attempt: () => attempt!,
    contextAttempt: () => contextAttempt!,
    setDirty(value: boolean) { dirty = value },
    setBusy(value: boolean) { busy = value },
  }
}

describe('useUnsavedChangesGuard', () => {
  it('allows clean navigation and asks before leaving a dirty view', async () => {
    const current = harness()
    const user = userEvent.setup()

    await expect(current.attempt()()).resolves.toBe(true)
    expect(current.contextAttempt()).toBe(current.attempt())
    current.setDirty(true)
    const cancelled = current.attempt()()
    expect(await screen.findByRole('alertdialog', { name: confirmation.title })).toBeVisible()
    await waitFor(() => expect(screen.getByRole('button', { name: '继续编辑' })).toHaveFocus())
    await user.click(screen.getByRole('button', { name: '继续编辑' }))
    await expect(cancelled).resolves.toBe(false)

    const confirmed = current.attempt()()
    await user.click(await screen.findByRole('button', { name: '放弃修改并离开' }))
    await expect(confirmed).resolves.toBe(true)
  })

  it('blocks busy navigation and shares one decision across duplicate dirty attempts', async () => {
    const current = harness()
    const user = userEvent.setup()
    current.setBusy(true)

    await expect(current.attempt()()).resolves.toBe(false)
    expect(current.onBusy).toHaveBeenCalledOnce()
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument()

    current.setBusy(false)
    current.setDirty(true)
    const first = current.attempt()()
    const duplicate = current.attempt()()
    expect(await screen.findByRole('alertdialog')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '放弃修改并离开' }))
    await expect(Promise.all([first, duplicate])).resolves.toEqual([true, true])
  })

  it('prevents window unload only while dirty or busy', () => {
    const current = harness(false)
    expect(current.registerNavigation).not.toHaveBeenCalled()
    expect(current.registerContextTransition).not.toHaveBeenCalled()

    const cleanEvent = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(cleanEvent)
    expect(cleanEvent.defaultPrevented).toBe(false)

    current.setDirty(true)
    const dirtyEvent = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(dirtyEvent)
    expect(dirtyEvent.defaultPrevented).toBe(true)

    current.setDirty(false)
    current.setBusy(true)
    const busyEvent = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(busyEvent)
    expect(busyEvent.defaultPrevented).toBe(true)
  })

  it('unregisters lifecycle hooks and settles a pending attempt on unmount', async () => {
    const current = harness()
    current.setDirty(true)
    const pending = current.attempt()()
    await nextTick()

    current.view.unmount()

    await expect(pending).resolves.toBe(false)
    expect(current.unregister).toHaveBeenCalledOnce()
    expect(current.unregisterContext).toHaveBeenCalledOnce()
    const afterUnmount = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(afterUnmount)
    expect(afterUnmount.defaultPrevented).toBe(false)
  })
})
