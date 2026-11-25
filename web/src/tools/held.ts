import { createLive, type Live } from '../panels/live'

/**
 * The keys held down right now, for the canvas's temporary tools and cursors (plan 2026-11-24,
 * stage 2): Space (the Hand, with Cmd the Zoom), Cmd or Ctrl (the Move tool), Alt (a brush's
 * eyedropper, the Zoom tool's out), Shift, Caps Lock (the precise brush cursor), and Ctrl alone
 * (held during a drag, it suspends snapping: plan 2026-11-24, stage 7). The editor
 * fills it from the window's key events (`watchHeldKeys`); a pointer event refreshes the
 * modifiers too, so a key pressed while another window had focus is not missed for long.
 */
export type HeldKeys = { space: boolean; meta: boolean; alt: boolean; shift: boolean; capsLock: boolean; ctrl?: boolean }

export const NO_KEYS: HeldKeys = { space: false, meta: false, alt: false, shift: false, capsLock: false }

const same = (a: HeldKeys, b: HeldKeys) => a.space === b.space && a.meta === b.meta && a.alt === b.alt && a.shift === b.shift && a.capsLock === b.capsLock && Boolean(a.ctrl) === Boolean(b.ctrl)

/** The held keys, as a live value (`useLive`). */
export function createHeldKeys(): Live<HeldKeys> {
  const live = createLive<HeldKeys>(NO_KEYS)
  const set = live.set
  return { ...live, set: (next) => (same(next, live.get()) ? undefined : set(next)) }
}

type Modifiers = { metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; getModifierState?: (key: string) => boolean }

/** The modifiers an event reports, with Space as `held` had it. */
export function modifiersOf(event: Modifiers, space: boolean): HeldKeys {
  return { space, meta: event.metaKey || event.ctrlKey, alt: event.altKey, shift: event.shiftKey, capsLock: event.getModifierState?.('CapsLock') ?? false, ctrl: event.ctrlKey }
}

/** An element that takes Space as a key of its own (a field, a button, a menu row). */
export function takesSpace(target: EventTarget | null) {
  if (!(target instanceof Element)) return false
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true
  if ((target as HTMLElement).isContentEditable) return true
  return Boolean(target.closest('button, [role="button"], [role="menuitem"], [role="tab"], [role="checkbox"], [role="slider"], [role="option"], a[href]'))
}
