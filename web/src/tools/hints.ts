import type { PhotoToolId } from './catalog.ts'

/** A usage hint: upstream's English text (drawn through `phrase()`) and the key names its `{placeholders}` take. */
export type ToolHint = { text: string; keys?: Readonly<Record<string, string>> }

const MARQUEE: ToolHint = { text: 'Drag to select  ·  {add} add  ·  {sub} subtract  ·  {both} intersect  ·  click to deselect', keys: { add: 'Shift', sub: 'Alt', both: 'Shift+Alt' } }
const LASSO: ToolHint = { text: 'Drag or click polygon points · hold Alt during lasso for straight segments · {add} add · {sub} before drawing subtracts', keys: { add: 'Shift', sub: 'Alt' } }
const TYPE: ToolHint = { text: 'Click to add text' }

/**
 * Each tool's usage hint, as upstream's options bar gives it (`panels.rs` `options_bar`, its
 * `hint(...)` calls); the status bar shows the active tool's. Upstream writes the marquee's
 * without a translation; Photo's phrases give it one.
 */
export const TOOL_HINTS: Readonly<Partial<Record<PhotoToolId, ToolHint>>> = {
  RectMarquee: MARQUEE,
  EllipseMarquee: MARQUEE,
  Move: { text: 'Drag to move the active layer' },
  Eyedropper: { text: 'Click to sample the foreground colour  ·  {key}-click for background', keys: { key: 'Alt' } },
  Zoom: { text: 'Click to zoom in  ·  {key}-click to zoom out  ·  drag right/left to zoom in/out', keys: { key: 'Alt' } },
  Hand: { text: 'Drag to pan  ·  hold Space with any tool' },
  Lasso: LASSO,
  PolygonLasso: LASSO,
  MagneticLasso: { text: 'Click, then move along an edge · click to fasten a point · {alt}-click for a straight segment · double-click to close', keys: { alt: 'Alt' } },
  Crop: {
    text: 'Drag a crop box · drag inside to move · edges resize ({ratio} ratio, {centre} centre) · Space moves while drawing · {commit} commits · Esc cancels',
    keys: { ratio: 'Shift', centre: 'Alt', commit: 'Enter' },
  },
  Gradient: { text: 'Drag to draw a gradient' },
  PaintBucket: { text: 'Click to fill similar colours' },
  Type: TYPE,
  VerticalType: TYPE,
}
