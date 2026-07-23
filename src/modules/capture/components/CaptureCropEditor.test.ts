import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { CaptureCropRecipe } from '../../../shared/api/bindings'
import CaptureCropEditor from './CaptureCropEditor.vue'

describe('CaptureCropEditor', () => {
  it('exposes a reachable crop canvas and eight resize handles', () => {
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '整页练习.png',
        busy: false,
      },
    })

    expect(screen.getByRole('region', { name: '裁剪画布' })).toHaveAttribute('tabindex', '0')
    const handles = view.container.querySelectorAll<HTMLButtonElement>('.resize-handle')
    expect(handles).toHaveLength(8)
    expect([...handles].every(handle => Boolean(handle.getAttribute('aria-label')))).toBe(true)
    expect(view.container.querySelector('[ondblclick]')).not.toBeInTheDocument()
  })

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
    const region = screen.getByRole('group', { name: '裁剪区域 1' })
    await user.click(region)
    await user.keyboard('{ArrowRight}')
    await user.keyboard('{Escape}')
    expect(view.emitted('close')).toHaveLength(1)
  })

  it('zooms the viewport without changing normalized crop output', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '题目.png',
        busy: false,
      },
    })

    await user.click(screen.getByRole('button', { name: '放大' }))
    expect(screen.getByText('125%')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '生成 1 张裁剪图' }))

    const requests = view.emitted('apply') as unknown[][]
    const recipes = requests[0]![0] as CaptureCropRecipe[]
    expect(recipes[0]!.rect).toEqual({ x: 0.06, y: 0.06, width: 0.88, height: 0.88 })
  })

  it('reorders the visual region filmstrip and emits recipes in that order', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '三道题.png',
        busy: false,
      },
    })

    await user.click(screen.getByRole('button', { name: '再框一道题' }))
    await user.click(screen.getByRole('button', { name: '再框一道题' }))
    expect(screen.getByRole('button', { name: '上移区域 1' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '下移区域 3' })).toBeDisabled()

    await user.click(screen.getByRole('button', { name: '上移区域 3' }))
    await user.click(screen.getByRole('button', { name: '生成 3 张裁剪图' }))

    const requests = view.emitted('apply') as unknown[][]
    const recipes = requests[0]![0] as CaptureCropRecipe[]
    expect(recipes.map(recipe => recipe.rect.x)).toEqual([0.06, 0.096, 0.078])
  })
})
