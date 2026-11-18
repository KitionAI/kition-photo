import { dragRect, ZOOM_TOOL, type KitTool } from '@kition/kit-editor'
import { toolParams } from '../options'
import { NO_REPAINT, dashed, lineDrag, type PhotoToolEnv } from './common'

/** The Eyedropper: the foreground colour (with Alt, the background colour) becomes the colour under the click, sampled as the options bar says. */
export function eyedropperTool(env: PhotoToolEnv): KitTool {
  const samples = [0, 0]
  return {
    id: 'Eyedropper',
    cursor: 'crosshair',
    begin: (pointer) => {
      const target = pointer.alt ? 1 : 0
      const sample = ++samples[target]
      void env.query<number[]>('document.sampleColor', { x: Math.floor(pointer.x), y: Math.floor(pointer.y), ...toolParams('Eyedropper', env.options) }).then((rgba) => {
        if (rgba && sample === samples[target]) env.pick([0, 1, 2].map((channel) => Math.round(rgba[channel] * 255)) as [number, number, number], pointer.alt)
      })
    },
  }
}

/** How fast a scrubby drag zooms: the zoom doubles every this many screen pixels. */
const SCRUB_DOUBLING = 100

/**
 * The Zoom tool: a click zooms in (Alt out); a drag with Scrubby Zoom zooms continuously (right
 * in, left out) about the press, else zooms to the dragged box.
 */
export function zoomTool(env: PhotoToolEnv): KitTool {
  return {
    ...ZOOM_TOOL,
    begin: (start, context) => (env.options.scrubbyZoom ? scrub(start, context) : zoomBox(start, context)),
  }
}

type Begin = KitTool['begin']

/** A Zoom drag without Scrubby Zoom: zooms to the dragged box; a click zooms a step. */
const zoomBox: Begin = (start, context) => {
  const click = ZOOM_TOOL.begin(start, context)
  return {
    move: (pointer, { overlay }) => {
      overlay.draw((canvas, toScreen) => {
        const area = dragRect(start, pointer)
        const a = toScreen(area.x, area.y)
        const b = toScreen(area.x + area.width, area.y + area.height)
        canvas.beginPath()
        canvas.rect(a.x + 0.5, a.y + 0.5, b.x - a.x, b.y - a.y)
        dashed(canvas)
      })
    },
    end: (pointer, next) => {
      const area = dragRect(start, pointer)
      if (area.width < 2 || area.height < 2) return click?.end?.(pointer, next)
      next.overlay.clear()
      next.viewport.fitArea({ x0: area.x, y0: area.y, x1: area.x + area.width, y1: area.y + area.height })
    },
  }
}

/** A Scrubby Zoom drag: right zooms in and left out, about the press; a click zooms a step. */
const scrub: Begin = (start, context) => {
  const click = ZOOM_TOOL.begin(start, context)
  const from = context.viewport.view.zoom
  let moved = false
  const update = (pointer: Parameters<Begin>[0], { viewport }: Parameters<Begin>[1]) => {
    const dx = pointer.clientX - start.clientX
    moved ||= Math.abs(dx) > 2
    if (moved) viewport.zoomTo(from * 2 ** (dx / SCRUB_DOUBLING), start.clientX, start.clientY)
  }
  return {
    move: update,
    end: (pointer, next) => {
      update(pointer, next)
      if (!moved) click?.end?.(pointer, next)
    },
  }
}

/** The Ruler: measures the dragged line (Image › Analysis keeps the measurement). */
export function rulerTool(env: PhotoToolEnv): KitTool {
  return lineDrag('Ruler', 'crosshair', (start, end) => {
    if (Math.hypot(end.x - start.x, end.y - start.y) < 1) return
    void env.run('image.analysis.rulerTool', { start: [start.x, start.y], end: [end.x, end.y] }, NO_REPAINT)
  })
}

/** The Note and Count tools: a click adds a note (with the bar's author and colour) or a count mark there. */
export function markTool(id: 'Note' | 'Count', env: PhotoToolEnv): KitTool {
  const command = id === 'Note' ? 'notes.add' : 'count.add'
  return {
    id,
    cursor: 'crosshair',
    begin: (pointer) => void env.run(command, { x: Math.round(pointer.x), y: Math.round(pointer.y), ...toolParams(id, env.options) }, NO_REPAINT),
  }
}
