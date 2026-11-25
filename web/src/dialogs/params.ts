// Imports carry `.ts` so `node --test` loads this module as it is.
import { BLEND_MODES as LAYER_BLEND_MODES } from '../tools/options.ts'

/**
 * A command's parameters as the generic dialog's fields (plan 2026-11-24, stage 1): a slider for a
 * number with a range, a number field for one without, a drop-down for a choice, a check box, a
 * colour, or a text field. The keys every command shares (which layer, region, target, colours for
 * this call, the undo coalescing) and the ones a person never sets (a random seed, raw JSON) are
 * left out, so the engine's defaults apply to them. Pure, so it is tested without a browser.
 */

type SchemaNode = {
  type?: string
  description?: string
  enum?: unknown[]
  default?: unknown
  minimum?: number
  maximum?: number
  oneOf?: SchemaNode[]
}

/** A command as the engine lists it (`KitCommand`'s fields this module reads). */
export type ParamsCommand = { id: string; params?: string | null; schema?: unknown }

/** Keys every command (or every pixel command) shares: not the dialog's to ask. */
export const SHARED_KEYS: ReadonlySet<string> = new Set(['at', 'coalesce', 'layer', 'region', 'target', 'backgroundColor', 'foregroundColor', 'seed', 'add', 'path', 'document', 'file', 'dir'])

export type ParamField =
  | { key: string; kind: 'slider'; min: number; max: number; softMax: number; decimals: number; initial: number }
  | { key: string; kind: 'number'; min: number; max: number; decimals: number; initial: number | null }
  | { key: string; kind: 'choice'; options: readonly string[]; initial: string }
  | { key: string; kind: 'check'; initial: boolean }
  | { key: string; kind: 'color'; initial: string }
  | { key: string; kind: 'text'; initial: string }

const NUMBER = String.raw`-?\d+(?:\.\d+)?`

/** The range and default the compact `params` text gives a key (`"radius":0.1..1000=1`), if any. */
export function rangeFromParams(params: string | null | undefined, key: string): { min: number; max: number; initial?: number } | null {
  if (!params) return null
  const match = new RegExp(String.raw`"${key}":(${NUMBER})\.\.(${NUMBER})(?:=(${NUMBER}))?`).exec(params)
  if (!match) return null
  const [min, max] = [Number(match[1]), Number(match[2])]
  if (!(min < max)) return null
  return match[3] === undefined ? { min, max } : { min, max, initial: Number(match[3]) }
}

/** The range a description gives ("0.1 to 1000", "-180 to 180"), if any. */
export function rangeFromDescription(description: string | undefined): { min: number; max: number } | null {
  const match = description ? new RegExp(String.raw`^\s*(${NUMBER})\s+to\s+(${NUMBER})`).exec(description) : null
  if (!match) return null
  const [min, max] = [Number(match[1]), Number(match[2])]
  return min < max ? { min, max } : null
}

/** Decimals a value keeps: integers none; a narrow range two, a wide one one. */
function decimalsFor(integer: boolean, min: number, max: number) {
  if (integer) return 0
  return max - min <= 20 ? 2 : 1
}

/**
 * The slider's own range: a very wide one (Gaussian Blur's radius to 1000) slides to 250, as
 * Photoshop's do, and the typed value still reaches the maximum.
 */
export function softMaxFor(min: number, max: number) {
  return max - min > 500 && min >= 0 ? Math.min(max, 250) : max
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

/**
 * Starting values for numbers whose engine default asks nothing useful (plan 2026-11-24, stage 7):
 * New Guide has no default position, and New Guide Layout's zero columns make no guides; these
 * start as Photoshop's dialogs do.
 */
// Written as [key, value] pairs: the engine's preferences scan (`prefs_usage.rs`) reads a key written
// as an object member in any web source as a preference being read.
const STARTING: Readonly<Record<string, Readonly<Record<string, number>>>> = Object.fromEntries(
  ([
    ['view.newGuide', [['position', 0]]],
    ['view.newGuideLayout', [['columns', 8], ['gutter', 20]]],
  ] as const).map(([command, values]) => [command, Object.fromEntries(values)]),
)

/** One key's field, or null when the dialog does not ask for it. */
export function fieldFor(command: ParamsCommand, key: string, node: SchemaNode): ParamField | null {
  if (SHARED_KEYS.has(key)) return null
  if (node.enum?.length && node.enum.every((value) => typeof value === 'string')) {
    const options = node.enum as string[]
    const initial = typeof node.default === 'string' && options.includes(node.default) ? node.default : options[0]
    return { key, kind: 'choice', options, initial }
  }
  if (node.type === 'boolean') return { key, kind: 'check', initial: node.default === true }
  if (node.type === 'number' || node.type === 'integer') {
    const integer = node.type === 'integer'
    const range =
      rangeFromParams(command.params, key) ??
      rangeFromDescription(node.description) ??
      (node.minimum !== undefined && node.maximum !== undefined && node.minimum < node.maximum ? { min: node.minimum, max: node.maximum } : null)
    const fallback = STARTING[command.id]?.[key] ?? (typeof node.default === 'number' ? node.default : undefined)
    if (range) {
      const initial = clamp(fallback ?? ('initial' in range && typeof range.initial === 'number' ? range.initial : Math.max(range.min, Math.min(0, range.max))), range.min, range.max)
      return { key, kind: 'slider', min: range.min, max: range.max, softMax: Math.max(initial, softMaxFor(range.min, range.max)), decimals: decimalsFor(integer, range.min, range.max), initial }
    }
    return { key, kind: 'number', min: node.minimum ?? -1_000_000, max: node.maximum ?? 1_000_000, decimals: integer ? 0 : 2, initial: fallback ?? null }
  }
  if (node.type === 'string') {
    if (/colou?r$/i.test(key)) return { key, kind: 'color', initial: typeof node.default === 'string' && /^#[0-9a-f]{6}$/i.test(node.default) ? node.default : '#000000' }
    // A free blend-mode name ("normal", "multiply", …) reads as a choice of the modes.
    if (key === 'mode' || key === 'blend' || key === 'blending') return { key, kind: 'choice', options: BLEND_MODES, initial: typeof node.default === 'string' && BLEND_MODES.includes(node.default) ? node.default : 'normal' }
    return { key, kind: 'text', initial: typeof node.default === 'string' ? node.default : '' }
  }
  // Arrays, objects, JSON and either-or values keep the engine's defaults.
  return null
}

/** The fields the generic dialog shows for `command`, in the schema's order. */
export function paramFields(command: ParamsCommand): ParamField[] {
  const schema = (command.schema ?? {}) as Record<string, SchemaNode>
  return Object.entries(schema).flatMap(([key, node]) => {
    const field = fieldFor(command, key, node)
    return field ? [field] : []
  })
}

export type ParamValues = Record<string, string | number | boolean | null>

/** The fields' starting values: the remembered ones where they still fit, else each field's own. */
export function initialValues(fields: readonly ParamField[], remembered: Readonly<Record<string, unknown>> = {}): ParamValues {
  return Object.fromEntries(
    fields.map((field) => {
      const kept = remembered[field.key]
      return [field.key, fits(field, kept) ? (kept as string | number | boolean | null) : field.initial]
    }),
  )
}

/** Whether `value` is one `field` can show. */
export function fits(field: ParamField, value: unknown): boolean {
  switch (field.kind) {
    case 'slider':
    case 'number':
      return (value === null && field.kind === 'number') || (typeof value === 'number' && Number.isFinite(value) && value >= field.min && value <= field.max)
    case 'choice':
      return typeof value === 'string' && field.options.includes(value)
    case 'check':
      return typeof value === 'boolean'
    case 'color':
      return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value)
    case 'text':
      return typeof value === 'string'
  }
}

/** The command's parameters from the fields' values: an empty text or number leaves its key out (the engine's default). */
export function paramsOf(fields: readonly ParamField[], values: ParamValues): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (const field of fields) {
    const value = values[field.key]
    if (value === null || value === undefined) continue
    if (field.kind === 'text' && value === '') continue
    out[field.key] = value
  }
  return out
}

/** The layer blend modes, as the engine names them (upstream's list, `tools/options.ts`). */
export const BLEND_MODES: string[] = LAYER_BLEND_MODES.map(([mode]) => mode)

/** A blend mode's English label ("Linear Dodge (Add)"), or null for another word. */
export function blendLabel(mode: string): string | null {
  return LAYER_BLEND_MODES.find(([value]) => value === mode)?.[1] ?? null
}

/**
 * Words for a key or a choice in English Title Case ("cellSize" → "Cell Size", "gray-gamma-2.2" →
 * "Gray Gamma 2.2"), the form upstream's translation catalogs list them in.
 */
export function humanize(word: string) {
  const spaced = word
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/[-_]+/g, ' ')
    .trim()
  return spaced.replace(/(^|\s)([a-z])/g, (_, space: string, letter: string) => `${space}${letter.toUpperCase()}`)
}
