// Checks that photo-status.json tracks exactly the items photo-upstream.json declares, that
// every "hidden" item explains why, that every follow-up (`followUps: [{ wave, item }]`, a gap a
// "done" entry leaves) is due in a later wave, and that the editor's menu catalog agrees with the
// status file and (once `pnpm --dir web engine` has built it) the engine's menu paths. Run with
// `pnpm --dir web test:parity`.
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { PHOTO_MENUS, SECOND_COPIES } from '../src/menus/catalog.ts'
import { hiddenReason, isHiddenOnWeb } from '../src/menus/hidden.ts'
import { KIT_TAKEOVERS } from '../src/menus/kitMenus.ts'
import { PHOTO_TOOLS, VIEW } from '../src/tools/catalog.ts'

const manifest = JSON.parse(readFileSync(new URL('./photo-upstream.json', import.meta.url), 'utf8'))
const status = JSON.parse(readFileSync(new URL('./photo-status.json', import.meta.url), 'utf8'))

/** Every key the manifest implies, in the same `menu:`/`tool:`/`dock:`/`options:` form the
 * plan's task describes. */
function manifestKeys(m) {
  const keys = new Set()
  for (const row of m.menus) {
    if (row.command === '---') continue
    keys.add(`menu:${row.path.join('/')}/${row.command}`)
  }
  for (const t of m.tools) keys.add(`tool:${t.id}`)
  for (const g of m.dock) for (const tab of g.tabs) keys.add(`dock:${g.group}/${tab}`)
  for (const o of m.options) for (const toolId of o.tools) for (const field of o.fields) keys.add(`options:${toolId}/${field}`)
  return keys
}

const expectedKeys = manifestKeys(manifest)
const statusKeys = new Set(Object.keys(status))

test('every manifest item has a status entry', () => {
  const missing = [...expectedKeys].filter((k) => !statusKeys.has(k))
  assert.deepEqual(missing, [], `missing status entries: ${missing.slice(0, 20).join(', ')}`)
})

test('no status entry names an item the manifest lacks', () => {
  const extra = [...statusKeys].filter((k) => !expectedKeys.has(k))
  assert.deepEqual(extra, [], `status entries with no manifest item: ${extra.slice(0, 20).join(', ')}`)
})

test('hidden entries have a non-empty reason', () => {
  const badHidden = Object.entries(status)
    .filter(([, v]) => v.status === 'hidden')
    .filter(([, v]) => typeof v.reason !== 'string' || v.reason.trim() === '')
    .map(([k]) => k)
  assert.deepEqual(badHidden, [], `hidden entries missing a reason: ${badHidden.slice(0, 20).join(', ')}`)
})

test('status entries use a known status and wave', () => {
  const bad = Object.entries(status)
    .filter(([, v]) => !['done', 'hidden', 'todo'].includes(v.status) || ![1, 2, 3].includes(v.wave))
    .map(([k]) => k)
  assert.deepEqual(bad, [], `status entries with an invalid status or wave: ${bad.slice(0, 20).join(', ')}`)
})

test('web-hidden menu rows, and only they, are hidden in the status file', () => {
  const wrong = manifest.menus
    .filter((row) => row.command !== '---')
    .filter((row) => (status[`menu:${row.path.join('/')}/${row.command}`]?.status === 'hidden') !== isHiddenOnWeb(row))
    .map((row) => row.command)
  assert.deepEqual(wrong, [], `status disagrees with isHiddenOnWeb: ${wrong.join(', ')}`)
})

const engineDir = new URL('../public/engine/', import.meta.url)
const sdk = existsSync(new URL('kition_photo_kit_bg.wasm', engineDir))
  ? await (async () => {
      const module = await import(new URL('kition_photo_kit.js', engineDir).href)
      module.initSync({ module: readFileSync(new URL('kition_photo_kit_bg.wasm', engineDir)) })
      return module
    })()
  : null
const engine = sdk ? new Map(JSON.parse(sdk.PhotoDocument.create('Untitled', 16, 16).commands()).map((command) => [command.id, command])) : null

/** Runs `command` with no values on a document with a painted, selected layer, and returns its error, if any. */
function runBare(command) {
  const doc = sdk.PhotoDocument.create('Untitled', 64, 64)
  for (const [setup, params] of [
    ['layer.new.layer', '{}'],
    ['select.all', '{}'],
    ['edit.fill', '{"contents":"foreground"}'],
  ])
    doc.execute(setup, params)
  try {
    doc.execute(command, '{}')
    return null
  } catch (error) {
    return String(error)
  }
}

/**
 * Engine commands a browser cannot run that the menus still draw (disabled), with why. Every other
 * one is hidden on the web.
 */
const WEB_LATER = { 'file.placeEmbedded': 'Places a file a browser file picker can give the engine as bytes.' }
const NO_FILE_SYSTEM = /not available on the web|missing `path`/

test("engine rows sit under the engine's menu path", { skip: engine ? false : 'the web engine is not built' }, () => {
  const differ = []
  const noMenu = []
  for (const row of PHOTO_MENUS) {
    const command = engine.get(row.command)
    if (!command) continue
    const path = row.path.join('/')
    if (!command.menu?.length) noMenu.push(row.command)
    else if (command.menu.join('/') !== path && SECOND_COPIES[row.command] !== path) differ.push(`${row.command}: ${path} vs ${command.menu.join('/')}`)
  }
  assert.deepEqual(differ, [], 'catalog paths that differ from the engine (Kit UI enforces under the engine path)')
  console.log(`engine commands without a menu path (controllable only by command: ids): ${noMenu.join(' ')}`)
})

test("every engine command with a menu path is a menu row, as upstream draws them, or hidden on the web", { skip: engine ? false : 'the web engine is not built' }, () => {
  const rows = new Set(PHOTO_MENUS.map((row) => row.command))
  const missing = [...engine.values()].filter((command) => command.menu?.length && !rows.has(command.id) && !hiddenReason(command.id)).map((command) => command.id)
  assert.deepEqual(missing, [], 'engine commands the menus lose (rerun parity/extract.mjs)')
})

test('engine rows a browser cannot run are hidden on the web', { skip: engine ? false : 'the web engine is not built' }, () => {
  const runnable = PHOTO_MENUS.filter((row) => engine.has(row.command) && !isHiddenOnWeb(row) && !WEB_LATER[row.command])
  const refused = [...new Set(runnable.map((row) => row.command))].filter((command) => NO_FILE_SYSTEM.test(runBare(command) ?? ''))
  assert.deepEqual(refused, [], 'rows the web engine refuses for want of a file system or a local path')
})

test('done menu rows run in the engine or are Kition rows', { skip: engine ? false : 'the web engine is not built' }, () => {
  const wrong = Object.entries(status)
    .filter(([key, value]) => key.startsWith('menu:') && value.status === 'done')
    .map(([key]) => key.split('/').pop())
    .filter((command) => !engine.has(command) && !KIT_TAKEOVERS[command])
  assert.deepEqual(wrong, [], `done rows the engine lacks: ${wrong.join(', ')}`)
})

test("every tool drives one of the engine's commands, or the view", { skip: engine ? false : 'the web engine is not built' }, () => {
  const missing = PHOTO_TOOLS.filter((tool) => tool.command !== VIEW && !engine.has(tool.command)).map((tool) => `${tool.id}: ${tool.command}`)
  assert.deepEqual(missing, [], 'tool commands the engine lacks')
})

test("follow-ups name an item and a wave after their entry's", () => {
  const bad = Object.entries(status)
    .flatMap(([key, value]) => (value.followUps === undefined ? [] : Array.isArray(value.followUps) ? value.followUps.map((followUp) => [key, value, followUp]) : [[key, value, null]]))
    .filter(([, value, followUp]) => !followUp || typeof followUp.item !== 'string' || followUp.item.trim() === '' || ![1, 2, 3].includes(followUp.wave) || followUp.wave <= value.wave)
    .map(([key, , followUp]) => `${key}: ${JSON.stringify(followUp)}`)
  assert.deepEqual(bad, [], 'a follow-up needs an item and a later wave than its entry')
})

test('wave 1 is finished: every wave-1 item is done or hidden, and every todo deferred from it says why', () => {
  const open = Object.entries(status)
    .filter(([, v]) => v.wave === 1 && v.status === 'todo')
    .map(([key]) => key)
  assert.deepEqual(open, [], 'wave-1 items still todo (finish them, or defer them to a later wave with a reason)')
  // Tools and dock tabs were all wave 1: one still todo was deferred, and names why.
  const silent = Object.entries(status)
    .filter(([key, v]) => v.status === 'todo' && (key.startsWith('tool:') || key.startsWith('dock:')) && !(typeof v.reason === 'string' && v.reason.trim()))
    .map(([key]) => key)
  assert.deepEqual(silent, [], 'deferred tools and dock tabs without a reason')
})

test('prints counts per wave and status, and the follow-ups', () => {
  const byWave = {}
  const byStatus = {}
  const followUps = {}
  for (const [key, v] of Object.entries(status)) {
    byWave[v.wave] = (byWave[v.wave] ?? 0) + 1
    byStatus[v.status] = (byStatus[v.status] ?? 0) + 1
    for (const followUp of v.followUps ?? []) (followUps[followUp.wave] ??= []).push(`${key}: ${followUp.item}`)
  }
  console.log(`parity manifest: ${expectedKeys.size} items`)
  console.log(`  by wave: ${JSON.stringify(byWave)}`)
  console.log(`  by status: ${JSON.stringify(byStatus)}`)
  console.log(`  follow-ups by wave: ${JSON.stringify(Object.fromEntries(Object.entries(followUps).map(([wave, items]) => [wave, items.length])))}`)
  for (const [wave, items] of Object.entries(followUps)) for (const item of items) console.log(`    wave ${wave}: ${item}`)
  // Todo items with a reason (deferred from wave 1), by reason.
  const deferred = new Map()
  for (const [key, v] of Object.entries(status)) if (v.status === 'todo' && v.reason) deferred.set(`wave ${v.wave}: ${v.reason}`, [...(deferred.get(`wave ${v.wave}: ${v.reason}`) ?? []), key])
  console.log(`  deferred with a reason: ${[...deferred.values()].flat().length}`)
  for (const [reason, keys] of deferred) console.log(`    ${keys.length} × ${reason}`)
})
