/**
 * The brush picker's and Brush Settings' arithmetic (plan 2026-11-24, stage 5): the engine's brush
 * (`brush.get`, what `tool.setBrush` answers) as the fields show it, a field's change as the patch
 * `tool.setBrush` takes, a picked preset's tip as the options bar's values, and the stroke a
 * preset's thumbnail draws. Pure, so `node --test` checks it.
 *
 * The options bar keeps the brush's size, hardness, mode, opacity, flow, smoothing and pressure
 * toggles (every stroke sends them); the engine keeps the rest of the brush (its tip, spacing,
 * angle, roundness and dynamics), which Brush Settings edits with `tool.setBrush`.
 */

type Dynamic = { jitter?: number; control?: string; minimum?: number }
/** The engine's brush as `brush.get` answers it (the fields this module reads). */
export type EngineBrush = {
  size?: number
  hardness?: number
  angle?: number
  roundness?: number
  spacing?: number
  spacingEnabled?: boolean
  pressureSize?: boolean
  pressureOpacity?: boolean
  tip?: unknown
  shapeDynamics?: { enabled?: boolean; size?: Dynamic }
  transfer?: { enabled?: boolean; opacity?: Dynamic }
}

/** Brush Settings' fields, in the units they show: degrees and percent. */
export type BrushSettings = {
  angle: number
  roundness: number
  spacing: number
  spacingEnabled: boolean
  shapeDynamics: boolean
  sizeJitter: number
  transfer: boolean
  opacityJitter: number
}

const percent = (fraction: unknown, fallback: number) => (typeof fraction === 'number' && Number.isFinite(fraction) ? Math.round(fraction * 1000) / 10 : fallback)
const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, Number.isFinite(value) ? value : low))

/** The engine's brush as Brush Settings shows it (Photoshop's defaults where it says nothing). */
export function settingsOf(brush: EngineBrush | null | undefined): BrushSettings {
  return {
    angle: typeof brush?.angle === 'number' ? Math.round(brush.angle) : 0,
    roundness: percent(brush?.roundness, 100),
    spacing: percent(brush?.spacing, 25),
    spacingEnabled: brush?.spacingEnabled !== false,
    shapeDynamics: brush?.shapeDynamics?.enabled === true,
    sizeJitter: percent(brush?.shapeDynamics?.size?.jitter, 0),
    transfer: brush?.transfer?.enabled === true,
    opacityJitter: percent(brush?.transfer?.opacity?.jitter, 0),
  }
}

/** The `tool.setBrush` patch for one field's new value; a jitter switches its section on, as Photoshop's does. */
export function settingsPatch(field: keyof BrushSettings, value: number | boolean): Record<string, unknown> {
  const number = typeof value === 'number' ? value : 0
  switch (field) {
    case 'angle':
      return { angle: clamp(number, -180, 180) }
    case 'roundness':
      return { roundness: clamp(number, 0, 100) / 100 }
    case 'spacing':
      return { spacing: clamp(number, 1, 1000) / 100, spacingEnabled: true }
    case 'spacingEnabled':
      return { spacingEnabled: value === true }
    case 'shapeDynamics':
      return { shapeDynamics: { enabled: value === true } }
    case 'sizeJitter':
      return { shapeDynamics: { enabled: true, size: { jitter: clamp(number, 0, 100) / 100 } } }
    case 'transfer':
      return { transfer: { enabled: value === true } }
    case 'opacityJitter':
      return { transfer: { enabled: true, opacity: { jitter: clamp(number, 0, 100) / 100 } } }
  }
}

/** What a picked preset gives the options bar: its size, hardness (percent) and pressure toggles. */
export function presetOptions(brush: EngineBrush | null | undefined): { size?: number; hardness?: number; pressureSize?: boolean; pressureOpacity?: boolean } {
  if (!brush) return {}
  const out: { size?: number; hardness?: number; pressureSize?: boolean; pressureOpacity?: boolean } = {}
  if (typeof brush.size === 'number' && Number.isFinite(brush.size)) out.size = clamp(Math.round(brush.size), 1, 5000)
  if (typeof brush.hardness === 'number' && Number.isFinite(brush.hardness)) out.hardness = clamp(Math.round(brush.hardness * 100), 0, 100)
  if (typeof brush.pressureSize === 'boolean') out.pressureSize = brush.pressureSize
  if (typeof brush.pressureOpacity === 'boolean') out.pressureOpacity = brush.pressureOpacity
  return out
}

/** A preset as `brush.presets.list` lists it. */
export type BrushPreset = { name: string; size?: number; hardness?: number; tip?: string; builtin?: boolean }

/** The presets from `brush.presets.list` (anything malformed left out). */
export function presetsOf(answer: unknown): BrushPreset[] {
  const list = (answer as { presets?: unknown } | null)?.presets
  return Array.isArray(list) ? list.filter((item): item is BrushPreset => !!item && typeof (item as BrushPreset).name === 'string') : []
}

/** A thumbnail's height and width (document pixels of the scratch picture each preset paints a row of). */
export const THUMB_WIDTH = 120
export const THUMB_HEIGHT = 30

/**
 * The stroke a preset's thumbnail paints in row `row`: an S across the row, light at both ends and
 * full in the middle, as Photoshop's preview stroke, `[x, y, pressure]` per point.
 */
export function thumbnailStroke(row: number, steps = 48): [number, number, number][] {
  const top = row * THUMB_HEIGHT
  const points: [number, number, number][] = []
  for (let step = 0; step <= steps; step += 1) {
    const t = step / steps
    const x = 10 + t * (THUMB_WIDTH - 20)
    const y = top + THUMB_HEIGHT / 2 + Math.sin(t * Math.PI * 2) * (THUMB_HEIGHT * 0.22)
    points.push([Math.round(x * 10) / 10, Math.round(y * 10) / 10, Math.round(Math.sin(t * Math.PI) * 80 + 20) / 100])
  }
  return points
}

/** The size a preset's thumbnail paints with: its own, kept inside the row. */
export function thumbnailSize(preset: BrushPreset) {
  const size = typeof preset.size === 'number' && Number.isFinite(preset.size) ? preset.size : 20
  return clamp(Math.round(size), 2, Math.round(THUMB_HEIGHT * 0.4))
}
