import type { KitPointer, KitTool } from '@kition/sdk/editor'
import type { PhotoInfo, PhotoLayer } from '../photo'
import type { PhotoToolId } from './catalog'

/**
 * The "Rasterize?" prompt (upstream's `rasterize_prompt`): painting the pixels of a type, shape,
 * object or fill layer with a pixel tool asks first, as Photoshop does, instead of failing.
 * OK runs the matching `layer.rasterize.*` command, then paints where the press was with the same
 * tool, so rasterizing and painting are two history states; Cancel does nothing.
 */

/** A layer kind (as `inspect()` names it) that must be rasterized before its pixels can be painted: the command that does it, and Photoshop's question. */
export type RasterizeKind = { command: string; message: string }

export const RASTERIZE_KINDS: Readonly<Record<string, RasterizeKind>> = {
  Type: {
    command: 'layer.rasterize.type',
    message: 'This type layer must be rasterized before proceeding. Its text will no longer be editable. Rasterize the type?',
  },
  Shape: {
    command: 'layer.rasterize.shape',
    message: 'This shape layer must be rasterized before proceeding. It will no longer be editable as a shape. Rasterize the shape?',
  },
  Object: {
    command: 'layer.rasterize.objectLayer',
    message: 'This object layer must be rasterized before proceeding. Its contents will no longer be editable. Rasterize the object layer?',
  },
  Fill: {
    command: 'layer.rasterize.fillContent',
    message: 'This fill layer must be rasterized before proceeding. Its fill content will no longer be editable. Rasterize the layer?',
  },
}

/** The brush-like tools (upstream's `is_brushlike`), which all paint the layer's pixels. */
const BRUSH_LIKE: ReadonlySet<PhotoToolId> = new Set<PhotoToolId>(['Brush', 'Pencil', 'MixerBrush', 'Eraser', 'BackgroundEraser', 'SpotHealing', 'Healing', 'CloneStamp', 'HistoryBrush', 'Blur', 'Sharpen', 'Smudge', 'Dodge', 'Burn', 'Sponge'])

/** Whether `tool` paints the layer's pixels (upstream's `paints_pixels`): the brush-like tools, the Paint Bucket, the Magic Eraser, and the Gradient in classic mode. */
export function paintsPixels(tool: PhotoToolId, gradientClassic: boolean) {
  return BRUSH_LIKE.has(tool) || tool === 'PaintBucket' || tool === 'MagicEraser' || (tool === 'Gradient' && gradientClassic)
}

/** The active layer and the groups it is in: their locks apply to it (upstream's `effective_locks`). */
function lineage(info: PhotoInfo): PhotoLayer[] {
  const index = info.layers.findIndex((layer) => layer.id === info.activeLayer)
  if (index < 0) return []
  const chain = [info.layers[index]]
  // The list runs top first, each group right above its contents: a group is the nearest row above at one level less deep.
  let depth = info.layers[index].depth
  for (let above = index - 1; above >= 0 && depth > 0; above -= 1) {
    if (info.layers[above].depth < depth) {
      chain.push(info.layers[above])
      depth = info.layers[above].depth
    }
  }
  return chain
}

/**
 * The rasterizing that painting the active layer needs first (upstream's `needed`), or null: the
 * paint goes to the layer's pixels (not its mask, an alpha channel, or the Quick Mask), and the
 * layer is a type, shape, object or fill layer whose pixels are not locked.
 */
export function rasterizeNeeded(info: PhotoInfo | null, maskTarget: boolean): (RasterizeKind & { layer: number }) | null {
  if (!info || maskTarget || info.channels?.quickMask || info.channels?.target.kind === 'alpha') return null
  const chain = lineage(info)
  const layer = chain[0]
  const kind = layer ? RASTERIZE_KINDS[layer.kind] : undefined
  if (!layer || layer.id === undefined || !kind) return null
  if (chain.some((item) => item.locks.all || item.locks.pixels)) return null
  return { ...kind, layer: layer.id }
}

/** A press held back by the prompt: what to rasterize, and the tool and press that paint once it is done. */
export type RasterizeAsk = RasterizeKind & { layer: number; tool: KitTool; pointer: KitPointer; context: Parameters<KitTool['begin']>[1] }

/**
 * `tool`, with its presses held back while `ask` says the layer must be rasterized first: the
 * press reaches the prompt (`open`, once while one is up) instead of the tool. Alt-click with the
 * Clone Stamp or Healing Brush only sets the source, so it goes through.
 */
export function withRasterizePrompt(tool: KitTool, id: PhotoToolId, ask: () => (RasterizeKind & { layer: number }) | null, open: (ask: RasterizeAsk) => void): KitTool {
  return {
    ...tool,
    begin(pointer, context) {
      const setsSource = (id === 'CloneStamp' || id === 'Healing') && pointer.alt
      const needed = setsSource ? null : ask()
      if (!needed) return tool.begin(pointer, context)
      open({ ...needed, tool, pointer, context })
    },
  }
}

/**
 * OK: rasterizes the layer (one history state), then paints where the press was with the same
 * tool (another). `run` reports a failure itself (the layer was rasterized meanwhile, say), and
 * nothing is painted then.
 */
export async function rasterizeThenPaint(ask: RasterizeAsk, run: (command: string, params: Record<string, unknown>) => Promise<boolean>, current: () => boolean = () => true) {
  if (!current()) return
  if (!(await run(ask.command, { layer: ask.layer }))) return
  if (!current()) return
  const gesture = ask.tool.begin(ask.pointer, ask.context)
  gesture?.end?.(ask.pointer, ask.context)
}
