// The Layer Style dialog's model (plan 2026-11-24, stage 5): node --test (part of `pnpm test:parity`).
import test from 'node:test'
import assert from 'node:assert/strict'
import { EFFECT_FIELDS, EFFECT_KINDS, blendKey, blendingChange, effectGradient, replaceEffects, setEffectGradient, setEffectParams, styleFromLayer, styleRows, toggleEffect } from './layerStyle.ts'
import { dialogFor } from './routes.ts'

const answer = {
  enabled: true,
  items: [
    { kind: 'dropShadow', enabled: true, params: { enabled: true, color: '#000000', opacity: 75, distance: 5 }, fx: { DropShadow: { distance: 5 } } },
    { kind: 'stroke', enabled: false, params: { enabled: false, size: 3 }, fx: { Stroke: { size: 3 } } },
    { kind: 'stroke', enabled: true, params: { enabled: true, size: 9 }, fx: { Stroke: { size: 9 } } },
    { kind: 'notAnEffect', enabled: true, params: {} },
  ],
}
const layer = { opacity: 0.5, fill: 1, blend: 'Linear Dodge (Add)' }

test('the dialog opens from what the layer has, unknown kinds left out', () => {
  const state = styleFromLayer(answer, layer)
  assert.deepEqual(
    state.entries.map((entry) => [entry.kind, entry.enabled, entry.original]),
    [
      ['dropShadow', true, true],
      ['stroke', false, true],
      ['stroke', true, true],
    ],
  )
  assert.equal('enabled' in state.entries[0].params, false, 'on/off is the entry’s, not a field')
  assert.deepEqual(state.blending, { blend: 'linearDodge', opacity: 50, fill: 100 })
  assert.deepEqual(styleFromLayer(null, {}).blending, { blend: 'normal', opacity: 100, fill: 100 })
})

test('blend mode labels read as the commands name them', () => {
  assert.equal(blendKey('Normal'), 'normal')
  assert.equal(blendKey('Color Burn'), 'colorBurn')
  assert.equal(blendKey('Linear Dodge (Add)'), 'linearDodge')
  assert.equal(blendKey('Pass Through'), 'normal')
  assert.equal(blendKey(undefined), 'normal')
})

test("the list has every kind in Photoshop's order, a row per effect", () => {
  const rows = styleRows(styleFromLayer(answer, layer))
  assert.deepEqual([...new Set(rows.map((row) => row.kind))], [...EFFECT_KINDS])
  assert.deepEqual(
    rows.filter((row) => row.kind === 'stroke').map((row) => [row.index, row.enabled]),
    [
      [1, false],
      [2, true],
    ],
  )
  assert.deepEqual(rows.find((row) => row.kind === 'satin'), { kind: 'satin', index: null, enabled: false })
  for (const kind of EFFECT_KINDS) assert.ok(EFFECT_FIELDS[kind].length > 0, kind)
})

test('checking an empty row adds the effect from its defaults; unchecking keeps it', () => {
  const start = styleFromLayer(answer, layer)
  const added = toggleEffect(start, 'outerGlow', null, true, { enabled: true, size: 5, color: '#ffffbe' })
  assert.equal(added.index, 3)
  assert.deepEqual(added.state.entries[3], { kind: 'outerGlow', enabled: true, params: { size: 5, color: '#ffffbe' }, fx: null, original: false })
  const off = toggleEffect(added.state, 'dropShadow', 0, false, {})
  assert.equal(off.state.entries[0].enabled, false)
  assert.equal(start.entries[0].enabled, true, 'the state is not changed in place')
})

test('replace sends every original effect and the added ones left on, each over its snapshot', () => {
  let state = styleFromLayer(answer, layer)
  state = toggleEffect(state, 'satin', null, true, { size: 14 }).state
  state = toggleEffect(state, 'colorOverlay', null, true, { color: '#ff0000' }).state
  state = toggleEffect(state, 'colorOverlay', 4, false, {}).state
  state = setEffectParams(state, 0, { distance: 12 })
  const effects = replaceEffects(state)
  assert.deepEqual(
    effects.map((effect) => [effect.kind, effect.params.enabled]),
    [
      ['dropShadow', true],
      ['stroke', false],
      ['stroke', true],
      ['satin', true],
    ],
  )
  assert.equal(effects[0].params.distance, 12)
  assert.deepEqual(effects[0].fx, { DropShadow: { distance: 5 } })
  assert.equal('fx' in effects[3], false, 'an added effect is built from its values')
})

test('blending options run only when they changed', () => {
  const start = styleFromLayer(answer, layer)
  assert.equal(blendingChange(start, start), null)
  const changed = { ...start, blending: { ...start.blending, fill: 40 } }
  assert.deepEqual(blendingChange(changed, start), { blend: 'linearDodge', opacity: 50, fillOpacity: 40 })
})

test("a gradient overlay's gradient goes in its snapshot; its end colours stay out", () => {
  const withOverlay = {
    items: [
      {
        kind: 'gradientOverlay',
        enabled: true,
        params: { from: '#000000', to: '#ffffff', style: 'linear', angle: 90, scale: 100, opacity: 100, blend: 'normal' },
        fx: { GradientOverlay: { common: { enabled: true, blend: 'Normal', opacity: 1 }, gradient: { stops: [], opacity_stops: [], style: 'Linear', angle: 90, scale: 1, reverse: false, align: true, offset: [0, 0] }, dither: false } },
      },
    ],
  }
  let state = styleFromLayer(withOverlay, layer)
  assert.deepEqual(effectGradient(state.entries[0]).stops, [
    { at: 0, color: '#000000' },
    { at: 1, color: '#ffffff' },
  ])
  state = setEffectGradient(state, 0, [{ at: 0, color: '#ff0000' }, { at: 0.5, color: '#00ff00' }, { at: 1, color: '#0000ff' }], [{ at: 0, opacity: 100 }, { at: 1, opacity: 20 }])
  const [effect] = replaceEffects(state)
  assert.equal('from' in effect.params, false)
  assert.equal('stops' in effect.params, false)
  const gradient = effect.fx.GradientOverlay.gradient
  assert.equal(gradient.stops.length, 3)
  assert.deepEqual(gradient.stops[1], [0.5, { mode: 'Rgb', c: [0, 1, 0, 0], alpha: 1 }])
  assert.deepEqual(gradient.opacity_stops, [
    [0, 1],
    [1, 0.2],
  ])
  assert.equal(gradient.style, 'Linear', 'the rest of the snapshot stays')
  assert.equal(effectGradient(state.entries[0]).stops.length, 3)
  // An overlay added in the dialog (no snapshot) gets one built from its fields.
  const added = toggleEffect(styleFromLayer(null, layer), 'gradientOverlay', null, true, { blend: 'multiply', opacity: 80, style: 'radial', angle: 45, scale: 100 })
  const built = replaceEffects(setEffectGradient(added.state, 0, [{ at: 0, color: '#112233' }, { at: 1, color: '#445566' }], []))[0].fx.GradientOverlay
  assert.deepEqual(built.common, { enabled: true, blend: 'Multiply', opacity: 0.8 })
  assert.equal(built.gradient.style, 'Radial')
})

test('every Layer › Layer Style row with a dialog opens Layer Style', () => {
  for (const kind of ['blendingOptions', 'dropShadow', 'innerShadow', 'outerGlow', 'innerGlow', 'bevelEmboss', 'satin', 'colorOverlay', 'gradientOverlay', 'patternOverlay', 'stroke']) {
    assert.equal(dialogFor({ id: `layer.layerStyle.${kind}`, title: 'Something…', asks: true }), 'layerStyle', kind)
  }
  assert.equal(dialogFor({ id: 'layer.layerStyle.globalLight', title: 'Global Light…', asks: true }), 'params')
  assert.equal(dialogFor({ id: 'layer.layerStyle.clear', title: 'Clear Layer Style', asks: false }), null)
})
