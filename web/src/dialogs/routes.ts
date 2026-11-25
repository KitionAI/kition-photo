// Imports carry `.ts` so `node --test` loads this module as it is.
import { isHiddenCommand } from '../menus/hidden.ts'

/**
 * Which dialog a command opens (plan 2026-11-24, stage 1): a menu row whose label ends in "…"
 * asks before it runs, as Photoshop's do. Pure, so the rules are tested without a browser.
 */

/** The purpose-built dialogs, and `params`, the generic one drawn from a command's parameters. */
export type DialogKind = 'levels' | 'curves' | 'hueSaturation' | 'brightnessContrast' | 'colorBalance' | 'imageSize' | 'canvasSize' | 'fill' | 'stroke' | 'params'

/** Commands with a dialog of their own; the adjustments' rows that make an adjustment layer share the adjustment's. */
export const PURPOSE_BUILT: Readonly<Record<string, DialogKind>> = {
  'image.adjustments.levels': 'levels',
  'layer.newAdjustmentLayer.levels': 'levels',
  'image.adjustments.curves': 'curves',
  'layer.newAdjustmentLayer.curves': 'curves',
  'image.adjustments.hueSaturation': 'hueSaturation',
  'layer.newAdjustmentLayer.hueSaturation': 'hueSaturation',
  'image.adjustments.brightnessContrast': 'brightnessContrast',
  'layer.newAdjustmentLayer.brightnessContrast': 'brightnessContrast',
  'image.adjustments.colorBalance': 'colorBalance',
  'layer.newAdjustmentLayer.colorBalance': 'colorBalance',
  'image.imageSize': 'imageSize',
  'image.canvasSize': 'canvasSize',
  'edit.fill': 'fill',
  'edit.stroke': 'stroke',
}

/** A command as the routing reads it: its id, its title ("Gaussian Blur…"), and whether it has a parameter a person can set. */
export type RoutedCommand = { id: string; title: string; asks: boolean }

/**
 * The dialog `command` opens instead of running, or null when it runs at once: a purpose-built
 * dialog, else the generic one for a "…" command with something to ask. A "…" command with nothing
 * to ask (Layer from Background, New Adjustment Layer › Invert) runs, as it has no defaults to
 * apply; the menus leave out the rows with nothing behind them (`menus/hidden.ts`).
 */
export function dialogFor(command: RoutedCommand): DialogKind | null {
  if (isHiddenCommand(command.id)) return null
  const own = PURPOSE_BUILT[command.id]
  if (own) return own
  return isEllipsis(command.title) && command.asks ? 'params' : null
}

/** Whether a label ends in "…" (or three dots), the mark of a row that asks first. */
export function isEllipsis(label: string) {
  return /(…|\.\.\.)\s*$/.test(label)
}

/** A label without its trailing "…", for a dialog's title. */
export function withoutEllipsis(label: string) {
  return label.replace(/\s*(…|\.\.\.)\s*$/, '')
}

/**
 * Whether a dialog previews its command on the canvas while it is open: filters, adjustments
 * (destructive or as a new adjustment layer), layer styles, Fade, matting and the colour profile
 * rows, as upstream's schema dialogs do. Sizing, naming and selection rows do not.
 */
export function previews(id: string) {
  if (id.startsWith('filter.') || id.startsWith('image.adjustments.') || id.startsWith('layer.newAdjustmentLayer.') || id.startsWith('layer.newFillLayer.')) return true
  if (id.startsWith('layer.layerStyle.') && id !== 'layer.layerStyle.globalLight') return true
  return ['edit.fade', 'layer.matting.defringe', 'layer.matting.colorDecontaminate', 'edit.assignProfile', 'edit.convertToProfile', 'type.warpText'].includes(id)
}
