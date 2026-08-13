type ScrollLockState = {
  depth: number
  previousOverflow: string
}

const scrollLockStates = new WeakMap<Document, ScrollLockState>()

export function acquireDialogScrollLock(ownerDocument: Document = document) {
  let state = scrollLockStates.get(ownerDocument)
  if (!state) {
    state = {
      depth: 0,
      previousOverflow: ownerDocument.body.style.overflow,
    }
    scrollLockStates.set(ownerDocument, state)
  }
  state.depth += 1
  ownerDocument.body.style.overflow = 'hidden'

  let released = false
  return () => {
    if (released) return
    released = true
    state.depth -= 1
    if (state.depth === 0) {
      ownerDocument.body.style.overflow = state.previousOverflow
      scrollLockStates.delete(ownerDocument)
    }
  }
}
