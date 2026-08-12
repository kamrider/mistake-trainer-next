export function createRecoverySingleFlight() {
  let active: Promise<boolean> | undefined

  return (operation: () => Promise<boolean>): Promise<boolean> => {
    if (active) return active
    const task = operation().finally(() => {
      if (active === task) active = undefined
    })
    active = task
    return task
  }
}
