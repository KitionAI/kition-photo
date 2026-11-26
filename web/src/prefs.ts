import { useKitStore } from '@kition/sdk/editor'

/**
 * Edit › Preferences › General (plan 2026-11-24, coordinator's decision for stage 2): only settings
 * that take effect in this editor. Kept in the kit store, so they last across sessions and every
 * Photo tab follows them.
 */
export type PhotoPrefs = {
  /** The painting tools' cursor: the brush's outline, or a precise crosshair (Caps Lock gives the other). */
  brushCursor: 'outline' | 'precise'
  /** The plain wheel zooms about the pointer instead of scrolling. */
  zoomWithWheel: boolean
  /** The Move tool's Show Transform Controls, set when the preference is saved. */
  showTransform: boolean
  /**
   * Use Shift Key for Tool Switch (plan 2026-11-24, stage 8): the engine's preference, which the
   * tool keys read; kept here too so it lasts, and given to the engine of each document opened.
   */
  shiftToolSwitch: boolean
}

/** The engine's preference behind Use Shift Key for Tool Switch (upstream's Preferences › Tools). */
export const SHIFT_SWITCH = 'tools.useShiftKeyForToolSwitch'

export const DEFAULT_PREFS: PhotoPrefs = { brushCursor: 'outline', zoomWithWheel: false, showTransform: false, shiftToolSwitch: false }

/** A stored value as preferences: each known field of the right kind, the default for the rest. */
export function validPrefs(raw: unknown): PhotoPrefs | undefined {
  if (!raw || typeof raw !== 'object') return undefined
  const value = raw as Record<string, unknown>
  return {
    brushCursor: value.brushCursor === 'precise' ? 'precise' : 'outline',
    zoomWithWheel: value.zoomWithWheel === true,
    showTransform: value.showTransform === true,
    shiftToolSwitch: value.shiftToolSwitch === true,
  }
}

/** The preferences and their setter (one owner: the editor passes them to the dialog). */
export function usePhotoPrefs() {
  return useKitStore<PhotoPrefs>('kit.photo', 'preferences', { initial: DEFAULT_PREFS, validate: validPrefs })
}
