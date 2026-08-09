import { describe, expect, it, vi } from 'vitest'
import type { OcrCapabilityStatus, OcrComponentStatus } from '../../../shared/api/bindings'
import { failure, success } from '../../../shared/api/app-result'
import { useOcrComponentManagement } from './useOcrComponentManagement'

const notInstalled: OcrComponentStatus = {
  id: 'ppocrv6_small',
  displayName: 'PP‑OCRv6 small',
  description: '面向题号定位。',
  state: 'not_installed',
  downloadBytes: 31_163_977,
  installedBytes: 0,
  recommended: true,
  installAllowed: true,
  statusDetail: '尚未下载。',
  sourceLabel: 'ModelScope',
  licenseLabel: 'Apache-2.0',
}
const installed: OcrComponentStatus = {
  ...notInstalled,
  state: 'installed',
  installedBytes: 31_163_977,
  statusDetail: '模型文件已经校验。',
}
const preprocessing: OcrComponentStatus = {
  ...notInstalled,
  id: 'opencv_preprocess',
  displayName: 'OpenCV 图像预处理',
  state: 'unavailable',
  downloadBytes: 0,
  recommended: false,
  installAllowed: false,
}

function capability(
  component: OcrComponentStatus,
  automaticRecognitionEnabled = false,
): OcrCapabilityStatus {
  return {
    assessment: {
      tier: 'balanced',
      logicalProcessorCount: 8,
      totalMemoryMb: 16_384,
      availableComponentStorageMb: 8192,
      avx2Supported: true,
      estimatedSuitable: true,
      recommendedComponentId: 'ppocrv6_small',
      summary: '本机预检通过。',
    },
    components: [component, preprocessing],
    recognitionFeature: {
      state: automaticRecognitionEnabled ? 'ready' : 'model_missing',
      requiredComponentId: 'ppocrv6_small',
      detail: automaticRecognitionEnabled ? '题号增强可用。' : '当前仍使用基础预切。',
    },
    automaticRecognitionEnabled,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness() {
  const fetchCapability = vi.fn(async () => success(capability(notInstalled)))
  const installComponent = vi.fn(async () => success(installed))
  const removeComponent = vi.fn(async () => success(notInstalled))
  const controller = useOcrComponentManagement({
    fetchCapability,
    installComponent,
    removeComponent,
  })
  return { controller, fetchCapability, installComponent, removeComponent }
}

describe('useOcrComponentManagement', () => {
  it('commits the command component before the full capability refresh finishes', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    const refreshed = deferred<ReturnType<typeof success<OcrCapabilityStatus>>>()
    current.fetchCapability.mockReturnValueOnce(refreshed.promise)

    const pending = current.controller.install('ppocrv6_small')
    await vi.waitFor(() => {
      expect(current.controller.capability.value?.components[0]).toEqual(installed)
    })
    expect(current.controller.busy.value).toBe(true)

    refreshed.resolve(success(capability(installed, true)))
    await expect(pending).resolves.toBe(true)
    expect(current.controller.capability.value).toEqual(capability(installed, true))
    expect(current.controller.message.value).toBe('题号定位增强已就绪，下次智能切图会自动使用，无需重启。')
    expect(current.controller.busy.value).toBe(false)
  })

  it('does not claim enhancement readiness when the refreshed capability remains disabled', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    current.fetchCapability.mockResolvedValueOnce(success(capability(installed, false)))

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(true)

    expect(current.controller.capability.value?.automaticRecognitionEnabled).toBe(false)
    expect(current.controller.message.value).toBe('本地模型已安装；当前仍使用基础预切。')
    expect(current.controller.message.value).not.toContain('增强已就绪')
  })

  it('keeps a confirmed install when the full refresh returns an application error', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    current.fetchCapability.mockResolvedValueOnce(
      failure('ocr_status_failed', '状态读取失败。', true, 'diag-status'),
    )

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(true)

    expect(current.controller.capability.value?.components[0]).toEqual(installed)
    expect(current.controller.capability.value?.automaticRecognitionEnabled).toBe(false)
    expect(current.controller.message.value).toBe(
      '本地模型已安装，但完整能力状态暂时未刷新；可稍后点击“刷新”确认当前模式。',
    )
  })

  it('keeps a confirmed removal when the full refresh throws', async () => {
    const current = harness()
    current.fetchCapability.mockResolvedValueOnce(success(capability(installed, true)))
    await current.controller.refreshCapability()
    current.fetchCapability.mockRejectedValueOnce(new Error('probe unavailable'))

    await expect(current.controller.remove('ppocrv6_small')).resolves.toBe(true)

    expect(current.controller.capability.value?.components[0]).toEqual(notInstalled)
    expect(current.controller.message.value).toBe(
      '本地模型已移除，但完整能力状态暂时未刷新；可稍后点击“刷新”确认当前模式。',
    )
    expect(current.controller.message.value).not.toContain('没有移除')
  })

  it('uses the authoritative full snapshot after a successful removal refresh', async () => {
    const current = harness()
    current.fetchCapability.mockResolvedValueOnce(success(capability(installed, true)))
    await current.controller.refreshCapability()
    current.fetchCapability.mockResolvedValueOnce(success(capability(notInstalled)))

    await expect(current.controller.remove('ppocrv6_small')).resolves.toBe(true)

    expect(current.controller.capability.value).toEqual(capability(notInstalled))
    expect(current.controller.message.value).toBe(
      '本地模型已移除；基础预切、手工裁剪和题库不受影响。',
    )
  })

  it('does not invent a capability snapshot when a command succeeds before the first probe', async () => {
    const current = harness()
    current.fetchCapability.mockResolvedValueOnce(
      failure('ocr_status_failed', '状态读取失败。', true, 'diag-status'),
    )

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(true)

    expect(current.controller.capability.value).toBeUndefined()
    expect(current.controller.message.value).toContain('本地模型已安装')
  })

  it('appends a confirmed component that was absent from an older capability snapshot', async () => {
    const current = harness()
    current.fetchCapability.mockResolvedValueOnce(success({
      ...capability(notInstalled),
      components: [preprocessing],
    }))
    await current.controller.refreshCapability()
    current.fetchCapability.mockResolvedValueOnce(
      failure('ocr_status_failed', '状态读取失败。', true, 'diag-status'),
    )

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(true)

    expect(current.controller.capability.value?.components).toEqual([preprocessing, installed])
  })

  it('uses application errors and stable transport failure messages only for failed commands', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    current.installComponent.mockResolvedValueOnce(
      failure('ocr_install_failed', '下载校验失败，请重试。', true, 'diag-install'),
    )
    current.removeComponent.mockRejectedValueOnce(new Error('bridge unavailable'))

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(false)
    expect(current.controller.message.value).toBe('下载校验失败，请重试。')
    expect(current.fetchCapability).toHaveBeenCalledTimes(1)

    await expect(current.controller.remove('ppocrv6_small')).resolves.toBe(false)
    expect(current.controller.message.value).toBe('模型没有移除；当前文件和功能状态保持不变。')
    expect(current.controller.capability.value).toEqual(capability(notInstalled))
  })

  it('uses the stable install failure message for a rejected install command', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    current.installComponent.mockRejectedValueOnce(new Error('bridge unavailable'))

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(false)

    expect(current.controller.message.value).toBe(
      '模型没有安装完成；临时文件已清理，基础预切仍可继续使用。',
    )
  })

  it('invalidates an older probe and blocks duplicate work during a component mutation', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    const oldProbe = deferred<ReturnType<typeof success<OcrCapabilityStatus>>>()
    current.fetchCapability.mockReturnValueOnce(oldProbe.promise)
    const stale = current.controller.refreshCapability()

    const installGate = deferred<ReturnType<typeof success<OcrComponentStatus>>>()
    current.installComponent.mockReturnValueOnce(installGate.promise)
    current.fetchCapability.mockResolvedValueOnce(success(capability(installed, true)))
    const mutation = current.controller.install('ppocrv6_small')
    expect(current.controller.busy.value).toBe(true)

    await expect(current.controller.install('ppocrv6_small')).resolves.toBe(false)
    await expect(current.controller.remove('ppocrv6_small')).resolves.toBe(false)
    await expect(current.controller.refreshCapability()).resolves.toBe(false)
    expect(current.installComponent).toHaveBeenCalledTimes(1)
    expect(current.removeComponent).not.toHaveBeenCalled()

    installGate.resolve(success(installed))
    await expect(mutation).resolves.toBe(true)
    oldProbe.resolve(success(capability(notInstalled)))
    await expect(stale).resolves.toBe(false)
    expect(current.controller.capability.value).toEqual(capability(installed, true))
  })

  it('ignores a stale thrown probe after a newer refresh commits', async () => {
    const current = harness()
    const staleProbe = deferred<ReturnType<typeof success<OcrCapabilityStatus>>>()
    current.fetchCapability
      .mockReturnValueOnce(staleProbe.promise)
      .mockResolvedValueOnce(success(capability(installed, true)))

    const stale = current.controller.refreshCapability()
    await expect(current.controller.refreshCapability()).resolves.toBe(true)
    staleProbe.reject(new Error('old probe failed'))

    await expect(stale).resolves.toBe(false)
    expect(current.controller.capability.value).toEqual(capability(installed, true))
    expect(current.controller.message.value).toBe('')
  })

  it('reports initial probe failures without discarding the last successful snapshot', async () => {
    const current = harness()
    await current.controller.refreshCapability()
    current.fetchCapability
      .mockResolvedValueOnce(failure('ocr_status_failed', '无法读取模型状态。', true, 'diag'))
      .mockRejectedValueOnce(new Error('offline'))

    await expect(current.controller.refreshCapability()).resolves.toBe(false)
    expect(current.controller.message.value).toBe('无法读取模型状态。')
    expect(current.controller.capability.value).toEqual(capability(notInstalled))
    await expect(current.controller.refreshCapability()).resolves.toBe(false)
    expect(current.controller.message.value).toBe('模型状态暂时无法读取；基础预切仍可继续使用。')
    expect(current.controller.capability.value).toEqual(capability(notInstalled))
  })
})
