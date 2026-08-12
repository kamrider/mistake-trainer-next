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

  it('enters four-corner perspective mode and emits a keyboard-adjusted quad', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '斜拍练习.png',
        busy: false,
      },
    })

    await user.click(screen.getByRole('button', { name: '透视矫正' }))
    const handles = view.container.querySelectorAll<HTMLButtonElement>('.perspective-handle')
    expect(handles).toHaveLength(4)
    const topLeft = screen.getByRole('button', { name: '透视左上角' })
    topLeft.focus()
    await user.keyboard('{ArrowRight}{ArrowDown}')
    await user.click(screen.getByRole('button', { name: '完成四角' }))
    await user.click(screen.getByRole('button', { name: '生成 1 张裁剪图' }))

    const recipes = (view.emitted('apply') as unknown[][])[0]![0] as CaptureCropRecipe[]
    expect(recipes[0]!.perspectiveQuad).toEqual({
      topLeft: { x: 0.005, y: 0.005 },
      topRight: { x: 1, y: 0 },
      bottomRight: { x: 1, y: 1 },
      bottomLeft: { x: 0, y: 1 },
    })
  })

  it('resets and can remove an enabled perspective correction', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '斜拍练习.png',
        busy: false,
      },
    })
    await user.click(screen.getByRole('button', { name: '透视矫正' }))
    screen.getByRole('button', { name: '透视左上角' }).focus()
    await user.keyboard('{ArrowRight}{ArrowDown}')
    await user.click(screen.getByRole('button', { name: '重置四角' }))
    await user.click(screen.getByRole('button', { name: '完成四角' }))
    await user.click(screen.getByRole('button', { name: '移除透视' }))
    await user.click(screen.getByRole('button', { name: '生成 1 张裁剪图' }))

    const recipes = (view.emitted('apply') as unknown[][])[0]![0] as CaptureCropRecipe[]
    expect(recipes[0]!.perspectiveQuad).toBeNull()
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

  it('locks every crop mutation while an atomic save is in progress', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '保存中的练习.png',
        busy: false,
      },
    })

    await user.click(screen.getByRole('button', { name: '再框一道题' }))
    await view.rerender({ busy: true })

    expect(screen.getByRole('dialog', { name: '裁出真正需要的题目范围' })).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByRole('button', { name: '删除区域 1' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '下移区域 1' })).toBeDisabled()
    expect([...view.container.querySelectorAll<HTMLButtonElement>('.resize-handle')].every(handle => handle.disabled)).toBe(true)

    const firstRegion = screen.getByRole('group', { name: '裁剪区域 1' })
    firstRegion.focus()
    await user.keyboard('{ArrowRight}')
    await user.keyboard('{Control>}z{/Control}')
    await user.keyboard('{Escape}')
    await user.click(view.container.querySelector<HTMLElement>('.crop-backdrop')!)

    expect(screen.getByRole('status')).toHaveTextContent('2 个区域')
    expect(view.emitted('close')).toBeUndefined()

    await view.rerender({ busy: false })
    await user.click(screen.getByRole('button', { name: '生成 2 张裁剪图' }))
    const recipes = (view.emitted('apply') as unknown[][])[0]![0] as CaptureCropRecipe[]
    expect(recipes).toHaveLength(2)
    expect(recipes[0]!.rect.x).toBe(0.06)
  })

  it('moves focus to the selected adjacent region after deleting a row', async () => {
    const user = userEvent.setup()
    render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '三道题.png',
        busy: false,
      },
    })

    await user.click(screen.getByRole('button', { name: '再框一道题' }))
    await user.click(screen.getByRole('button', { name: '再框一道题' }))
    await user.click(screen.getByRole('button', { name: '删除区域 2' }))

    expect(screen.getByRole('button', { name: '选择区域 1' })).toHaveFocus()
    expect(screen.getByRole('button', { name: '选择区域 1' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('status')).toHaveTextContent('2 个区域')
  })

  it('edits proposed regions without creating crop assets', async () => {
    const user = userEvent.setup()
    const view = render(CaptureCropEditor, {
      props: {
        dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
        itemName: '整页练习.png',
        busy: false,
        mode: 'proposal',
        initialRecipes: [{
          rect: { x: 0.1, y: 0.2, width: 0.7, height: 0.4 },
          rotationDegrees: 0,
          outputMediaType: 'image/png',
          maxEdge: 4096,
          jpegQuality: 90,
        }],
      },
    })

    expect(screen.getByText('这里只调整建议边界')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '保存 1 个建议区域' }))
    expect(view.emitted('apply')).toBeUndefined()
    const proposals = view.emitted('saveProposal') as unknown[][]
    expect(proposals[0]?.[0]).toEqual([{
      rect: { x: 0.1, y: 0.2, width: 0.7, height: 0.4 },
      perspectiveQuad: null,
      rotationDegrees: 0,
      outputMediaType: 'image/png',
      maxEdge: 4096,
      jpegQuality: 90,
    }])
  })
})
