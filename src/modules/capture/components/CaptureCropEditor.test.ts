import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { CaptureCropRecipe } from '../../../shared/api/bindings'
import CaptureCropEditor from './CaptureCropEditor.vue'

describe('CaptureCropEditor', () => {
  it('creates multiple ordered regions and emits one atomic crop request', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '整页练习.png',
        busy: false,
      },
    })

    expect(screen.getByRole('dialog', { name: '裁出真正需要的题目范围' })).toBeVisible()
    await user.click(screen.getByRole('button', { name: '再框一道题' }))
    expect(screen.getAllByText(/区域 [12]/)).toHaveLength(2)
    await user.click(screen.getByRole('button', { name: '生成 2 张裁剪图' }))

    const requests = view.emitted('apply') as unknown[][]
    const recipes = requests[0]![0] as CaptureCropRecipe[]
    expect(requests).toHaveLength(1)
    expect(recipes).toHaveLength(2)
    expect(recipes[0]).toMatchObject({
      rotationDegrees: 0,
      outputMediaType: 'image/png',
      maxEdge: 4096,
    })
  })

  it('supports keyboard nudging and Escape without a hidden double-click gesture', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '题目.png',
        busy: false,
      },
    })
    const region = screen.getByRole('button', { name: '裁剪区域 1' })
    await user.click(region)
    await user.keyboard('{ArrowRight}')
    await user.keyboard('{Escape}')
    expect(view.emitted('close')).toHaveLength(1)
  })
})
