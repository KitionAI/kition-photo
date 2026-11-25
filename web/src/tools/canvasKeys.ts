import type { PhotoToolId } from './catalog'
import { TOOL_FIELDS, type OptionKey } from './options'
import { livePreview, type LivePreview } from './canvas/live'
import type { Preview } from '../dialogs/preview'

/**
 * What a number key sets with `tool` active (plan 2026-11-24, stage 2), as Photoshop's: a painting
 * tool's Opacity (with Shift its Flow, where it has one), the fills' opacity, the Background
 * Eraser's Tolerance; tools with an exposure or strength of their own take none; every other tool
 * the active layer's Opacity (with Shift its Fill).
 */
export function digitTarget(tool: PhotoToolId, shift: boolean): { option: OptionKey } | { layer: 'opacity' | 'fill' } | null {
  const fields = TOOL_FIELDS[tool]
  if (tool === 'Gradient' || tool === 'PaintBucket') return { option: 'fillOpacity' }
  if (tool === 'MagicEraser') return { option: 'eraserOpacity' }
  if (tool === 'BackgroundEraser') return { option: 'bgTolerance' }
  if (fields.includes('opacity')) return { option: shift && fields.includes('flow') ? 'flow' : 'opacity' }
  if (fields.includes('size')) return null
  return { layer: shift ? 'fill' : 'opacity' }
}

/** How long (ms) after the last arrow key the nudges so far close as one history step. */
export const NUDGE_BUNDLE_MS = 1000

/**
 * Arrow-key nudges, bundled as Photoshop's: presses of the same thing that follow each other within
 * a second are one history step (each press previews the whole offset so far, taking the previous
 * press's step away; a step someone else made meanwhile is never taken away, `createPreview`).
 */
export class Nudges {
  private open: { kind: string; total: { x: number; y: number }; params: Record<string, unknown>; live: LivePreview; timer: ReturnType<typeof setTimeout> | null } | null = null

  constructor(
    private readonly preview: (command: string) => Preview,
    private readonly failed: () => void,
  ) {}

  /** Nudges `kind` (its command takes `dx`, `dy`, plus `extra`) by (`dx`, `dy`). */
  nudge(kind: string, command: string, dx: number, dy: number, extra: Record<string, unknown> = {}) {
    if (this.open && this.open.kind !== kind) this.close()
    const open = (this.open ??= { kind, total: { x: 0, y: 0 }, params: {}, live: livePreview(this.preview(command), this.failed), timer: null })
    open.total = { x: open.total.x + dx, y: open.total.y + dy }
    open.params = { dx: open.total.x, dy: open.total.y, ...extra }
    open.live.push(open.params)
    if (open.timer) clearTimeout(open.timer)
    open.timer = setTimeout(() => this.close(), NUDGE_BUNDLE_MS)
  }

  /** Closes the bundle: its last preview stays as the one step. */
  close() {
    const open = this.open
    this.open = null
    if (!open) return
    if (open.timer) clearTimeout(open.timer)
    void open.live.finish(open.params)
  }
}
