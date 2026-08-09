import { readonly, ref, type Ref } from 'vue'
import type { SubjectPreferences } from '../../shared/api/bindings'

interface SubjectPreferenceDraftOptions {
  preferences: Ref<SubjectPreferences | undefined>
  builtinSubjects: readonly string[]
  onChanged: () => void
}

function subjectKey(value: string): string {
  return value.trim().normalize('NFKC').toLocaleLowerCase()
}

export function useSubjectPreferenceDraft(options: SubjectPreferenceDraftOptions) {
  const customSubject = ref('')
  const message = ref('')

  function updateCustomSubject(value: string) {
    customSubject.value = value
    message.value = ''
  }

  function clearMessage() {
    message.value = ''
  }

  function commit(preferences: SubjectPreferences) {
    options.preferences.value = preferences
    message.value = ''
    options.onChanged()
  }

  function addCustomSubject(): boolean {
    const preferences = options.preferences.value
    if (!preferences) return false
    const value = customSubject.value.trim()
    if (!value) {
      message.value = '请输入自定义科目名称。'
      return false
    }
    if (value.length > 40) {
      message.value = '自定义科目名称最多 40 个字符。'
      return false
    }
    const key = subjectKey(value)
    if (
      options.builtinSubjects.some(subject => subjectKey(subject) === key)
      || preferences.customSubjects.some(subject => subjectKey(subject) === key)
    ) {
      message.value = `“${value}”已在科目列表中。`
      return false
    }
    if (preferences.customSubjects.length >= 20) {
      message.value = '自定义科目最多 20 个。'
      return false
    }

    commit({
      ...preferences,
      customSubjects: [...preferences.customSubjects, value],
      enabledSubjects: preferences.enabledSubjects.includes(value)
        ? [...preferences.enabledSubjects]
        : [...preferences.enabledSubjects, value],
    })
    customSubject.value = ''
    return true
  }

  function removeCustomSubject(subject: string): boolean {
    const preferences = options.preferences.value
    if (!preferences) return false
    const key = subjectKey(subject)
    const storedSubject = preferences.customSubjects.find(value => subjectKey(value) === key)
    if (!storedSubject) return false
    if (
      preferences.enabledSubjects.includes(storedSubject)
      && preferences.enabledSubjects.length === 1
    ) {
      message.value = `至少保留一个常用科目；请先启用其他科目，再删除“${storedSubject}”。`
      return false
    }

    commit({
      ...preferences,
      customSubjects: preferences.customSubjects.filter(value => value !== storedSubject),
      enabledSubjects: preferences.enabledSubjects.filter(value => value !== storedSubject),
    })
    return true
  }

  function toggleSubject(subject: string, enabled: boolean): boolean {
    const preferences = options.preferences.value
    if (!preferences) return false
    const currentlyEnabled = preferences.enabledSubjects.includes(subject)
    if (currentlyEnabled === enabled) return false
    if (!enabled && currentlyEnabled && preferences.enabledSubjects.length === 1) {
      message.value = '至少保留一个常用科目。'
      return false
    }

    commit({
      ...preferences,
      enabledSubjects: enabled
        ? [...preferences.enabledSubjects, subject]
        : preferences.enabledSubjects.filter(value => value !== subject),
    })
    return true
  }

  function updateCaptureSound(enabled: boolean): boolean {
    const preferences = options.preferences.value
    if (!preferences || preferences.captureSoundEnabled === enabled) return false
    commit({ ...preferences, captureSoundEnabled: enabled })
    return true
  }

  return {
    customSubject: readonly(customSubject),
    message: readonly(message),
    updateCustomSubject,
    clearMessage,
    addCustomSubject,
    removeCustomSubject,
    toggleSubject,
    updateCaptureSound,
  }
}
