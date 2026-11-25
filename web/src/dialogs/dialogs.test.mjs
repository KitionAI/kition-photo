// The command dialogs' pure logic (plan 2026-11-24, stage 1): which dialog a command opens, the
// generic dialog's fields, the preview's run-and-undo, and Levels', Curves' and the size dialogs'
// numbers. Run with `pnpm --dir web test:parity` (node --test).
import test from 'node:test'
import assert from 'node:assert/strict'
import { dialogFor, isEllipsis, previews, withoutEllipsis, PURPOSE_BUILT } from './routes.ts'
import { fieldFor, initialValues, paramFields, paramsOf, rangeFromDescription, rangeFromParams, softMaxFor, humanize, SHARED_KEYS } from './params.ts'
import { createPreview, END_GRACE_MS, paramsKey } from './preview.ts'
import { curvesParams, fromEngineCurve, gammaAt, gammaForMidtone, gammaFraction, LEVELS_IDENTITY, levelsFrom, levelsParams, midtoneInput, setLevel, toEngineCurve } from './levels.ts'
import { anchorArrow, fromPixels, keptProportion, memorySize, side, toPixels } from './sizes.ts'
import { isHiddenCommand, notBuiltReason } from '../menus/hidden.ts'

test('a "…" command with something to ask opens the generic dialog; one with nothing to ask runs', () => {
  assert.equal(dialogFor({ id: 'filter.blur.gaussianBlur', title: 'Gaussian Blur…', asks: true }), 'params')
  assert.equal(dialogFor({ id: 'layer.new.layerFromBackground', title: 'Layer from Background…', asks: false }), null)
  assert.equal(dialogFor({ id: 'select.all', title: 'All', asks: false }), null)
  // A row without "…" runs even when it has parameters (Image › Auto Tone).
  assert.equal(dialogFor({ id: 'image.autoTone', title: 'Auto Tone', asks: true }), null)
  assert.equal(isEllipsis('Fill...'), true)
  assert.equal(withoutEllipsis('Gaussian Blur…'), 'Gaussian Blur')
})

test('the purpose-built dialogs serve the destructive adjustment and its adjustment layer alike', () => {
  for (const kind of ['levels', 'curves', 'hueSaturation', 'brightnessContrast', 'colorBalance']) {
    assert.equal(dialogFor({ id: `image.adjustments.${kind}`, title: 'x…', asks: true }), kind)
    assert.equal(dialogFor({ id: `layer.newAdjustmentLayer.${kind}`, title: 'x…', asks: true }), kind)
  }
  assert.equal(PURPOSE_BUILT['image.imageSize'], 'imageSize')
  assert.equal(PURPOSE_BUILT['image.canvasSize'], 'canvasSize')
  assert.equal(PURPOSE_BUILT['edit.fill'], 'fill')
  assert.equal(PURPOSE_BUILT['edit.stroke'], 'stroke')
})

test('rows with nothing behind them are hidden and open no dialog', () => {
  for (const command of ['edit.preferences.interface', 'edit.preferences.rawDefaults', 'edit.keyboardShortcuts', 'filter.filterGallery', 'view.newGuide', 'image.variables.define', 'filter.distort.displace']) {
    assert.ok(notBuiltReason(command)?.trim(), command)
    assert.equal(isHiddenCommand(command), true, command)
    assert.equal(dialogFor({ id: command, title: 'x…', asks: true }), null, command)
  }
  // Preferences › General is the editor's own settings dialog (plan 2026-11-24, stage 2): shown, and it asks.
  assert.equal(isHiddenCommand('edit.preferences.general'), false)
  assert.equal(dialogFor({ id: 'edit.preferences.general', title: 'General…', asks: false }), 'preferences')
  // The web-hidden rows stay hidden as well.
  assert.equal(isHiddenCommand('file.saveACopy'), true)
  assert.equal(notBuiltReason('filter.blur.gaussianBlur'), null)
})

test('filters, adjustments and layer styles preview; sizing and naming rows do not', () => {
  for (const command of ['filter.blur.gaussianBlur', 'image.adjustments.levels', 'layer.newAdjustmentLayer.curves', 'layer.layerStyle.dropShadow', 'edit.fade']) assert.equal(previews(command), true, command)
  for (const command of ['image.imageSize', 'image.canvasSize', 'layer.new.layer', 'select.modify.feather', 'layer.layerStyle.globalLight']) assert.equal(previews(command), false, command)
})

const BLUR = {
  id: 'filter.blur.gaussianBlur',
  params: '{"radius":0.1..1000=1}',
  schema: { at: { type: 'array' }, coalesce: { type: 'string' }, layer: { oneOf: [{ type: 'integer' }] }, radius: { default: 1, description: '0.1 to 1000', type: 'number' }, region: { type: 'json' }, target: { oneOf: [{ type: 'string' }] } },
}

test("the generic dialog asks for a command's own parameters, not the keys every command shares", () => {
  const fields = paramFields(BLUR)
  assert.deepEqual(fields.map((field) => field.key), ['radius'])
  assert.deepEqual(fields[0], { key: 'radius', kind: 'slider', min: 0.1, max: 1000, softMax: 250, decimals: 1, initial: 1 })
  for (const key of ['at', 'coalesce', 'layer', 'region', 'target', 'seed']) assert.ok(SHARED_KEYS.has(key), key)
})

test('a field takes its range from the params text, the description, or the schema', () => {
  assert.deepEqual(rangeFromParams('{"amount":0.1..400=12.5,"seed":u32=0}', 'amount'), { min: 0.1, max: 400, initial: 12.5 })
  assert.deepEqual(rangeFromParams('{"angle":-359.99..359.99=0}', 'angle'), { min: -359.99, max: 359.99, initial: 0 })
  assert.equal(rangeFromParams('{"amount":0.1..400}', 'radius'), null)
  assert.deepEqual(rangeFromDescription('-180 to 180'), { min: -180, max: 180 })
  assert.equal(rangeFromDescription('percent'), null)
  assert.deepEqual(fieldFor({ id: 'x' }, 'count', { type: 'integer', minimum: 1, maximum: 9 }), { key: 'count', kind: 'slider', min: 1, max: 9, softMax: 9, decimals: 0, initial: 1 })
  assert.equal(softMaxFor(-100, 100), 100)
  assert.equal(softMaxFor(0.1, 1000), 250)
})

test('choices, check boxes, colours, blend modes and text become their controls; JSON keeps the default', () => {
  assert.deepEqual(fieldFor({ id: 'x' }, 'distribution', { enum: ['uniform', 'gaussian'], type: 'string' }), { key: 'distribution', kind: 'choice', options: ['uniform', 'gaussian'], initial: 'uniform' })
  assert.equal(fieldFor({ id: 'x' }, 'profile', { enum: ['working', 'srgb'], type: 'string' }).initial, 'working')
  assert.deepEqual(fieldFor({ id: 'x' }, 'monochromatic', { type: 'boolean' }), { key: 'monochromatic', kind: 'check', initial: false })
  assert.deepEqual(fieldFor({ id: 'x' }, 'tintColor', { type: 'string', default: '#e1d3b3' }), { key: 'tintColor', kind: 'color', initial: '#e1d3b3' })
  assert.equal(fieldFor({ id: 'x' }, 'mode', { type: 'string', default: 'multiply' }).kind, 'choice')
  assert.equal(fieldFor({ id: 'x' }, 'mode', { type: 'string', default: 'multiply' }).initial, 'multiply')
  assert.deepEqual(fieldFor({ id: 'x' }, 'name', { type: 'string' }), { key: 'name', kind: 'text', initial: '' })
  assert.equal(fieldFor({ id: 'x' }, 'points', { type: 'array' }), null)
  assert.equal(fieldFor({ id: 'x' }, 'reds', { oneOf: [{ type: 'array' }] }), null)
})

test('the values start from the remembered ones that still fit, and an empty text leaves its key out', () => {
  const fields = [
    { key: 'radius', kind: 'slider', min: 0.1, max: 1000, softMax: 250, decimals: 1, initial: 1 },
    { key: 'name', kind: 'text', initial: '' },
    { key: 'width', kind: 'number', min: 1, max: 9, decimals: 0, initial: null },
  ]
  assert.deepEqual(initialValues(fields, { radius: 5, name: 7 }), { radius: 5, name: '', width: null })
  assert.deepEqual(initialValues(fields, { radius: 5000 }), { radius: 1, name: '', width: null })
  assert.deepEqual(paramsOf(fields, { radius: 5, name: '', width: null }), { radius: 5 })
  assert.deepEqual(paramsOf(fields, { radius: 5, name: 'Sky', width: 3 }), { radius: 5, name: 'Sky', width: 3 })
})

test('names read as words, as upstream catalogs list them', () => {
  assert.equal(humanize('radius'), 'Radius')
  assert.equal(humanize('cellSize'), 'Cell Size')
  assert.equal(humanize('gray-gamma-2.2'), 'Gray Gamma 2.2')
})

/** An engine that keeps a history of runs: `run` pushes a step, `undo` pops one, every change moves the revision. */
function fakeEngine() {
  const engine = { steps: [], revision: 0, runs: 0, refuse: false }
  return {
    engine,
    api: {
      run: async (params) => {
        if (engine.refuse) throw new Error('refused')
        engine.runs += 1
        engine.steps.push(paramsKey(params))
        engine.revision += 1
      },
      undo: async () => {
        engine.steps.pop()
        engine.revision += 1
      },
      revision: () => engine.revision,
    },
  }
}

/** Timers the test runs by hand. */
function fakeTimers() {
  const pending = new Set()
  return {
    schedule: (task) => {
      const handle = { task }
      pending.add(handle)
      return handle
    },
    unschedule: (handle) => pending.delete(handle),
    flush: () => {
      for (const handle of [...pending]) {
        pending.delete(handle)
        handle.task()
      }
    },
  }
}

test('a preview runs, a change undoes and runs again, and Cancel leaves no step', async () => {
  const { engine, api } = fakeEngine()
  const timers = fakeTimers()
  const preview = createPreview(api, timers.schedule, timers.unschedule)
  await preview.show({ radius: 1 })
  await preview.show({ radius: 5 })
  assert.deepEqual(engine.steps, [paramsKey({ radius: 5 })])
  await preview.drop()
  assert.deepEqual(engine.steps, [])
  assert.equal(preview.shown, false)
})

test('OK keeps a preview that shows its values (no second run), and adds exactly one step', async () => {
  const { engine, api } = fakeEngine()
  const timers = fakeTimers()
  const preview = createPreview(api, timers.schedule, timers.unschedule)
  await preview.show({ radius: 5 })
  // PreviewDialog ends the preview before OK; the commit right after claims it.
  preview.end()
  await preview.commit({ radius: 5 })
  timers.flush()
  await preview.drop()
  assert.deepEqual(engine.steps, [paramsKey({ radius: 5 })])
  assert.equal(engine.runs, 1)
})

test('OK with values the preview does not show replaces it; without a preview it runs once', async () => {
  const { engine, api } = fakeEngine()
  const preview = createPreview(api)
  await preview.show({ radius: 5 })
  await preview.commit({ radius: 9 })
  assert.deepEqual(engine.steps, [paramsKey({ radius: 9 })])
  const plain = fakeEngine()
  await createPreview(plain.api).commit({ width: 10 })
  assert.deepEqual(plain.engine.steps, [paramsKey({ width: 10 })])
})

test('turning Preview off drops the preview once its grace has passed', async () => {
  const { engine, api } = fakeEngine()
  const timers = fakeTimers()
  const preview = createPreview(api, timers.schedule, timers.unschedule)
  await preview.show({ radius: 5 })
  preview.end()
  assert.equal(engine.steps.length, 1)
  timers.flush()
  await preview.drop()
  assert.deepEqual(engine.steps, [])
  assert.ok(END_GRACE_MS > 0)
})

test('a preview is not undone after someone else changed the document, nor after a run that changed nothing', async () => {
  const { engine, api } = fakeEngine()
  const preview = createPreview(api)
  await preview.show({ radius: 5 })
  // A peer's edit lands on top of the preview.
  engine.steps.push('peer')
  engine.revision += 1
  await preview.drop()
  assert.deepEqual(engine.steps, [paramsKey({ radius: 5 }), 'peer'])
  // A run that moves no revision has no step to undo.
  const still = { run: async () => {}, undo: async () => assert.fail('nothing to undo'), revision: () => 7 }
  const quiet = createPreview(still)
  await quiet.show({ radius: 1 })
  await quiet.drop()
})

test('a refused run fails the preview (the dialog shows why) and the next one still runs', async () => {
  const { engine, api } = fakeEngine()
  const preview = createPreview(api)
  engine.refuse = true
  await assert.rejects(preview.show({ radius: 5 }), /refused/)
  engine.refuse = false
  await preview.show({ radius: 6 })
  assert.deepEqual(engine.steps, [paramsKey({ radius: 6 })])
})

test("Levels' midtone handle sits at 0.5^gamma between black and white", () => {
  assert.equal(gammaFraction(1), 0.5)
  assert.equal(gammaAt(0.5), 1)
  assert.equal(gammaAt(0.25), 2)
  assert.equal(midtoneInput(LEVELS_IDENTITY), 127.5)
  assert.equal(gammaForMidtone({ ...LEVELS_IDENTITY, inBlack: 55 }, 155), 1)
  assert.equal(gammaAt(0.999999999), 0.01)
})

test('Levels keep the engine ranges: black at least 2 under white, gamma 0.01 to 9.99', () => {
  assert.equal(setLevel({ ...LEVELS_IDENTITY, inWhite: 100 }, 'inBlack', 150).inBlack, 98)
  assert.equal(setLevel({ ...LEVELS_IDENTITY, inBlack: 100 }, 'inWhite', 50).inWhite, 102)
  assert.equal(setLevel(LEVELS_IDENTITY, 'gamma', 20).gamma, 9.99)
  assert.equal(setLevel(LEVELS_IDENTITY, 'outWhite', 300).outWhite, 255)
})

test("Levels' and Curves' parameters: the composite at the top level, a colour channel only when it moves", () => {
  const identity = { rgb: LEVELS_IDENTITY, red: LEVELS_IDENTITY, green: LEVELS_IDENTITY, blue: LEVELS_IDENTITY }
  assert.deepEqual(levelsParams({ ...identity, rgb: { ...LEVELS_IDENTITY, inBlack: 10 } }), { ...LEVELS_IDENTITY, inBlack: 10 })
  assert.deepEqual(levelsParams({ ...identity, red: { ...LEVELS_IDENTITY, gamma: 1.5 } }).red, { ...LEVELS_IDENTITY, gamma: 1.5 })
  assert.deepEqual(levelsFrom({ inBlack: 10, red: { gamma: 1.5 } }).red, { ...LEVELS_IDENTITY, gamma: 1.5 })
  const line = [[0, 0], [1, 1]]
  assert.deepEqual(curvesParams({ rgb: [[0, 0], [0.5, 0.6], [1, 1]], red: line, green: line, blue: [[0, 0.1], [1, 1]] }), { points: [[0, 0], [128, 153], [255, 255]], blue: [[0, 26], [255, 255]] })
  assert.deepEqual(toEngineCurve([[1, 1], [0, 0]]), [[0, 0], [255, 255]])
  assert.deepEqual(fromEngineCurve([[0, 0], [255, 255]]), [[0, 0], [1, 1]])
  assert.deepEqual(fromEngineCurve('broken'), [[0, 0], [1, 1]])
})

test('sizes convert between units at the resolution, keep proportions, and stay in the engine range', () => {
  assert.equal(toPixels(50, 'percent', 1200, 72), 600)
  assert.equal(toPixels(2, 'inches', 0, 300), 600)
  assert.equal(Math.round(toPixels(2.54, 'centimeters', 0, 300)), 300)
  assert.equal(fromPixels(600, 'percent', 1200, 72), 50)
  assert.equal(fromPixels(144, 'points', 0, 72), 144)
  assert.equal(keptProportion('width', 600, 1200, 800), 400)
  assert.equal(keptProportion('height', 400, 1200, 800), 600)
  assert.equal(side(0.4), null)
  assert.equal(side(300001), null)
  assert.equal(side(599.6), 600)
  assert.equal(memorySize(1200, 800, 3), '2.7M')
  assert.equal(anchorArrow('center', 'center'), '●')
  assert.equal(anchorArrow('center', 'topLeft'), '↖')
  assert.equal(anchorArrow('topLeft', 'bottomRight'), '')
})
