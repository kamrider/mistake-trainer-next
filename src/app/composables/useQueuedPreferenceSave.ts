import { readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'

interface QueuedPreferenceSaveOptions<TInput, TOutput> {
  snapshot: () => TInput | undefined
  persist: (input: TInput) => Promise<AppResult<TOutput>>
  applySaved: (output: TOutput) => void
  validate?: (input: TInput) => string | undefined
  successMessage: string
  failureMessage: string
  queuedMessage: string
}

export function useQueuedPreferenceSave<TInput, TOutput>(
  options: QueuedPreferenceSaveOptions<TInput, TOutput>,
) {
  const saving = ref(false)
  const dirty = ref(false)
  const message = ref('')
  const revision = ref(0)

  function markChanged() {
    revision.value += 1
    dirty.value = true
    message.value = saving.value ? options.queuedMessage : ''
  }

  async function save(): Promise<boolean> {
    if (saving.value || !options.snapshot()) return false
    saving.value = true
    message.value = ''

    try {
      while (true) {
        const input = options.snapshot()
        if (!input) return false
        const validationMessage = options.validate?.(input)
        if (validationMessage) {
          message.value = validationMessage
          return false
        }

        const requestRevision = revision.value
        const result = await options.persist(input)
        if (!result.ok) {
          message.value = result.error.userMessage
          return false
        }
        if (requestRevision !== revision.value) continue

        options.applySaved(result.data)
        dirty.value = false
        message.value = options.successMessage
        return true
      }
    }
    catch {
      message.value = options.failureMessage
      return false
    }
    finally {
      saving.value = false
    }
  }

  return {
    saving: readonly(saving),
    dirty: readonly(dirty),
    message: readonly(message),
    revision: readonly(revision),
    markChanged,
    save,
  }
}
