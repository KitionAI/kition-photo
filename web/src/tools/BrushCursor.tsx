import { useLive, type Live } from '../panels/live'

/** The pointer over the canvas in the canvas's CSS pixels, or null off it. */
export type CanvasPoint = { x: number; y: number } | null

/** Below this diameter (screen pixels) the outline is too small to read: the crosshair shows instead. */
export const BRUSH_OUTLINE_MIN = 4

/** The brush outline's screen diameter at `zoom`, or null when the crosshair shows instead. */
export function outlineDiameter(size: number, zoom: number | null) {
  const diameter = size * (zoom ?? 1)
  return diameter >= BRUSH_OUTLINE_MIN ? diameter : null
}

/**
 * The brush outline cursor (plan 2026-11-24, stage 2): a circle as wide as the brush lands at the
 * current zoom, a dashed inner ring where a soft brush starts to fade (its hardness), and a centre
 * dot; drawn over the canvas at the pointer (white over black, so it shows on any picture). The
 * editor hides the system cursor while it shows.
 */
export function BrushCursor({ pointer, diameter, hardness }: { pointer: Live<CanvasPoint>; diameter: number; hardness: number }) {
  const at = useLive(pointer)
  if (!at) return null
  const side = Math.ceil(diameter) + 4
  const centre = side / 2
  const radius = diameter / 2
  const inner = (radius * Math.min(100, Math.max(0, hardness))) / 100
  return (
    <svg className="photo-brush-cursor" data-testid="photo-brush-cursor" data-diameter={Math.round(diameter)} data-hardness={hardness} width={side} height={side} style={{ left: at.x - centre, top: at.y - centre }} aria-hidden="true">
      <circle cx={centre} cy={centre} r={radius} fill="none" stroke="rgba(0, 0, 0, 0.75)" strokeWidth={1} />
      <circle cx={centre} cy={centre} r={Math.max(0.5, radius - 1)} fill="none" stroke="rgba(255, 255, 255, 0.9)" strokeWidth={1} />
      {hardness < 100 && inner >= 2 ? <circle cx={centre} cy={centre} r={inner} fill="none" stroke="rgba(255, 255, 255, 0.85)" strokeWidth={1} strokeDasharray="2 3" data-testid="photo-brush-cursor-hardness" /> : null}
      <circle cx={centre} cy={centre} r={1} fill="rgba(255, 255, 255, 0.9)" stroke="rgba(0, 0, 0, 0.6)" strokeWidth={0.5} />
    </svg>
  )
}
