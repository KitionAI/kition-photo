// The options bars' labels: every declared field and every label a bar draws has a Chinese
// translation, from upstream's catalog (`phrase()`) or Photo's own strings. Run with
// `pnpm --dir web test:parity`.
import { readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { OPTION_CONTROLS, controlOf } from './controls.ts'
import { BLEND_MODES, CROP_RATIOS, DEFAULT_OPTIONS, TOOL_FIELDS } from '../tools/options.ts'
import { CATALOGS, PHRASES } from '../strings.ts'

const upstream = new Map(
  readFileSync(new URL('../i18n/upstream/zh-hans.tsv', import.meta.url), 'utf8')
    .split('\n')
    .filter((line) => !line.startsWith('#'))
    .map((line) => line.split('\t'))
    .filter((columns) => columns.length >= 3 && columns[0] === '')
    .map((columns) => [columns[1].replace(/\\(.)/g, '$1'), columns[2]]),
)
const phrased = (english) => upstream.has(english) || english in PHRASES['zh-CN']
const own = (key) => key in CATALOGS['en-US'] && key in CATALOGS['zh-CN']

const read = (dir) =>
  readdirSync(new URL(dir, import.meta.url))
    .filter((name) => name.endsWith('.tsx') || name.endsWith('.ts'))
    .map((name) => readFileSync(new URL(dir + name, import.meta.url), 'utf8'))
    .join('\n')
const sources = readdirSync(new URL('.', import.meta.url))
  .filter((name) => name.endsWith('.tsx'))
  .map((name) => readFileSync(new URL(name, import.meta.url), 'utf8'))
  .join('\n')
// The dock's panels draw upstream labels and Photo strings too.
const panelSources = read('../panels/')
const literalsIn = (text, call) => [...text.matchAll(new RegExp(`\\b${call}\\((['"])((?:(?!\\1)[^\\\\]|\\\\.)*)\\1`, 'g'))].map((match) => match[2].replace(/\\(.)/g, '$1'))
const literals = (call) => [...sources.matchAll(new RegExp(`\\b${call}\\((['"])((?:(?!\\1)[^\\\\]|\\\\.)*)\\1`, 'g'))].map((match) => match[2].replace(/\\(.)/g, '$1'))

test('every declared options-bar field has a label in both languages', () => {
  const missing = Object.entries(OPTION_CONTROLS).filter(([, label]) => !own(label) && !phrased(label))
  assert.deepEqual(missing, [])
})

test("every upstream label a bar draws has upstream's (or Photo's) Chinese", () => {
  const used = literals('phrase')
  assert.ok(used.length > 100, `${used.length} labels`)
  assert.deepEqual(
    used.filter((english) => !phrased(english)),
    [],
  )
  assert.deepEqual(
    [...BLEND_MODES, ...CROP_RATIOS].map(([, label]) => label).filter((english) => !phrased(english)),
    [],
  )
})

test("every Photo string a bar draws is in both of Photo's catalogs", () => {
  const used = literals('t')
  assert.ok(used.length >= 5, `${used.length} strings`)
  assert.deepEqual(
    used.filter((key) => !own(key)),
    [],
  )
})

test('every option is drawn as a declared field', () => {
  assert.deepEqual(
    Object.keys(DEFAULT_OPTIONS).filter((key) => !(controlOf(key) in OPTION_CONTROLS)),
    [],
  )
})

// Each tool's bar is held to its own list in the flow test (the bars choose fields by tool as they
// render); here, across the bars, nothing is drawn that no tool lists, nor listed that none draws.
test('the bars draw only options the tools list (TOOL_FIELDS), and every listed one', () => {
  // An option a bar draws: a bound field (`k="…"`), one it sets itself (`set({ key`), or one a field shows (`option="…"`).
  const drawn = new Set([...sources.matchAll(/\bk="(\w+)"/g), ...sources.matchAll(/\bset\(\{ (\w+)/g), ...sources.matchAll(/\boption="(\w+)"/g)].map((match) => match[1]))
  const listed = new Set(Object.values(TOOL_FIELDS).flat())
  assert.deepEqual(
    [...drawn].filter((key) => !listed.has(key)),
    [],
    'drawn but listed for no tool',
  )
  assert.deepEqual(
    [...listed].filter((key) => !drawn.has(key)),
    [],
    'listed but drawn by no bar',
  )
})

test("every upstream label and Photo string the dock's panels draw has its Chinese", () => {
  assert.deepEqual(
    literalsIn(panelSources, 'phrase').filter((english) => !phrased(english)),
    [],
  )
  assert.deepEqual(
    literalsIn(panelSources, 't').filter((key) => !own(key)),
    [],
  )
})
