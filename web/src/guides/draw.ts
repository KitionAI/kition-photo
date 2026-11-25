import type { Guides } from './snap'

type ToScreen = (x: number, y: number) => { x: number; y: number }

/** Guides' colour (Photoshop's cyan) and a dragged guide's. */
export const GUIDE_COLOUR = '#22c7f0'

/** A guide dragged out of a ruler, or moved: where it is now, and which one it was (null: a new one). */
export type GuideDraft = { orientation: 'horizontal' | 'vertical'; position: number; index: number | null; leaving: boolean }

/** The canvas's size in its CSS pixels (the decoration's context is scaled to them). */
function canvasSize(context: CanvasRenderingContext2D) {
  const ratio = context.getTransform().a || 1
  return { width: context.canvas.width / ratio, height: context.canvas.height / ratio }
}

/** The guides across the whole canvas (a moved one where it is dragged to), one screen pixel wide at any zoom. */
export function drawGuides(context: CanvasRenderingContext2D, toScreen: ToScreen, guides: Guides, draft: GuideDraft | null, locked: boolean) {
  const { width, height } = canvasSize(context)
  context.save()
  context.setLineDash([])
  context.lineWidth = 1
  const line = (orientation: 'horizontal' | 'vertical', at: number) => {
    const p = toScreen(at, at)
    if (orientation === 'vertical') {
      const x = Math.round(p.x) + 0.5
      context.moveTo(x, 0)
      context.lineTo(x, height)
    } else {
      const y = Math.round(p.y) + 0.5
      context.moveTo(0, y)
      context.lineTo(width, y)
    }
  }
  context.beginPath()
  guides.vertical.forEach((at, index) => {
    if (!(draft && draft.orientation === 'vertical' && draft.index === index)) line('vertical', at)
  })
  guides.horizontal.forEach((at, index) => {
    if (!(draft && draft.orientation === 'horizontal' && draft.index === index)) line('horizontal', at)
  })
  context.strokeStyle = GUIDE_COLOUR
  context.globalAlpha = locked ? 0.7 : 1
  context.stroke()
  if (draft) {
    context.globalAlpha = draft.leaving ? 0.35 : 1
    context.beginPath()
    line(draft.orientation, draft.position)
    context.stroke()
  }
  context.restore()
}

/** The grid over the document: a line every `minor` pixels, darker every `major`; left out where its lines would crowd under 4 screen pixels. */
export function drawGrid(context: CanvasRenderingContext2D, toScreen: ToScreen, size: { width: number; height: number }, spacing: { major: number; minor: number }) {
  const origin = toScreen(0, 0)
  const zoom = toScreen(1, 0).x - origin.x
  if (!(zoom > 0)) return
  const step = spacing.minor * zoom >= 4 ? spacing.minor : spacing.major * zoom >= 4 ? spacing.major : 0
  if (!step) return
  const view = canvasSize(context)
  const end = toScreen(size.width, size.height)
  const x0 = Math.max(0, Math.floor(-origin.x / zoom / step) * step)
  const y0 = Math.max(0, Math.floor(-origin.y / zoom / step) * step)
  const x1 = Math.min(size.width, (view.width - origin.x) / zoom)
  const y1 = Math.min(size.height, (view.height - origin.y) / zoom)
  context.save()
  context.setLineDash([])
  context.lineWidth = 1
  for (const major of [false, true]) {
    context.beginPath()
    for (let x = x0; x <= x1; x += step) {
      if ((Math.abs(x / spacing.major - Math.round(x / spacing.major)) < 1e-6) !== major) continue
      const sx = Math.round(origin.x + x * zoom) + 0.5
      context.moveTo(sx, Math.max(0, origin.y))
      context.lineTo(sx, Math.min(view.height, end.y))
    }
    for (let y = y0; y <= y1; y += step) {
      if ((Math.abs(y / spacing.major - Math.round(y / spacing.major)) < 1e-6) !== major) continue
      const sy = Math.round(origin.y + y * zoom) + 0.5
      context.moveTo(Math.max(0, origin.x), sy)
      context.lineTo(Math.min(view.width, end.x), sy)
    }
    context.strokeStyle = major ? 'rgba(128, 128, 128, 0.85)' : 'rgba(128, 128, 128, 0.4)'
    context.stroke()
  }
  context.restore()
}

/** Photoshop's pixel grid: from 600 % up, a faint line between every two pixels of the document. */
export const PIXEL_GRID_ZOOM = 6

export function drawPixelGrid(context: CanvasRenderingContext2D, toScreen: ToScreen, size: { width: number; height: number }) {
  const origin = toScreen(0, 0)
  const zoom = toScreen(1, 0).x - origin.x
  if (zoom < PIXEL_GRID_ZOOM) return
  const view = canvasSize(context)
  const x0 = Math.max(0, Math.floor(-origin.x / zoom))
  const y0 = Math.max(0, Math.floor(-origin.y / zoom))
  const x1 = Math.min(size.width, Math.ceil((view.width - origin.x) / zoom))
  const y1 = Math.min(size.height, Math.ceil((view.height - origin.y) / zoom))
  const top = Math.max(0, origin.y)
  const bottom = Math.min(view.height, origin.y + size.height * zoom)
  const left = Math.max(0, origin.x)
  const right = Math.min(view.width, origin.x + size.width * zoom)
  context.save()
  context.setLineDash([])
  context.lineWidth = 1
  context.beginPath()
  for (let x = x0; x <= x1; x += 1) {
    const sx = Math.round(origin.x + x * zoom) + 0.5
    context.moveTo(sx, top)
    context.lineTo(sx, bottom)
  }
  for (let y = y0; y <= y1; y += 1) {
    const sy = Math.round(origin.y + y * zoom) + 0.5
    context.moveTo(left, sy)
    context.lineTo(right, sy)
  }
  context.strokeStyle = 'rgba(160, 160, 160, 0.35)'
  context.stroke()
  context.restore()
}
