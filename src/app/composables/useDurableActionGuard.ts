import { onBeforeUnmount, onMounted } from 'vue'

export type DurableActionAttempt = () => boolean

interface DurableActionGuardOptions {
  busy: () => boolean
  onBlocked: () => void
  registerContextTransition?: (attempt: DurableActionAttempt) => () => void
}

export function useDurableActionGuard(options: DurableActionGuardOptions) {
  function attemptLeave() {
    if (!options.busy()) return true
    options.onBlocked()
    return false
  }

  const unregisterContextTransition = options.registerContextTransition?.(attemptLeave)

  function handleBeforeUnload(event: BeforeUnloadEvent) {
    if (!options.busy()) return
    event.preventDefault()
    event.returnValue = true
  }

  onMounted(() => window.addEventListener('beforeunload', handleBeforeUnload))
  onBeforeUnmount(() => {
    window.removeEventListener('beforeunload', handleBeforeUnload)
    unregisterContextTransition?.()
  })

  return { attemptLeave }
}
