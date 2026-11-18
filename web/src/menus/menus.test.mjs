// The menu catalog's rules: engine paths, Kition's own rows, web-hidden commands, and the Window
// toggles. Run with `pnpm --dir web test:parity` (Node loads the TypeScript modules directly).
import { readFileSync } from 'node:fs'
import test from 'node:test'
import assert from 'node:assert/strict'
import { KIT_ROWS, PHOTO_MENUS, enginePaths } from './catalog.ts'
import { hiddenReason, isHiddenOnWeb } from './hidden.ts'
import { DEFAULT_WINDOWS, DOCK_GROUPS, DOCK_TABS, VIEW_ROWS, windowCommand, windowShown, zoomStep } from './kitMenus.ts'

const upstream = JSON.parse(readFileSync(new URL('../../parity/photo-upstream.json', import.meta.url), 'utf8'))
const row = (command, path = ['File']) => ({ path, label: command, shortcut: null, command })
const at = (command) => PHOTO_MENUS.filter((item) => item.command === command)

test('upstream submenus take the engine names', () => {
  const rows = enginePaths([row('filter.other.highPass', ['Filter', 'Other…']), row('filter.other.offset', ['Filter', 'Other…', 'Deeper']), row('filter.blur.gaussianBlur', ['Filter', 'Blur'])])
  assert.deepEqual(
    rows.map((item) => item.path.join('/')),
    ['Filter/Other', 'Filter/Other/Deeper', 'Filter/Blur'],
  )
  assert.equal(at('filter.other.highPass')[0].path.join('/'), 'Filter/Other')
  assert.equal(
    PHOTO_MENUS.some((item) => item.path.join('/') === 'Filter/Other…'),
    false,
  )
})

test('keeps the declared second copy of Keyboard Shortcuts', () => {
  assert.deepEqual(
    at('edit.keyboardShortcuts').map((item) => item.path.join('/')),
    ['Edit', 'Window/Workspace'],
  )
})

test('Kition rows replace upstream Window rows in place, keeping their labels and keys', () => {
  const window = PHOTO_MENUS.filter((item) => item.path.join('/') === 'Window' && item.command !== '---')
  const upstreamWindow = upstream.menus.filter((item) => item.path.join('/') === 'Window' && item.command !== '---')
  // One more row: the status bar's toggle, right after the toolbar's.
  assert.equal(window.length, upstreamWindow.length + 1)
  const layers = at('kit:window.panel.layers')[0]
  assert.deepEqual([layers.label, layers.shortcut], ['Layers', 'F7'])
  assert.deepEqual([at('kit:window.panel.color')[0].label, at('kit:window.panel.color')[0].shortcut], ['Color', 'F6'])
  assert.equal(at('window.panel.layers').length, 0)
  const commands = window.map((item) => item.command)
  assert.equal(commands.indexOf('kit:window.status'), commands.indexOf('kit:window.toolbar') + 1)
  assert.equal(commands.indexOf('kit:window.options') < commands.indexOf('kit:window.toolbar'), true)
})

test('declares Kition rows for every dock tab with a panel, the bars, Type › Panels, the View zoom rows, and Help', () => {
  assert.deepEqual(
    DOCK_GROUPS.slice(),
    upstream.dock.map((group) => group.group),
  )
  // Each group's tabs with a panel, in upstream's order, its first tab first.
  for (const group of upstream.dock) {
    const tabs = DOCK_TABS[group.group]
    assert.equal(tabs[0], group.group)
    assert.deepEqual(tabs.slice(), group.tabs.filter((tab) => tabs.includes(tab)))
  }
  const tabs = Object.values(DOCK_TABS).flat()
  assert.deepEqual(
    KIT_ROWS.map((item) => item.command).sort(),
    [
      ...tabs.map((tab) => `kit:window.panel.${tab}`),
      'kit:type.panels.character',
      'kit:type.panels.paragraph',
      ...VIEW_ROWS.map((command) => `kit:${command}`),
      'kit:window.toolbar',
      'kit:window.options',
      'kit:window.status',
      'kit:help.about',
      'kit:help.shortcuts',
    ].sort(),
  )
  // Upstream's labels and keys stay: F8 Info, Cmd+1 100%.
  assert.deepEqual([at('kit:window.panel.info')[0].label, at('kit:window.panel.info')[0].shortcut], ['Info', 'F8'])
  assert.deepEqual([at('kit:view.actualPixels')[0].label, at('kit:view.actualPixels')[0].shortcut], ['100%', 'Cmd+1'])
  assert.deepEqual(at('kit:type.panels.character')[0].path, ['Type', 'Panels'])
  // Window rows of panels to come stay upstream's (drawn disabled).
  assert.equal(at('window.panel.gradients').length, 1)
  const help = PHOTO_MENUS.filter((item) => item.path[0] === 'Help')
  assert.equal(PHOTO_MENUS.at(-1).path[0], 'Help')
  assert.deepEqual(
    help.filter((item) => item.command !== '---').map((item) => item.command),
    ['kit:help.shortcuts', 'kit:help.about'],
  )
})

test("kit rows' ids never collide with Photo's declared controls", () => {
  const ids = KIT_ROWS.map((item) => `control:${item.command.slice('kit:'.length)}`)
  for (const declared of ['control:options', 'control:colors', 'control:quickMask', 'control:toolRail.columns', 'control:status']) assert.equal(ids.includes(declared), false, declared)
  assert.equal(new Set(ids).size, ids.length)
})

test('hides commands that need a local file system or the host', () => {
  for (const command of [
    'file.new',
    'file.open',
    'file.close',
    'file.closeAll',
    'file.closeOthers',
    'file.save',
    'file.saveAs',
    'file.exit',
    'file.revert',
    'file.saveACopy',
    'file.openAs',
    'file.placeLinked',
    'file.automate.batch',
    'file.automate.photomerge',
    'file.scripts.browse',
    'file.import.notes',
    'file.export.layersToFiles',
    'file.revealInFinder',
    'layer.objectLayers.revealInFinder',
    'layer.objectLayers.relinkToFile',
    'layer.videoLayers.newVideoLayerFromFile',
    'edit.presets.migratePresets',
    'plugin.install',
    'file.newFromClipboard',
    'file.scripts.imageProcessor',
    'file.scripts.loadFilesIntoStack',
    'file.scripts.statistics',
    'file.export.artboardsToPdf',
    'file.print',
    'file.printOneCopy',
    'file.package',
    'layer.quickExportAsPng',
    'layer.exportAs',
    'file.automate.cropAndStraightenPhotos',
    'edit.preferences.historyLog',
  ]) {
    assert.equal(isHiddenOnWeb(row(command)), true, command)
    assert.ok(hiddenReason(command)?.trim(), command)
  }
  for (const command of ['file.automate.fitImage', 'file.automate.conditionalModeChange', 'file.placeEmbedded', 'file.export.quickExportAsPng', 'file.scripts.scriptEventsManager', 'filter.blur.gaussianBlur', 'kit:help.about', '---']) {
    assert.equal(isHiddenOnWeb(row(command)), false, command)
    assert.equal(hiddenReason(command), null, command)
  }
  // Crop and Straighten Photos edits the open scan, but opens a document per photo it finds.
  assert.match(hiddenReason('file.automate.cropAndStraightenPhotos'), /document/)
  // The audited rows, the engine's rows a browser cannot run, those that open new
  // documents, and the History Log preferences page (it writes a log to a local file).
  assert.equal(PHOTO_MENUS.filter(isHiddenOnWeb).length, 45)
})

test("draws the engine's commands upstream's catalog lacks, where upstream draws them", () => {
  const commands = PHOTO_MENUS.map((item) => item.command)
  const engineOnly = ['layer.mergeDown', 'layer.releaseClippingMask', 'filter.other.colorToAlpha', 'filter.render.relight', 'type.antiAlias.none', 'layer.objectLayers.stackMode.none', 'file.newFromClipboard', 'type.create', 'shape.create', 'type.rasterize', 'shape.rasterize', 'layer.vectorMask.add', 'layer.vectorMask.fromPath', 'plugin.install']
  for (const command of engineOnly) assert.equal(at(command).length, 1, command)
  // Upstream's PLACE_AFTER: right after the catalog row they belong next to.
  assert.equal(commands.indexOf('file.newFromClipboard'), commands.indexOf('file.new') + 1)
  assert.equal(commands.indexOf('filter.render.relight'), commands.indexOf('filter.render.lightingEffects') + 1)
  // The others close their submenu: nothing of the same path follows them.
  const last = (command) => {
    const index = commands.indexOf(command)
    const path = PHOTO_MENUS[index].path.join('/')
    return PHOTO_MENUS.slice(index + 1).every((item) => item.path.join('/') !== path || engineOnly.includes(item.command))
  }
  for (const command of ['layer.mergeDown', 'filter.other.colorToAlpha', 'type.antiAlias.none', 'layer.vectorMask.fromPath']) assert.equal(last(command), true, command)
  assert.deepEqual(at('filter.other.colorToAlpha')[0].path, ['Filter', 'Other'])
  // Upstream's own shell rows (its Help links and themes) stay out.
  assert.equal(commands.some((command) => command.startsWith('help.') || command.startsWith('window.theme.')), false)
})

test('Window rows show or hide their tab as upstream, Type › Panels rows always show it, and both answer their check marks', () => {
  // The dock's answer: Layers' group on Channels, Color's collapsed on Color.
  const dock = (group) => ({ layers: { tab: 'channels', collapsed: false }, color: { tab: 'color', collapsed: true } })[group] ?? null
  assert.equal(windowShown(DEFAULT_WINDOWS, 'kit:window.panel.layers'), true)
  assert.equal(windowShown(DEFAULT_WINDOWS, 'kit:window.panel.layers', dock), false)
  assert.equal(windowShown(DEFAULT_WINDOWS, 'kit:window.panel.channels', dock), true)
  // Its tab forward: the row hides the group.
  const off = windowCommand(DEFAULT_WINDOWS, 'kit:window.panel.layers')
  assert.deepEqual([off.state.panels.layers, off.reveal], [false, null])
  assert.equal(windowShown(off.state, 'kit:window.panel.layers'), false)
  assert.equal(windowShown(off.state, 'kit:window.panel.color'), true)
  // Another tab forward, or the group collapsed: the row brings its tab forward, expanded.
  const layers = windowCommand(DEFAULT_WINDOWS, 'kit:window.panel.layers', dock)
  assert.deepEqual([layers.state, layers.reveal], [DEFAULT_WINDOWS, 'layers'])
  assert.deepEqual(windowCommand(DEFAULT_WINDOWS, 'kit:window.panel.color', dock).reveal, 'color')
  // A hidden group shows on the row's tab.
  const info = windowCommand(DEFAULT_WINDOWS, 'kit:window.panel.info')
  assert.deepEqual([info.state.panels.navigator, info.reveal], [true, 'info'])
  // Type › Panels › Character shows the Character group and its tab, and never hides it.
  const character = windowCommand(DEFAULT_WINDOWS, 'kit:type.panels.character')
  assert.deepEqual([character.state.panels.character, character.reveal], [true, 'character'])
  const again = windowCommand(character.state, 'kit:type.panels.character')
  assert.deepEqual([again.state.panels.character, again.reveal], [true, 'character'])
  assert.equal(windowShown(character.state, 'kit:type.panels.character'), true)
  assert.equal(windowShown(character.state, 'kit:type.panels.paragraph'), false)
  for (const command of ['kit:window.toolbar', 'kit:window.options', 'kit:window.status']) {
    assert.equal(windowShown(windowCommand(DEFAULT_WINDOWS, command).state, command), false, command)
  }
  for (const command of ['kit:help.about', 'layer.duplicate', 'kit:view.zoomIn', 'kit:window.panel.gradients']) {
    assert.equal(windowShown(DEFAULT_WINDOWS, command), undefined, command)
    assert.deepEqual(windowCommand(DEFAULT_WINDOWS, command), { state: DEFAULT_WINDOWS, reveal: null }, command)
  }
})

test("zooms in and out through Photoshop's steps", () => {
  assert.equal(zoomStep(1, 1), 2)
  assert.equal(zoomStep(1, -1), 0.6667)
  assert.equal(zoomStep(0.4, 1), 0.5)
  assert.equal(zoomStep(0.4, -1), 0.333)
  assert.equal(zoomStep(32, 1), 32)
  assert.equal(zoomStep(0.01, -1), 0.01)
})
