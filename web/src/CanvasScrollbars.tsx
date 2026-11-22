import { useSyncExternalStore, type RefObject } from 'react'
import { useKitEditor, type KitCanvasHandle, type KitCanvasView, type RasterRect } from '@kition/sdk/editor'
import { Scrollbar } from '@kition/tessera/ui'

/** The canvas's view as the scrollbars read it (the editor's live view): a scroll renders them, not the editor around them. */
export type ViewStore = { get: () => KitCanvasView | null; subscribe: (listener: () => void) => () => void }

/**
 * The canvas's scrollbars, over its right and bottom edges. Each is there only while the content
 * (`content`, in document pixels at the view's zoom) is longer than the view along its axis; it
 * shows where the view is in the content and moves it. Rendered inside the canvas (its children).
 */
export function CanvasScrollbars({ views, content, canvas }: {
  views: ViewStore
  content: (view: KitCanvasView) => RasterRect | null
  canvas: RefObject<KitCanvasHandle | null>
}) {
  const { t } = useKitEditor()
  const view = useSyncExternalStore(views.subscribe, views.get)
  const area = view && view.zoom > 0 ? content(view) : null
  if (!view || !area) return null
  const pageX = view.width / view.zoom
  const pageY = view.height / view.zoom
  const maxX = area.x1 - area.x0 - pageX
  const maxY = area.y1 - area.y0 - pageY
  // Longer than the view by more than a screen pixel.
  const horizontal = maxX * view.zoom > 1
  const vertical = maxY * view.zoom > 1
  if (!horizontal && !vertical) return null
  return (
    <div className="photo-scrollbars" data-both={horizontal && vertical ? true : undefined} data-testid="photo-scrollbars">
      {vertical ? <Scrollbar label={t('verticalScroll')} value={view.y - area.y0} max={maxY} page={pageY} onChange={(value) => canvas.current?.panTo(view.x, area.y0 + value)} /> : null}
      {horizontal ? <Scrollbar label={t('horizontalScroll')} orientation="horizontal" value={view.x - area.x0} max={maxX} page={pageX} onChange={(value) => canvas.current?.panTo(area.x0 + value, view.y)} /> : null}
    </div>
  )
}
