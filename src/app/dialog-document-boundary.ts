import { acquireDialogScrollLock } from './dialog-scroll-lock'

type InertState = {
  depth: number
  wasInert: boolean
}

const inertStates = new WeakMap<HTMLElement, InertState>()

export function acquireDialogBackgroundInert(modalRoot: HTMLElement) {
  const acquired: HTMLElement[] = []
  const body = modalRoot.ownerDocument.body
  let current: HTMLElement | null = modalRoot

  while (current && current !== body) {
    const parentElement: HTMLElement | null = current.parentElement
    if (!parentElement) break
    for (const sibling of parentElement.children) {
      if (sibling === current || !(sibling instanceof HTMLElement)) continue
      let state = inertStates.get(sibling)
      if (!state) {
        state = {
          depth: 0,
          wasInert: sibling.hasAttribute('inert'),
        }
        inertStates.set(sibling, state)
      }
      state.depth += 1
      sibling.setAttribute('inert', '')
      acquired.push(sibling)
    }
    current = parentElement
  }

  let released = false
  return () => {
    if (released) return
    released = true
    for (const element of [...acquired].reverse()) {
      const state = inertStates.get(element)
      if (!state) continue
      state.depth -= 1
      if (state.depth === 0) {
        if (!state.wasInert) element.removeAttribute('inert')
        inertStates.delete(element)
      }
    }
  }
}

export function acquireDialogDocumentBoundary(modalRoot: HTMLElement) {
  const releaseScrollLock = acquireDialogScrollLock(modalRoot.ownerDocument)
  const releaseBackgroundInert = acquireDialogBackgroundInert(modalRoot)
  let released = false

  return () => {
    if (released) return
    released = true
    releaseBackgroundInert()
    releaseScrollLock()
  }
}
