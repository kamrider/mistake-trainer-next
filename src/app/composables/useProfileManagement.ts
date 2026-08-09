import { readonly, ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type { ProfileOverview, ProfileSummary } from '../../shared/api/bindings'

interface ProfileManagementOptions {
  enabled: boolean
  listProfiles: () => Promise<AppResult<ProfileOverview>>
  scheduleSync: () => void
  refreshWorkspace: () => Promise<unknown>
}

export interface ProfileMutationPolicy {
  refreshWorkspace: boolean
  scheduleSync: boolean
}

type ProfileMutation = () => Promise<AppResult<ProfileOverview>>

export function useProfileManagement(options: ProfileManagementOptions) {
  const profiles = ref<ProfileSummary[]>([])
  const activeProfileId = ref('')
  const busy = ref(false)
  const errorMessage = ref('')
  let activeOperation: 'load' | 'mutation' | undefined
  let refreshQueued = false

  function applyOverview(overview: ProfileOverview) {
    profiles.value = overview.profiles
    activeProfileId.value = overview.activeProfileId
  }

  async function performLoad(silent: boolean): Promise<boolean> {
    busy.value = true
    activeOperation = 'load'
    if (!silent) errorMessage.value = ''
    try {
      const result = await options.listProfiles()
      if (!result.ok) {
        if (!silent) errorMessage.value = result.error.userMessage
        return false
      }
      applyOverview(result.data)
      return true
    }
    catch {
      if (!silent) errorMessage.value = '学习档案没有读取成功，请重新打开应用后重试。'
      return false
    }
    finally {
      activeOperation = undefined
      busy.value = false
    }
  }

  async function loadProfiles(): Promise<boolean> {
    if (!options.enabled) return false
    if (busy.value) {
      if (activeOperation === 'mutation') refreshQueued = true
      return false
    }
    return performLoad(false)
  }

  async function mutateProfile(
    operation: ProfileMutation,
    policy: ProfileMutationPolicy,
  ): Promise<boolean> {
    if (!options.enabled || busy.value) return false

    busy.value = true
    errorMessage.value = ''
    activeOperation = 'mutation'
    try {
      let result: AppResult<ProfileOverview>
      try {
        result = await operation()
      }
      catch {
        errorMessage.value = '学习档案没有完成这次操作，请稍后重试。'
        return false
      }

      if (!result.ok) {
        errorMessage.value = result.error.userMessage
        return false
      }

      applyOverview(result.data)
      if (policy.scheduleSync) {
        try {
          options.scheduleSync()
        }
        catch {
          // The profile command is already durable; sync scheduling remains best-effort.
        }
      }
      if (policy.refreshWorkspace) {
        try {
          await options.refreshWorkspace()
        }
        catch {
          // The active overview is already applied; navigation cannot revoke that success.
        }
      }
      return true
    }
    finally {
      activeOperation = undefined
      busy.value = false
      if (refreshQueued) {
        refreshQueued = false
        await performLoad(true)
      }
    }
  }

  return {
    profiles: readonly(profiles),
    activeProfileId: readonly(activeProfileId),
    busy: readonly(busy),
    errorMessage: readonly(errorMessage),
    loadProfiles,
    mutateProfile,
  }
}
