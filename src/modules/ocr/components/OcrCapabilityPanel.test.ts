import { render, screen, within } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import OcrCapabilityPanel from './OcrCapabilityPanel.vue'

describe('OcrCapabilityPanel', () => {
  it('presents model-free visual splitting as the available mode', () => {
    render(OcrCapabilityPanel)

    const panel = screen.getByRole('region', { name: '智能功能模式' })
    const mode = within(panel).getByRole('article', { name: '智能切图（已开放）' })
    expect(mode).toHaveTextContent('已开放')
    expect(mode).toHaveTextContent('不读取文字，不使用 OCR')
    expect(mode).toHaveTextContent('不下载 small / medium 模型')
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
})
