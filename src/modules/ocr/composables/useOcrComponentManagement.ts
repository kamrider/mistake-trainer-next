import { readonly, ref, shallowReadonly, shallowRef } from 'vue'
import type { AppResult } from '../../../shared/api/app-result'
import type {
  OcrCapabilityStatus,
  OcrComponentId,
  OcrComponentStatus,
} from '../../../shared/api/bindings'

interface OcrComponentManagementOptions {
  fetchCapability: () => Promise<AppResult<OcrCapabilityStatus>>
  installComponent: (componentId: OcrComponentId) => Promise<AppResult<OcrComponentStatus>>
  removeComponent: (componentId: OcrComponentId) => Promise<AppResult<OcrComponentStatus>>
}

type MutationKind = 'install' | 'remove'

function mergeConfirmedComponent(
  current: OcrCapabilityStatus | undefined,
  component: OcrComponentStatus,
) {
  if (!current) return current
  const index = current.components.findIndex(item => item.id === component.id)
  const components = index === -1
    ? [...current.components, component]
    : current.components.map(item => item.id === component.id ? component : item)
  return { ...current, components }
}

function partialSuccessMessage(kind: MutationKind) {
  const outcome = kind === 'install' ? '已安装' : '已移除'
  return `本地模型${outcome}，但完整能力状态暂时未刷新；可稍后点击“刷新”确认当前模式。`
}

function commandFailureMessage(kind: MutationKind) {
  return kind === 'install'
    ? '模型没有安装完成；临时文件已清理，基础预切仍可继续使用。'
    : '模型没有移除；当前文件和功能状态保持不变。'
}

export function useOcrComponentManagement(options: OcrComponentManagementOptions) {
  const capability = shallowRef<OcrCapabilityStatus>()
  const busy = ref(false)
  const message = ref('')
  let requestEpoch = 0

  async function refreshCapability(): Promise<boolean> {
    if (busy.value) return false
    const epoch = ++requestEpoch
    try {
      const result = await options.fetchCapability()
      if (epoch !== requestEpoch) return false
      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }
      capability.value = result.data
      message.value = ''
      return true
    }
    catch {
      if (epoch !== requestEpoch) return false
      message.value = '模型状态暂时无法读取；基础预切仍可继续使用。'
      return false
    }
  }

  async function mutate(kind: MutationKind, componentId: OcrComponentId): Promise<boolean> {
    if (busy.value) return false
    busy.value = true
    ++requestEpoch
    message.value = kind === 'install'
      ? '正在下载并校验本地模型；现有题库和采集内容不会改变。'
      : '正在移除本地模型；现有题库和采集内容不会改变。'

    try {
      let result: AppResult<OcrComponentStatus>
      try {
        result = await (kind === 'install'
          ? options.installComponent(componentId)
          : options.removeComponent(componentId))
      }
      catch {
        message.value = commandFailureMessage(kind)
        return false
      }

      if (!result.ok) {
        message.value = result.error.userMessage
        return false
      }

      capability.value = mergeConfirmedComponent(capability.value, result.data)

      let refreshed: AppResult<OcrCapabilityStatus>
      try {
        refreshed = await options.fetchCapability()
      }
      catch {
        message.value = partialSuccessMessage(kind)
        return true
      }

      if (!refreshed.ok) {
        message.value = partialSuccessMessage(kind)
        return true
      }

      capability.value = refreshed.data
      if (kind === 'remove') {
        message.value = '本地模型已移除；基础预切、手工裁剪和题库不受影响。'
      }
      else if (refreshed.data.automaticRecognitionEnabled) {
        message.value = '题号定位增强已就绪，下次智能切图会自动使用，无需重启。'
      }
      else {
        message.value = '本地模型已安装；当前仍使用基础预切。'
      }
      return true
    }
    finally {
      busy.value = false
    }
  }

  return {
    capability: shallowReadonly(capability),
    busy: readonly(busy),
    message: readonly(message),
    refreshCapability,
    install: (componentId: OcrComponentId) => mutate('install', componentId),
    remove: (componentId: OcrComponentId) => mutate('remove', componentId),
  }
}
