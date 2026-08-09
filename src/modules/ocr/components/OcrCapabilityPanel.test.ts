import { render, screen, within } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import OcrCapabilityPanel from './OcrCapabilityPanel.vue'

describe('OcrCapabilityPanel', () => {
  it('presents model-free visual splitting as the available mode', () => {
    render(OcrCapabilityPanel)

    const panel = screen.getByRole('region', { name: '智能功能模式' })
    const mode = within(panel).getByRole('article', { name: '智能切图（已开放）' })
    expect(mode).toHaveTextContent('已开放')
    expect(mode).toHaveTextContent('基础预切无需模型，也不会联网')
    expect(mode).toHaveTextContent('small 是切题主力')
    expect(mode).toHaveTextContent('只进入素材牌库')
  })

  it('labels full recognition as a separate unavailable future mode', () => {
    render(OcrCapabilityPanel)

    const mode = screen.getByRole('article', { name: '全自动识题（未开放）' })
    expect(mode).toHaveTextContent('未开放')
    expect(mode).toHaveTextContent('识别科目、题干与答案')
    expect(mode).toHaveTextContent('题答匹配、自动组卡和一键导出')
    expect(mode).toHaveTextContent('当前版本不会运行或发起下载')
  })

  it('offers no model download or execution control for the future mode', () => {
    render(OcrCapabilityPanel)

    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    expect(screen.queryByRole('link')).not.toBeInTheDocument()
  })

  it('does not show enhancement as enabled when a confirmed removal leaves stale derived status', () => {
    render(OcrCapabilityPanel, {
      props: {
        status: {
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
          components: [{
            id: 'ppocrv6_small',
            displayName: 'PP‑OCRv6 small',
            description: '面向题号定位。',
            state: 'not_installed',
            downloadBytes: 31_163_977,
            installedBytes: 0,
            recommended: true,
            installAllowed: true,
            statusDetail: '本地模型已移除。',
            sourceLabel: 'ModelScope',
            licenseLabel: 'Apache-2.0',
          }],
          recognitionFeature: {
            state: 'ready',
            requiredComponentId: 'ppocrv6_small',
            detail: '旧的派生状态尚未刷新。',
          },
          automaticRecognitionEnabled: true,
        },
      },
    })

    const mode = screen.getByRole('article', { name: '智能切图（已开放）' })
    expect(mode).toHaveTextContent('基础版已开放')
    expect(mode).not.toHaveTextContent('题号增强已启用')
    expect(within(mode).getByRole('button', { name: '启用更准切题' })).toBeEnabled()
  })
})
