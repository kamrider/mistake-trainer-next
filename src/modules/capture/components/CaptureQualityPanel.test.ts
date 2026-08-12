import { fireEvent, render, screen } from '@testing-library/vue'
import { describe, expect, it } from 'vitest'
import CaptureQualityPanel from './CaptureQualityPanel.vue'

describe('CaptureQualityPanel', () => {
  it('explains issues and keeps all choices explicit', async () => {
    const view = render(CaptureQualityPanel, {
      props: {
        busy: false,
        report: {
          itemId: 'item-1',
          issues: ['possible_edge_cut', 'skewed'],
          sharpnessScore: 0.4,
          darkFraction: 0.02,
          brightFraction: 0.5,
          contrastScore: 0.7,
          suggestedRotationDegrees: -2.4,
          suggestedCrop: null,
        },
      },
    })

    expect(screen.getByText(/内容贴近图片边缘/)).toBeInTheDocument()
    expect(screen.getByText('建议旋转 -2.4°')).toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: '继续使用' }))
    await fireEvent.click(screen.getByRole('button', { name: '重新选择' }))
    await fireEvent.click(screen.getByRole('button', { name: '打开裁剪修正' }))
    expect(view.emitted('dismiss')).toHaveLength(1)
    expect(view.emitted('reselect')).toHaveLength(1)
    expect(view.emitted('crop')).toHaveLength(1)
  })

  it('shows a quiet success state when no issue is detected', () => {
    render(CaptureQualityPanel, {
      props: {
        busy: false,
        report: {
          itemId: 'item-1', issues: [], sharpnessScore: 0.3, darkFraction: 0,
          brightFraction: 0.6, contrastScore: 0.7, suggestedRotationDegrees: 0,
          suggestedCrop: null,
        },
      },
    })
    expect(screen.getByText('图片质量看起来良好')).toBeInTheDocument()
  })
})
