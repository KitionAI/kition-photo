import type { ToolOptions } from '../tools/options'

/**
 * A type layer's character and paragraph style as the Type options bar, the Properties panel, and
 * the Character and Paragraph panels show it (upstream's `type_tool::styles_at` and
 * `type_sections`), read from `type.info`, or from the Type tool's options for new type; and the
 * way an edit reaches either. Pure, so the flow of values is tested without a browser.
 */

/** A run's style as `type.info` reports it (upstream's `CharStyle`, snake case). */
export type RunStyle = {
  font_family: string
  font_style: string
  size_pt: number
  color: { c: number[] }
  tracking?: number
  leading_pt?: number | null
  baseline_shift_pt?: number
  horizontal_scale?: number
  vertical_scale?: number
  underline?: boolean
  strikethrough?: boolean
  faux_bold?: boolean
  faux_italic?: boolean
  kerning?: string
  kern?: number
  caps?: string
}

/** A paragraph's style as `type.info` reports it (upstream's `ParagraphStyle`). */
export type ParagraphStyle = {
  align?: string
  first_line_indent_pt?: number
  start_indent_pt?: number
  end_indent_pt?: number
  space_before_pt?: number
  space_after_pt?: number
  auto_leading?: number
  hyphenate?: boolean
}

/** What `type.info` answers that the editor reads. */
export type TypeInfo = { antialias: string; orientation: string; transform: number[] | { m: number[] }; runs: { start?: number; end?: number; style: RunStyle }[]; paragraphs: { start?: number; end?: number; style: ParagraphStyle }[] }

/** The entry of `list` (runs or paragraphs, with character `start` and `end`) holding character `at`: the one it is inside, else the one ending there, else the first. */
function entryAt<T extends { start?: number; end?: number }>(list: readonly T[], at: number | undefined): T {
  if (at == null) return list[0]
  return list.find((item) => item.start != null && item.end != null && at >= item.start && at < item.end) ?? list.find((item) => item.end === at) ?? list[0]
}

/** Upstream's paragraph alignments, in its order (`type_sections`), with their tooltips. */
export const ALIGNMENTS = [
  ['left', 'Left align text'],
  ['center', 'Center text'],
  ['right', 'Right align text'],
  ['justifyLeft', 'Justify last left'],
  ['justifyCenter', 'Justify last centered'],
  ['justifyRight', 'Justify last right'],
  ['justifyAll', 'Justify all'],
] as const

export type Alignment = (typeof ALIGNMENTS)[number][0]

/** Every value the Character and Paragraph panels show, in their units (sizes as the layer shows them). */
export type TypeValues = {
  font: string
  fontStyle: string
  size: number
  /** The leading shown: the set one, or the font size times the auto-leading when Auto. */
  leading: number
  autoLeading: boolean
  /** Metrics, Optical, or a whole number in 1/1000 em (upstream's `kerning_label`). */
  kerning: string
  tracking: number
  verticalScale: number
  horizontalScale: number
  baselineShift: number
  color: string
  fauxBold: boolean
  fauxItalic: boolean
  caps: 'normal' | 'small' | 'all'
  underline: boolean
  strikethrough: boolean
  align: Alignment
  startIndent: number
  endIndent: number
  firstLineIndent: number
  spaceBefore: number
  spaceAfter: number
  hyphenate: boolean
}

/**
 * The engine's anti-aliasing values (`AntiAlias`, as `type.info` reports them) and the choices
 * they show as. Windows and Windows LCD come from imported files; the bar offers them only on a
 * layer that has one.
 */
const ANTI_ALIAS: Record<string, string> = { None: 'none', Sharp: 'sharp', Crisp: 'crisp', Strong: 'strong', Smooth: 'smooth', Windows: 'windows', WindowsLcd: 'windowsLcd' }

/** Photoshop's auto-leading, a multiple of the font size (upstream's `ParagraphStyle::default`). */
const AUTO_LEADING = 1.2

const tenth = (value: number) => Math.round(value * 10) / 10

/** An engine alignment (`JustifyLeft`) as the panels name it (`justifyLeft`). */
export function alignOf(engine: string | undefined): Alignment {
  const name = engine ? engine[0].toLowerCase() + engine.slice(1) : 'left'
  return (ALIGNMENTS.find(([value]) => value === name)?.[0] ?? 'left') as Alignment
}

/** An engine capitalization (`AllCaps`) as `type.setStyle` takes it (`all`). */
export const capsOf = (engine: string | undefined): TypeValues['caps'] => (engine === 'AllCaps' ? 'all' : engine === 'SmallCaps' ? 'small' : 'normal')

/** The Kerning field's text (upstream's `kerning_label`): a manual value wins, else the mode, Off showing as 0. */
export function kerningLabel(mode: string | undefined, kern: number | undefined): string {
  if (kern != null && kern !== 0 && Number.isFinite(kern)) return String(Math.round(kern))
  if (mode === 'Optical') return 'Optical'
  if (mode === 'Off') return '0'
  return 'Metrics'
}

/** A typed or picked kerning (upstream's `parse_kerning`): Metrics or Optical in any case, or a number in -1000..10000 (rounded); null for anything else. */
export function parseKerning(text: string): 'Metrics' | 'Optical' | number | null {
  const trimmed = text.trim()
  const word = trimmed.toLowerCase()
  if (word === 'metrics') return 'Metrics'
  if (word === 'optical') return 'Optical'
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)$/.test(trimmed)) return null
  const value = Number(trimmed)
  return value >= -1000 && value <= 10000 ? Math.round(value) : null
}

/** The kerning presets upstream's combo lists. */
export const KERNING_PRESETS = ['Metrics', 'Optical', '0', '-100', '-75', '-50', '-25', '-10', '-5', '5', '10', '25', '50', '75', '100', '200'] as const

/** How much a layer's transform scales its type: sizes show multiplied by it and are set divided by it (upstream's `shown_scale`). */
export function transformScale(transform: TypeInfo['transform'] | undefined): number {
  const m = Array.isArray(transform) ? transform : transform?.m
  const scale = m ? Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2])) : 1
  return Number.isFinite(scale) && scale > 1e-3 ? scale : 1
}

/** A type layer: its id, orientation, transform scale, anti-aliasing, and the first run's and paragraph's style as the panels show them. */
export type TypeLayer = TypeValues & { layer: number; vertical: boolean; scale: number; antialias: string }

/**
 * Type layer `layer`'s style from its `type.info`, or null without runs; `hex` writes its colour
 * (0 to 255 per channel) as `#rrggbb` (kit-editor's `rgbToHex`). `at`: the character whose run and
 * paragraph show (a selection's start while text is edited on the canvas); the first otherwise.
 */
export function typeLayerOf(layer: number, info: TypeInfo | null, hex: (rgb: readonly [number, number, number]) => string, at?: number): TypeLayer | null {
  if (!info?.runs?.length) return null
  const scale = transformScale(info.transform)
  const run = entryAt(info.runs, at).style
  const paragraph = (info.paragraphs?.length ? entryAt(info.paragraphs, at) : undefined)?.style ?? {}
  const autoLeading = run.leading_pt == null
  return {
    layer,
    vertical: info.orientation === 'Vertical',
    scale,
    antialias: ANTI_ALIAS[info.antialias] ?? 'smooth',
    font: run.font_family,
    fontStyle: run.font_style || 'Regular',
    size: run.size_pt,
    leading: tenth((run.leading_pt ?? run.size_pt * (paragraph.auto_leading ?? AUTO_LEADING)) * scale),
    autoLeading,
    kerning: kerningLabel(run.kerning, run.kern),
    tracking: run.tracking ?? 0,
    verticalScale: tenth((run.vertical_scale ?? 1) * 100),
    horizontalScale: tenth((run.horizontal_scale ?? 1) * 100),
    baselineShift: run.baseline_shift_pt ?? 0,
    color: hex([0, 1, 2].map((channel) => Math.min(1, Math.max(0, run.color.c[channel] ?? 0)) * 255) as [number, number, number]),
    fauxBold: run.faux_bold ?? false,
    fauxItalic: run.faux_italic ?? false,
    caps: capsOf(run.caps),
    underline: run.underline ?? false,
    strikethrough: run.strikethrough ?? false,
    align: alignOf(paragraph.align),
    startIndent: paragraph.start_indent_pt ?? 0,
    endIndent: paragraph.end_indent_pt ?? 0,
    firstLineIndent: paragraph.first_line_indent_pt ?? 0,
    spaceBefore: paragraph.space_before_pt ?? 0,
    spaceAfter: paragraph.space_after_pt ?? 0,
    hyphenate: paragraph.hyphenate ?? false,
  }
}

/** The size a type layer shows: its points as its transform scales them, to a tenth. */
export const shownSize = (target: TypeLayer) => tenth(target.size * target.scale)

/**
 * The Type tool's own options (upstream's `type_font`, `type_style`, `type_size`, `type_align`)
 * and the colour: what new type takes explicitly, so what the panels let a person set while no
 * type layer is active. Every other field there is disabled: new type takes it from the engine's
 * default type styles (Type › Save Default Type Styles), which the editor cannot read yet.
 */
export const TOOL_FIELDS = ['font', 'fontStyle', 'size', 'align', 'color'] as const
export type ToolField = (typeof TOOL_FIELDS)[number]

/** The alignments the Type tool keeps (its bar's three); a justified one is a layer's own. */
export const TOOL_ALIGNMENTS: readonly Alignment[] = ['left', 'center', 'right']

/**
 * What the panels show for new type: the Type tool's options and the foreground colour, the rest
 * at the engine's stock defaults (`CharStyle::default`, `ParagraphStyle::default`), disabled.
 */
export function typeValuesOf(o: ToolOptions, foreground: string): TypeValues {
  return {
    font: o.font,
    fontStyle: o.fontStyle,
    size: o.fontSize,
    leading: tenth(o.fontSize * AUTO_LEADING),
    autoLeading: true,
    kerning: 'Metrics',
    tracking: 0,
    verticalScale: 100,
    horizontalScale: 100,
    baselineShift: 0,
    color: foreground,
    fauxBold: false,
    fauxItalic: false,
    caps: 'normal',
    underline: false,
    strikethrough: false,
    align: alignOf(o.textAlign),
    startIndent: 0,
    endIndent: 0,
    firstLineIndent: 0,
    spaceBefore: 0,
    spaceAfter: 0,
    hyphenate: false,
  }
}

/** The keys a panel edit names: each panel field but the colour (the foreground's, without a layer). */
export type TypeEdit = Partial<Omit<TypeValues, 'color' | 'autoLeading'>>

/** The panel's keys `type.setStyle` takes as they are. */
const SAME = ['font', 'fontStyle', 'tracking', 'verticalScale', 'horizontalScale', 'baselineShift', 'fauxBold', 'fauxItalic', 'caps', 'underline', 'strikethrough', 'startIndent', 'endIndent', 'firstLineIndent', 'spaceBefore', 'spaceAfter', 'hyphenate'] as const

/**
 * An edit as `type.setStyle` takes it for a layer whose transform scales its type by `scale`
 * (sizes and leading set divided by it, as upstream's `apply`), and the Type tool options it
 * sets: the font, its style, the size, and the alignment (one of the tool's three), which the
 * tool keeps whether a layer is edited (upstream's `apply` keeps `type_font` and `type_size`, its
 * bar the rest) or new type is (no layer active).
 */
export function typeEdit(edit: TypeEdit, scale = 1): { style: Record<string, unknown>; options: Partial<ToolOptions> } {
  const style: Record<string, unknown> = {}
  const options: Partial<ToolOptions> = {}
  for (const key of SAME) if (edit[key] !== undefined) style[key] = edit[key]
  if (edit.font !== undefined) options.font = edit.font
  if (edit.fontStyle !== undefined) options.fontStyle = edit.fontStyle
  if (edit.size !== undefined) {
    style.size = edit.size / scale
    options.fontSize = Math.min(1296, Math.max(1, edit.size))
  }
  if (edit.leading !== undefined) style.leading = edit.leading / scale
  if (edit.kerning !== undefined) {
    const kerning = parseKerning(edit.kerning)
    if (kerning !== null) style.kerning = typeof kerning === 'number' ? kerning : kerning.toLowerCase()
  }
  if (edit.align !== undefined) {
    style.align = edit.align
    if (TOOL_ALIGNMENTS.includes(edit.align)) options.textAlign = edit.align
  }
  return { style, options }
}
