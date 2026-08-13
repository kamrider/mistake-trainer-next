import { onBeforeUnmount, ref, type Ref } from 'vue'

export interface ActionConfirmationRequest {
  eyebrow?: string
  title: string
  description: string
  confirmLabel: string
  cancelLabel?: string
  tone?: 'danger' | 'warning'
}

export interface ActionConfirmationController {
  current: Readonly<Ref<ActionConfirmationRequest | undefined>>
  ask: (request: ActionConfirmationRequest) => Promise<boolean>
  confirm: () => void
  cancel: () => void
}

export function useActionConfirmation(): ActionConfirmationController {
  const current = ref<ActionConfirmationRequest>()
  let resolvePending: ((confirmed: boolean) => void) | undefined

  function settle(confirmed: boolean) {
    const resolve = resolvePending
    resolvePending = undefined
    current.value = undefined
    resolve?.(confirmed)
  }

  function ask(request: ActionConfirmationRequest): Promise<boolean> {
    if (resolvePending) return Promise.resolve(false)
    current.value = request
    return new Promise((resolve) => {
      resolvePending = resolve
    })
  }

  onBeforeUnmount(() => settle(false))

  return {
    current,
    ask,
    confirm: () => settle(true),
    cancel: () => settle(false),
  }
}
