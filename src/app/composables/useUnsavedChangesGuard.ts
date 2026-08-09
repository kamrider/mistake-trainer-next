import { onBeforeUnmount, onMounted } from 'vue'
import type { ActionConfirmationRequest } from './useActionConfirmation'
import { useActionConfirmation } from './useActionConfirmation'

export type NavigationAttempt = () => boolean | Promise<boolean>

interface UnsavedChangesGuardOptions {
  dirty: () => boolean
  busy: () => boolean
  onBusy: () => void
  registerNavigation?: (attempt: NavigationAttempt) => () => void
  registerContextTransition?: (attempt: NavigationAttempt) => () => void
  confirmation: ActionConfirmationRequest
}

export function useUnsavedChangesGuard(options: UnsavedChangesGuardOptions) {
  const confirmation = useActionConfirmation()
  let pendingDecision: Promise<boolean> | undefined

  async function attemptLeave(): Promise<boolean> {
    if (options.busy()) {
      options.onBusy()
      return false
    }
    if (!options.dirty()) return true
    if (pendingDecision) return pendingDecision
    const decision = confirmation.ask(options.confirmation).finally(() => {
      pendingDecision = undefined
    })
    pendingDecision = decision
    return decision
  }

  const unregisterNavigation = options.registerNavigation?.(attemptLeave)
  const unregisterContextTransition = options.registerContextTransition?.(attemptLeave)

  function handleBeforeUnload(event: BeforeUnloadEvent) {
    if (!options.dirty() && !options.busy()) return
    event.preventDefault()
    event.returnValue = true
  }

  onMounted(() => window.addEventListener('beforeunload', handleBeforeUnload))
  onBeforeUnmount(() => {
    window.removeEventListener('beforeunload', handleBeforeUnload)
    unregisterNavigation?.()
    unregisterContextTransition?.()
  })

  return {
    current: confirmation.current,
    confirm: confirmation.confirm,
    cancel: confirmation.cancel,
    attemptLeave,
  }
}
