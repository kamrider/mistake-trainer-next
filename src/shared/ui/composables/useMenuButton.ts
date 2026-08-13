import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

type MenuFocusEdge = 'first' | 'last'

function enabledMenuItems(menu: HTMLElement | null) {
  return Array.from(
    menu?.querySelectorAll<HTMLElement>(
      '[role="menuitem"]:not(:disabled):not([aria-disabled="true"])',
    ) ?? [],
  )
}

export function useMenuButton() {
  const activeMenuKey = ref('')
  let menuLauncher: HTMLButtonElement | null = null

  function resolveMenu(launcher = menuLauncher) {
    const menuId = launcher?.getAttribute('aria-controls')
    if (!menuId) return null
    const menu = document.getElementById(menuId)
    return menu instanceof HTMLElement ? menu : null
  }

  function getMenuLauncher() {
    return menuLauncher
  }

  function closeMenu({ restoreFocus = false } = {}) {
    const launcher = menuLauncher
    activeMenuKey.value = ''
    menuLauncher = null
    if (restoreFocus && launcher?.isConnected) {
      void nextTick(() => launcher.focus())
    }
  }

  async function openMenu(
    launcher: HTMLButtonElement,
    key: string,
    edge: MenuFocusEdge = 'first',
  ) {
    menuLauncher = launcher
    activeMenuKey.value = key
    await nextTick()
    const items = enabledMenuItems(resolveMenu(launcher))
    const destination = edge === 'last' ? items.at(-1) : items[0]
    destination?.focus()
  }

  async function toggleMenu(event: MouseEvent, key: string) {
    const launcher = event.currentTarget
    if (!(launcher instanceof HTMLButtonElement)) return
    if (activeMenuKey.value === key) closeMenu({ restoreFocus: true })
    else await openMenu(launcher, key)
  }

  async function handleMenuButtonKeydown(event: KeyboardEvent, key: string) {
    const launcher = event.currentTarget
    if (!(launcher instanceof HTMLButtonElement)) return
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      await openMenu(launcher, key, event.key === 'ArrowUp' ? 'last' : 'first')
    }
    else if (event.key === 'Escape' && activeMenuKey.value === key) {
      event.preventDefault()
      event.stopPropagation()
      closeMenu({ restoreFocus: true })
    }
  }

  function handleMenuKeydown(event: KeyboardEvent) {
    const menu = event.currentTarget
    if (!(menu instanceof HTMLElement)) return
    const items = enabledMenuItems(menu)
    const currentIndex = items.indexOf(document.activeElement as HTMLElement)

    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      closeMenu({ restoreFocus: true })
      return
    }
    if (event.key === 'Tab') {
      closeMenu()
      return
    }
    if (currentIndex < 0 || items.length === 0) return

    let nextIndex: number
    if (event.key === 'ArrowDown' || event.key === 'ArrowRight')
      nextIndex = (currentIndex + 1) % items.length
    else if (event.key === 'ArrowUp' || event.key === 'ArrowLeft')
      nextIndex = (currentIndex - 1 + items.length) % items.length
    else if (event.key === 'Home')
      nextIndex = 0
    else if (event.key === 'End')
      nextIndex = items.length - 1
    else
      return

    event.preventDefault()
    items[nextIndex]?.focus()
  }

  function handleDocumentPointer(event: PointerEvent) {
    if (!activeMenuKey.value) return
    const target = event.target
    const menu = resolveMenu()
    if (
      !(target instanceof Node)
      || (!menuLauncher?.contains(target) && !menu?.contains(target))
    ) {
      closeMenu()
    }
  }

  onMounted(() => {
    document.addEventListener('pointerdown', handleDocumentPointer)
  })

  onBeforeUnmount(() => {
    document.removeEventListener('pointerdown', handleDocumentPointer)
    closeMenu()
  })

  return {
    activeMenuKey,
    closeMenu,
    getMenuLauncher,
    toggleMenu,
    handleMenuButtonKeydown,
    handleMenuKeydown,
  }
}
