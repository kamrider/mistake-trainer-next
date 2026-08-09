import { describe, expect, it, vi } from 'vitest'
import { failure, success } from '../../shared/api/app-result'
import { useQueuedPreferenceSave } from './useQueuedPreferenceSave'

interface PreferenceInput {
  mode: string
}

interface PreferenceOutput extends PreferenceInput {
  version: number
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((finish, fail) => { resolve = finish; reject = fail })
  return { promise, resolve, reject }
}

function harness(initial: PreferenceInput | undefined = { mode: 'initial' }) {
  let current: PreferenceInput | undefined = initial
  const applied: PreferenceOutput[] = []
  const persist = vi.fn(async (input: PreferenceInput) => success({ ...input, version: 1 }))
  const controller = useQueuedPreferenceSave<PreferenceInput, PreferenceOutput>({
    snapshot: () => current ? { ...current } : undefined,
    persist,
    applySaved: output => applied.push(output),
    successMessage: '偏好已保存。',
    failureMessage: '偏好没有保存成功。',
    queuedMessage: '检测到新修改，完成当前保存后会自动继续。',
  })
  return {
    controller,
    persist,
    applied,
    setCurrent(value: PreferenceInput | undefined) { current = value },
  }
}

describe('useQueuedPreferenceSave', () => {
  it('applies the authoritative result for a stable draft', async () => {
    const current = harness()
    expect(current.controller.revision.value).toBe(0)
    current.controller.markChanged()
    expect(current.controller.revision.value).toBe(1)

    await expect(current.controller.save()).resolves.toBe(true)

    expect(current.persist).toHaveBeenCalledWith({ mode: 'initial' })
    expect(current.applied).toEqual([{ mode: 'initial', version: 1 }])
    expect(current.controller.dirty.value).toBe(false)
    expect(current.controller.saving.value).toBe(false)
    expect(current.controller.message.value).toBe('偏好已保存。')
    expect(current.controller.revision.value).toBe(1)

    current.controller.markChanged()
    expect(current.controller.dirty.value).toBe(true)
    expect(current.controller.message.value).toBe('')
  })

  it('returns false without persistence when no snapshot exists', async () => {
    const current = harness()
    current.setCurrent(undefined)
    current.controller.markChanged()

    await expect(current.controller.save()).resolves.toBe(false)

    expect(current.persist).not.toHaveBeenCalled()
    expect(current.controller.dirty.value).toBe(true)
    expect(current.controller.saving.value).toBe(false)
  })

  it('ignores an older result and automatically persists the latest edited draft', async () => {
    const current = harness({ mode: 'first' })
    const first = deferred<ReturnType<typeof success<PreferenceOutput>>>()
    current.persist
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(success({ mode: 'latest', version: 2 }))
    current.controller.markChanged()

    const pending = current.controller.save()
    current.setCurrent({ mode: 'latest' })
    current.controller.markChanged()
    expect(current.controller.message.value).toBe('检测到新修改，完成当前保存后会自动继续。')

    first.resolve(success({ mode: 'first', version: 1 }))
    await vi.waitFor(() => expect(current.persist).toHaveBeenCalledTimes(2))
    expect(current.persist).toHaveBeenNthCalledWith(2, { mode: 'latest' })
    expect(current.applied).not.toContainEqual({ mode: 'first', version: 1 })

    await expect(pending).resolves.toBe(true)
    expect(current.applied).toEqual([{ mode: 'latest', version: 2 }])
    expect(current.controller.dirty.value).toBe(false)
    expect(current.controller.message.value).toBe('偏好已保存。')
  })

  it('coalesces repeated edits and duplicate save requests into one latest follow-up', async () => {
    const current = harness({ mode: 'first' })
    const first = deferred<ReturnType<typeof success<PreferenceOutput>>>()
    current.persist
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(success({ mode: 'third', version: 3 }))
    current.controller.markChanged()

    const pending = current.controller.save()
    await expect(current.controller.save()).resolves.toBe(false)
    current.setCurrent({ mode: 'second' })
    current.controller.markChanged()
    current.setCurrent({ mode: 'third' })
    current.controller.markChanged()
    expect(current.persist).toHaveBeenCalledTimes(1)

    first.resolve(success({ mode: 'first', version: 1 }))
    await expect(pending).resolves.toBe(true)

    expect(current.persist).toHaveBeenCalledTimes(2)
    expect(current.persist).toHaveBeenLastCalledWith({ mode: 'third' })
    expect(current.applied).toEqual([{ mode: 'third', version: 3 }])
  })

  it('stops safely if a queued draft disappears before the follow-up starts', async () => {
    const current = harness({ mode: 'first' })
    const first = deferred<ReturnType<typeof success<PreferenceOutput>>>()
    current.persist.mockReturnValueOnce(first.promise)
    current.controller.markChanged()

    const pending = current.controller.save()
    current.setCurrent(undefined)
    current.controller.markChanged()
    first.resolve(success({ mode: 'first', version: 1 }))

    await expect(pending).resolves.toBe(false)
    expect(current.persist).toHaveBeenCalledTimes(1)
    expect(current.applied).toEqual([])
    expect(current.controller.dirty.value).toBe(true)
    expect(current.controller.saving.value).toBe(false)
  })

  it('preserves dirty state and the draft for application and transport failures', async () => {
    const current = harness()
    current.persist
      .mockResolvedValueOnce(failure('save_failed', '服务端拒绝了偏好。', true, 'diag-save'))
      .mockRejectedValueOnce(new Error('bridge unavailable'))
    current.controller.markChanged()

    await expect(current.controller.save()).resolves.toBe(false)
    expect(current.controller.message.value).toBe('服务端拒绝了偏好。')
    expect(current.controller.dirty.value).toBe(true)
    expect(current.applied).toEqual([])

    await expect(current.controller.save()).resolves.toBe(false)
    expect(current.controller.message.value).toBe('偏好没有保存成功。')
    expect(current.controller.dirty.value).toBe(true)
    expect(current.controller.saving.value).toBe(false)
  })

  it('validates every queued snapshot before persistence', async () => {
    const current = harness({ mode: '' })
    const validate = vi.fn((input: PreferenceInput) => input.mode ? undefined : '请选择偏好。')
    const controller = useQueuedPreferenceSave<PreferenceInput, PreferenceOutput>({
      snapshot: () => ({ mode: '' }),
      persist: current.persist,
      applySaved: output => current.applied.push(output),
      validate,
      successMessage: '偏好已保存。',
      failureMessage: '偏好没有保存成功。',
      queuedMessage: '会自动继续保存。',
    })
    controller.markChanged()

    await expect(controller.save()).resolves.toBe(false)

    expect(validate).toHaveBeenCalledWith({ mode: '' })
    expect(current.persist).not.toHaveBeenCalled()
    expect(controller.message.value).toBe('请选择偏好。')
    expect(controller.dirty.value).toBe(true)
  })
})
