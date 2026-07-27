import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type {
  CaptureRecognitionJob,
  OcrRecognitionFeatureStatus,
} from '../../../shared/api/bindings'
import CaptureRecognitionEntry from './CaptureRecognitionEntry.vue'

function feature(state: OcrRecognitionFeatureStatus['state']): OcrRecognitionFeatureStatus {
  return {
    state,
    requiredComponentId: state === 'ready' ? 'opencv_preprocess' : 'ppocrv6_small',
    detail: state === 'evidence_gate_pending'
      ? '智能分题仍在真实题图验证中；顺序模板和手工整理可继续使用。'
      : '智能分题需要已校验的 PP‑OCRv6 small 本地模型。',
  }
}

function job(state: CaptureRecognitionJob['state'], processedItems = 12): CaptureRecognitionJob {
  return {
    id: 'job-1',
    batchId: 'batch-1',
    state,
    totalItems: 50,
    processedItems,
    suggestions: [],
    createdAtUtcMs: 1,
    updatedAtUtcMs: 2,
  }
}

describe('CaptureRecognitionEntry', () => {
  it('explains the evidence gate without presenting a dead start button', () => {
    render(CaptureRecognitionEntry, {
      props: {
        batchState: 'organizing',
        unassignedCount: 12,
        feature: feature('evidence_gate_pending'),
      },
    })

    expect(screen.getByText('准确率验证完成后开放')).toBeVisible()
    expect(
      screen.getByText('当前不会运行识别，也不会改动原图；你可以继续用顺序模板或手工拖拽。'),
    ).toBeVisible()
    expect(screen.getByRole('link', { name: '使用顺序模板' })).toHaveAttribute(
      'href',
      '#capture-layout-templates',
    )
    expect(screen.queryByRole('button', { name: /分析 12 张/ })).not.toBeInTheDocument()
  })

  it('explains a missing runtime without sending the user to model setup', () => {
    const view = render(CaptureRecognitionEntry, {
      props: {
        batchState: 'organizing',
        unassignedCount: 12,
        feature: feature('runtime_missing'),
      },
    })

    expect(view.container.querySelector('.is-explain_runtime')).not.toBeNull()
    expect(
      view.container.querySelector('a[href="#capture-layout-templates"]'),
    ).not.toBeNull()
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('does not offer a model download when a legacy capability reports model missing', () => {
    render(CaptureRecognitionEntry, {
      props: {
        batchState: 'organizing',
        unassignedCount: 12,
        feature: feature('model_missing'),
      },
    })

    expect(screen.getByText('本地切图组件暂不可用')).toBeVisible()
    expect(screen.queryByRole('button', { name: /模型/ })).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: '使用顺序模板' })).toBeVisible()
  })

  it('shows a model-free material-library disclosure before starting local splitting', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionEntry, {
      props: {
        batchState: 'organizing',
        unassignedCount: 12,
        feature: feature('ready'),
      },
    })

    expect(screen.getByText('智能切图 · 已开放')).toBeVisible()
    expect(screen.getByText('只看版面和留白，不读取文字；原图和现有题卡不会改变。')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '智能切分 12 张素材' }))
    expect(screen.getByText('将切分 12 张未分配素材')).toBeVisible()
    expect(screen.getByText('只在这台电脑运行')).toBeVisible()
    expect(screen.getByText('不使用 OCR，不下载模型')).toBeVisible()
    expect(screen.getByText('不会新建或改动题卡')).toBeVisible()
    expect(screen.getByText('原图始终保留')).toBeVisible()
    expect(screen.getByText('切图继承原素材的题图 / 答案角色')).toBeVisible()
    expect(screen.getByText('一页多题会建议拆成多张素材图片')).toBeVisible()
    expect(screen.getByText('低可信页面保持整页并提示手工整理')).toBeVisible()
    expect(screen.getByText('确认后的切图全部进入素材牌库')).toBeVisible()
    expect(view.emitted('start')).toBeUndefined()

    await user.click(screen.getByRole('button', { name: '开始切图' }))
    expect(view.emitted('start')).toHaveLength(1)
  })

  it('shows compact progress and leaves cancellation available', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionEntry, {
      props: {
        batchState: 'organizing',
        unassignedCount: 38,
        feature: feature('ready'),
        job: job('running'),
      },
    })

    expect(screen.getByRole('status')).toHaveTextContent('已分析 12 / 50')
    expect(screen.getByText('只分析版面和留白；你可以继续整理题卡，切图结果不会自动应用。')).toBeVisible()
    expect(screen.getByRole('progressbar', { name: '智能切图进度 12 / 50' })).toHaveValue(12)
    await view.rerender({ job: job('running', 13) })
    expect(screen.getAllByRole('status')).toHaveLength(1)
    expect(screen.getByRole('status')).toHaveTextContent('已分析 13 / 50')
    await user.click(screen.getByRole('button', { name: '停止识别' }))
    expect(view.emitted('cancel')).toEqual([['job-1']])
  })

  it('resumes review even when no images remain unassigned', async () => {
    const user = userEvent.setup()
    const view = render(CaptureRecognitionEntry, {
      props: {
        batchState: 'organizing',
        unassignedCount: 0,
        feature: feature('ready'),
        job: job('review', 50),
      },
    })

    expect(screen.getByText('先确认，再放入素材牌库')).toBeVisible()
    await user.click(screen.getByRole('button', { name: '查看切图建议' }))
    expect(view.emitted('resume')).toEqual([['job-1']])
  })
})
