import { useCallback, useEffect, useRef, useState, type RefObject } from 'react'
import type { KitChange, KitEngine, RasterDecoration } from '@kition/sdk/editor'
import { drawCropProposal, drawPathOutline, drawSelectionAnts, drawTransformBox, type PathData } from './tools/canvas'
import type { CropBox, Point, Quad } from './tools/canvas/geometry'

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
 * What Photo draws over the image: the selection's marching ants, traced by the engine at the
 * current zoom (its `selectionOutline` extra, also kept in `outline` for the selection tools), the
 * work path while a path tool is active, and the marks (`CanvasMarks`).
 */
export function usePhotoOverlays(doc: KitEngine | null, change: KitChange, hasSelection: boolean, zoom: number | null, showPath: boolean, marks: RefObject<CanvasMarks>) {
  const [segments, setSegments] = useState<Int32Array | null>(null)
  const [path, setPath] = useState<PathData | null>(null)
  const outline = useRef<Int32Array | null>(null)
  outline.current = segments
  // Trace at display resolution: one sample per screen pixel when zoomed out.
  const step = zoom && zoom < 1 ? Math.max(1, Math.floor(1 / zoom)) : 1
  useEffect(() => {
    if (!doc || !hasSelection) {
      setSegments(null)
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
  }, [change.revision, doc, hasSelection, step])
  useEffect(() => {
    if (!doc || !showPath) {
      setPath(null)
      return undefined
    }
    let current = true
    // `path.info` fails while there is no work path.
    doc.execute('path.info', { name: 'work' }).then(
      (answer) => {
        if (current) setPath((JSON.parse(answer) as { path: PathData }).path)
      },
      () => {
        if (current) setPath(null)
      },
    )
    return () => {
      current = false
    }
  }, [change.revision, doc, showPath])
  const decorate = useCallback<RasterDecoration>(
    (context, toScreen) => {
      const now = marks.current ?? NO_MARKS
      if (path) drawPathOutline(path, context, toScreen)
      if (segments && !now.hideAnts) drawSelectionAnts(segments, context, toScreen)
      if (now.crop && doc) drawCropProposal(now.crop.frame, context, toScreen, { width: doc.width, height: doc.height }, now.crop.thirds)
      if (now.transform) drawTransformBox(now.transform.quad, context, toScreen, now.transform.handles, now.transform.reference)
    },
    [doc, marks, path, segments],
  )
  return { decorate, animated: Boolean(segments), outline }
}
