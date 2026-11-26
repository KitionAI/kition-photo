/**
 * The keyboard stays the editor's (plan 2026-11-24, stage 8): one guard for every way focus can be
 * lost, instead of a focus call after each control.
 *
 * - A control that goes away or turns disabled while it has the focus (Layers' Add Mask, Free
 *   Transform's ✓ and ⊘, a preset in the brush picker closed with Escape) leaves the focus on the
 *   page's body, where no editor key is heard. The guard gives it back to the workspace, and a key
 *   pressed before that (a disabled control loses the focus without any event) is passed on to the
 *   workspace as if it had been pressed there.
 * - When the last dialog closes, the focus goes to the workspace rather than back to the control
 *   that opened the dialog (a menu bar title, where the Enter that confirmed the dialog would open
 *   the menu, and the next Enter run its first row).
 * - Photo's dialogs are modal as the platform's are: Enter is the default button from anywhere in
 *   the dialog (a field, a slider, a checkbox, a button reached with Tab), Escape is Cancel (the
 *   dialog's own), and a press outside the dialog neither cancels it nor reaches the canvas.
 */

/** Photo's dialogs: the ones with the shared OK button (`DIALOG_BUTTONS` in `dialogs/DialogFrame.tsx`). */
const OK = '[data-kit-ui-id="control:dialog.ok"]'
/** Controls inside a dialog that keep Enter (an open list, a rename field), as kit-editor's dialogs leave them. */
const OWN_ENTER = 'textarea, .kui-rename, [role="menu"], [role="combobox"], [role="listbox"], .kui-popover, [data-kui-own-enter], [aria-expanded="true"]'
/** Inputs whose Enter kit-editor's dialog already turns into OK (after the field has taken its value). */
const NOT_TEXT = new Set(['button', 'submit', 'reset', 'file', 'image', 'checkbox', 'radio', 'range', 'color'])
/** Where the focus is the person's own choice and must stay (typing somewhere). */
const TYPING = 'input, textarea, select, [contenteditable=""], [contenteditable="true"]'

const isPage = (element: Element | null) => element == null || element === document.body || element === document.documentElement

/** The dialogs open now, the last one on top. */
const dialogs = () => [...document.querySelectorAll<HTMLElement>('.kui-dialog')]

/** The dialog an Enter at `target` confirms: the one it is in, else the top one; only Photo's dialogs. */
function dialogFor(target: EventTarget | null): HTMLElement | null {
  const own = target instanceof Element ? target.closest<HTMLElement>('.kui-dialog') : null
  const dialog = own ?? dialogs().at(-1) ?? null
  return dialog?.querySelector(OK) ? dialog : null
}

/** Whether kit-editor's dialog answers this Enter itself: a text field among the dialog's fields. */
function dialogTakesEnter(target: EventTarget | null, dialog: HTMLElement) {
  return target instanceof HTMLInputElement && !NOT_TEXT.has(target.type) && dialog.contains(target) && target.closest('.kui-preview-fields') != null && !target.closest(OWN_ENTER)
}

/**
 * Installs the guard on this document; `home` answers the element the keyboard goes back to (the
 * workspace). Answers the function that removes it.
 */
export function installFocusGuard(home: () => HTMLElement | null): () => void {
  let frame = 0
  const goHome = () => {
    const element = home()
    if (!element?.isConnected) return false
    element.focus({ preventScroll: true })
    return document.activeElement === element
  }
  // Focus left for nowhere (its control went away): back to the workspace, unless a dialog is open
  // (its own keys are window-wide) or the window itself lost the focus.
  const lost = () => {
    frame = 0
    if (!document.hasFocus() || !isPage(document.activeElement) || dialogs().length) return
    goHome()
  }
  const onFocusOut = (event: FocusEvent) => {
    if (event.relatedTarget == null && !frame) frame = requestAnimationFrame(lost)
  }

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.isComposing || event.keyCode === 229) return
    // Enter in one of Photo's dialogs is OK, whatever has the focus in it.
    if (event.key === 'Enter' && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
      const dialog = dialogFor(event.target)
      if (dialog) {
        if (event.target instanceof Element && dialog.contains(event.target) && event.target.closest(OWN_ENTER)) return
        if (dialogTakesEnter(event.target, dialog)) return
        event.preventDefault()
        event.stopPropagation()
        const ok = dialog.querySelector<HTMLButtonElement>(OK)
        if (ok && !ok.disabled) ok.click()
        return
      }
    }
    // A key that found the focus on the page's body (a disabled control lost it without a word):
    // the workspace takes the focus and hears the key.
    if (!isPage(event.target instanceof Element ? event.target : null) || dialogs().length || !event.isTrusted) return
    if (!goHome()) return
    event.preventDefault()
    event.stopImmediatePropagation()
    home()?.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: event.key,
        code: event.code,
        location: event.location,
        repeat: event.repeat,
        altKey: event.altKey,
        ctrlKey: event.ctrlKey,
        metaKey: event.metaKey,
        shiftKey: event.shiftKey,
        bubbles: true,
        cancelable: true,
        composed: true,
      }),
    )
  }

  // A press outside a Photo dialog (on its backdrop) does nothing: the dialog stays, the canvas
  // under it never hears it. The Color Picker's eyedropper reads such presses itself, on the window.
  const onPointerDown = (event: PointerEvent) => {
    const target = event.target
    if (!(target instanceof HTMLElement) || !target.classList.contains('kui-dialog-backdrop')) return
    const dialog = target.querySelector(':scope > .kui-dialog')
    if (!dialog?.querySelector(OK)) return
    event.stopPropagation()
    event.preventDefault()
  }

  // The last dialog closing: the keyboard goes to the workspace (before the key that closed it
  // can act on the control the dialog gave the focus back to), unless the person is typing somewhere.
  let open = dialogs().length
  const observer = new MutationObserver(() => {
    const now = document.querySelectorAll('.kui-dialog').length
    const closed = open > 0 && now === 0
    open = now
    if (!closed) return
    const active = document.activeElement
    if (active && !isPage(active) && active.matches(TYPING) && !active.closest('[role="menubar"], [role="menu"]')) return
    goHome()
  })
  observer.observe(document.body, { childList: true, subtree: true })

  document.addEventListener('focusout', onFocusOut, true)
  window.addEventListener('keydown', onKeyDown, true)
  window.addEventListener('pointerdown', onPointerDown, true)
  return () => {
    if (frame) cancelAnimationFrame(frame)
    observer.disconnect()
    document.removeEventListener('focusout', onFocusOut, true)
    window.removeEventListener('keydown', onKeyDown, true)
    window.removeEventListener('pointerdown', onPointerDown, true)
  }
}
