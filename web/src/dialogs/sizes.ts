/**
 * Image Size, Canvas Size and New Document's numbers (Photoshop's dialogs): a length in pixels,
 * percent of the current size, inches, centimetres, millimetres or points at the document's
 * resolution, and the proportions Image Size keeps. Pure.
 */

export type LengthUnit = 'pixels' | 'percent' | 'inches' | 'centimeters' | 'millimeters' | 'points'

/** The units each dialog offers, in Photoshop's order. */
export const IMAGE_UNITS: readonly LengthUnit[] = ['pixels', 'percent', 'inches', 'centimeters', 'millimeters', 'points']
export const DOCUMENT_UNITS: readonly LengthUnit[] = ['pixels', 'inches', 'centimeters', 'millimeters', 'points']

/** The engine's largest canvas side. */
export const MAX_SIDE = 300000

const PER_INCH: Readonly<Record<Exclude<LengthUnit, 'pixels' | 'percent'>, number>> = { inches: 1, centimeters: 2.54, millimeters: 25.4, points: 72 }

/** `value` in `unit` as pixels, given the side it is a percent of and the resolution (pixels per inch). */
export function toPixels(value: number, unit: LengthUnit, base: number, resolution: number) {
  if (!Number.isFinite(value)) return NaN
  if (unit === 'pixels') return value
  if (unit === 'percent') return (value / 100) * base
  return (value / PER_INCH[unit]) * resolution
}

/** `pixels` in `unit` (see `toPixels`). */
export function fromPixels(pixels: number, unit: LengthUnit, base: number, resolution: number) {
  if (unit === 'pixels') return pixels
  if (unit === 'percent') return base > 0 ? (pixels / base) * 100 : 100
  return (pixels / resolution) * PER_INCH[unit]
}

/** Decimals a value in `unit` shows: whole pixels, two for the rest. */
export function unitDecimals(unit: LengthUnit) {
  return unit === 'pixels' ? 0 : 2
}

/** Rounded to the unit's decimals. */
export function roundIn(value: number, unit: LengthUnit) {
  const factor = 10 ** unitDecimals(unit)
  return Math.round(value * factor) / factor
}

/** A side in whole pixels the engine takes (1..300000), or null for a value that is none. */
export function side(pixels: number): number | null {
  if (!Number.isFinite(pixels)) return null
  const rounded = Math.round(pixels)
  return rounded >= 1 && rounded <= MAX_SIDE ? rounded : null
}

/** The other side when proportions are kept: `changed` became `pixels`, the original size was `width` by `height`. */
export function keptProportion(changed: 'width' | 'height', pixels: number, width: number, height: number) {
  if (width <= 0 || height <= 0) return pixels
  return changed === 'width' ? (pixels * height) / width : (pixels * width) / height
}

/** An image's size in memory as Photoshop's dialogs show it: 8 bits per channel by default, in K or M. */
export function memorySize(width: number, height: number, channels: number, bitsPerChannel = 8) {
  const bytes = width * height * channels * Math.max(1, bitsPerChannel / 8)
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(1)}G`
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)}M`
  return `${Math.max(1, Math.round(bytes / 1024))}K`
}

/** Canvas Size's anchor grid, row by row (the engine's names). */
export const ANCHORS = ['topLeft', 'top', 'topRight', 'left', 'center', 'right', 'bottomLeft', 'bottom', 'bottomRight'] as const
export type Anchor = (typeof ANCHORS)[number]

/**
 * The arrow an anchor grid cell shows (Photoshop's): the anchor cell is empty, its neighbours
 * point away from it, the rest show nothing.
 */
export function anchorArrow(anchor: Anchor, cell: Anchor): string {
  const [ar, ac] = [Math.floor(ANCHORS.indexOf(anchor) / 3), ANCHORS.indexOf(anchor) % 3]
  const [r, c] = [Math.floor(ANCHORS.indexOf(cell) / 3), ANCHORS.indexOf(cell) % 3]
  const [dr, dc] = [r - ar, c - ac]
  if (dr === 0 && dc === 0) return '●'
  if (Math.abs(dr) > 1 || Math.abs(dc) > 1) return ''
  const arrows: Record<string, string> = { '-1,-1': '↖', '-1,0': '↑', '-1,1': '↗', '0,-1': '←', '0,1': '→', '1,-1': '↙', '1,0': '↓', '1,1': '↘' }
  return arrows[`${dr},${dc}`] ?? ''
}
