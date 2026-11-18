// The tool catalog against upstream's manifest, its strings, and upstream's tool keys. Run with
// `pnpm --dir web test:parity` (Node loads the TypeScript modules directly).
import { readFileSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { PHOTO_TOOLS, VIEW, isToolKey, toolForKey } from './catalog.ts'
import { PHRASES } from '../strings.ts'

const upstream = JSON.parse(readFileSync(new URL('../../parity/photo-upstream.json', import.meta.url), 'utf8'))
const status = JSON.parse(readFileSync(new URL('../../parity/photo-status.json', import.meta.url), 'utf8'))

const anything = () => true

test("has upstream's 49 tools in its order, sections, slots, and keys", () => {
  assert.equal(PHOTO_TOOLS.length, 49)
  assert.deepEqual(
    PHOTO_TOOLS.map((tool) => [tool.id, tool.section, tool.slot, tool.key]),
    upstream.tools.map((tool) => [tool.id, tool.section, tool.slot, tool.key]),
  )
})

test('a slot is one rail-wide group, and no two slots share one', () => {
  const slots = new Map()
  for (const tool of PHOTO_TOOLS) {
    const key = `${tool.section}/${tool.slot}`
    assert.equal(slots.get(key) ?? tool.group, tool.group, key)
    slots.set(key, tool.group)
  }
  assert.equal(new Set(slots.values()).size, slots.size)
})

test("every tool's label is upstream's, with a Chinese name upstream's catalog or Photo's phrases give", () => {
  assert.deepEqual(
    PHOTO_TOOLS.map((tool) => tool.label),
    upstream.tools.map((tool) => tool.label),
  )
  const zhHans = new Set(
    readFileSync(new URL('../i18n/upstream/zh-hans.tsv', import.meta.url), 'utf8')
      .split('\n')
      .map((line) => line.split('\t'))
      .filter((columns) => columns[0] === '' && columns[2])
      .map((columns) => columns[1]),
  )
  const missing = PHOTO_TOOLS.filter((tool) => !zhHans.has(tool.label) && !PHRASES['zh-CN'][tool.label]).map((tool) => tool.label)
  assert.deepEqual(missing, [])
})

test('every tool runs an engine command or moves the view, and its status says so', () => {
  for (const tool of PHOTO_TOOLS) {
    assert.ok(tool.command === VIEW || /^[a-z]+(\.[A-Za-z0-9]+)+$/.test(tool.command), tool.id)
    const entry = status[`tool:${tool.id}`]
    assert.ok(entry.status === 'done' || (entry.status === 'todo' && entry.reason), `tool:${tool.id} is done, or todo with a reason`)
  }
})

const DEFAULT = false
const SHIFT_SWITCH = true

test('by default the key cycles within the active slot, with or without Shift, as upstream', () => {
  for (const shift of [false, true]) {
    assert.equal(toolForKey('b', shift, DEFAULT, 'Move', anything), 'Brush')
    assert.equal(toolForKey('b', shift, DEFAULT, 'Brush', anything), 'Pencil')
    assert.equal(toolForKey('b', shift, DEFAULT, 'Pencil', anything), 'MixerBrush')
    assert.equal(toolForKey('b', shift, DEFAULT, 'MixerBrush', anything), 'Brush')
  }
  assert.equal(toolForKey('q', false, DEFAULT, 'Move', anything), null)
})

test("from another slot the key selects the slot's first tool, not the last used one", () => {
  assert.equal(toolForKey('m', false, DEFAULT, 'Move', anything), 'RectMarquee')
  assert.equal(toolForKey('m', false, SHIFT_SWITCH, 'Move', anything), 'RectMarquee')
  assert.equal(toolForKey('m', true, SHIFT_SWITCH, 'Move', anything), 'RectMarquee')
})

test('with Use Shift Key for Tool Switch, Shift and the key cycles and the key keeps the tool', () => {
  assert.equal(toolForKey('b', true, SHIFT_SWITCH, 'Brush', anything), 'Pencil')
  assert.equal(toolForKey('b', true, SHIFT_SWITCH, 'MixerBrush', anything), 'Brush')
  assert.equal(toolForKey('b', false, SHIFT_SWITCH, 'MixerBrush', anything), 'MixerBrush')
})

test('a key never chooses a tool the host denies, in either mode', () => {
  for (const [shift, mode] of [
    [false, DEFAULT],
    [true, SHIFT_SWITCH],
  ]) {
    // Cycling passes over it.
    assert.equal(toolForKey('b', shift, mode, 'MixerBrush', (id) => id !== 'Brush'), 'Pencil')
    assert.equal(toolForKey('b', shift, mode, 'Brush', (id) => id !== 'Pencil'), 'MixerBrush')
    // From another slot, the slot's first allowed tool; a slot with none gives nothing.
    assert.equal(toolForKey('b', shift, mode, 'Lasso', (id) => id !== 'Brush'), 'Pencil')
    assert.equal(toolForKey('y', shift, mode, 'Move', (id) => id !== 'HistoryBrush'), null)
  }
})

test('tool keys are the letters upstream binds', () => {
  assert.equal(isToolKey('b'), true)
  assert.equal(isToolKey('x'), false)
})
