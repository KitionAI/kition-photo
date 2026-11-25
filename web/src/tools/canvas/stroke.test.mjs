// The live stroke's traffic with the engine (plan 2026-11-24, stage 3): a frame's points go in one
// call, one call at a time; the areas that changed are drawn again; a refused begin or step stops
// the preview once; Escape draws the area again from the document. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { LIVE_COMMANDS, areaOf, liveStroke } from './stroke.ts'

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

/** A fake engine: frames run when `nextFrame` is called; pushes wait for `release`. */
function fakeEngine({ begin = { stroke: 7, damage: [0, 0, 10, 10], seed: 42 }, refuseBegin = false, refusePush = false } = {}) {
  const calls = []
  const frames = []
  const waiting = []
  const engine = {
    begin: async (command, params) => {
      calls.push(['begin', command, params])
      if (refuseBegin) throw new Error('refused')
      return JSON.stringify(begin)
    },
    push: (stroke, points) => {
      calls.push(['push', stroke, Array.from(points)])
      if (refusePush) return Promise.reject(new Error('off the range'))
      return new Promise((resolve) => waiting.push(() => resolve([points[0] - 5, points[1] - 5, points[0] + 5, points[1] + 5])))
    },
    cancel: async (stroke) => {
      calls.push(['cancel', stroke])
      return [0, 0, 50, 50]
    },
    repaint: async (area) => {
      calls.push(['repaint', area])
    },
    frame: (callback) => frames.push(callback),
  }
  return {
    engine,
    calls,
    nextFrame: async () => {
      const due = frames.splice(0)
      for (const callback of due) callback()
      await tick()
    },
    release: async () => {
      waiting.shift()?.()
      await tick()
      await tick()
    },
  }
}

test('only the commands the engine previews go live', () => {
  for (const command of ['paint.stroke', 'paint.pencil', 'paint.cloneStamp', 'paint.healingBrush', 'paint.historyBrush', 'paint.dodge', 'paint.burn', 'paint.sponge', 'paint.blur', 'paint.sharpen', 'paint.smudge']) assert.ok(LIVE_COMMANDS.has(command), command)
  for (const command of ['paint.spotHealing', 'paint.mixerBrush', 'paint.backgroundEraser', 'paint.bucket']) assert.ok(!LIVE_COMMANDS.has(command), command)
  assert.equal(areaOf([0, 0, 0, 0]), null)
  assert.deepEqual(areaOf([1, 2, 3, 4]), { x0: 1, y0: 2, x1: 3, y1: 4 })
})

test("a frame's points go in one call, one call at a time, and each changed area is drawn again", async () => {
  const fake = fakeEngine()
  let failures = 0
  const stroke = liveStroke(fake.engine, 'paint.stroke', { points: [[1, 1, 1]], size: 10 }, () => (failures += 1))
  await tick()
  assert.deepEqual(fake.calls.slice(0, 2), [['begin', 'paint.stroke', { points: [[1, 1, 1]], size: 10 }], ['repaint', { x0: 0, y0: 0, x1: 10, y1: 10 }]])
  stroke.add([20, 20, 1])
  stroke.add([30, 30, 0.5])
  await fake.nextFrame()
  assert.deepEqual(fake.calls[2], ['push', 7, [20, 20, 1, 30, 30, 0.5]])
  // While that call is under way, two frames' points wait and then go together.
  stroke.add([40, 40, 1])
  await fake.nextFrame()
  stroke.add([50, 50, 1])
  await fake.nextFrame()
  assert.equal(fake.calls.filter(([kind]) => kind === 'push').length, 1)
  await fake.release()
  assert.deepEqual(fake.calls[3], ['repaint', { x0: 15, y0: 15, x1: 25, y1: 25 }])
  await fake.nextFrame()
  assert.deepEqual(fake.calls[4], ['push', 7, [40, 40, 1, 50, 50, 1]])
  await fake.release()
  assert.deepEqual(await stroke.settled, { seed: 42 })
  await stroke.close(true)
  assert.deepEqual(fake.calls.at(-1), ['cancel', 7], 'after a commit the preview goes without a repaint of its own')
  assert.equal(failures, 0)
  stroke.add([60, 60, 1])
  await fake.nextFrame()
  assert.equal(fake.calls.filter(([kind]) => kind === 'push').length, 2, 'a closed stroke sends nothing')
})

test('Escape draws the area again from the document', async () => {
  const fake = fakeEngine({ begin: { stroke: 3, damage: [0, 0, 0, 0], seed: null } })
  const stroke = liveStroke(fake.engine, 'paint.dodge', { points: [[1, 1, 1]] }, () => assert.fail('no failure'))
  await tick()
  assert.equal(fake.calls.filter(([kind]) => kind === 'repaint').length, 0, 'an empty area is not drawn')
  assert.deepEqual(await stroke.settled, {}, 'no seed for a tool without jitter')
  await stroke.close(false)
  assert.deepEqual(fake.calls.slice(-2), [['cancel', 3], ['repaint', { x0: 0, y0: 0, x1: 50, y1: 50 }]])
})

test('a refused begin stops the preview once; the release paints as before', async () => {
  const fake = fakeEngine({ refuseBegin: true })
  let failures = 0
  const stroke = liveStroke(fake.engine, 'paint.blur', { points: [[1, 1, 1]], sampleAllLayers: true }, () => (failures += 1))
  stroke.add([5, 5, 1])
  await fake.nextFrame()
  assert.equal(await stroke.settled, null)
  assert.equal(stroke.failed, true)
  assert.equal(failures, 1)
  await stroke.close(false)
  assert.deepEqual(fake.calls.map(([kind]) => kind), ['begin'])
})

test('a refused step takes the preview away and stops it once', async () => {
  const fake = fakeEngine({ refusePush: true })
  let failures = 0
  const stroke = liveStroke(fake.engine, 'paint.stroke', { points: [[1, 1, 1]] }, () => (failures += 1))
  await tick()
  stroke.add([5, 5, 1])
  await fake.nextFrame()
  await tick()
  stroke.add([6, 6, 1])
  await fake.nextFrame()
  assert.equal(failures, 1)
  assert.equal(stroke.failed, true)
  assert.equal(await stroke.settled, null)
  assert.deepEqual(fake.calls.map(([kind]) => kind), ['begin', 'repaint', 'push', 'cancel', 'repaint'])
})
