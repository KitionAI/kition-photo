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

/** The Quick Actions each layer kind offers (upstream `props_layout::quick_actions`): label and command. */
export const QUICK_ACTIONS: Readonly<Record<string, readonly (readonly [string, string])[]>> = {
  Pixel: [
    ['Remove Background', 'layer.removeBackground'],
    ['Select Subject', 'select.subject'],
  ],
  Type: [
    ['Convert to Shape', 'type.convertToShape'],
    ['Convert to Paragraph Text', 'type.convertToParagraphText'],
    ['Convert to Point Text', 'type.convertToPointText'],
    ['Create Warped Text', 'type.warpText'],
    ['Rasterize Type', 'type.rasterizeTypeLayer'],
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
