/**
 * A brush stroke drawn by the engine while the pointer moves (plan 2026-11-24, stage 3): the
 * engine's live stroke (`strokeBegin`, `strokePush`, `strokeCancel` of `bindings/wasm`, see its
 * `stroke.rs`) paints the stroke so far onto a copy of the document, and the canvas draws again
 * only the area each step changed. Pointer moves are gathered and sent once per animation frame,
 * one call at a time: a fast pointer never queues up work in the engine. The release commits the
 * stroke as before, by running the tool's command with every point (and the seed the preview drew
 * its jitter with); Escape drops it, and the canvas draws the area again from the document.
 */
import type { RasterRect } from '@kition/sdk/editor'

/** The commands whose strokes the engine shows while they are drawn; the others paint on release. */
export const LIVE_COMMANDS: ReadonlySet<string> = new Set([
  'paint.stroke',
  'paint.pencil',
  'paint.cloneStamp',
  'paint.healingBrush',
  'paint.historyBrush',
  'paint.dodge',
  'paint.burn',
  'paint.sponge',
  'paint.blur',
  'paint.sharpen',
  'paint.smudge',
])

/** What a live stroke needs from the editor: the engine's calls and the canvas's repaint. */
export type StrokeEngine = {
  /** `strokeBegin`: the answer as JSON (`{stroke, damage, seed}`). */
  begin: (command: string, params: Record<string, unknown>) => Promise<string>
  /** `strokePush`: the area that changed, `[x0, y0, x1, y1]`. */
  push: (stroke: number, points: Float64Array) => Promise<ArrayLike<number>>
  /** `strokeCancel`: the area the preview covered. */
  cancel: (stroke: number) => Promise<ArrayLike<number>>
  /** Draws `area` of the canvas again from the engine. */
  repaint: (area: RasterRect) => Promise<unknown>
  /** Runs `callback` before the next frame is drawn (`requestAnimationFrame`). */
  frame?: (callback: () => void) => void
}

export type LiveStroke = {
  /** Adds a point `[x, y, pressure]`; the engine gets the frame's points together. */
  add: (point: readonly number[]) => void
  /** Whether the stroke shows no preview (the engine would not begin it, or a step failed): the tool draws its own line then. */
  readonly failed: boolean
  /**
   * What the commit adds to the command's params (the seed the preview drew with), once the engine
   * answered the begin; null when it shows no preview.
   */
  readonly settled: Promise<Record<string, unknown> | null>
  /** Ends the preview: after the commit (`committed`), or without one, which draws the area again from the document. */
  close: (committed: boolean) => Promise<void>
}

/** `[x0, y0, x1, y1]` as an area, or null when it holds no pixel. */
export function areaOf(corners: ArrayLike<number> | null | undefined): RasterRect | null {
  if (!corners || corners.length < 4) return null
  const area = { x0: corners[0], y0: corners[1], x1: corners[2], y1: corners[3] }
  return area.x1 > area.x0 && area.y1 > area.y0 ? area : null
}

/**
 * Begins a live stroke of `command` with `params` (its first points included). `failed` is called
 * once when the preview stops (it could not begin, or a step failed), so the tool shows its own line.
 */
export function liveStroke(engine: StrokeEngine, command: string, params: Record<string, unknown>, failed: () => void): LiveStroke {
  const frame = engine.frame ?? ((callback: () => void) => void requestAnimationFrame(callback))
  let stroke: number | null = null
  let seed: unknown = null
  let broken = false
  let closed = false
  let queue: number[] = []
  let scheduled = false
  // The step under way (a push and its repaint); the next waits for it.
  let busy: Promise<void> | null = null
  // A preview that stops is taken away: the canvas shows the document, and the tool its own line.
  const fail = () => {
    if (broken) return
    broken = true
    queue = []
    failed()
    const id = stroke
    if (id != null && !closed) void engine.cancel(id).then((corners) => {
      const area = areaOf(corners)
      return area ? engine.repaint(area) : undefined
    }).catch(() => undefined)
  }
  const run = (work: () => Promise<void>) => {
    busy = work()
      .catch(fail)
      .finally(() => {
        busy = null
        if (queue.length) schedule()
      })
  }
  const pump = () => {
    if (stroke == null || busy || broken || closed || !queue.length) return
    const id = stroke
    const points = Float64Array.from(queue)
    queue = []
    run(async () => {
      const area = areaOf(await engine.push(id, points))
      if (area) await engine.repaint(area)
    })
  }
  const schedule = () => {
    if (scheduled) return
    scheduled = true
    frame(() => {
      scheduled = false
      pump()
    })
  }
  const begun = engine.begin(command, params).then(
    (text) => {
      const answer = JSON.parse(text) as { stroke?: number; damage?: number[]; seed?: unknown }
      if (typeof answer.stroke !== 'number') throw new Error('the engine began no stroke')
      stroke = answer.stroke
      seed = answer.seed ?? null
      const area = areaOf(answer.damage)
      if (closed) return
      run(async () => {
        if (area) await engine.repaint(area)
      })
    },
    () => fail(),
  )
  const settle = async () => {
    await begun.catch(fail)
    while (busy) await busy
  }
  return {
    add(point) {
      if (broken || closed) return
      queue.push(point[0], point[1], point[2] ?? 1)
      schedule()
    },
    get failed() {
      return broken
    },
    get settled() {
      return begun.then(() => (broken || stroke == null ? null : seed == null ? {} : { seed }))
    },
    async close(committed) {
      closed = true
      queue = []
      await settle()
      if (stroke == null) return
      const area = areaOf(await engine.cancel(stroke).catch(() => null))
      if (!committed && area) await engine.repaint(area).catch(() => undefined)
    },
  }
}
