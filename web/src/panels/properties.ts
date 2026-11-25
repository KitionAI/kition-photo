/**
 * The Properties panel's arithmetic (upstream `doc_props_ui.rs`, `props_layout.rs`), kept apart
 * from its components so the parity tests run it in Node.
 */

/** `image.canvasSize` params for a new width or height, keeping the aspect ratio when linked; null when nothing changes (upstream `canvas_resize_params`). */
export function canvasResizeParams(old: { width: number; height: number }, width: number | null, height: number | null, linked: boolean) {
  const [ow, oh] = [old.width, old.height]
  let w = Math.max(1, Math.round(width ?? ow))
  let h = Math.max(1, Math.round(height ?? oh))
  if (linked) {
    if (width != null) h = Math.max(1, Math.round((w * oh) / ow))
    else if (height != null) w = Math.max(1, Math.round((h * ow) / oh))
  }
  return w === ow && h === oh ? null : { width: w, height: h, anchor: 'center' }
}

/** A layer's content box: `[x0, y0, x1, y1]`. */
export type Box = readonly [number, number, number, number]

/**
 * A typed X, Y, W or H as the command the Move tool or Free Transform would run (plan 2026-11-24,
 * stage 5): X and Y move the layer (`layer.translate`), W and H scale its content from its top
 * left corner (`edit.transform`); null when the value is the same, or not a size.
 */
export function boundsEdit(layer: number, box: Box, field: 'x' | 'y' | 'w' | 'h', value: number): { command: string; params: Record<string, unknown> } | null {
  if (!Number.isFinite(value)) return null
  const typed = Math.round(value)
  const [x0, y0, x1, y1] = box
  if (field === 'x' || field === 'y') {
    const delta = typed - (field === 'x' ? x0 : y0)
    if (delta === 0) return null
    return { command: 'layer.translate', params: { layer, dx: field === 'x' ? delta : 0, dy: field === 'y' ? delta : 0 } }
  }
  if (typed < 1 || typed === (field === 'w' ? x1 - x0 : y1 - y0)) return null
  const right = field === 'w' ? x0 + typed : x1
  const bottom = field === 'h' ? y0 + typed : y1
  return {
    command: 'edit.transform',
    params: {
      layer,
      rect: [x0, y0, x1, y1],
      quad: [
        [x0, y0],
        [right, y0],
        [right, bottom],
        [x0, bottom],
      ],
      interpolation: 'bicubic',
    },
  }
}

/** The Quick Actions each layer kind offers (upstream `props_layout::quick_actions`): label and command. */
export const QUICK_ACTIONS: Readonly<Record<string, readonly (readonly [string, string])[]>> = {
  Pixel: [
    ['Remove Background', 'layer.removeBackground'],
    ['Select Subject', 'select.subject'],
  ],
  Type: [
    ['Convert to Shape', 'text.convertToShape'],
    ['Convert to Paragraph Text', 'text.convertToParagraphText'],
    ['Convert to Point Text', 'text.convertToPointText'],
    ['Create Warped Text', 'text.warpText'],
    ['Rasterize Type', 'text.rasterizeTypeLayer'],
  ],
  Shape: [
    ['Rasterize Shape', 'layer.rasterize.shape'],
    ['Convert to Object Layer', 'layer.objectLayers.convertToObjectLayer'],
  ],
  Object: [
    ['Edit Contents', 'layer.objectLayers.editContents'],
    ['Convert to Layers', 'layer.objectLayers.convertToLayers'],
    ['Rasterize Object Layer', 'layer.rasterize.objectLayer'],
  ],
  Group: [
    ['Ungroup Layers', 'layer.ungroupLayers'],
    ['Convert to Object Layer', 'layer.objectLayers.convertToObjectLayer'],
  ],
  Fill: [['Rasterize Fill Content', 'layer.rasterize.fillContent']],
}

/** The Properties header's kind (upstream `props_layout::kind_label`): an adjustment's own name, "Artboard", or "<Kind> Layer". */
export function kindLabel(kind: string, adjustment: string | null, artboard: boolean) {
  if (adjustment) return adjustment
  if (artboard) return 'Artboard'
  return `${kind} Layer`
}

/** Upstream's colour modes in Photoshop's order: the mode `inspect` reports, its label, and its Image › Mode command. */
export const MODES: readonly (readonly [string, string, string])[] = [
  ['Bitmap', 'Bitmap', 'image.mode.bitmap'],
  ['Grayscale', 'Grayscale', 'image.mode.grayscale'],
  ['Duotone', 'Duotone', 'image.mode.duotone'],
  ['Indexed', 'Indexed Color', 'image.mode.indexedColor'],
  ['Rgb', 'RGB Color', 'image.mode.rgb'],
  ['Cmyk', 'CMYK Color', 'image.mode.cmyk'],
  ['Lab', 'Lab Color', 'image.mode.lab'],
  ['Multichannel', 'Multichannel', 'image.mode.multichannel'],
]

/** Upstream's bit depths: bits per channel, label, and command. */
export const DEPTHS: readonly (readonly [string, string, string])[] = [
  ['8', '8 Bits/Channel', 'image.mode.bits8'],
  ['16', '16 Bits/Channel', 'image.mode.bits16'],
  ['32', '32 Bits/Channel', 'image.mode.bits32'],
]
