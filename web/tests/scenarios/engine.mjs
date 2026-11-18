import assert from 'node:assert/strict'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const engine = new URL('../../public/engine/', import.meta.url)
for (const file of ['kition_photo_kit.js', 'kition_photo_kit_bg.wasm']) {
  assert.ok(existsSync(new URL(file, engine)), `Missing ${file}; run pnpm engine before testing`)
}
export const sdk = await import(new URL('kition_photo_kit.js', engine).href)
sdk.initSync({ module: readFileSync(new URL('kition_photo_kit_bg.wasm', engine)) })
export const font = new URL('../../public/fonts/DroidSansFallbackFull.ttf', import.meta.url)
export const cases = JSON.parse(readFileSync(new URL('./cases.json', import.meta.url), 'utf8'))
assert.equal(new Set(cases.map((scenario) => scenario.id)).size, cases.length, 'Duplicate scenario IDs')

export function loadFont() {
  assert.ok(existsSync(font), 'Chinese scenario requires kition-fonts; rebuild with KITION_FONTS_DIR set')
  assert.ok(sdk.addFont(readFileSync(font)) > 0, 'CJK font must load successfully')
}

export function inspect(doc) { return JSON.parse(doc.inspect()) }
export function execute(doc, command, params = {}) { return JSON.parse(doc.execute(command, JSON.stringify(params))) }
export function pixels(doc) { return doc.render(0, 0, 1, doc.width, doc.height) }

// Stable file semantics, excluding session IDs, selection/focus and history, which a PSD does not store.
export function structure(state) {
  const layer = (item) => Object.fromEntries([
    ...['name', 'kind', 'opacity', 'blend', 'visible', 'clipped', 'fill', 'hasMask'].map((key) => [key, item[key]]),
    ...(item.children ? [['children', item.children.map(layer)]] : []),
  ])
  return { width: state.width, height: state.height, mode: state.mode, depth: state.depth, layers: state.layers.map(layer) }
}

function fields(actual, expected, label) {
  for (const [path, value] of Object.entries(expected ?? {})) {
    assert.deepEqual(path.split('.').reduce((node, key) => node?.[key], actual), value, `${label}.${path}`)
  }
}

export function check(doc, expected = {}, result) {
  const state = inspect(doc)
  fields(state, expected.document, 'document')
  fields(state.layers.find((layer) => layer.id === state.activeLayer), expected.activeLayer, 'activeLayer')
  fields(result, expected.result, 'result')
  for (const { at: [x, y], rgba, tolerance = 1 } of expected.pixels ?? []) {
    assert.ok(x >= 0 && x < doc.width && y >= 0 && y < doc.height, 'Pixel assertion must be inside the canvas')
    const actual = [...doc.render(x, y, 1, 1, 1)]
    assert.equal(actual.length, 4)
    actual.forEach((channel, i) => assert.ok(Math.abs(channel - rgba[i]) <= tolerance,
      `pixel (${x},${y}) expected ${rgba}, got ${actual} (tolerance ${tolerance})`))
  }
}

export function snapshot(doc) { return { pixels: pixels(doc), structure: structure(inspect(doc)), png: doc.save('checkpoint.png') } }

function differenceSvg(actual, expected, width, height) {
  const cells = []
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const offset = (y * width + x) * 4
      const delta = Math.max(...[0, 1, 2, 3].map((c) => Math.abs(actual[offset + c] - expected[offset + c])))
      if (delta) cells.push(`<rect x="${x}" y="${y}" width="1" height="1" fill="rgb(${Math.min(255, delta * 8)},0,0)"/>`)
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}" shape-rendering="crispEdges"><title>RGBA difference, maximum channel delta amplified 8 times</title><rect width="100%" height="100%" fill="black"/>${cells.join('')}</svg>`
}
export function same(doc, saved, pixelsOnly = false, tolerance = {}) {
  assert.ok(saved, 'Checkpoint must exist')
  assert.deepEqual([doc.width, doc.height], [saved.structure.width, saved.structure.height], 'Canvas dimensions')
  const actual = pixels(doc)
  assert.equal(actual.length, saved.pixels.length)
  let changed = 0, maxDifference = 0
  for (let i = 0; i < actual.length; i++) {
    const delta = Math.abs(actual[i] - saved.pixels[i])
    if (delta > (tolerance.pixels ?? 0)) changed++
    maxDifference = Math.max(delta, maxDifference)
  }
  assert.equal(changed, 0, `${changed} RGBA channels changed; maximum difference ${maxDifference}`)
  if (!pixelsOnly) {
    const actualStructure = structure(inspect(doc))
    const compare = (actual, expected) => {
      assert.equal(actual.length, expected.length, 'Layer count')
      for (let i = 0; i < actual.length; i++) {
        assert.ok(Math.abs(actual[i].opacity - expected[i].opacity) <= (tolerance.opacity ?? 0), 'Layer opacity')
        actual[i].opacity = expected[i].opacity
        if (actual[i].children && expected[i].children) compare(actual[i].children, expected[i].children)
      }
    }
    compare(actualStructure.layers, saved.structure.layers)
    assert.deepEqual(actualStructure, saved.structure, 'Layer structure and properties')
  }
}

export function createFixture(name = 'Scenario.psd', commands = [], width = 128, height = 96) {
  const doc = sdk.PhotoDocument.create(name, width, height)
  try {
    for (const { command, params } of commands) execute(doc, command, params)
    return Buffer.from(doc.save(name))
  } finally { doc.free() }
}

export async function runScenario(scenario, onStep = () => {}) {
  let doc = sdk.PhotoDocument.create(`${scenario.id}.psd`, scenario.width, scenario.height)
  const checkpoints = new Map()
  let index = 0
  try {
    if (scenario.font) loadFont()
    for (const step of scenario.steps) {
      index++
      let result
      if (step.command) {
        if (step.error) {
          const before = doc.inspect()
          assert.throws(() => execute(doc, step.command, step.params), new RegExp(step.error, 'i'))
          assert.equal(doc.inspect(), before, 'Rejected command must not mutate state or history')
        } else result = execute(doc, step.command, step.params)
      } else if (step.checkpoint) {
        checkpoints.set(step.checkpoint, snapshot(doc))
      } else if (step.reopen) {
        const name = `${scenario.id}.${step.reopen}`
        const dirty = doc.dirty
        const bytes = doc.save(name)
        assert.ok(bytes.length > 0, 'Save produced bytes')
        assert.equal(doc.dirty, dirty, 'Serializing must not acknowledge a host write')
        const reopened = sdk.PhotoDocument.open(name, bytes)
        doc.free()
        doc = reopened
        assert.equal(doc.dirty, false, 'Reopened file is clean')
      } else if (step.invalidOpen) {
        const before = doc.inspect()
        assert.throws(() => sdk.PhotoDocument.open('broken.psd', new Uint8Array([56, 66, 80, 83])))
        assert.equal(doc.inspect(), before, 'Failed open must leave existing document intact')
      } else throw new Error(`Unknown scenario step: ${JSON.stringify(step)}`)
      check(doc, step.expect, result)
      if (step.equals) same(doc, checkpoints.get(step.equals), step.pixelsOnly, step.tolerance)
      await onStep({ index, step, state: inspect(doc), doc })
    }
  } catch (error) {
    const dir = new URL(`../../test-results/engine/${scenario.id}/`, import.meta.url)
    mkdirSync(dir, { recursive: true })
    writeFileSync(new URL('failure.json', dir), JSON.stringify({ scenario: scenario.id, step: index, operation: scenario.steps[index - 1], state: inspect(doc), error: String(error) }, null, 2))
    writeFileSync(new URL('actual.png', dir), doc.save('actual.png'))
    const expected = checkpoints.get(scenario.steps[index - 1]?.equals)
    if (expected) {
      writeFileSync(new URL('expected.png', dir), expected.png)
      if (doc.width === expected.structure.width && doc.height === expected.structure.height) {
        writeFileSync(new URL('difference.svg', dir), differenceSvg(pixels(doc), expected.pixels, doc.width, doc.height))
      }
    }
    throw new Error(`${scenario.id}, step ${index}: ${error.message ?? error}\nArtifacts: ${fileURLToPath(dir)}`, { cause: error })
  } finally { doc.free() }
}
