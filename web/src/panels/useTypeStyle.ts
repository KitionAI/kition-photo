import { useLayoutEffect, useRef } from 'react'
import { useKitEditor } from '@kition/sdk/editor'
import type { ToolOptions } from '../tools/options'
import { styleForFamily, useTypeLayer } from '../options/typeLayer'
import { TOOL_FIELDS, shownSize, typeEdit, typeValuesOf, type ToolField, type TypeEdit, type TypeLayer, type TypeValues } from '../options/typeStyle'
import { useGestures } from './gesture'
import { useTypeEditingTarget } from '../type/context'

/** What a type style is for: the active type layer, else new type (the Type tool's options and the foreground). */
export type TypeTarget = {
  /** The active type layer, or undefined (no type layer active, or new text being typed: then the edits are for it). */
  layer: number | undefined
  options: ToolOptions
  setOptions: (patch: Partial<ToolOptions>) => void
  foreground: string
  setForeground: (hex: string) => void
}

/** The anti-aliasing values the Type tool keeps (its bar's five); Windows and Windows LCD are an imported layer's own. */
const TOOL_ANTI_ALIAS = ['none', 'sharp', 'crisp', 'strong', 'smooth']

// The options bar, Character and Properties can edit the same target. A family lookup from
// any of them is superseded by a later choice for that target, even in another control.
type FontChoice = Pick<ToolOptions, 'font' | 'fontStyle'>
const fontRequests = new WeakMap<object, Map<number | 'tool', Promise<FontChoice | null>>>()

/** A draft captures the chosen font at submission, even while its style lookup is pending. */
export function pendingTypeFont(doc: object | null) {
  return doc ? fontRequests.get(doc)?.get('tool') : undefined
}

/** Save includes family choices whose metadata has not yet produced a style command. */
export function pendingFontChoices(doc: object | null) {
  return doc ? [...(fontRequests.get(doc)?.values() ?? [])] : []
}

/**
 * The one way the editor reads and edits type style (upstream's `type_tool::target`, `apply`,
 * and `type_options`), for the Type options bar, the Properties panel's type sections, and the
 * Character and Paragraph panels. With a type layer active, its first run and paragraph show, and
 * an edit restyles its whole text (`type.setStyle`; every change of a field's drag shares one
 * history step); the font, its style, the size, and the alignment become the Type tool's too.
 * Without one, the tool's own options (font, style, size, alignment) and the foreground colour
 * show and change, for new type; every other field is disabled (`editable`), new type taking it
 * from the engine's default type styles.
 *
 * While the layer's text is edited on the canvas (plan 2026-11-24, stage 4), the fields show the
 * style at the selection's start, and an edit restyles the selected characters only (the whole
 * text when nothing is selected), in the edit session's one history step.
 */
export function useTypeStyle(target: TypeTarget) {
  const { run, query, session } = useKitEditor()
  const documentNow = useRef(session.doc)
  documentNow.current = session.doc
  const mounted = useRef(true)
  useLayoutEffect(() => {
    mounted.current = true
    return () => { mounted.current = false }
  }, [])
  const editing = useTypeEditingTarget()
  const edited = editing && target.layer != null && editing.layer === target.layer ? editing : null
  const layer = useTypeLayer(target.layer, edited?.range?.[0])
  const gestures = useGestures(`type-${layer?.layer ?? 'tool'}`)
  /** A command on the layer: in the edit session's step (on its selected range), else on its own. */
  const runOnLayer = (command: string, params: Record<string, unknown>, coalesce?: string) => {
    if (!layer) return
    if (edited) void edited.run(command, { ...params, ...(edited.range && command === 'type.setStyle' ? { range: edited.range } : {}) })
    else void run(command, { layer: layer.layer, ...params, ...(coalesce ? { coalesce } : {}) })
  }
  const values: TypeValues = layer ? { ...layer, size: shownSize(layer) } : typeValuesOf(target.options, target.foreground)

  /** Whether `field` can change now: every field for a layer, the tool's own for new type. */
  const editable = (field: keyof TypeValues) => layer != null || (TOOL_FIELDS as readonly string[]).includes(field as ToolField)

  /** Applies `edit`; `field` and `final` keep a field's drag one history step. */
  const apply = (edit: TypeEdit, field: string, final = true) => {
    const { style, options } = typeEdit(edit, layer?.scale ?? 1)
    if (Object.keys(options).length) target.setOptions(options)
    if (layer && Object.keys(style).length) runOnLayer('type.setStyle', style, gestures.key(field, final))
  }

  const setFont = async (font: string) => {
    const open = session.doc
    if (!open) return
    let requests = fontRequests.get(open)
    if (!requests) {
      requests = new Map()
      fontRequests.set(open, requests)
    }
    const key = layer?.layer ?? 'tool'
    const request = styleForFamily(query, font, values.fontStyle).then((fontStyle) => fontStyle === null ? null : { font, fontStyle })
    requests.set(key, request)
    try {
      const choice = await request
      if (choice === null || !mounted.current || documentNow.current !== open || requests.get(key) !== request) return
      apply(choice, 'font')
    } finally {
      if (requests.get(key) === request) requests.delete(key)
    }
  }

  /** The text colour: the layer's, or the foreground for new type (upstream's Type bar swatch). */
  const setColor = (color: string, final: boolean) => {
    if (layer) runOnLayer('type.setStyle', { color }, gestures.key('color', final))
    else target.setForeground(color)
  }

  /** The anti-aliasing: the layer's (Type › Anti-Alias), and the tool's for new type. */
  const setAntialias = (value: string) => {
    if (TOOL_ANTI_ALIAS.includes(value)) target.setOptions({ textAntiAlias: value })
    if (layer) runOnLayer(`type.antiAlias.${value}`, {})
  }

  /** The layer's orientation (Type › Orientation); new type takes the tool's (Horizontal or Vertical Type). */
  const setOrientation = (vertical: boolean) => {
    if (layer && layer.vertical !== vertical) runOnLayer(`type.orientation.${vertical ? 'vertical' : 'horizontal'}`, {})
  }

  return {
    layer: layer as TypeLayer | null,
    values,
    antialias: layer?.antialias ?? target.options.textAntiAlias,
    editable,
    apply,
    setFont,
    setColor,
    setAntialias,
    setOrientation,
  }
}

export type TypeStyle = ReturnType<typeof useTypeStyle>
