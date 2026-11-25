import type { KitEngineInfo, RasterRect } from '@kition/sdk/editor'

/** A layer's locks (Layers panel Lock buttons). */
export type PhotoLocks = { transparency: boolean; pixels: boolean; position: boolean; artboard: boolean; all: boolean }

/** One of a layer's effects (layer styles), as the Layers panel lists them under it. */
export type PhotoEffect = { kind: string; key: string; enabled: boolean }

/**
 * A layer as the editor lists it, top of the stack first and each group right above its contents;
 * `background` is the document's Background layer (the bottom pixel layer named Background, its
 * position and transparency locked), `folded` that a group above it is closed (the Layers panel
 * leaves it out).
 */
export type PhotoLayer = {
  id: number | undefined
  name: string
  kind: string
  visible: boolean
  opacity?: number
  fill?: number
  blend?: string
  selected: boolean
  depth: number
  background: boolean
  locks: PhotoLocks
  clipped: boolean
  hasMask: boolean
  /** Whether the layer mask applies, and whether it moves with the layer (meaningless without a mask). */
  maskEnabled: boolean
  maskLinked: boolean
  /** The layer mask's Density (0 to 100 %) and Feather (px), Properties › Masks; 100 and 0 without a mask. */
  maskDensity: number
  maskFeather: number
  linked: boolean
  /** A group's disclosure; undefined for other layers. */
  expanded?: boolean
  folded: boolean
  effects: { enabled: boolean; items: PhotoEffect[] } | null
  /** An adjustment layer's kind (`levels`, `hueSaturation`…) and settings as `layer.setAdjustment` takes them; null for other layers. */
  adjustment: { kind: string; params: Record<string, unknown>; lookup: string | null } | null
  /** Whether the layer is an artboard (a group with a canvas of its own). */
  artboard: boolean
}

/** A channel as the Channels panel lists it. */
export type PhotoChannels = {
  composite: string
  colors: { name: string; visible: boolean }[]
  alpha: { index: number; name: string; visible: boolean; spot: unknown }[]
  /** The layer mask shown on the canvas (Alt-click), or null. */
  layerMaskView: { layer: number; mode: string } | null
  quickMask: { visible: boolean } | null
  /** What pixel commands edit: the composite, one color channel, or an alpha channel. */
  target: { kind: 'composite' | 'color' | 'alpha'; index?: number }
  compositeVisible: boolean
}

/** What the editor reads from the engine's `inspect()`. */
export type PhotoInfo = {
  width: number
  height: number
  mode: string
  depth: number
  layers: PhotoLayer[]
  activeLayer: number | null
  /** The active layer's content bounds, for the Move tool. */
  activeBounds: RasterRect | null
  hasSelection: boolean
  /** The selection's bounds as [x, y, width, height], or null without one (the Info panel's W and H). */
  selectionBounds: readonly [number, number, number, number] | null
  channels: PhotoChannels | null
  /** Layers whose effects rows the Layers panel folds. */
  effectsCollapsed: readonly number[]
  /** The document's name and resolution (pixels per inch). */
  name: string
  resolution: number
  /** The History panel's states: those Undo steps back through, the current one last, at `historyIndex`; then those Redo steps through, the next first. */
  history: readonly string[]
  historyIndex: number
  historyRedo: readonly string[]
}

type InspectedLayer = {
  id?: number
  name?: string
  kind?: string
  visible?: boolean
  opacity?: number
  fill?: number
  blend?: string
  selected?: boolean
  clipped?: boolean
  hasMask?: boolean
  maskEnabled?: boolean | null
  maskLinked?: boolean | null
  maskDensity?: number | null
  maskFeather?: number | null
  linkGroup?: number | null
  expanded?: boolean
  bounds?: [number, number, number, number] | null
  locks?: Partial<PhotoLocks>
  effects?: { enabled?: boolean; items?: Partial<PhotoEffect>[] }
  adjustment?: { ColorLookup?: { name?: string }; Unsupported?: unknown } | string | null
  adjustmentKind?: string
  adjustmentParams?: Record<string, unknown>
  artboard?: unknown
  children?: InspectedLayer[]
}

const NO_LOCKS: PhotoLocks = { transparency: false, pixels: false, position: false, artboard: false, all: false }

function find(layers: readonly InspectedLayer[] | undefined, id: number | null | undefined): InspectedLayer | null {
  for (const layer of layers ?? []) {
    if (layer.id === id) return layer
    const inner = find(layer.children, id)
    if (inner) return inner
  }
  return null
}

/** An adjustment layer's kind and settings; one kept as imported (upstream `Unsupported`) is `unsupported`. */
function adjustmentOf(layer: InspectedLayer): NonNullable<PhotoLayer['adjustment']> {
  const serialized = typeof layer.adjustment === 'object' ? layer.adjustment : null
  return {
    kind: serialized?.Unsupported !== undefined ? 'unsupported' : (layer.adjustmentKind ?? ''),
    params: layer.adjustmentParams ?? {},
    lookup: serialized?.ColorLookup?.name ?? null,
  }
}

function flatten(layers: readonly InspectedLayer[] | undefined, depth: number, folded: boolean, out: PhotoLayer[]) {
  for (const [index, layer] of (layers ?? []).entries()) {
    const locks = { ...NO_LOCKS, ...layer.locks }
    // The engine's and upstream's rule (`is_background`): the bottom pixel layer named Background, its position and transparency locked.
    const background = depth === 0 && index === (layers?.length ?? 0) - 1 && layer.kind === 'Pixel' && layer.name === 'Background' && locks.position && locks.transparency
    const items = (layer.effects?.items ?? []).map((item) => ({ kind: item.kind ?? '', key: item.key ?? '', enabled: item.enabled ?? true }))
    out.push({
      id: layer.id,
      name: layer.name ?? '',
      kind: layer.kind ?? '',
      visible: layer.visible ?? true,
      opacity: layer.opacity,
      fill: layer.fill,
      blend: layer.blend,
      selected: layer.selected ?? false,
      depth,
      background,
      locks,
      clipped: layer.clipped ?? false,
      hasMask: layer.hasMask ?? false,
      maskEnabled: layer.maskEnabled ?? true,
      maskLinked: layer.maskLinked ?? true,
      maskDensity: layer.maskDensity ?? 100,
      maskFeather: layer.maskFeather ?? 0,
      linked: layer.linkGroup != null,
      expanded: layer.kind === 'Group' ? (layer.expanded ?? true) : undefined,
      folded,
      effects: items.length ? { enabled: layer.effects?.enabled ?? true, items } : null,
      adjustment: layer.adjustmentKind ? adjustmentOf(layer) : null,
      artboard: layer.artboard != null,
    })
    flatten(layer.children, depth + 1, folded || layer.expanded === false, out)
  }
}

export function describe(info: KitEngineInfo): PhotoInfo {
  const inspected = info.inspected as {
    name?: string
    mode?: string
    depth?: number
    resolution?: number
    activeLayer?: number | null
    layers?: InspectedLayer[]
    hasSelection?: boolean
    selectionBounds?: [number, number, number, number] | null
    channels?: PhotoChannels | null
    effectsCollapsed?: number[]
    history?: string[]
    historyIndex?: number
    historyRedo?: string[]
  }
  const layers: PhotoLayer[] = []
  flatten(inspected.layers, 0, false, layers)
  const bounds = find(inspected.layers, inspected.activeLayer)?.bounds
  return {
    width: info.width,
    height: info.height,
    mode: inspected.mode ?? '',
    depth: inspected.depth ?? 8,
    layers,
    activeLayer: inspected.activeLayer ?? null,
    activeBounds: bounds && bounds[2] > 0 && bounds[3] > 0 ? { x0: bounds[0], y0: bounds[1], x1: bounds[0] + bounds[2], y1: bounds[1] + bounds[3] } : null,
    hasSelection: inspected.hasSelection === true,
    selectionBounds: inspected.selectionBounds ?? null,
    channels: inspected.channels ?? null,
    effectsCollapsed: inspected.effectsCollapsed ?? [],
    name: inspected.name ?? '',
    resolution: inspected.resolution ?? 72,
    history: inspected.history ?? [],
    historyIndex: inspected.historyIndex ?? Math.max(0, (inspected.history?.length ?? 1) - 1),
    historyRedo: inspected.historyRedo ?? [],
  }
}

/** How far past the picture's edge the view scrolls, in screen pixels. */
export const SCROLL_MARGIN = 96

/**
 * Where scrolling stops, in document pixels, for a view `shown` screen pixels large at `zoom`:
 * along each axis, a picture longer than the view (with `SCROLL_MARGIN` screen pixels of canvas
 * at each end) keeps the view inside it, and a shorter one stays whole inside the view, where it
 * may be moved. So the view never shows more empty canvas than it must.
 */
export function picturePanBounds(width: number, height: number, shown: { zoom: number; width: number; height: number } | null): RasterRect | null {
  if (!shown || !(shown.zoom > 0) || !(width > 0) || !(height > 0)) return null
  const margin = SCROLL_MARGIN / shown.zoom
  const axis = (length: number, extent: number) => {
    // Shorter than the view: the view's start runs from where the picture's far margin meets the
    // view's far edge to where its near margin meets the near edge.
    if (extent > length + margin * 2) return [length + margin - extent, extent - margin]
    return [-margin, length + margin]
  }
  const [x0, x1] = axis(width, shown.width / shown.zoom)
  const [y0, y1] = axis(height, shown.height / shown.zoom)
  return { x0, y0, x1, y1 }
}

/** The picture and `SCROLL_MARGIN` screen pixels of canvas around it, in document pixels at `zoom`: what the scrollbars measure. */
export function pictureArea(width: number, height: number, zoom: number): RasterRect | null {
  if (!(zoom > 0) || !(width > 0) || !(height > 0)) return null
  const margin = SCROLL_MARGIN / zoom
  return { x0: -margin, y0: -margin, x1: width + margin, y1: height + margin }
}
