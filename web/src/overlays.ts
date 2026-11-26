import { useCallback, useEffect, useRef, useState, type RefObject } from 'react'
import type { KitChange, KitEngine, RasterDecoration } from '@kition/sdk/editor'
import { accent, drawCropProposal, drawReadout, drawSelectionAnts, drawTransformBox } from './tools/canvas/common'
import type { CropBox, Point, Quad } from './tools/canvas/geometry'
import { drawGrid, drawGuides, drawPixelGrid, type GuideDraft } from './guides/draw'
import { drawSnapLines, NO_GUIDES, type Box, type Guides, type SnapLine } from './guides/snap'
import { DEFAULT_VIEW, gridSpacing, shown, type ViewSettings } from './guides/view'
import { drawPathEditing } from './paths/draw'
import type { PathView } from './paths/store'

/**
 * What the editor marks on the canvas besides the selection and the work path (plan 2026-11-24,
 * stage 2): the Crop tool's frame, Free Transform's box (or Show Transform Controls', or the Move
 * tool's thin mark of the selected layer), and whether the selection's ants hide while a gesture
 * draws them moved. Kept in a ref the canvas reads on every frame, so a drag redraws it without
 * rendering the editor.
 */
export type CanvasMarks = {
  crop: { frame: CropBox; thirds: boolean } | null
  transform: { quad: Quad; handles: boolean; reference: Point | null } | null
  hideAnts: boolean
}

export const NO_MARKS: CanvasMarks = { crop: null, transform: null, hideAnts: false }

/**
 * View's extras (plan 2026-11-24, stage 7), read on every frame: the guides (and one dragged), the
 * grid and pixel grid, the paths the path tools edit, the active layer's edges, a snap's lines,
 * and which of them View › Show shows.
 */
export type CanvasExtras = {
  settings: ViewSettings
  guides: Guides
  guideDraft: GuideDraft | null
  resolution: number
  /** The path tools' paths (read as the frame is drawn), or null while no path tool is chosen. */
  paths: { get: () => PathView } | null
  tool: string
  layerEdges: Box | null
  snapLines: readonly SnapLine[]
}

export const NO_EXTRAS: CanvasExtras = { settings: DEFAULT_VIEW, guides: NO_GUIDES, guideDraft: null, resolution: 72, paths: null, tool: '', layerEdges: null, snapLines: [] }

/**
 * What Photo draws over the image: the selection's marching ants, traced by the engine at the
 * current zoom (its `selectionOutline` extra, also kept in `outline` for the selection tools), the
 * paths the path tools edit, the marks (`CanvasMarks`) and View's extras (`CanvasExtras`).
 */
/** `version` changes with the extras (a View setting, the guides, the tool), so the canvas draws them again. */
export function usePhotoOverlays(doc: KitEngine | null, change: KitChange, hasSelection: boolean, zoom: number | null, marks: RefObject<CanvasMarks>, extras: RefObject<CanvasExtras>, version = '') {
  const [segments, setSegments] = useState<Int32Array | null>(null)
  const outline = useRef<Int32Array | null>(null)
  outline.current = segments
  // Holds (Quick Selection's stroke): the ants stay while the selection is between two of its steps.
  const held = useRef(0)
  const [released, setReleased] = useState(0)
  const hold = useCallback(() => {
    held.current += 1
    let done = false
    return () => {
      if (done) return
      done = true
      held.current -= 1
      if (!held.current) setReleased((count) => count + 1)
    }
  }, [])
  // Trace at display resolution: one sample per screen pixel when zoomed out.
  const step = zoom && zoom < 1 ? Math.max(1, Math.floor(1 / zoom)) : 1
  useEffect(() => {
    if (!doc || !hasSelection) {
      if (!held.current) setSegments(null)
      return undefined
    }
    let current = true
    doc.invoke<Int32Array>('selectionOutline', 0, 0, doc.width, doc.height, step).then(
      (next) => {
        if (current) setSegments(next.length ? next : null)
      },
      () => undefined,
    )
    return () => {
      current = false
    }
  }, [change.revision, doc, hasSelection, step, released])
  const decorate = useCallback<RasterDecoration>(
    (context, toScreen) => {
      const now = marks.current ?? NO_MARKS
      const more = extras.current ?? NO_EXTRAS
      const size = doc ? { width: doc.width, height: doc.height } : null
      if (size && shown(more.settings, 'pixelGrid')) drawPixelGrid(context, toScreen, size)
      if (size && shown(more.settings, 'grid')) drawGrid(context, toScreen, size, gridSpacing(more.resolution))
      if (more.paths && shown(more.settings, 'targetPath')) drawPathEditing(context, toScreen, more.paths.get(), more.tool)
      if (segments && !now.hideAnts && shown(more.settings, 'selectionEdges')) drawSelectionAnts(segments, context, toScreen)
      if (more.layerEdges && shown(more.settings, 'layerEdges')) {
        const a = toScreen(more.layerEdges.x0, more.layerEdges.y0)
        const b = toScreen(more.layerEdges.x1, more.layerEdges.y1)
        context.setLineDash([])
        context.lineWidth = 1
        context.strokeStyle = accent()
        context.strokeRect(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5, Math.round(b.x - a.x), Math.round(b.y - a.y))
      }
      if (now.crop && doc) drawCropProposal(now.crop.frame, context, toScreen, { width: doc.width, height: doc.height }, now.crop.thirds)
      if (now.transform) drawTransformBox(now.transform.quad, context, toScreen, now.transform.handles, now.transform.reference)
      if (shown(more.settings, 'guides') || more.guideDraft) drawGuides(context, toScreen, shown(more.settings, 'guides') ? more.guides : NO_GUIDES, more.guideDraft, more.settings.lockGuides)
      if (more.guideDraft) {
        const draft = more.guideDraft
        const p = toScreen(draft.position, draft.position)
        drawReadout(context, `${draft.orientation === 'vertical' ? 'X' : 'Y'}: ${Math.round(draft.position * 10) / 10} px`, draft.orientation === 'vertical' ? { x: p.x, y: 4 } : { x: 4, y: p.y })
      }
      drawSnapLines(context, toScreen, more.snapLines)
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `version` stands for what `extras` holds
    [doc, marks, extras, segments, version],
  )
  return { decorate, animated: Boolean(segments), outline, hold }
}
