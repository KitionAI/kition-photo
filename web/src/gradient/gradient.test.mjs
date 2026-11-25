// The gradient editor's model (plan 2026-11-24, stage 5): node --test (part of `pnpm test:parity`).
import test from 'node:test'
import assert from 'node:assert/strict'
import { addColorStop, addOpacityStop, bakeMidpoints, bend, colorAt, cssGradient, fromEngine, midpointPlaces, moveStop, opacityAt, removeStop, resolveColor, setMidpoint, setStop, toEngine } from './gradient.ts'

const FG = '#000000'
const BG = '#ffffff'
const blackWhite = () => fromEngine({ stops: [[0, '#000000'], [1, '#ffffff']], transparency: [] })

test('reads the engine form sorted, with a midpoint per segment', () => {
  const draft = fromEngine({ stops: [[1, '#ffffff'], [0, 'foreground'], [0.5, '#ff0000']], transparency: [[1, 0], [0, 100]], midpoints: [0.3] })
  assert.deepEqual(draft.stops.map((stop) => stop.at), [0, 0.5, 1])
  assert.deepEqual(draft.opacity, [{ at: 0, opacity: 100 }, { at: 1, opacity: 0 }])
  assert.deepEqual(draft.midpoints, [0.3, 0.5])
  // Too few stops: foreground to background.
  assert.deepEqual(fromEngine({ stops: [[0, '#123456']] }).stops, [{ at: 0, color: 'foreground' }, { at: 1, color: 'background' }])
  assert.deepEqual(fromEngine(null).midpoints, [0.5])
})

test('writes the engine form, midpoints only when one is bent', () => {
  assert.deepEqual(toEngine(blackWhite()), { stops: [[0, '#000000'], [1, '#ffffff']], transparency: [] })
  const bent = setMidpoint(blackWhite(), 0, 0.25)
  assert.deepEqual(toEngine(bent).midpoints, [0.25])
})

test('colours and opacities between stops, midpoints bending the mix', () => {
  const draft = blackWhite()
  assert.equal(colorAt(draft, 0, FG, BG), '#000000')
  assert.equal(colorAt(draft, 0.5, FG, BG), '#808080')
  assert.equal(colorAt(draft, 1, FG, BG), '#ffffff')
  assert.equal(bend(0.25, 0.25), 0.5)
  assert.equal(colorAt(setMidpoint(draft, 0, 0.25), 0.25, FG, BG), '#808080')
  assert.equal(resolveColor('foreground', '#112233', BG), '#112233')
  assert.equal(resolveColor('background', FG, '#445566'), '#445566')
  assert.equal(opacityAt(draft, 0.3), 100)
  const faded = { ...draft, opacity: [{ at: 0, opacity: 100 }, { at: 1, opacity: 0 }] }
  assert.equal(opacityAt(faded, 0.25), 75)
  assert.match(cssGradient(faded, FG, BG), /^linear-gradient\(to right, rgba\(0, 0, 0, 1\) 0%, .*rgba\(255, 255, 255, 0\) 100%\)$/)
})

test('a stop added takes the colour or opacity there; the segment it splits resets', () => {
  const bent = setMidpoint(blackWhite(), 0, 0.3)
  const { draft, index } = addColorStop(bent, 0.5, FG, BG)
  assert.equal(index, 2)
  assert.deepEqual(draft.stops[2], { at: 0.5, color: colorAt(bent, 0.5, FG, BG) })
  assert.deepEqual(draft.midpoints, [0.5, 0.5])
  const opacity = addOpacityStop({ ...blackWhite(), opacity: [{ at: 0, opacity: 100 }, { at: 1, opacity: 0 }] }, 0.5)
  assert.deepEqual(opacity.draft.opacity[2], { at: 0.5, opacity: 50 })
})

test('stops move, take values and go, keeping at least two colour stops', () => {
  const three = addColorStop(blackWhite(), 0.5, FG, BG).draft
  const moved = moveStop(three, 'color', 2, 1.4)
  assert.equal(moved.stops[2].at, 1)
  const red = setStop(three, 'color', 2, '#ff0000')
  assert.equal(colorAt(red, 0.5, FG, BG), '#ff0000')
  const fewer = removeStop(three, 'color', 2)
  assert.equal(fewer.stops.length, 2)
  assert.equal(removeStop(fewer, 'color', 0), null)
  const withOpacity = addOpacityStop(three, 0.2).draft
  assert.equal(removeStop(withOpacity, 'opacity', 0).opacity.length, 0)
  assert.equal(setStop(withOpacity, 'opacity', 0, 150).opacity[0].opacity, 100)
})

test('midpoints sit between their stops and bake into stops where a target keeps none', () => {
  const three = addColorStop(blackWhite(), 0.5, FG, BG).draft
  assert.deepEqual(midpointPlaces(three), [0.25, 0.75])
  const bent = setMidpoint(three, 1, 0.2)
  assert.deepEqual(midpointPlaces(bent).map((place) => Math.round(place * 100) / 100), [0.25, 0.6])
  assert.equal(setMidpoint(three, 0, 0).midpoints[0], 0.05)
  const baked = bakeMidpoints(bent, FG, BG)
  assert.equal(baked.stops.length, 4)
  assert.deepEqual(baked.stops[2][0], 0.6)
  assert.equal('midpoints' in baked, false)
})
