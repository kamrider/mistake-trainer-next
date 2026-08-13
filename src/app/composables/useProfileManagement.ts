import { computed, readonly, ref, type ComputedRef, type Ref } from 'vue'
import type { AppResult } from '../../shared/api/app-result'
import type { ProfileOverview, ProfileSummary } from '../../shared/api/bindings'

export interface ProfileOperations {
  list: () => Promise<AppResult<ProfileOverview>>
  create: (name: string) => Promise<AppResult<ProfileOverview>>
  rename: (profileId: string, name: string) => Promise<AppResult<ProfileOverview>>
  remove: (profileId: string, confirmationName: string) => Promise<AppResult<ProfileOverview>>
  select: (profileId: string) => Promise<AppResult<ProfileOverview>>
}

export interface ProfileManagementOptions {
  enabled: boolean
  operations: ProfileOperations
  attemptWorkspaceTransition: () => Promise<boolean>
  scheduleSync: () => void
  refreshWorkspace: () => Promise<unknown>
}

export interface ProfileManagementController {
  profiles: Readonly<Ref<readonly Readonly<ProfileSummary>[]>>
  activeProfileId: Readonly<Ref<string>>
  shellProfiles: ComputedRef<ProfileSummary[]>
  shellActiveProfileId: ComputedRef<string>
  busy: Readonly<Ref<boolean>>
  errorMessage: Readonly<Ref<string>>
  loadProfiles: () => Promise<boolean>
  createProfile: (name: string) => Promise<boolean>
  renameProfile: (profileId: string, name: string) => Promise<boolean>
  deleteProfile: (profileId: string, confirmationName: string) => Promise<boolean>
  selectProfile: (profileId: string) => Promise<boolean>
}

interface ProfileMutationPolicy {
  refreshWorkspace: boolean
  scheduleSync: boolean
}

type ProfileMutation = () => Promise<AppResult<ProfileOverview>>

const previewProfile: ProfileSummary = {
  id: 'preview-profile',
  name: '本机学习档案',
  createdAtUtcMs: 0,
  updatedAtUtcMs: 0,
  revision: 1,
}

export function useProfileManagement(
  options: ProfileManagementOptions,
): ProfileManagementController {
  const profiles = ref<ProfileSummary[]>([])
  const activeProfileId = ref('')
  const busy = ref(false)
  const errorMessage = ref('')
  const shellProfiles = computed(() =>
    profiles.value.length || options.enabled ? [...profiles.value] : [previewProfile],
  )
  const shellActiveProfileId = computed(() =>
    activeProfileId.value || (options.enabled ? '' : previewProfile.id),
  )
  let operationReserved = false
  let activeOperation: 'load' | 'mutation' | undefined
  let refreshQueued = false

  function applyOverview(overview: ProfileOverview) {
    profiles.value = overview.profiles
    activeProfileId.value = overview.activeProfileId
  }

  async function performLoad(silent: boolean): Promise<boolean> {
    operationReserved = true
    busy.value = true
    activeOperation = 'load'
    if (!silent) errorMessage.value = ''
    try {
      const result = await options.operations.list()
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
      operationReserved = false
    }
  }

  async function loadProfiles(): Promise<boolean> {
    if (!options.enabled) return false
    if (operationReserved) {
      if (activeOperation === 'mutation') refreshQueued = true
      return false
    }
    return performLoad(false)
  }

  async function performMutation(
    operation: ProfileMutation,
    policy: ProfileMutationPolicy,
    requiresWorkspaceTransition = false,
  ): Promise<boolean> {
    if (!options.enabled || operationReserved) return false

    operationReserved = true
    activeOperation = 'mutation'
    try {
      if (requiresWorkspaceTransition) {
        try {
          if (!await options.attemptWorkspaceTransition()) return false
        }
        catch {
          return false
        }
      }

      busy.value = true
      errorMessage.value = ''
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
          // The overview is already applied; navigation cannot revoke native success.
        }
      }
      return true
    }
    finally {
      activeOperation = undefined
      busy.value = false
      operationReserved = false
      if (refreshQueued) {
        refreshQueued = false
        await performLoad(true)
      }
    }
  }

  function createProfile(name: string): Promise<boolean> {
    return performMutation(
      () => options.operations.create(name),
      { refreshWorkspace: true, scheduleSync: true },
      true,
    )
  }

  function renameProfile(profileId: string, name: string): Promise<boolean> {
    return performMutation(
      () => options.operations.rename(profileId, name),
      { refreshWorkspace: false, scheduleSync: true },
    )
  }

  async function deleteProfile(
    profileId: string,
    confirmationName: string,
  ): Promise<boolean> {
    const deletesActiveProfile = profileId === activeProfileId.value
    return performMutation(
      () => options.operations.remove(profileId, confirmationName),
      { refreshWorkspace: deletesActiveProfile, scheduleSync: true },
      deletesActiveProfile,
    )
  }

  function selectProfile(profileId: string): Promise<boolean> {
    if (!options.enabled || operationReserved || profileId === activeProfileId.value) {
      return Promise.resolve(false)
    }
    return performMutation(
      () => options.operations.select(profileId),
      { refreshWorkspace: true, scheduleSync: false },
      true,
    )
  }

  return {
    profiles: readonly(profiles),
    activeProfileId: readonly(activeProfileId),
    shellProfiles,
    shellActiveProfileId,
    busy: readonly(busy),
    errorMessage: readonly(errorMessage),
    loadProfiles,
    createProfile,
    renameProfile,
    deleteProfile,
    selectProfile,
  }
}
