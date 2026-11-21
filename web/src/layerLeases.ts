import { useEffect, useState } from 'react'
import type { KitEngine, KitLease, KitLeases, KitTool } from '@kition/sdk/editor'

/**
 * Painting a layer's pixels while someone else paints the same layer does not merge into anything
 * either of them meant (Kition platform rule M21): so a pixel tool takes a lease on the layer
 * (`layer:<objectId>`, the layer's KDM object id, the same on every replica) through the host
 * before its stroke lands, keeps it while the person keeps painting, and lets it go a moment after
 * the last stroke. Someone else's lease shows on the layer's row, and a stroke on that layer is
 * refused, with who is painting it. Without co-editing every stroke goes ahead as before.
 */

/** How long the lease outlives the last stroke, so a run of strokes keeps it. */
export const PAINT_IDLE_MS = 3_000
/** One renewal's length; the host renews it while it is held. */
export const PAINT_LEASE_TTL = 15_000

export function layerScope(objectId: string) {
  return `layer:${objectId}`
}

/** Parses the engine's `layerObjectIds` answer into local layer id → object id. */
export function parseLayerObjectIds(text: string): Map<number, string> {
  const ids = new Map<number, string>()
  try {
    const value = JSON.parse(text) as Record<string, unknown>
    for (const [layer, object] of Object.entries(value ?? {})) if (typeof object === 'string' && Number.isFinite(Number(layer))) ids.set(Number(layer), object)
  } catch {
    // An engine without the extra: no layer has an object id, so nothing is leased.
  }
  return ids
}

const NO_IDS = new Map<number, string>()

/** Each layer's object id, asked again after every change; empty while `enabled` is false (no co-editing). */
export function useLayerObjectIds(doc: KitEngine | null, revision: number, enabled: boolean): Map<number, string> {
  const [ids, setIds] = useState(NO_IDS)
  useEffect(() => {
    if (!doc || !enabled) {
      setIds(NO_IDS)
      return undefined
    }
    let cancelled = false
    doc.invoke<string>('layerObjectIds').then(
      (text) => {
        if (!cancelled) setIds(parseLayerObjectIds(text))
      },
      () => undefined,
    )
    return () => {
      cancelled = true
    }
  }, [doc, enabled, revision])
  return ids
}

/** Who else holds a lease on each layer (by local layer id), from the document's leases. */
export function layersLeasedByOthers(ids: ReadonlyMap<number, string>, leases: readonly KitLease[]): Map<number, string> {
  const leased = new Map<number, string>()
  const byScope = new Map(leases.filter((lease) => !lease.holder.self).map((lease) => [lease.scope, lease.holder.name]))
  const whole = byScope.get('*')
  for (const [layer, object] of ids) {
    const holder = byScope.get(layerScope(object)) ?? whole
    if (holder !== undefined) leased.set(layer, holder)
  }
  return leased
}

type Timers = { setTimeout: (run: () => void, ms: number) => unknown; clearTimeout: (handle: unknown) => void }

/**
 * The lease a pixel tool's strokes take on the layer they paint. `begin` is called at a painting
 * press (false: someone else paints the layer, and `refuse` said who); `allowed` before the
 * stroke's command runs (it waits for the host's answer; false: refused); `idle` after a stroke,
 * which lets the lease go `PAINT_IDLE_MS` later unless another stroke starts; `setScope` follows
 * the active layer, letting go of the lease on the one before.
 */
export class PaintLease {
  private scope: string | null = null
  private held: string | null = null
  private pending: Promise<boolean> | null = null
  private timer: unknown = null
  private readonly timers: Timers
  private readonly leases: KitLeases
  private readonly refuse: (holder: string) => void

  constructor(leases: KitLeases, refuse: (holder: string) => void, timers?: Timers) {
    this.leases = leases
    this.refuse = refuse
    this.timers = timers ?? { setTimeout: (run, ms) => setTimeout(run, ms), clearTimeout: (handle) => clearTimeout(handle as number) }
  }

  setScope(scope: string | null) {
    if (scope === this.scope) return
    this.release()
    this.scope = scope
  }

  begin(): boolean {
    const scope = this.scope
    if (!scope || !this.leases.available) return true
    // A lease this tool holds is its own, even before the host has said so.
    const other = this.held === scope ? null : this.leases.heldByOther(scope)
    if (other) {
      this.refuse(other.holder.name)
      return false
    }
    this.cancelIdle()
    this.pending = this.leases.acquire(scope, PAINT_LEASE_TTL).then((outcome) => {
      if (outcome.granted) {
        if (outcome.lease && this.scope === scope) this.held = scope
        else if (outcome.lease) void this.leases.release(scope)
        return true
      }
      if (outcome.reason === 'held') this.refuse(outcome.holder?.name ?? '')
      return false
    })
    return true
  }

  /** Whether the stroke that began last may land. */
  async allowed(): Promise<boolean> {
    const pending = this.pending
    this.pending = null
    return pending ? pending : true
  }

  /** Whether a stroke's lease answer is still to be awaited. */
  get waiting() {
    return this.pending !== null
  }

  /** The answer for the stroke that began last, if still to be taken. */
  get outcome(): Promise<boolean> | null {
    return this.pending
  }

  idle() {
    this.cancelIdle()
    if (!this.held && !this.pending) return
    this.timer = this.timers.setTimeout(() => {
      this.timer = null
      if (!this.pending) this.release()
    }, PAINT_IDLE_MS)
  }

  release() {
    this.cancelIdle()
    const held = this.held
    this.held = null
    if (held) void this.leases.release(held)
  }

  private cancelIdle() {
    if (this.timer === null) return
    this.timers.clearTimeout(this.timer)
    this.timer = null
  }
}

/**
 * `tool`, with its painting presses taking the layer's lease first: a press on a layer someone
 * else paints is refused (nothing is drawn), and the end of each stroke lets the lease idle.
 * Presses that paint nothing go straight through (`paints` false, an Alt-click that sets a source).
 */
export function withPaintLease(tool: KitTool, paints: (pointer: Parameters<KitTool['begin']>[0]) => boolean, lease: PaintLease): KitTool {
  return {
    ...tool,
    begin(pointer, context) {
      if (!paints(pointer)) return tool.begin(pointer, context)
      if (!lease.begin()) return undefined
      const gesture = tool.begin(pointer, context)
      if (!gesture) {
        lease.idle()
        return gesture
      }
      return {
        ...gesture,
        end: (end, endContext) => {
          // A refused stroke lands nothing: its preview goes too.
          void lease.outcome?.then((granted) => granted || endContext.overlay.clear())
          gesture.end?.(end, endContext)
          lease.idle()
        },
        cancel: (cancelContext) => {
          gesture.cancel?.(cancelContext)
          void lease.allowed()
          lease.idle()
        },
      }
    },
  }
}
