import { describe, expect, it } from 'vitest'
import { getDialogFocusableElements, trapDialogFocus } from './dialog-focus'

function tabEvent(shiftKey = false) {
  return new KeyboardEvent('keydown', {
    key: 'Tab',
    shiftKey,
    bubbles: true,
    cancelable: true,
  })
}

describe('trapDialogFocus', () => {
  it('wraps enabled controls and recovers focus from outside the container', () => {
    const outside = document.createElement('button')
    const container = document.createElement('section')
    container.tabIndex = -1
    const first = document.createElement('button')
    const disabled = document.createElement('button')
    disabled.disabled = true
    const last = document.createElement('button')
    const skippedLink = document.createElement('a')
    skippedLink.href = '#ignored'
    skippedLink.tabIndex = -1
    const hidden = document.createElement('button')
    hidden.hidden = true
    const inertGroup = document.createElement('div')
    inertGroup.setAttribute('inert', '')
    const inertButton = document.createElement('button')
    inertGroup.append(inertButton)
    container.append(first, disabled, last, skippedLink, hidden, inertGroup)
    document.body.append(outside, container)

    try {
      expect(getDialogFocusableElements(container)).toEqual([first, last])

      first.focus()
      const reverse = tabEvent(true)
      trapDialogFocus(reverse, container)
      expect(last).toHaveFocus()
      expect(reverse.defaultPrevented).toBe(true)

      last.focus()
      const forward = tabEvent()
      trapDialogFocus(forward, container)
      expect(first).toHaveFocus()
      expect(forward.defaultPrevented).toBe(true)

      outside.focus()
      trapDialogFocus(tabEvent(), container)
      expect(first).toHaveFocus()
      outside.focus()
      trapDialogFocus(tabEvent(true), container)
      expect(last).toHaveFocus()
    }
    finally {
      outside.remove()
      container.remove()
    }
  })

  it('focuses an empty container and ignores non-Tab keys', () => {
    const container = document.createElement('section')
    container.tabIndex = -1
    document.body.append(container)

    try {
      const enter = new KeyboardEvent('keydown', { key: 'Enter', cancelable: true })
      trapDialogFocus(enter, container)
      expect(enter.defaultPrevented).toBe(false)
      expect(container).not.toHaveFocus()

      const tab = tabEvent()
      trapDialogFocus(tab, container)
      expect(tab.defaultPrevented).toBe(true)
      expect(container).toHaveFocus()
    }
    finally {
      container.remove()
    }
  })
})
