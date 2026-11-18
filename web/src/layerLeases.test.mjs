// Layer leases: a pixel tool's strokes take the layer's lease, refuse a layer someone else
// paints, and let the lease go a moment after the last stroke.
import test from 'node:test'
import assert from 'node:assert/strict'
import { PAINT_IDLE_MS, PaintLease, layerScope, layersLeasedByOthers, parseLayerObjectIds, withPaintLease } from './layerLeases.ts'

const alice = { principalId: 'p1', name: 'Alice', self: false }
const me = { principalId: 'p2', name: 'Bob', self: true }

/** A kit's lease store as `kitLeases` answers it, with the host's answers scripted. */
function leases({ others = [], refuse = null, available = true } = {}) {
  const calls = []
  return {
    calls,
    available,
    list: () => others,
    subscribe: () => () => undefined,
    heldByOther: (scope) => others.find((lease) => !lease.holder.self && (lease.scope === scope || lease.scope === '*')) ?? null,
    acquire: async (scope, ttl) => {
      calls.push(['acquire', scope, ttl])
      if (refuse) return { granted: false, reason: 'held', holder: refuse }
      return { granted: true, lease: { scope, holder: me } }
    },
    release: async (scope) => {
      calls.push(['release', scope])
    },
  }
}

function timers() {
  const scheduled = []
  return {
    scheduled,
    setTimeout: (run, ms) => scheduled.push({ run, ms }) - 1,
    clearTimeout: (handle) => {
      if (scheduled[handle]) scheduled[handle].run = () => undefined
    },
    fire: () => {
      for (const entry of scheduled.splice(0)) entry.run()
    },
  }
}

test('reads the engine\'s layer object ids and marks the layers others paint', () => {
  const ids = parseLayerObjectIds('{"3":"obj-a","7":"obj-b","x":"bad","9":4}')
  assert.deepEqual([...ids], [[3, 'obj-a'], [7, 'obj-b']])
  assert.deepEqual([...parseLayerObjectIds('not json')], [])
  const leased = layersLeasedByOthers(ids, [{ scope: layerScope('obj-a'), holder: alice }, { scope: layerScope('obj-b'), holder: me }])
  assert.deepEqual([...leased], [[3, 'Alice']])
  // Someone holding the whole document holds every layer.
  assert.deepEqual([...layersLeasedByOthers(ids, [{ scope: '*', holder: alice }])], [[3, 'Alice'], [7, 'Alice']])
})

test('a stroke takes the layer\'s lease, keeps it through a run of strokes, and lets it go once idle', async () => {
  const store = leases()
  const clock = timers()
  const lease = new PaintLease(store, () => assert.fail('nothing is refused'), clock)
  lease.setScope(layerScope('obj-a'))
  assert.equal(lease.begin(), true)
  assert.equal(await lease.allowed(), true)
  lease.idle()
  assert.equal(clock.scheduled.at(-1).ms, PAINT_IDLE_MS)
  // The next stroke starts before the lease idles out: it is renewed, not released.
  assert.equal(lease.begin(), true)
  assert.equal(await lease.allowed(), true)
  lease.idle()
  clock.fire()
  assert.deepEqual(store.calls, [['acquire', 'layer:obj-a', 15000], ['acquire', 'layer:obj-a', 15000], ['release', 'layer:obj-a']])
})

test('a layer someone else paints is refused at the press, and a refusal from the host stops the stroke', async () => {
  const refused = []
  const busy = new PaintLease(leases({ others: [{ scope: layerScope('obj-a'), holder: alice }] }), (name) => refused.push(name), timers())
  busy.setScope(layerScope('obj-a'))
  assert.equal(busy.begin(), false)
  assert.deepEqual(refused, ['Alice'])

  const raced = new PaintLease(leases({ refuse: alice }), (name) => refused.push(`late ${name}`), timers())
  raced.setScope(layerScope('obj-a'))
  assert.equal(raced.begin(), true)
  assert.equal(await raced.allowed(), false)
  assert.deepEqual(refused, ['Alice', 'late Alice'])
})

test('changing the active layer lets go of the lease on the one before; no co-editing takes none', async () => {
  const store = leases()
  const lease = new PaintLease(store, () => undefined, timers())
  lease.setScope(layerScope('obj-a'))
  lease.begin()
  await lease.allowed()
  lease.setScope(layerScope('obj-b'))
  assert.deepEqual(store.calls.at(-1), ['release', 'layer:obj-a'])

  const solo = leases({ available: false })
  const alone = new PaintLease(solo, () => undefined, timers())
  alone.setScope(layerScope('obj-a'))
  assert.equal(alone.begin(), true)
  assert.equal(alone.waiting, false)
  assert.deepEqual(solo.calls, [])
})

test('the tool wrapper refuses a press before the tool sees it, and clears a refused stroke\'s preview', async () => {
  const began = []
  const tool = { id: 'Brush', cursor: 'crosshair', begin: (pointer) => (began.push(pointer.x), { end: () => began.push('end') }) }
  const busy = new PaintLease(leases({ others: [{ scope: 'layer:obj-a', holder: alice }] }), () => undefined, timers())
  busy.setScope('layer:obj-a')
  const wrapped = withPaintLease(tool, () => true, busy)
  assert.equal(wrapped.begin({ x: 1, alt: false }, {}), undefined)
  assert.deepEqual(began, [])

  let cleared = 0
  const context = { overlay: { clear: () => (cleared += 1) } }
  const raced = new PaintLease(leases({ refuse: alice }), () => undefined, timers())
  raced.setScope('layer:obj-a')
  const gesture = withPaintLease(tool, () => true, raced).begin({ x: 2, alt: false }, context)
  gesture.end({ x: 3 }, context)
  await new Promise((resolve) => setTimeout(resolve, 0))
  assert.deepEqual(began, [2, 'end'])
  assert.equal(cleared, 1)

  // A press that paints nothing (an Alt-click that sets a source) goes straight through.
  assert.ok(withPaintLease(tool, () => false, busy).begin({ x: 4, alt: true }, {}))
})
