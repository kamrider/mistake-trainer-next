const dialogFocusableSelector = [
  'button:not([disabled])',
  '[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'summary',
  '[tabindex]:not([tabindex="-1"])',
].join(', ')

export function getDialogFocusableElements(container: HTMLElement | undefined) {
  if (!container) return []
  return Array.from(
    container.querySelectorAll<HTMLElement>(dialogFocusableSelector),
  ).filter(element =>
    element.getAttribute('tabindex') !== '-1'
    && !element.closest('[hidden], [inert]'),
  )
}

export function trapDialogFocus(
  event: KeyboardEvent,
  container: HTMLElement | undefined,
) {
  if (event.key !== 'Tab' || !container) return
  const focusable = getDialogFocusableElements(container)

  if (!focusable.length) {
    event.preventDefault()
    container.focus()
    return
  }

  const first = focusable[0]!
  const last = focusable.at(-1)!
  const active = document.activeElement
  const activeOutsideRing = active === container
    || !(active instanceof Node)
    || !container.contains(active)

  if (event.shiftKey && (active === first || activeOutsideRing)) {
    event.preventDefault()
    last.focus()
  }
  else if (!event.shiftKey && (active === last || activeOutsideRing)) {
    event.preventDefault()
    first.focus()
  }
}
