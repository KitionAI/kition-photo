// The canvas's right-click menu by tool (plan 2026-11-24, stage 7): which rows each tool offers, in
// what state, and that they name engine commands the engine has (when it is built) or the
// editor's own actions. Run with `pnpm --dir web test:parity`.
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { canvasMenuRows, rowCommand, tidy } from './rows.ts'
import { PHRASES } from '../strings.ts'

const base = { tool: 'Gradient', layersOnly: false, hasSelection: false, layer: { kind: 'Pixel', background: false }, layersHere: [], activeLayer: 3, typeEditing: false, path: null, pathPart: null, penOpen: false, cropping: false, sampleSize: 'point' }
const labels = (rows) => rows.map((row) => (row.kind === 'separator' ? '---' : row.label))
const commands = (rows) => rows.map(rowCommand).filter(Boolean)

test('Move lists the layers under the pointer, topmost first, the active one checked, then the layer commands', () => {
  const rows = canvasMenuRows({ ...base, tool: 'Move', layersHere: [{ id: 5, name: 'Ellipse 1' }, { id: 3, name: 'Background' }] })
  assert.deepEqual(labels(rows).slice(0, 4), ['Select a layer', 'Ellipse 1', 'Background', '---'])
  assert.equal(rows[2].checked, true)
  assert.deepEqual(rows[1].action, { layer: 5 })
  assert.ok(commands(rows).includes('layer.duplicate'))
})

test('Cmd-right-click lists only the layers, whatever the tool', () => {
  const rows = canvasMenuRows({ ...base, tool: 'Brush', layersOnly: true, layersHere: [{ id: 5, name: 'Ellipse 1' }] })
  assert.deepEqual(labels(rows), ['Select a layer', 'Ellipse 1'])
})

test('a selection tool: with a selection what to do with it, without one how to get one', () => {
  const withSelection = canvasMenuRows({ ...base, tool: 'RectMarquee', hasSelection: true })
  for (const command of ['edit.deselectAll', 'select.inverse', 'select.modify.feather', 'layer.new.layerViaCopy', 'layer.new.layerViaCut', 'edit.fill', 'edit.stroke', 'select.saveSelection']) assert.ok(commands(withSelection).includes(command), command)
  assert.ok(withSelection.some((row) => row.kind === 'item' && row.action.own === 'freeTransform'))
  assert.deepEqual(commands(canvasMenuRows({ ...base, tool: 'Lasso' })), ['edit.selectAll', 'select.reselect', 'select.colorRange', 'select.loadSelection'])
})

test('Type: commit and cancel while editing, then the type layer’s commands', () => {
  const rows = canvasMenuRows({ ...base, tool: 'Type', typeEditing: true, layer: { kind: 'Type', background: false } })
  assert.deepEqual(labels(rows).slice(0, 3), ['Commit', 'Cancel', '---'])
  for (const command of ['text.rasterizeTypeLayer', 'text.convertToWorkPath', 'text.convertToShape', 'text.convertToParagraphText', 'text.antiAlias.sharp']) assert.ok(commands(rows).includes(command), command)
  assert.equal(rows.find((row) => row.label === 'Edit Type').disabled, true)
  // Not on type: the layer commands.
  assert.ok(commands(canvasMenuRows({ ...base, tool: 'Type' })).includes('layer.duplicate'))
})

test('the path tools: the point under the pointer, then the work path’s or the shape’s commands', () => {
  const work = canvasMenuRows({ ...base, tool: 'DirectSelection', path: 'work', pathPart: 'anchor' })
  assert.equal(work[0].label, 'Delete Anchor Point')
  assert.deepEqual(commands(work), ['path.toSelection', 'path.fill', 'path.stroke', 'layer.vectorMask.fromPath', 'path.delete'])
  assert.deepEqual(work.find((row) => rowCommand(row) === 'path.fill').action.params, { name: 'work' })
  const shape = canvasMenuRows({ ...base, tool: 'Rectangle', path: 'shape', pathPart: 'segment' })
  assert.equal(shape[0].label, 'Add Anchor Point')
  assert.deepEqual(shape.find((row) => rowCommand(row) === 'path.toSelection').action.params, { name: 'layer' })
  assert.equal(canvasMenuRows({ ...base, tool: 'Pen', penOpen: true, path: 'work' })[0].label, 'Close Path')
})

test('Crop, the Eyedropper, the Hand and Zoom, and every other tool', () => {
  assert.deepEqual(canvasMenuRows({ ...base, tool: 'Crop' }).map((row) => [row.label, row.disabled]), [['Crop', true], ['Cancel', true]])
  const sizes = canvasMenuRows({ ...base, tool: 'Eyedropper', sampleSize: '5' })
  assert.equal(sizes.length, 7)
  assert.equal(sizes.find((row) => row.checked).label, '5 by 5 Average')
  assert.deepEqual(canvasMenuRows({ ...base, tool: 'Zoom' }).filter((row) => row.kind === 'item').map((row) => row.action.value)[0], 'kit:view.fitOnScreen')
  assert.ok(commands(canvasMenuRows(base)).includes('layer.layerStyle.blendingOptions'))
})

test('separators never lead, trail or come twice', () => {
  assert.deepEqual(labels(tidy([{ kind: 'separator' }, { kind: 'item', label: 'A', action: { own: 'view' } }, { kind: 'separator' }, { kind: 'separator' }, { kind: 'item', label: 'B', action: { own: 'view' } }, { kind: 'separator' }])), ['A', '---', 'B'])
})

const tools = ['Move', 'RectMarquee', 'Lasso', 'Type', 'Pen', 'DirectSelection', 'Rectangle', 'Hand', 'Crop', 'Eyedropper', 'Gradient', 'Note']
const states = [{}, { hasSelection: true }, { typeEditing: true, layer: { kind: 'Type', background: false } }, { path: 'work', pathPart: 'segment', penOpen: true }, { path: 'shape', pathPart: 'anchor' }, { layersOnly: true, layersHere: [{ id: 1, name: 'x' }] }]
const allRows = () => tools.flatMap((tool) => states.flatMap((state) => canvasMenuRows({ ...base, ...state, tool })))

test('every label has a Chinese name, from upstream’s catalog or Photo’s phrases', () => {
  const zhHans = new Set(
    readFileSync(new URL('../i18n/upstream/zh-hans.tsv', import.meta.url), 'utf8')
      .split('\n')
      .map((line) => line.split('\t'))
      .filter((columns) => columns[0] === '' && columns[2])
      .map((columns) => columns[1]),
  )
  const missing = [...new Set(allRows().filter((row) => row.kind !== 'separator' && !(row.kind === 'item' && 'layer' in row.action)).map((row) => row.label))].filter((label) => !zhHans.has(label) && !PHRASES['zh-CN'][label])
  assert.deepEqual(missing, [])
})

const engineDir = new URL('../../public/engine/', import.meta.url)
const built = existsSync(new URL('kition_photo_kit_bg.wasm', engineDir))
test('every command a row runs is one the engine has', { skip: built ? false : 'the web engine is not built' }, async () => {
  const module = await import(new URL('kition_photo_kit.js', engineDir).href)
  module.initSync({ module: readFileSync(new URL('kition_photo_kit_bg.wasm', engineDir)) })
  const engine = new Set(JSON.parse(module.PhotoDocument.create('Untitled', 16, 16).commands()).map((command) => command.id))
  const unknown = [...new Set(allRows().map(rowCommand).filter(Boolean))].filter((command) => !engine.has(command))
  assert.deepEqual(unknown, [])
})
