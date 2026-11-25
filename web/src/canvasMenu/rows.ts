/**
 * The canvas's right-click menu (plan 2026-11-24, stage 7): which rows a right click (or the menu
 * key, or Shift+F10) offers, by tool and by what is under the pointer, as Photoshop's. Pure, so the
 * rows are tested without a browser (`rows.test.mjs`); `CanvasMenu.tsx` draws them, translating
 * each label (upstream's English phrases) and greying a command the engine refuses now.
 */

/** What a row does: an engine command (a "…" one opens its dialog), a layer to select, or the editor's own action. */
export type CanvasMenuAction =
  | { command: string; params?: Record<string, unknown> }
  | { layer: number }
  | { own: OwnAction; value?: string }

/** The editor's own actions a row can take (no engine command stands for them). */
export type OwnAction = 'freeTransform' | 'editType' | 'commitType' | 'cancelType' | 'cropCommit' | 'cropCancel' | 'sampleSize' | 'view' | 'deleteAnchor' | 'addAnchor' | 'closePath'

export type CanvasMenuRow =
  | { kind: 'separator' }
  | { kind: 'heading'; label: string }
  | { kind: 'item'; label: string; action: CanvasMenuAction; checked?: boolean; disabled?: boolean }

/** What the menu depends on, read when it opens. */
export type CanvasMenuContext = {
  tool: string
  /** Cmd (Ctrl) held: the layers under the pointer, whatever the tool (Photoshop's rule). */
  layersOnly: boolean
  hasSelection: boolean
  /** The active layer's kind (`Pixel`, `Type`, `Shape`…), or null without one. */
  layer: { kind: string; background: boolean } | null
  /** The layers with pixels under the pointer, topmost first, and which is active (Move, Cmd). */
  layersHere: readonly { id: number; name: string }[]
  activeLayer: number | null
  /** Text is being edited on the canvas (or typed into a new text field). */
  typeEditing: boolean
  /** The path the path tools work on under the pointer: the work path, a shape layer's, or none. */
  path: 'work' | 'shape' | null
  /** The pointer is on an anchor of that path (Delete Anchor Point), or on a segment (Add Anchor Point). */
  pathPart: 'anchor' | 'segment' | null
  /** The Pen's path is still being drawn (Close Path). */
  penOpen: boolean
  /** The Crop tool has a frame to commit. */
  cropping: boolean
  /** The Eyedropper's Sample Size option. */
  sampleSize: string
}

const SEP: CanvasMenuRow = { kind: 'separator' }
const item = (label: string, action: CanvasMenuAction, extra: { checked?: boolean; disabled?: boolean } = {}): CanvasMenuRow => ({ kind: 'item', label, action, ...extra })
const command = (label: string, id: string, params?: Record<string, unknown>) => item(label, params ? { command: id, params } : { command: id })

export const SELECTION_TOOLS: ReadonlySet<string> = new Set(['RectMarquee', 'EllipseMarquee', 'SingleRowMarquee', 'SingleColumnMarquee', 'Lasso', 'PolygonLasso', 'MagneticLasso', 'MagicWand', 'ObjectSelection'])
export const TYPE_TOOLS: ReadonlySet<string> = new Set(['Type', 'VerticalType'])
export const PATH_TOOLS: ReadonlySet<string> = new Set(['Pen', 'ConvertPoint', 'PathSelection', 'DirectSelection', 'Rectangle', 'EllipseShape', 'Triangle', 'Polygon', 'Line', 'CustomShape'])
export const VIEW_TOOLS: ReadonlySet<string> = new Set(['Hand', 'Zoom'])

/** The Eyedropper's sample sizes, as the options bar has them, with upstream's labels. */
export const SAMPLE_SIZES: readonly (readonly [string, string])[] = [
  ['point', 'Point Sample'],
  ['3', '3 by 3 Average'],
  ['5', '5 by 5 Average'],
  ['11', '11 by 11 Average'],
  ['31', '31 by 31 Average'],
  ['51', '51 by 51 Average'],
  ['101', '101 by 101 Average'],
]

/** The rows every tool without a menu of its own shows: the active layer's commands (upstream's canvas layer menu). */
function layerRows(): CanvasMenuRow[] {
  return [
    command('Blending Options…', 'layer.layerStyle.blendingOptions'),
    command('Duplicate Layer…', 'layer.duplicate'),
    command('Delete Layer', 'layer.delete'),
    command('Group from Layers…', 'layer.new.groupFromLayers'),
    SEP,
    command('Convert to Object Layer', 'layer.objectLayers.convertToObjectLayer'),
    command('Rasterize Layer', 'layer.rasterize.layer'),
    SEP,
    item('Free Transform', { own: 'freeTransform' }),
    SEP,
    command('Color Range…', 'select.colorRange'),
    command('Load Selection…', 'select.loadSelection'),
    command('Reselect', 'select.reselect'),
  ]
}

/** The layers under the pointer, topmost first, the active one checked; choosing one selects it. */
function layersHereRows(context: CanvasMenuContext): CanvasMenuRow[] {
  if (!context.layersHere.length) return []
  return [{ kind: 'heading', label: 'Select a layer' }, ...context.layersHere.map((layer) => item(layer.name, { layer: layer.id }, { checked: layer.id === context.activeLayer }))]
}

/** A selection tool's menu: with a selection, what to do with it; without one, how to get one back. */
function selectionRows(context: CanvasMenuContext): CanvasMenuRow[] {
  if (!context.hasSelection)
    return [command('Select All', 'edit.selectAll'), command('Reselect', 'select.reselect'), command('Color Range…', 'select.colorRange'), command('Load Selection…', 'select.loadSelection')]
  return [
    command('Deselect', 'edit.deselectAll'),
    command('Select Inverse', 'select.inverse'),
    command('Feather…', 'select.modify.feather'),
    SEP,
    command('Save Selection…', 'select.saveSelection'),
    command('Make Work Path…', 'select.toWorkPath'),
    SEP,
    command('Layer via Copy', 'layer.new.layerViaCopy'),
    command('Layer via Cut', 'layer.new.layerViaCut'),
    command('New Layer…', 'layer.new.layer'),
    SEP,
    item('Free Transform', { own: 'freeTransform' }),
    SEP,
    command('Fill…', 'edit.fill'),
    command('Stroke…', 'edit.stroke'),
    SEP,
    command('Last Filter', 'filter.lastFilter'),
    command('Fade…', 'edit.fade'),
  ]
}

/** The Type tools' menu: commit or cancel an edit, then what to make of the type layer. */
function typeRows(context: CanvasMenuContext): CanvasMenuRow[] {
  const editing: CanvasMenuRow[] = context.typeEditing ? [item('Commit', { own: 'commitType' }), item('Cancel', { own: 'cancelType' }), SEP] : []
  if (context.layer?.kind !== 'Type') return [...editing, ...layerRows()]
  return [
    ...editing,
    item('Edit Type', { own: 'editType' }, { disabled: context.typeEditing }),
    SEP,
    command('Rasterize Type', 'text.rasterizeTypeLayer'),
    command('Create Work Path', 'text.convertToWorkPath'),
    command('Convert to Shape', 'text.convertToShape'),
    SEP,
    command('Convert to Paragraph Text', 'text.convertToParagraphText'),
    command('Convert to Point Text', 'text.convertToPointText'),
    SEP,
    command('Horizontal', 'text.orientation.horizontal'),
    command('Vertical', 'text.orientation.vertical'),
    SEP,
    ...['None', 'Sharp', 'Crisp', 'Strong', 'Smooth'].map((label) => command(label, `text.antiAlias.${label.toLowerCase()}`)),
  ]
}

/** The path and shape tools' menu: the anchor or segment under the pointer, then the path's commands. */
function pathRows(context: CanvasMenuContext): CanvasMenuRow[] {
  const point: CanvasMenuRow[] = []
  if (context.penOpen) point.push(item('Close Path', { own: 'closePath' }))
  if (context.pathPart === 'anchor') point.push(item('Delete Anchor Point', { own: 'deleteAnchor' }))
  if (context.pathPart === 'segment') point.push(item('Add Anchor Point', { own: 'addAnchor' }))
  if (point.length) point.push(SEP)
  if (context.path === 'work')
    return [
      ...point,
      command('Make Selection', 'path.toSelection', { name: 'work' }),
      command('Fill Path', 'path.fill', { name: 'work' }),
      command('Stroke Path', 'path.stroke', { name: 'work' }),
      SEP,
      command('Create Vector Mask', 'layer.vectorMask.fromPath', { name: 'work' }),
      command('Delete Path', 'path.delete', { name: 'work' }),
    ]
  if (context.path === 'shape')
    return [...point, command('Make Selection', 'path.toSelection', { name: 'layer' }), SEP, item('Free Transform Path', { own: 'freeTransform' }), SEP, command('Rasterize Layer', 'layer.rasterize.layer'), command('Delete Layer', 'layer.delete')]
  return [...point, ...layerRows()]
}

/** The Hand and Zoom tools' menu: the View menu's zoom rows (Kition's own view commands). */
function viewRows(): CanvasMenuRow[] {
  return [
    item('Fit on Screen', { own: 'view', value: 'kit:view.fitOnScreen' }),
    item('100%', { own: 'view', value: 'kit:view.actualPixels' }),
    item('200%', { own: 'view', value: 'kit:view.twoHundredPercent' }),
    item('Print Size', { own: 'view', value: 'kit:view.printSize' }),
    SEP,
    item('Zoom In', { own: 'view', value: 'kit:view.zoomIn' }),
    item('Zoom Out', { own: 'view', value: 'kit:view.zoomOut' }),
  ]
}

/**
 * The rows of the canvas's menu for `context`, separators tidied (none first, last or twice).
 * Brush tools are not asked: their right click opens the brush preset picker.
 */
export function canvasMenuRows(context: CanvasMenuContext): CanvasMenuRow[] {
  let rows: CanvasMenuRow[]
  if (context.layersOnly) rows = layersHereRows(context)
  else if (context.tool === 'Move') rows = [...layersHereRows(context), SEP, ...layerRows()]
  else if (SELECTION_TOOLS.has(context.tool)) rows = selectionRows(context)
  else if (TYPE_TOOLS.has(context.tool)) rows = typeRows(context)
  else if (PATH_TOOLS.has(context.tool)) rows = pathRows(context)
  else if (VIEW_TOOLS.has(context.tool)) rows = viewRows()
  else if (context.tool === 'Crop') rows = [item('Crop', { own: 'cropCommit' }, { disabled: !context.cropping }), item('Cancel', { own: 'cropCancel' }, { disabled: !context.cropping })]
  else if (context.tool === 'Eyedropper') rows = SAMPLE_SIZES.map(([value, label]) => item(label, { own: 'sampleSize', value }, { checked: context.sampleSize === value }))
  else rows = layerRows()
  return tidy(rows)
}

/** Without a separator at either end or two in a row. */
export function tidy(rows: readonly CanvasMenuRow[]): CanvasMenuRow[] {
  const out: CanvasMenuRow[] = []
  for (const row of rows) {
    if (row.kind === 'separator' && (!out.length || out[out.length - 1].kind === 'separator')) continue
    out.push(row)
  }
  while (out.length && out[out.length - 1].kind === 'separator') out.pop()
  return out
}

/** The engine command a row runs, if any (for its enabled state). */
export function rowCommand(row: CanvasMenuRow): string | null {
  return row.kind === 'item' && 'command' in row.action ? row.action.command : null
}
