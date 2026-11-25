// The canvas's right-click menu in a real browser (plan 2026-11-24, stage 7): node tests/context-menu.mjs [chromium|webkit]
// Real right clicks and keys on dev.html; every check reads the engine's state (layers, selection,
// history, the document's size) or the options bar, and prints PASS or FAIL per step.
import { openEditor } from './canvas-harness.mjs'

const which = process.argv[2] ?? 'chromium'
const e = await openEditor(which)
const { page, kit, step, until, details, inspect, menuAt, menuLabels, choose, selectLayer, layerNamed, drag, settle, shot, workspace, primary, view } = e
try {
  const ellipse = await layerNamed('Ellipse 1')
  const [ex, ey, ew, eh] = ellipse.bounds
  const centre = [ex + ew / 2, ey + eh / 2]
  const background = await layerNamed('Background')

  // ---------------------------------------------------------------- Move: the layers under the pointer
  await choose('Move')
  const before = (await inspect()).revision
  await menuAt(...centre)
  const listed = await menuLabels()
  await shot('menu-move')
  step('Move: a right click lists the layers under the pointer, topmost first, then the layer commands', listed[0] === 'Select a layer' && listed[1] === 'Ellipse 1' && listed.includes('Background') && listed.includes('Duplicate Layer…') && listed.indexOf('Ellipse 1') < listed.indexOf('Background'), listed.slice(0, 6).join(' | '))
  step('opening the menu edits nothing', (await inspect()).revision === before)
  await kit.getByTestId(`photo-canvas-menu-layer-${background.id}`).click()
  await until(async () => (await details()).activeLayer === background.id, 30)
  step('choosing a listed layer makes it the active layer', (await details()).activeLayer === background.id)
  step('the menu closes after a choice', (await kit.getByTestId('photo-canvas-menu').count()) === 0)

  // ---------------------------------------------------------------- Cmd: the layers, whatever the tool
  await choose('Gradient')
  const unchanged = (await inspect()).revision
  await menuAt(...centre, [primary])
  const onlyLayers = await menuLabels()
  step('Cmd-right-click with another tool lists only the layers under the pointer', onlyLayers[0] === 'Select a layer' && onlyLayers.includes('Ellipse 1') && !onlyLayers.includes('Duplicate Layer…'), onlyLayers.join(' | '))
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('photo-canvas-menu').count()) === 0, 20)
  step('Escape closes the menu and changes nothing', (await kit.getByTestId('photo-canvas-menu').count()) === 0 && (await inspect()).revision === unchanged)

  // ---------------------------------------------------------------- the default menu: layer commands, a "…" row asks first
  await selectLayer(ellipse.id)
  const layers = (await details()).layers.length
  await menuAt(...centre)
  await kit.getByTestId('photo-canvas-menu-layer-duplicate').click()
  const asked = await until(async () => (await kit.getByTestId('photo-dialog-ok').count()) === 1, 30)
  step('Duplicate Layer… from the menu opens its dialog instead of running', asked && (await details()).layers.length === layers)
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await details()).layers.length === layers + 1, 30)
  step('its OK duplicates the layer', (await details()).layers.length === layers + 1)
  await workspace.focus()

  // ---------------------------------------------------------------- the brush tools keep the brush picker
  await choose('Brush')
  await e.click(...centre, { button: 'right' })
  const picker = await until(async () => (await kit.getByTestId('photo-brush-picker').count()) === 1, 30)
  step('a brush tool’s right click still opens the brush preset picker, not the menu', picker && (await kit.getByTestId('photo-canvas-menu').count()) === 0)
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('photo-brush-picker').count()) === 0, 20)
  await workspace.focus()

  // ---------------------------------------------------------------- a selection tool
  await choose('RectMarquee')
  await menuAt(100, 100)
  const without = await menuLabels()
  step('a marquee without a selection offers Select All, Reselect, Color Range…, Load Selection…', JSON.stringify(without) === JSON.stringify(['Select All', 'Reselect', 'Color Range…', 'Load Selection…']), without.join(' | '))
  await kit.getByTestId('photo-canvas-menu-edit-selectAll').click()
  await until(async () => (await details()).hasSelection, 30)
  const doc = await details()
  step('Select All from the menu selects the whole canvas', JSON.stringify(doc.selectionBounds) === JSON.stringify([0, 0, doc.width, doc.height]), JSON.stringify(doc.selectionBounds))
  await workspace.focus()
  await kit.getByTestId('photo-menu-Select').click()
  await kit.getByTestId('photo-cmd-edit.deselectAll').click()
  await until(async () => !(await details()).hasSelection, 30)
  await workspace.focus()
  await drag([ex - 10, ey - 10], [ex + ew / 2, ey + eh / 2])
  await until(async () => (await details()).hasSelection, 30)
  await selectLayer(ellipse.id)
  await menuAt(ex + 5, ey + 5)
  const withSelection = await menuLabels()
  await shot('menu-selection')
  step('with a selection: Deselect, Select Inverse, Feather…, Layer via Copy and Cut, Free Transform, Fill…, Stroke…', ['Deselect', 'Select Inverse', 'Feather…', 'Save Selection…', 'Layer via Copy', 'Layer via Cut', 'Free Transform', 'Fill…', 'Stroke…'].every((label) => withSelection.includes(label)), withSelection.join(' | '))
  const historyBefore = (await details()).history.length
  await kit.getByTestId('photo-canvas-menu-select-modify-feather').click()
  const feather = await until(async () => (await kit.getByTestId('photo-dialog-ok').count()) === 1, 30)
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('photo-dialog-ok').count()) === 0, 30)
  step('Feather… opens its dialog; Escape leaves the history as it was', feather && (await details()).history.length === historyBefore)
  await workspace.focus()
  await menuAt(ex + 5, ey + 5)
  await kit.getByTestId('photo-canvas-menu-edit-deselectAll').click()
  await until(async () => !(await details()).hasSelection, 30)
  step('Deselect from the menu drops the selection', !(await details()).hasSelection)
  await workspace.focus()
  await drag([ex - 10, ey - 10], [ex + ew / 2, ey + eh / 2])
  await until(async () => (await details()).hasSelection, 30)
  const count = (await details()).layers.length
  await menuAt(ex + 5, ey + 5)
  await kit.getByTestId('photo-canvas-menu-layer-new-layerViaCopy').click()
  await until(async () => (await details()).layers.length === count + 1, 30)
  step('Layer via Copy makes a layer of the selected pixels', (await details()).layers.length === count + 1)
  await workspace.focus()

  // ---------------------------------------------------------------- the menu key and Shift+F10
  await choose('Lasso')
  await workspace.focus()
  await page.keyboard.press('Shift+F10')
  const byKeys = await until(async () => (await kit.getByTestId('photo-canvas-menu').count()) === 1, 30)
  step('Shift+F10 opens the same menu (at the canvas when the pointer is elsewhere)', byKeys && (await menuLabels()).includes('Select All'))
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('photo-canvas-menu').count()) === 0, 20)
  await workspace.focus()

  // ---------------------------------------------------------------- the Eyedropper's sample size
  await choose('Eyedropper')
  await menuAt(...centre)
  const sizes = await menuLabels()
  step('the Eyedropper offers its seven sample sizes', sizes.length === 7 && sizes[0] === 'Point Sample', sizes.join(' | '))
  await kit.getByTestId('photo-canvas-menu-sampleSize-11').click()
  await until(async () => (await kit.getByTestId('photo-option-sampleSize').inputValue()) === '11', 30)
  step('choosing 11 by 11 Average sets the options bar’s Sample Size', (await kit.getByTestId('photo-option-sampleSize').inputValue()) === '11')
  await workspace.focus()

  // ---------------------------------------------------------------- the Hand and Zoom tools
  await choose('Hand')
  await menuAt(...centre)
  await kit.getByTestId('photo-canvas-menu-view-kit-view-actualPixels').click()
  await until(async () => (await view()).zoom === 1, 30)
  step('the Hand tool’s 100% shows the picture at 100 %', (await view()).zoom === 1, String((await view()).zoom))
  await menuAt((await view()).x + 50, (await view()).y + 50)
  await kit.getByTestId('photo-canvas-menu-view-kit-view-fitOnScreen').click()
  await until(async () => (await view()).zoom < 1, 30)
  step('and Fit on Screen fits it again', (await view()).zoom < 1)
  await workspace.focus()

  // ---------------------------------------------------------------- Type
  const title = await layerNamed('Northern Lights')
  await selectLayer(title.id)
  await choose('Type')
  const [tx, ty, tw, th] = title.bounds
  await menuAt(tx + tw + 20, ty + th / 2)
  const typeRows = await menuLabels()
  step('Type on a type layer offers Edit Type, Rasterize Type, Create Work Path, Convert to Shape and the anti-aliasing', ['Edit Type', 'Rasterize Type', 'Create Work Path', 'Convert to Shape', 'Sharp'].every((label) => typeRows.includes(label)), typeRows.join(' | '))
  await kit.getByTestId('photo-canvas-menu-text-rasterizeTypeLayer').click()
  await until(async () => (await details()).layers.find((layer) => layer.id === title.id)?.kind === 'Pixel', 40)
  step('Rasterize Type makes the type layer a pixel layer', (await details()).layers.find((layer) => layer.id === title.id)?.kind === 'Pixel')
  await workspace.focus()
  await settle()

  // ---------------------------------------------------------------- Crop (last: it changes the canvas)
  await choose('Crop')
  const size = await details()
  await drag([100, 100], [500, 400])
  await page.waitForTimeout(300)
  await menuAt(300, 250)
  await shot('menu-crop')
  await kit.getByTestId('photo-canvas-menu-cropCommit').click()
  await until(async () => (await details()).width !== size.width, 40)
  // WebKit rounds native mouse positions: the frame may come out a pixel larger.
  step('Crop’s menu commits the frame', Math.abs((await details()).width - 400) <= 1 && Math.abs((await details()).height - 300) <= 1, `${(await details()).width} x ${(await details()).height}`)
  await e.finish()
} catch (error) {
  await e.fail(error)
}
