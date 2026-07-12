import { createMemoryHistory } from 'vue-router'
import { describe, expect, it } from 'vitest'
import { createAppRouter } from './router'

describe('app router', () => {
  it('defines each primary workspace exactly once', () => {
    const router = createAppRouter(createMemoryHistory())
    const names = router.getRoutes().map((route) => route.name)

    expect(names).toEqual(expect.arrayContaining([
      'dashboard',
      'inbox',
      'library',
      'review',
      'report',
      'settings',
    ]))
    expect(new Set(names).size).toBe(names.length)
  })

  it('loads the core review room with the shell to avoid an unstyled first frame', () => {
    const router = createAppRouter(createMemoryHistory())
    const review = router.getRoutes().find((route) => route.name === 'review')

    expect(review).toBeDefined()
    expect(typeof review?.components?.default).not.toBe('function')
  })

  it('uses a real eager library workspace instead of the placeholder route', () => {
    const router = createAppRouter(createMemoryHistory())
    const library = router.getRoutes().find((route) => route.name === 'library')

    expect(library).toBeDefined()
    expect(typeof library?.components?.default).not.toBe('function')
  })

  it('uses a real eager capture workspace instead of the placeholder route', () => {
    const router = createAppRouter(createMemoryHistory())
    const inbox = router.getRoutes().find((route) => route.name === 'inbox')

    expect(inbox).toBeDefined()
    expect(typeof inbox?.components?.default).not.toBe('function')
  })
})
