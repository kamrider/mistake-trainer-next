import { render, screen } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { createMemoryHistory } from 'vue-router'
import { describe, expect, it } from 'vitest'
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
})
