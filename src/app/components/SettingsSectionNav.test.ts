import { fireEvent, render, screen, waitFor } from '@testing-library/vue'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SettingsSectionLink } from '../settings-section-catalog'
import SettingsSectionNav from './SettingsSectionNav.vue'

const sections: SettingsSectionLink[] = [
  { id: 'settings-sync', label: '同步账户', hint: '本地与云端', group: '账户与同步' },
  { id: 'settings-subjects', label: '科目配置', hint: '采集常用项', group: '学习体验' },
  { id: 'settings-review', label: '训练节奏', hint: '专注插曲', group: '学习体验' },
]

let observerCallback: IntersectionObserverCallback
const observe = vi.fn()
const disconnect = vi.fn()

class FakeIntersectionObserver implements IntersectionObserver {
  readonly root = null
  readonly rootMargin = '-120px 0px -55% 0px'
  readonly thresholds = [0, 0.25, 0.6]

  constructor(callback: IntersectionObserverCallback) {
    observerCallback = callback
  }

  observe = observe
  disconnect = disconnect
  unobserve = vi.fn()
  takeRecords = vi.fn(() => [])
}

function installMatchMedia(reducedMotion = false) {
  vi.stubGlobal('matchMedia', vi.fn(() => ({
    matches: reducedMotion,
    media: '(prefers-reduced-motion: reduce)',
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })))
}

function addAnchors() {
  return sections.map((section) => {
    const element = document.createElement('section')
    element.id = section.id
    element.dataset.settingsNavTest = 'true'
    element.scrollIntoView = vi.fn()
    document.body.append(element)
    return element
  })
}

function intersectionEntry(target: Element, top: number): IntersectionObserverEntry {
  const rect = {
    x: 0,
    y: top,
    top,
    left: 0,
    right: 600,
    bottom: top + 300,
    width: 600,
    height: 300,
    toJSON: () => ({}),
  }
  return {
    target,
    isIntersecting: true,
    intersectionRatio: 0.8,
    boundingClientRect: rect,
    intersectionRect: rect,
    rootBounds: null,
    time: 1,
  }
}

describe('SettingsSectionNav', () => {
  beforeEach(() => {
    observe.mockClear()
    disconnect.mockClear()
    vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver)
    installMatchMedia()
  })

  afterEach(() => {
    document.querySelectorAll('[data-settings-nav-test]').forEach(element => element.remove())
    vi.unstubAllGlobals()
  })

  it('smoothly jumps to a section and exposes the active location', async () => {
    const anchors = addAnchors()
    const view = render(SettingsSectionNav, { props: { sections } })

    expect(screen.getByRole('navigation', { name: '设置目录' })).toBeVisible()
    expect(screen.getByRole('button', { name: '查看更多设置' })).toBeVisible()
    expect(screen.getByRole('button', { name: '查看前面的设置' })).toBeVisible()
    expect(document.querySelector('.settings-section-indicator')).toHaveAttribute('aria-hidden', 'true')
    expect(screen.getByRole('group', { name: '账户与同步' })).toBeVisible()
    expect(screen.getByRole('group', { name: '学习体验' })).toBeVisible()
    expect(screen.getByRole('button', { name: /科目配置/ }))
      .toHaveAttribute('aria-controls', 'settings-subjects')
    expect(view.container.querySelector('.settings-section-scroller'))
      .toHaveAttribute('tabindex', '0')
    await userEvent.click(screen.getByRole('button', { name: /科目配置/ }))

    expect(anchors[1]!.scrollIntoView).toHaveBeenCalledWith({
      block: 'start',
      behavior: 'smooth',
    })
    expect(screen.getByRole('button', { name: /科目配置/ }))
      .toHaveAttribute('aria-current', 'location')
  })

  it('reveals and operates the horizontal directory controls when content is hidden', async () => {
    addAnchors()
    const view = render(SettingsSectionNav, { props: { sections } })
    const scroller = view.container.querySelector<HTMLElement>('.settings-section-scroller')!
    const scrollBy = vi.fn()
    scroller.scrollBy = scrollBy
    Object.defineProperty(scroller, 'clientWidth', { configurable: true, value: 240 })
    Object.defineProperty(scroller, 'scrollWidth', { configurable: true, value: 700 })
    Object.defineProperty(scroller, 'scrollLeft', { configurable: true, writable: true, value: 0 })
    window.dispatchEvent(new Event('resize'))

    const next = screen.getByRole('button', { name: '查看更多设置' })
    await waitFor(() => expect(next).toBeEnabled())
    await userEvent.click(next)
    expect(scrollBy).toHaveBeenCalledWith({ left: 180, behavior: 'smooth' })

    scroller.scrollLeft = 120
    await fireEvent.scroll(scroller)
    expect(screen.getByRole('button', { name: '查看前面的设置' })).toBeEnabled()
  })

  it('uses instant scrolling when Windows requests reduced motion', async () => {
    const anchors = addAnchors()
    installMatchMedia(true)
    render(SettingsSectionNav, { props: { sections } })

    await userEvent.click(screen.getByRole('button', { name: /训练节奏/ }))

    expect(anchors[2]!.scrollIntoView).toHaveBeenCalledWith({
      block: 'start',
      behavior: 'auto',
    })
  })

  it('follows the section nearest the sticky reading line and disconnects cleanly', async () => {
    const anchors = addAnchors()
    const view = render(SettingsSectionNav, { props: { sections } })

    await waitFor(() => expect(observe).toHaveBeenCalledTimes(3))
    const scroller = view.container.querySelector<HTMLElement>('.settings-section-scroller')!
    const reviewButton = screen.getByRole('button', { name: /训练节奏/ })
    const scrollTo = vi.fn()
    scroller.scrollTo = scrollTo
    Object.defineProperty(scroller, 'clientWidth', { configurable: true, value: 200 })
    Object.defineProperty(reviewButton, 'offsetLeft', { configurable: true, value: 440 })
    Object.defineProperty(reviewButton, 'offsetWidth', { configurable: true, value: 100 })
    observerCallback(
      [intersectionEntry(anchors[2]!, 128)],
      {} as IntersectionObserver,
    )

    await waitFor(() => expect(reviewButton).toHaveAttribute('aria-current', 'location'))
    expect(scrollTo).toHaveBeenCalledWith({ left: 390, behavior: 'smooth' })
    view.unmount()
    expect(disconnect).toHaveBeenCalled()
  })
})
