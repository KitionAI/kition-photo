/**
 * Paths as the engine's `path.info` gives them (subpaths of knots: an anchor, its in and out
 * direction points), and the geometry the Pen, Direct Selection, Path Selection and Convert Point
 * tools need (plan 2026-11-24, stage 7): hit tests on anchors, direction points and segments, a
 * segment split where it was clicked, an anchor taken out, anchors moved, a direction point
 * dragged with Photoshop's smooth-point rule, a point converted. Pure: `bezier.test.mjs`.
 */

export type Pt = [number, number]
export type PathKnot = { anchor: Pt; in: Pt; out: Pt; smooth?: boolean }
export type PathSubpath = { closed: boolean; knots: PathKnot[]; op?: string }
export type PathData = { subpaths: PathSubpath[]; fillRule?: string; inverted?: boolean }
/** An anchor by its subpath and knot index (the engine's `[subpath, knot]`). */
export type AnchorRef = readonly [number, number]

const lerp = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
const distance = (a: readonly number[], b: readonly number[]) => Math.hypot(a[0] - b[0], a[1] - b[1])
const same = (a: Pt, b: Pt) => a[0] === b[0] && a[1] === b[1]
const copy = (path: PathData): PathData => ({ ...path, subpaths: path.subpaths.map((subpath) => ({ ...subpath, knots: subpath.knots.map((knot) => ({ ...knot, anchor: [...knot.anchor] as Pt, in: [...knot.in] as Pt, out: [...knot.out] as Pt })) })) })

/** The point at `t` on the cubic `p0 p1 p2 p3`. */
export function cubicAt(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: number): Pt {
  const u = 1 - t
  const a = u * u * u, b = 3 * u * u * t, c = 3 * u * t * t, d = t * t * t
  return [a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1]]
}

/** How many segments a subpath has: one between each pair of knots, and one back to the first when closed. */
export function segmentCount(subpath: PathSubpath) {
  const n = subpath.knots.length
  return n < 2 ? 0 : subpath.closed ? n : n - 1
}

/** Segment `index` of a subpath as its four control points (from knot `index` to the next). */
export function segmentPoints(subpath: PathSubpath, index: number): [Pt, Pt, Pt, Pt] {
  const a = subpath.knots[index]
  const b = subpath.knots[(index + 1) % subpath.knots.length]
  return [a.anchor, a.out, b.in, b.anchor]
}

/** The point of a segment nearest `p`: its `t` and how far it is (sampled, then refined). */
export function nearestOnSegment(points: [Pt, Pt, Pt, Pt], p: Pt): { t: number; distance: number } {
  const steps = 32
  let best = { t: 0, distance: Infinity }
  for (let i = 0; i <= steps; i += 1) {
    const t = i / steps
    const d = distance(cubicAt(...points, t), p)
    if (d < best.distance) best = { t, distance: d }
  }
  // Narrow down around the best sample.
  let low = Math.max(0, best.t - 1 / steps)
  let high = Math.min(1, best.t + 1 / steps)
  for (let i = 0; i < 24; i += 1) {
    const a = low + (high - low) / 3
    const b = high - (high - low) / 3
    if (distance(cubicAt(...points, a), p) < distance(cubicAt(...points, b), p)) high = b
    else low = a
  }
  const t = (low + high) / 2
  const d = distance(cubicAt(...points, t), p)
  return d < best.distance ? { t, distance: d } : best
}

/** The anchor nearest `p` within `reach` (document pixels), or null. */
export function hitAnchor(path: PathData, p: Pt, reach: number): AnchorRef | null {
  let best: { at: AnchorRef; distance: number } | null = null
  path.subpaths.forEach((subpath, s) =>
    subpath.knots.forEach((knot, k) => {
      const d = distance(knot.anchor, p)
      if (d <= reach && (!best || d < best.distance)) best = { at: [s, k], distance: d }
    }),
  )
  return (best as { at: AnchorRef } | null)?.at ?? null
}

/** A direction point of one of `anchors` nearest `p` within `reach`, or null (a retracted one is not there to grab). */
export function hitHandle(path: PathData, anchors: readonly AnchorRef[], p: Pt, reach: number): { subpath: number; knot: number; handle: 'in' | 'out' } | null {
  let best: { at: { subpath: number; knot: number; handle: 'in' | 'out' }; distance: number } | null = null
  for (const [s, k] of anchors) {
    const knot = path.subpaths[s]?.knots[k]
    if (!knot) continue
    for (const handle of ['in', 'out'] as const) {
      if (same(knot[handle], knot.anchor)) continue
      const d = distance(knot[handle], p)
      if (d <= reach && (!best || d < best.distance)) best = { at: { subpath: s, knot: k, handle }, distance: d }
    }
  }
  return best?.at ?? null
}

/** The segment nearest `p` within `reach`: its subpath, the knot it leaves, and where (`t`), or null. */
export function hitSegment(path: PathData, p: Pt, reach: number): { subpath: number; knot: number; t: number } | null {
  let best: { at: { subpath: number; knot: number; t: number }; distance: number } | null = null
  path.subpaths.forEach((subpath, s) => {
    for (let k = 0; k < segmentCount(subpath); k += 1) {
      const near = nearestOnSegment(segmentPoints(subpath, k), p)
      if (near.distance <= reach && (!best || near.distance < best.distance)) best = { at: { subpath: s, knot: k, t: near.t }, distance: near.distance }
    }
  })
  return (best as { at: { subpath: number; knot: number; t: number } } | null)?.at ?? null
}

/** The bounds of a subpath's anchors and direction points [x0, y0, x1, y1]. */
export function subpathBounds(subpath: PathSubpath): [number, number, number, number] {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
  for (const knot of subpath.knots)
    for (const [x, y] of [knot.anchor, knot.in, knot.out]) {
      x0 = Math.min(x0, x)
      y0 = Math.min(y0, y)
      x1 = Math.max(x1, x)
      y1 = Math.max(y1, y)
    }
  return [x0, y0, x1, y1]
}

/** The subpath a press at `p` picks (on it, or inside its bounds), or null: Path Selection's hit. */
export function hitSubpath(path: PathData, p: Pt, reach: number): number | null {
  const on = hitSegment(path, p, reach) ?? null
  if (on) return on.subpath
  const anchor = hitAnchor(path, p, reach)
  if (anchor) return anchor[0]
  for (let s = path.subpaths.length - 1; s >= 0; s -= 1) {
    const [x0, y0, x1, y1] = subpathBounds(path.subpaths[s])
    if (p[0] >= x0 - reach && p[0] <= x1 + reach && p[1] >= y0 - reach && p[1] <= y1 + reach) return s
  }
  return null
}

/** The path with a new anchor where segment `knot` of `subpath` is at `t`, the curve's shape kept (de Casteljau). */
export function splitSegment(path: PathData, subpath: number, knot: number, t: number): PathData {
  const out = copy(path)
  const sub = out.subpaths[subpath]
  if (!sub || knot >= segmentCount(sub)) return out
  const next = (knot + 1) % sub.knots.length
  const [p0, p1, p2, p3] = segmentPoints(sub, knot)
  const a = lerp(p0, p1, t), b = lerp(p1, p2, t), c = lerp(p2, p3, t)
  const d = lerp(a, b, t), e = lerp(b, c, t)
  const anchor = lerp(d, e, t)
  sub.knots[knot].out = a
  sub.knots[next].in = c
  const curved = !(same(p0, p1) && same(p2, p3))
  sub.knots.splice(knot + 1, 0, { anchor, in: curved ? d : anchor, out: curved ? e : anchor, smooth: curved })
  return out
}

/** The path without anchor [`subpath`, `knot`]; a subpath left without anchors goes too. */
export function deleteAnchor(path: PathData, subpath: number, knot: number): PathData {
  const out = copy(path)
  const sub = out.subpaths[subpath]
  if (!sub?.knots[knot]) return out
  sub.knots.splice(knot, 1)
  if (sub.knots.length < 2) sub.closed = false
  if (!sub.knots.length) out.subpaths.splice(subpath, 1)
  return out
}

/** The path without `anchors` (Direct Selection's Delete). */
export function deleteAnchors(path: PathData, anchors: readonly AnchorRef[]): PathData {
  // From the last, so the earlier indices still name the same knots.
  return [...anchors].sort((a, b) => b[0] - a[0] || b[1] - a[1]).reduce((next, [s, k]) => deleteAnchor(next, s, k), path)
}

/** The path with `anchors` (and their direction points) moved by (`dx`, `dy`), as `path.moveAnchors`. */
export function moveAnchors(path: PathData, anchors: readonly AnchorRef[], dx: number, dy: number): PathData {
  const out = copy(path)
  for (const [s, k] of anchors) {
    const knot = out.subpaths[s]?.knots[k]
    if (!knot) continue
    for (const key of ['anchor', 'in', 'out'] as const) knot[key] = [knot[key][0] + dx, knot[key][1] + dy]
  }
  return out
}

/** The whole path moved by (`dx`, `dy`) (Path Selection's drag). */
export function movePath(path: PathData, dx: number, dy: number): PathData {
  return moveAnchors(path, path.subpaths.flatMap((subpath, s) => subpath.knots.map((_, k) => [s, k] as const)), dx, dy)
}

/**
 * The path with direction point `handle` of knot [`subpath`, `knot`] at `to`, as `path.moveHandle`:
 * a smooth point turns its other direction point to stay in line (keeping its length), unless
 * `independent` (Alt, the Convert Point tool), which makes it a corner.
 */
export function moveHandle(path: PathData, subpath: number, knot: number, handle: 'in' | 'out', to: Pt, independent: boolean): PathData {
  const out = copy(path)
  const k = out.subpaths[subpath]?.knots[knot]
  if (!k) return out
  k[handle] = [to[0], to[1]]
  const other = handle === 'in' ? 'out' : 'in'
  if (independent) k.smooth = false
  else if (k.smooth) {
    const length = distance(k[other], k.anchor)
    const away = distance(to, k.anchor)
    if (away > 1e-9 && length > 1e-9) k[other] = [k.anchor[0] - ((to[0] - k.anchor[0]) / away) * length, k.anchor[1] - ((to[1] - k.anchor[1]) / away) * length]
  }
  return out
}

/** The path with knot [`subpath`, `knot`] converted, as `path.convertPoint`: no `outHandle` makes a corner with retracted direction points; one makes a smooth point with it and its mirror. */
export function convertPoint(path: PathData, subpath: number, knot: number, outHandle: Pt | null): PathData {
  const out = copy(path)
  const k = out.subpaths[subpath]?.knots[knot]
  if (!k) return out
  if (!outHandle) {
    k.in = [...k.anchor] as Pt
    k.out = [...k.anchor] as Pt
    k.smooth = false
  } else {
    k.out = [outHandle[0], outHandle[1]]
    k.in = [2 * k.anchor[0] - outHandle[0], 2 * k.anchor[1] - outHandle[1]]
    k.smooth = true
  }
  return out
}

/** Whether a knot has direction points out of its anchor (a curve through it). */
export function curved(knot: PathKnot) {
  return !same(knot.in, knot.anchor) || !same(knot.out, knot.anchor)
}

/** The anchors inside the box [x0, y0, x1, y1] (Direct Selection's marquee). */
export function anchorsIn(path: PathData, box: readonly [number, number, number, number]): AnchorRef[] {
  const [x0, y0, x1, y1] = [Math.min(box[0], box[2]), Math.min(box[1], box[3]), Math.max(box[0], box[2]), Math.max(box[1], box[3])]
  return path.subpaths.flatMap((subpath, s) => subpath.knots.flatMap((knot, k) => (knot.anchor[0] >= x0 && knot.anchor[0] <= x1 && knot.anchor[1] >= y0 && knot.anchor[1] <= y1 ? [[s, k] as const] : [])))
}

/** Every anchor of subpath `s`. */
export function subpathAnchors(path: PathData, s: number): AnchorRef[] {
  return (path.subpaths[s]?.knots ?? []).map((_, k) => [s, k] as const)
}
