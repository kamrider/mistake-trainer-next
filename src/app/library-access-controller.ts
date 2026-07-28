import type { InjectionKey } from 'vue'

export interface LibraryAccessController {
  enterRestarting: () => void
}

export const libraryAccessControllerKey: InjectionKey<LibraryAccessController>
  = Symbol('library-access-controller')
