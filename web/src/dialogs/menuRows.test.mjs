// The "…" menu rows against the built web engine (plan 2026-11-24, stage 1): none runs bare with
// the engine's defaults, every generic dialog's control has a Chinese label, and every dialog's
// starting values run without the engine refusing them. Skipped until `pnpm --dir web engine`
// has built the engine. Run with `pnpm --dir web test:parity` (node --test).
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { PHOTO_MENUS } from '../menus/catalog.ts'
import { isHiddenInMenus } from '../menus/hidden.ts'
import { CATALOGS } from '../strings.ts'
import { blendLabel, humanize, initialValues, paramFields, paramsOf } from './params.ts'
import { dialogFor, isEllipsis } from './routes.ts'

const engineDir = new URL('../../public/engine/', import.meta.url)
const sdk = existsSync(new URL('kition_photo_kit_bg.wasm', engineDir))
  ? await (async () => {
      const module = await import(new URL('kition_photo_kit.js', engineDir).href)
      module.initSync({ module: readFileSync(new URL('kition_photo_kit_bg.wasm', engineDir)) })
      return module
    })()
  : null
const skip = sdk ? false : 'the web engine is not built'
const engine = sdk ? new Map(JSON.parse(sdk.PhotoDocument.create('Untitled', 16, 16).commands()).map((command) => [command.id, command])) : new Map()

/** The "…" rows the menus draw that the engine has, with the dialog each opens (null: runs at once). */
const rows = () =>
  [...new Map(PHOTO_MENUS.filter((row) => isEllipsis(row.label) && !isHiddenInMenus(row) && engine.has(row.command)).map((row) => [row.command, row])).values()].map((row) => {
    const command = engine.get(row.command)
    return { command, kind: dialogFor({ id: command.id, title: command.title, asks: paramFields(command).length > 0 }) }
  })

test('no "…" row runs with the engine\'s defaults: each opens a dialog, or has nothing at all to ask', { skip }, () => {
  const bare = rows().filter((row) => row.kind === null).map((row) => row.command.id)
  // Layer from Background and New Adjustment Layer › Invert take no values: running them is all there is.
  assert.deepEqual(bare.sort(), ['layer.new.layerFromBackground', 'layer.newAdjustmentLayer.invert'])
  for (const id of bare) {
    const own = Object.keys(engine.get(id).schema ?? {}).filter((key) => !['at', 'coalesce', 'layer', 'region', 'path'].includes(key))
    assert.deepEqual(own, [], `${id} has values to ask`)
  }
})

/** Upstream's Chinese phrases (plain UI text, the first column empty). */
const upstreamChinese = new Set(
  readFileSync(new URL('../i18n/upstream/zh-hans.tsv', import.meta.url), 'utf8')
    .split('\n')
    .filter((line) => !line.startsWith('#'))
    .map((line) => line.split('\t'))
    .filter(([context, , chinese]) => context === '' && chinese)
    .map(([, english]) => english),
)

test("every generic dialog's control and choice has a Chinese label (the editor's own or upstream's)", { skip }, () => {
  const zh = CATALOGS['zh-CN']
  const en = CATALOGS['en-US']
  const missing = []
  for (const { command, kind } of rows()) {
    if (kind !== 'params') continue
    for (const field of paramFields(command)) {
      if (!zh[`param.${field.key}`] && !upstreamChinese.has(humanize(field.key))) missing.push(`${command.id}: ${field.key}`)
      if (field.kind === 'choice') for (const option of field.options) if (!zh[`option.${option}`] && !upstreamChinese.has(blendLabel(option) ?? humanize(option))) missing.push(`${command.id}: ${field.key}=${option}`)
    }
  }
  assert.deepEqual(missing, [], 'labels without a Chinese translation (add them to strings.ts)')
  // The editor's own labels exist in both languages.
  const own = (catalog) => Object.keys(catalog).filter((key) => key.startsWith('param.') || key.startsWith('option.') || key.startsWith('dialog.')).sort()
  assert.deepEqual(own(zh), own(en))
})

/** A document with a painted, selected layer: what most dialogs act on. */
function paintedDocument() {
  const doc = sdk.PhotoDocument.create('Untitled', 64, 64)
  for (const [command, params] of [
    ['layer.new.layer', {}],
    ['edit.selectAll', {}],
    ['edit.fill', { contents: 'foreground' }],
    ['select.saveSelection', { name: 'Alpha 1' }],
  ])
    doc.execute(command, JSON.stringify(params))
  return doc
}

test("every generic dialog's starting values run in the engine (the rows that failed bare included)", { skip }, () => {
  const refused = []
  let ran = 0
  for (const { command, kind } of rows()) {
    if (kind !== 'params') continue
    const doc = paintedDocument()
    try {
      if (JSON.parse(doc.disabledCommands()).includes(command.id)) continue
      const fields = paramFields(command)
      const params = paramsOf(fields, initialValues(fields))
      // Load Selection's channel comes from the document (its saved selections first).
      if (command.id === 'select.loadSelection') params.channel = 'Alpha 1'
      doc.execute(command.id, JSON.stringify(params))
      ran += 1
    } catch (error) {
      refused.push(`${command.id}: ${String(error).slice(0, 120)}`)
    } finally {
      doc.free()
    }
  }
  assert.deepEqual(refused, [], 'dialogs whose starting values the engine refuses')
  assert.ok(ran > 100, `ran ${ran}`)
})
