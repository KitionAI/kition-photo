/**
 * The canvas gestures' arithmetic (plan 2026-11-24, stage 2), kept pure so `node --test` checks it:
 * the brush keys' steps, the number keys' opacity, Shift's constraints, the nudge step, the wheel's
 * zoom step, the Free Transform box's handles and drags, and the Crop box's.
 */

export type Point = { x: number; y: number }
/** A transform box's four corners, clockwise from the top-left (the engine's `quad`). */
export type Quad = [Point, Point, Point, Point]

// ------------------------------------------------------------------ brush keys

/** The sizes `[` and `]` step through, as Photoshop's: 1–10 by 1, to 50 by 5, to 100 by 10, then 125 … 300, then by 100 to 5000. */
export const BRUSH_SIZES: readonly number[] = [
  ...Array.from({ length: 10 }, (_, index) => index + 1),
  ...Array.from({ length: 8 }, (_, index) => 15 + index * 5),
  60, 70, 80, 90, 100, 125, 150, 175, 200, 250, 300,
  ...Array.from({ length: 47 }, (_, index) => 400 + index * 100),
]

/** The brush size after `]` (`direction` 1) or `[` (-1): the next size of the table past `size`; a size between two snaps to the table. */
export function brushSizeStep(size: number, direction: 1 | -1): number {
  if (direction > 0) return BRUSH_SIZES.find((step) => step > size + 1e-9) ?? BRUSH_SIZES[BRUSH_SIZES.length - 1]
  for (let index = BRUSH_SIZES.length - 1; index >= 0; index -= 1) if (BRUSH_SIZES[index] < size - 1e-9) return BRUSH_SIZES[index]
  return BRUSH_SIZES[0]
}

/** Shift+`]` and Shift+`[`: the hardness rounded to a quarter, then one quarter up or down, within 0–100 %. */
export function hardnessStep(hardness: number, direction: 1 | -1): number {
  const quarter = Math.round(hardness / 25) * 25
  return Math.min(100, Math.max(0, quarter + direction * 25))
}

/** How long after one number key a second one still makes a two-digit value (4 then 5 = 45 %). */
export const DIGIT_PAIR_MS = 700

export type DigitEntry = { digit: number; at: number } | null

/**
 * A number key's percentage, as Photoshop's: 1 is 10 % … 9 is 90 %, 0 is 100 %; two keys typed
 * quickly give the exact value (4, 5: 45 %; 0, 0: 100 %). `entry` is the key before (null when
 * none is waiting); the answer says what waits for the next key.
 */
export function digitPercent(entry: DigitEntry, digit: number, now: number): { percent: number; entry: DigitEntry } {
  if (entry && now - entry.at <= DIGIT_PAIR_MS) {
    const value = entry.digit * 10 + digit
    return { percent: value === 0 ? 100 : value, entry: null }
  }
  return { percent: digit === 0 ? 100 : digit * 10, entry: { digit, at: now } }
}

// ------------------------------------------------------------------ constraints

/** How far (screen pixels) a Shift-drag runs before its direction locks. */
export const LOCK_AFTER_PX = 3

/**
 * A Shift brush stroke: once the pointer has run `LOCK_AFTER_PX` screen pixels from `origin`, the
 * stroke keeps to the axis it ran further along; until then it stays at the origin.
 */
export function lockAxis(origin: Point, pointer: Point, zoom: number, locked: 'h' | 'v' | null): { point: Point; locked: 'h' | 'v' | null } {
  const dx = pointer.x - origin.x
  const dy = pointer.y - origin.y
  let axis = locked
  if (!axis) {
    if (Math.hypot(dx, dy) * zoom < LOCK_AFTER_PX) return { point: { ...origin }, locked: null }
    axis = Math.abs(dx) >= Math.abs(dy) ? 'h' : 'v'
  }
  return { point: axis === 'h' ? { x: pointer.x, y: origin.y } : { x: origin.x, y: pointer.y }, locked: axis }
}

/** A Shift move: the offset held to the nearest of eight directions (the axes and the diagonals). */
export function constrainEight(dx: number, dy: number): Point {
  if (!dx && !dy) return { x: 0, y: 0 }
  const octant = Math.round(Math.atan2(dy, dx) / (Math.PI / 4))
  const angle = (octant * Math.PI) / 4
  // Project onto the direction: an axis keeps the run along it, a diagonal the mean of both.
  const length = dx * Math.cos(angle) + dy * Math.sin(angle)
  const x = Math.round(length * Math.cos(angle))
  const y = Math.round(length * Math.sin(angle))
  return { x: Object.is(x, -0) ? 0 : x, y: Object.is(y, -0) ? 0 : y }
}

/** A point held to the nearest 45° from `from` (the Polygonal Lasso's Shift). */
export function snap45(from: Point, to: Point): Point {
  const dx = to.x - from.x
  const dy = to.y - from.y
  const length = Math.hypot(dx, dy)
  if (!length) return { ...to }
  const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4)
  return { x: from.x + length * Math.cos(angle), y: from.y + length * Math.sin(angle) }
}

/** An arrow key's nudge in document pixels: one screen pixel (at least one document pixel), or 10 with Shift. */
export function nudgeStep(zoom: number, shift: boolean): number {
  if (shift) return 10
  return zoom > 0 ? Math.max(1, Math.floor(1 / zoom)) : 1
}

/** A marquee drawn from its centre (Alt after the press): the box `start` ± the drag, squared with `square`. */
export function centredBox(start: Point, pointer: Point, square: boolean): { x: number; y: number; width: number; height: number } {
  let dx = Math.abs(pointer.x - start.x)
  let dy = Math.abs(pointer.y - start.y)
  if (square) dx = dy = Math.max(dx, dy)
  const x0 = Math.round(start.x - dx)
  const y0 = Math.round(start.y - dy)
  return { x: x0, y: y0, width: Math.round(start.x + dx) - x0, height: Math.round(start.y + dy) - y0 }
}

// ------------------------------------------------------------------ the wheel

/** The most a wheel notch zooms by (Photoshop's step is about this). */
export const WHEEL_NOTCH_ZOOM = 1.2
const WHEEL_SPEED = 0.01

/** A wheel event at least this many pixels (or in lines or pages) is a mouse notch; smaller ones are a trackpad's. */
export const WHEEL_NOTCH_PX = 50

/**
 * The zoom factor of one wheel event with Cmd, Ctrl or Alt: a trackpad's small deltas (and a
 * pinch's) zoom as before, e^(−delta/100); a mouse notch (about 100 pixels, or lines) ×1.2 or ÷1.2.
 */
export function wheelZoomFactor(deltaY: number, deltaMode = 0): number {
  if (!deltaY) return 1
  if (deltaMode !== 0 || Math.abs(deltaY) >= WHEEL_NOTCH_PX) return deltaY < 0 ? WHEEL_NOTCH_ZOOM : 1 / WHEEL_NOTCH_ZOOM
  return Math.exp(-deltaY * WHEEL_SPEED)
}

// ------------------------------------------------------------------ selection outlines

/** Whether document point (`x`, `y`) is inside a selection outline traced as segments `[ax, ay, bx, by, …]` (even–odd). */
export function insideOutline(segments: ArrayLike<number>, x: number, y: number): boolean {
  let inside = false
  for (let index = 0; index + 3 < segments.length; index += 4) {
    const ax = segments[index], ay = segments[index + 1], bx = segments[index + 2], by = segments[index + 3]
    if (ay > y !== by > y && x < ax + ((y - ay) * (bx - ax)) / (by - ay)) inside = !inside
  }
  return inside
}

// ------------------------------------------------------------------ the transform box

export const rectQuad = (x0: number, y0: number, x1: number, y1: number): Quad => [
  { x: x0, y: y0 },
  { x: x1, y: y0 },
  { x: x1, y: y1 },
  { x: x0, y: y1 },
]

export const quadCentre = (quad: Quad): Point => ({ x: (quad[0].x + quad[1].x + quad[2].x + quad[3].x) / 4, y: (quad[0].y + quad[1].y + quad[2].y + quad[3].y) / 4 })

/** A handle of a box: its eight (corners 0–3 clockwise from the top-left, edges `n e s w`), the centre (reference) point, inside, or outside (rotate). */
export type Handle = 'nw' | 'ne' | 'se' | 'sw' | 'n' | 'e' | 's' | 'w' | 'centre' | 'inside' | 'rotate'

/** Each handle's place in the box's own square (0–1 across, 0–1 down). */
export const HANDLE_AT: Readonly<Record<Exclude<Handle, 'centre' | 'inside' | 'rotate'>, readonly [number, number]>> = {
  nw: [0, 0], n: [0.5, 0], ne: [1, 0], e: [1, 0.5], se: [1, 1], s: [0.5, 1], sw: [0, 1], w: [0, 0.5],
}
export const CORNERS = ['nw', 'ne', 'se', 'sw'] as const
export const EDGES = ['n', 'e', 's', 'w'] as const

/** A point of the box at (`s`, `t`) of its own square, bilinear between the corners (exact for a parallelogram). */
export function quadPoint(quad: Quad, s: number, t: number): Point {
  const top = { x: quad[0].x + (quad[1].x - quad[0].x) * s, y: quad[0].y + (quad[1].y - quad[0].y) * s }
  const bottom = { x: quad[3].x + (quad[2].x - quad[3].x) * s, y: quad[3].y + (quad[2].y - quad[3].y) * s }
  return { x: top.x + (bottom.x - top.x) * t, y: top.y + (bottom.y - top.y) * t }
}

/** Whether `p` is inside the box (any convex quad, either winding). */
export function insideQuad(quad: Quad, p: Point): boolean {
  let sign = 0
  for (let index = 0; index < 4; index += 1) {
    const a = quad[index], b = quad[(index + 1) % 4]
    const cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
    if (Math.abs(cross) < 1e-12) continue
    const side = Math.sign(cross)
    if (sign && side !== sign) return false
    sign = side
  }
  return true
}

/** How near (screen pixels) the pointer must be to grab a handle. */
export const HANDLE_REACH = 7

/**
 * The handle under screen point `p` of a box drawn at screen corners `screen`: a handle within
 * `HANDLE_REACH`, the reference point, inside, or outside (rotate; `rotates` false: none).
 */
export function handleAt(screen: Quad, centre: Point | null, p: Point, rotates = true): Handle | null {
  let best: Handle | null = null
  let nearest = HANDLE_REACH
  for (const [handle, [s, t]] of Object.entries(HANDLE_AT) as [Handle, readonly [number, number]][]) {
    const at = quadPoint(screen, s, t)
    const distance = Math.hypot(at.x - p.x, at.y - p.y)
    if (distance <= nearest) {
      nearest = distance
      best = handle
    }
  }
  if (best) return best
  if (centre && Math.hypot(centre.x - p.x, centre.y - p.y) <= HANDLE_REACH) return 'centre'
  if (insideQuad(screen, p)) return 'inside'
  return rotates ? 'rotate' : null
}

/** The cursor for a handle of a box turned by `angle` radians (the resize arrows turn with it). */
export function handleCursor(handle: Handle, angle: number): string {
  if (handle === 'inside') return 'move'
  if (handle === 'rotate') return 'alias'
  if (handle === 'centre') return 'crosshair'
  const [s, t] = HANDLE_AT[handle]
  const direction = Math.atan2(t - 0.5, s - 0.5) + angle
  const octant = ((Math.round(direction / (Math.PI / 4)) % 8) + 8) % 8
  return ['ew-resize', 'nwse-resize', 'ns-resize', 'nesw-resize'][octant % 4]
}

type Affine = { a: number; b: number; c: number; d: number; e: number; f: number }
const apply = (m: Affine, p: Point): Point => ({ x: m.a * p.x + m.c * p.y + m.e, y: m.b * p.x + m.d * p.y + m.f })

/** The box's frame: its own square (0–1) to the document, by its top-left, top-right and bottom-left corners. */
function frameOf(quad: Quad): Affine {
  return { a: quad[1].x - quad[0].x, b: quad[1].y - quad[0].y, c: quad[3].x - quad[0].x, d: quad[3].y - quad[0].y, e: quad[0].x, f: quad[0].y }
}

function invert(m: Affine): Affine | null {
  const det = m.a * m.d - m.b * m.c
  if (Math.abs(det) < 1e-12) return null
  return { a: m.d / det, b: -m.b / det, c: -m.c / det, d: m.a / det, e: (m.c * m.f - m.d * m.e) / det, f: (m.b * m.e - m.a * m.f) / det }
}

/** The box's turn, in radians: the angle of its top edge. */
export const quadAngle = (quad: Quad) => Math.atan2(quad[1].y - quad[0].y, quad[1].x - quad[0].x)

export const rotatePoint = (p: Point, about: Point, angle: number): Point => {
  const cos = Math.cos(angle), sin = Math.sin(angle)
  const dx = p.x - about.x, dy = p.y - about.y
  return { x: about.x + dx * cos - dy * sin, y: about.y + dx * sin + dy * cos }
}

export type TransformKeys = { shift: boolean; alt: boolean; meta: boolean }

/**
 * Free Transform: the box after dragging `handle` from `from` to `to` (document points), starting
 * from box `start` with reference point `reference`, as Photoshop's classic transform: a corner or
 * edge scales (Shift keeps the proportions, Alt about the reference point), Cmd on a corner moves
 * it alone (distort) and on an edge slides it along itself (skew); inside moves (Shift: eight
 * directions); outside rotates about the reference point (Shift: 15° steps).
 */
export function dragTransform(start: Quad, reference: Point, handle: Handle, from: Point, to: Point, keys: TransformKeys): Quad {
  const dx = to.x - from.x
  const dy = to.y - from.y
  if (handle === 'inside') {
    const step = keys.shift ? constrainEight(dx, dy) : { x: dx, y: dy }
    return start.map((p) => ({ x: p.x + step.x, y: p.y + step.y })) as Quad
  }
  if (handle === 'rotate') {
    let turn = Math.atan2(to.y - reference.y, to.x - reference.x) - Math.atan2(from.y - reference.y, from.x - reference.x)
    if (keys.shift) {
      const now = quadAngle(start)
      const step = Math.PI / 12
      turn = Math.round((now + turn) / step) * step - now
    }
    return start.map((p) => rotatePoint(p, reference, turn)) as Quad
  }
  if (handle === 'centre') return start
  const corner = (CORNERS as readonly string[]).indexOf(handle)
  if (keys.meta && corner >= 0) {
    const out = start.map((p) => ({ ...p })) as Quad
    out[corner] = { x: start[corner].x + dx, y: start[corner].y + dy }
    return out
  }
  if (keys.meta) {
    // Skew: the edge's two corners slide along the edge.
    const edge = (EDGES as readonly string[]).indexOf(handle)
    const a = edge, b = (edge + 1) % 4
    const along = { x: start[b].x - start[a].x, y: start[b].y - start[a].y }
    const length = Math.hypot(along.x, along.y) || 1
    const run = (dx * along.x + dy * along.y) / length
    const shift = { x: (along.x / length) * run, y: (along.y / length) * run }
    const out = start.map((p) => ({ ...p })) as Quad
    out[a] = { x: start[a].x + shift.x, y: start[a].y + shift.y }
    out[b] = { x: start[b].x + shift.x, y: start[b].y + shift.y }
    return out
  }
  // Scale in the box's own square: the handle follows the pointer, the anchor stays.
  const frame = frameOf(start)
  const back = invert(frame)
  if (!back) return start
  const [hs, ht] = HANDLE_AT[handle as keyof typeof HANDLE_AT]
  const anchor = keys.alt ? apply(back, reference) : { x: 1 - hs, y: 1 - ht }
  const local = apply(back, { x: from.x + dx + (quadPoint(start, hs, ht).x - from.x), y: from.y + dy + (quadPoint(start, hs, ht).y - from.y) })
  let sx = hs === 0.5 ? 1 : (local.x - anchor.x) / (hs - anchor.x || 1)
  let sy = ht === 0.5 ? 1 : (local.y - anchor.y) / (ht - anchor.y || 1)
  if (keys.shift) {
    if (hs === 0.5) sx = sy
    else if (ht === 0.5) sy = sx
    else {
      const most = Math.max(Math.abs(sx), Math.abs(sy))
      sx = Math.sign(sx || 1) * most
      sy = Math.sign(sy || 1) * most
    }
  }
  // Never collapse the box to a line.
  const least = 1e-3
  if (Math.abs(sx) < least) sx = Math.sign(sx || 1) * least
  if (Math.abs(sy) < least) sy = Math.sign(sy || 1) * least
  return start.map((p) => {
    const q = apply(back, p)
    return apply(frame, { x: anchor.x + (q.x - anchor.x) * sx, y: anchor.y + (q.y - anchor.y) * sy })
  }) as Quad
}

/** The options bar's numbers for a box over source `rect` [x0, y0, x1, y1]: its reference point, its width and height in % of the source's, and its turn in degrees. */
export function transformFields(quad: Quad, rect: readonly [number, number, number, number], reference: Point) {
  const width = Math.hypot(quad[1].x - quad[0].x, quad[1].y - quad[0].y)
  const height = Math.hypot(quad[3].x - quad[0].x, quad[3].y - quad[0].y)
  const degrees = (quadAngle(quad) * 180) / Math.PI
  return {
    x: reference.x,
    y: reference.y,
    w: (width / Math.max(1e-9, rect[2] - rect[0])) * 100,
    h: (height / Math.max(1e-9, rect[3] - rect[1])) * 100,
    angle: Math.abs(degrees) < 1e-9 ? 0 : degrees,
  }
}

/** The box the options bar's numbers describe: the source scaled by `w` and `h` %, turned by `angle`°, its centre at (`x`, `y`). */
export function quadFromFields(rect: readonly [number, number, number, number], fields: { x: number; y: number; w: number; h: number; angle: number }): Quad {
  const halfW = ((rect[2] - rect[0]) * fields.w) / 200
  const halfH = ((rect[3] - rect[1]) * fields.h) / 200
  const centre = { x: fields.x, y: fields.y }
  const turn = (fields.angle * Math.PI) / 180
  return [
    { x: centre.x - halfW, y: centre.y - halfH },
    { x: centre.x + halfW, y: centre.y - halfH },
    { x: centre.x + halfW, y: centre.y + halfH },
    { x: centre.x - halfW, y: centre.y + halfH },
  ].map((p) => rotatePoint(p, centre, turn)) as Quad
}

/** Whether a box is still its source rectangle (an untouched transform commits nothing). */
export function sameQuad(a: Quad, b: Quad, tolerance = 1e-6): boolean {
  return a.every((p, index) => Math.abs(p.x - b[index].x) <= tolerance && Math.abs(p.y - b[index].y) <= tolerance)
}

// ------------------------------------------------------------------ the crop box

/** A crop frame: a document rectangle turned clockwise by `angle` degrees about its centre (the engine's `image.crop`). */
export type CropBox = { x: number; y: number; width: number; height: number; angle: number }

/** The crop frame's corners, clockwise from its top-left. */
export function cropQuad(box: CropBox): Quad {
  const centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
  const turn = (box.angle * Math.PI) / 180
  return rectQuad(box.x, box.y, box.x + box.width, box.y + box.height).map((p) => rotatePoint(p, centre, turn)) as Quad
}

/**
 * The crop frame after dragging `handle` from `from` to `to` (document points): an edge or corner
 * moves in the frame's own turned axes (held to `ratio` [w, h] when given; Shift keeps the frame's
 * own proportions), inside moves it, outside turns it (Shift: 15° steps). Sides stay at least a pixel.
 */
export function dragCrop(start: CropBox, handle: Handle, from: Point, to: Point, keys: { shift: boolean }, ratio: readonly [number, number] | null): CropBox {
  const centre = { x: start.x + start.width / 2, y: start.y + start.height / 2 }
  if (handle === 'inside') return { ...start, x: start.x + (to.x - from.x), y: start.y + (to.y - from.y) }
  if (handle === 'rotate') {
    let angle = start.angle + ((Math.atan2(to.y - centre.y, to.x - centre.x) - Math.atan2(from.y - centre.y, from.x - centre.x)) * 180) / Math.PI
    if (keys.shift) angle = Math.round(angle / 15) * 15
    angle = ((((angle + 180) % 360) + 360) % 360) - 180
    return { ...start, angle }
  }
  if (handle === 'centre') return start
  const turn = (start.angle * Math.PI) / 180
  // The pointer's run in the frame's own (unturned) axes.
  const run = rotatePoint({ x: to.x - from.x, y: to.y - from.y }, { x: 0, y: 0 }, -turn)
  const [hs, ht] = HANDLE_AT[handle as keyof typeof HANDLE_AT]
  let x0 = 0, y0 = 0, x1 = start.width, y1 = start.height
  if (hs === 0) x0 += run.x
  if (hs === 1) x1 += run.x
  if (ht === 0) y0 += run.y
  if (ht === 1) y1 += run.y
  if (x1 - x0 < 1) {
    if (hs === 0) x0 = x1 - 1
    else x1 = x0 + 1
  }
  if (y1 - y0 < 1) {
    if (ht === 0) y0 = y1 - 1
    else y1 = y0 + 1
  }
  const held = ratio ?? (keys.shift ? ([start.width, start.height] as const) : null)
  if (held && held[0] > 0 && held[1] > 0) {
    const want = held[0] / held[1]
    let width = x1 - x0, height = y1 - y0
    // A side handle sets one side; a corner the larger of the two.
    if (ht === 0.5 || (hs !== 0.5 && width / want >= height)) height = width / want
    else width = height * want
    if (hs === 0) x0 = x1 - width
    else if (hs === 1) x1 = x0 + width
    else {
      x0 = (start.width - width) / 2
      x1 = x0 + width
    }
    if (ht === 0) y0 = y1 - height
    else if (ht === 1) y1 = y0 + height
    else {
      y0 = (start.height - height) / 2
      y1 = y0 + height
    }
  }
  // Back to the document: the frame's new centre, turned with it.
  const local = { x: (x0 + x1) / 2 - start.width / 2, y: (y0 + y1) / 2 - start.height / 2 }
  const moved = rotatePoint(local, { x: 0, y: 0 }, turn)
  const nextCentre = { x: centre.x + moved.x, y: centre.y + moved.y }
  const width = x1 - x0, height = y1 - y0
  return { x: nextCentre.x - width / 2, y: nextCentre.y - height / 2, width, height, angle: start.angle }
}

/** The crop frame in whole pixels, as `image.crop` takes it. */
export function cropParams(box: CropBox) {
  const width = Math.max(1, Math.round(box.width))
  const height = Math.max(1, Math.round(box.height))
  const x = Math.round(box.x + box.width / 2 - width / 2)
  const y = Math.round(box.y + box.height / 2 - height / 2)
  return { x, y, width, height, ...(Math.abs(box.angle) > 1e-6 ? { angle: Math.round(box.angle * 100) / 100 } : {}) }
}
