import { render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { createAppRouter } from './router'

describe('App', () => {
  it('navigates from the training desk to the library workspace', async () => {
    const user = userEvent.setup()
    const router = createAppRouter(createMemoryHistory())
    await router.push('/')
    await router.isReady()

    render(App, { global: { plugins: [router] } })
    await user.click(screen.getByRole('button', { name: '题库' }))

    expect(await screen.findByRole('heading', { name: '题库' })).toBeVisible()
  })

  it('keeps rendering content while cycling through every sidebar page', async () => {
    const user = userEvent.setup()
    const router = createAppRouter(createMemoryHistory())
    const warning = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    await router.push('/')
    await router.isReady()

    const view = render(App, {
      global: {
        plugins: [router],
        stubs: { transition: false },
      },
    })
    const destinations = [
      ['采集整理', 'inbox'],
      ['题库', 'library'],
      ['训练室', 'review'],
      ['学习报告', 'report'],
      ['设置', 'settings'],
      ['训练台', 'dashboard'],
    ] as const

    for (let cycle = 0; cycle < 3; cycle += 1) {
      for (const [label, routeName] of destinations) {
        await user.click(screen.getByRole('button', { name: label }))
        await waitFor(() => {
          expect(router.currentRoute.value.name).toBe(routeName)
          expect(view.container.querySelector('.route-page')?.childElementCount).toBeGreaterThan(0)
        })
      }
    }

    expect(warning).not.toHaveBeenCalledWith(
      expect.stringContaining('renders non-element root node'),
    )
    warning.mockRestore()
  })

  it('shows a recoverable message instead of a blank page when a route throws', async () => {
    const router = createAppRouter(createMemoryHistory())
    router.addRoute({
      name: 'broken-route',
      path: '/broken-route',
      component: { setup() { throw new Error('render failed') }, template: '<div />' },
      meta: { shellPage: 'settings' },
    })
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    await router.push('/broken-route')
    await router.isReady()

    render(App, { global: { plugins: [router] } })

    expect(await screen.findByRole('alert')).toHaveTextContent('这个页面暂时打不开')
    expect(screen.getByRole('button', { name: '回到训练台' })).toBeVisible()
    error.mockRestore()
  })
})
