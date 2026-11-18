import { useCallback, useEffect, useState } from 'react'
import type { KitChange, KitEngine, RasterDecoration, RasterRect } from '@kition/kit-editor'
import { drawCropProposal, drawPathOutline, drawSelectionAnts, drawTransformBox, type PathData } from './tools/canvas'

export type CropArea = { x: number; y: number; width: number; height: number }
/** A proposed crop, and whether it shows the rule-of-thirds lines. */
export type CropProposal = { area: CropArea; thirds: boolean }

/**
 * What Photo draws over the image: the selection's marching ants, traced by the engine at the
 * current zoom (its `selectionOutline` extra), the work path while a path tool is active, a
 * proposed crop, and the Move tool's transform box (Show Transform Controls).
 */
export function usePhotoOverlays(doc: KitEngine | null, change: KitChange, hasSelection: boolean, zoom: number | null, crop: CropProposal | null, showPath: boolean, transformBox: RasterRect | null) {
  const [segments, setSegments] = useState<Int32Array | null>(null)
  const [path, setPath] = useState<PathData | null>(null)
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
      if (path) drawPathOutline(path, context, toScreen)
      if (segments) drawSelectionAnts(segments, context, toScreen)
      if (crop && doc) drawCropProposal(crop.area, context, toScreen, { width: doc.width, height: doc.height }, crop.thirds)
      if (transformBox) drawTransformBox(transformBox, context, toScreen)
    },
    [crop, doc, path, segments, transformBox],
  )
  return { decorate: segments || crop || path || transformBox ? decorate : null, animated: Boolean(segments) }
}
