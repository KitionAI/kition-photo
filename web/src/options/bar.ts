import type { PhotoToolId } from '../tools/catalog'
import type { ToolOptions } from '../tools/options'

/** What every tool's options bar gets: the tool, its options and how to change them, and the editor state some bars show or act on. */
export type BarProps = {
  tool: PhotoToolId
  o: ToolOptions
  set: (patch: Partial<ToolOptions>) => void
  foreground: string
  background: string
  setForeground: (hex: string) => void
  /** The active layer: its id and kind (`Pixel`, `Type`, `Shape`…). */
  layer: { id: number | undefined; kind: string } | null
  /** The Crop tool's proposed area: whether there is one, and its commit and cancel. */
  crop: { proposed: boolean; commit: () => void; cancel: () => void }
  /** A draft keeps the Type bar until submission finishes; its commit/cancel actions are only available before submission. */
  text: { editing: boolean; submitting: boolean; commit: () => void; cancel: () => void }
  /** Whether the Clone Stamp's and Healing Brush's source is set. */
  cloneSourceSet: boolean
  /** The view: fit the image on screen, or show it at 100 %. */
  view: { fit: () => void; actual: () => void }
  /** Free Transform's open box, as the bar's numbers (its reference point, W and H in %, the angle), or null. */
  transform?: { fields: TransformFields; set: (patch: Partial<TransformFields>) => void; commit: () => void; cancel: () => void } | null
}

export type TransformFields = { x: number; y: number; w: number; h: number; angle: number }
