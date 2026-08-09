import type { InjectionKey } from 'vue'

export type WorkspaceTransitionAttempt = () => boolean | Promise<boolean>

export interface WorkspaceTransitionGuard {
  register: (attempt: WorkspaceTransitionAttempt) => () => void
  attempt: () => Promise<boolean>
}

export const workspaceTransitionGuardKey: InjectionKey<WorkspaceTransitionGuard>
  = Symbol('workspace-transition-guard')

export function createWorkspaceTransitionGuard(): WorkspaceTransitionGuard {
  const registered = new Set<WorkspaceTransitionAttempt>()
  let pendingDecision: Promise<boolean> | undefined

  async function evaluate() {
    for (const attempt of [...registered]) {
      if (!await attempt()) return false
    }
    return true
  }

  function attempt() {
    if (pendingDecision) return pendingDecision
    const decision = evaluate().finally(() => {
      pendingDecision = undefined
    })
    pendingDecision = decision
    return decision
  }

  return {
    register(candidate) {
      registered.add(candidate)
      return () => {
        registered.delete(candidate)
      }
    },
    attempt,
  }
}
