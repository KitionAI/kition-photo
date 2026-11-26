// The fixes after the final walkthrough (plan 2026-11-24, stage 8) in a real browser: node tests/polish.mjs [chromium|webkit]
// Real clicks, drags and keys on dev.html; each check reads the engine's state through the
// development host or what the page shows, and prints PASS or FAIL per step.
import { openEditor } from './canvas-harness.mjs'

const which = process.argv[2] ?? 'chromium'
const e = await openEditor(which)
const { page, kit, step, until, details, choose, workspace, selectLayer, tool, shot } = e
const focused = () => kit.locator('body').evaluate(() => document.activeElement?.getAttribute('data-testid') ?? document.activeElement?.tagName ?? null)
/** Presses `key` and waits for the tool to become `expected` (answers the tool then). */
const pressFor = async (key, expected) => {
  await page.keyboard.press(key)
  await until(async () => (await tool()) === expected, 20)
  return tool()
}
try {
  // ---------------------------------------------------------------- 1. the keyboard never falls to the page
  // A control that goes away or turns disabled under the focus leaves the keys with the editor.
  const doc = await details()
  const pixel = doc.layers.find((layer) => layer.kind === 'Pixel' && !layer.background && !layer.hasMask) ?? doc.layers.find((layer) => layer.kind === 'Pixel')
  await selectLayer(pixel.id)
  await choose('Move')
  await kit.getByTestId('photo-layers-mask').click()
  await until(async () => (await details()).layers.find((layer) => layer.id === pixel.id)?.hasMask === true, 40)
  step('after Layers’ Add Mask, B chooses the Brush at once (the focus is back with the editor)', (await pressFor('b', 'Brush')) === 'Brush', `${await tool()}, focus ${await focused()}`)
  const shape = doc.layers.find((layer) => layer.kind !== 'Group' && !layer.background && layer.id !== pixel.id) ?? pixel
  await selectLayer(shape.id)
  await choose('Move')
  await page.keyboard.press(`${e.primary}+t`)
  await until(async () => (await kit.getByTestId('photo-transform-commit').count()) === 1, 30)
  await kit.getByTestId('photo-transform-commit').click()
  await until(async () => (await kit.getByTestId('photo-transform-commit').count()) === 0, 40)
  step('after Free Transform’s ✓, M chooses the Rectangular Marquee at once', (await pressFor('m', 'RectMarquee')) === 'RectMarquee', `${await tool()}, focus ${await focused()}`)
  await choose('Brush')
  const middle = await e.at(doc.width / 2, doc.height / 2)
  await page.mouse.click(middle.x, middle.y, { button: 'right' })
  await until(async () => (await kit.getByTestId('photo-brush-picker-preset-3').count()) === 1, 40)
  await kit.getByTestId('photo-brush-picker-preset-3').click()
  await page.waitForTimeout(300)
  await page.keyboard.press('Escape')
  await until(async () => (await kit.getByTestId('photo-brush-picker').count()) === 0, 20)
  const size = () => kit.getByTestId('photo-option-size').locator('input').inputValue()
  const before = Number(await size())
  await page.keyboard.press(']')
  const grew = await until(async () => Number(await size()) > before, 20)
  step('after a preset picked and the brush picker closed with Escape, ] makes the brush bigger', grew, `${before} → ${await size()}, focus ${await focused()}`)
  await shot('stage8-focus')
  await workspace.focus()

  // ---------------------------------------------------------------- 4. Use Shift Key for Tool Switch
  // On (the default since stage 9, as Photoshop's): the key again keeps the tool and Shift with it
  // cycles. Off (Preferences › General): the key again cycles, as upstream; the engine's tool keys follow at once.
  const keepsBrush = async () => {
    await choose('Move')
    await pressFor('b', 'Brush')
    return until(async () => {
      await page.keyboard.press('b')
      await page.waitForTimeout(100)
      return (await tool()) === 'Brush'
    }, 10)
  }
  const keptAtFirst = await keepsBrush()
  const shiftedAtFirst = (await pressFor('Shift+B', 'Pencil')) === 'Pencil'
  const prefsDialog = kit.getByTestId('photo-dialog')
  await page.keyboard.press(`${e.primary}+k`)
  await until(async () => (await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').count()) === 1, 40)
  const onAtFirst = await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').isChecked()
  await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').uncheck()
  await page.keyboard.press('Enter')
  await until(async () => (await prefsDialog.count()) === 0, 40)
  await choose('Move')
  await pressFor('b', 'Brush')
  const cycles = (await pressFor('b', 'Pencil')) === 'Pencil'
  step('Preferences › General’s Use Shift Key for Tool Switch: on by default (B keeps the Brush, Shift+B cycles); off, B B cycles', keptAtFirst && shiftedAtFirst && onAtFirst && cycles, JSON.stringify({ keptAtFirst, shiftedAtFirst, onAtFirst, cycles }))
  await page.keyboard.press(`${e.primary}+k`)
  await until(async () => (await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').count()) === 1, 40)
  const shownOff = !(await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').isChecked())
  await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').check()
  await page.keyboard.press('Enter')
  await until(async () => (await prefsDialog.count()) === 0, 40)
  step('the preference shows as set when Preferences opens again, and on again B keeps the Brush', shownOff && (await keepsBrush()), `${shownOff} ${await tool()}`)
  await workspace.focus()

  // Plan 2026-11-24, stage 9: the Window menu has no greyed-out rows for panels that do not exist.
  await kit.getByTestId('photo-menu-Window').click()
  const windowRows = await kit.locator('[role="menu"] [data-kit-ui-id^="command:window.panel."], [role="menu"] [data-testid^="photo-cmd-window.panel."]').count()
  const disabledPanels = await kit.locator('[role="menu"] [aria-disabled="true"]').evaluateAll((rows) => rows.map((row) => row.textContent?.trim() ?? '').filter(Boolean))
  await page.keyboard.press('Escape')
  await workspace.focus()
  step('the Window menu lists no panel Kition Photo does not have, and nothing in it is greyed out', windowRows === 0 && disabledPanels.length === 0, JSON.stringify({ windowRows, disabledPanels }))

  // ---------------------------------------------------------------- 6. messages that name the cause
  // What the page shows (its visible text: the engine's own words stay folded under Details).
  const shown = () => kit.locator('body').evaluate(() => [...document.querySelectorAll('[role="alert"], [role="status"]')].map((element) => element.innerText.replace(/\s+/g, ' ')).join(' | '))
  const says = (text) => until(async () => (await shown()).includes(text), 40)
  const menuRow = async (menu, command) => {
    await kit.getByTestId(`photo-menu-${menu}`).click()
    const row = kit.getByTestId(`photo-cmd-${command}`)
    await row.waitFor()
    return row
  }
  // The sample's Layer 1 has no pixels (its mask, added above, changes nothing about that).
  await selectLayer(pixel.id)
  await page.keyboard.press(`${e.primary}+a`)
  const layersBefore = (await details()).layers.length
  await page.keyboard.press(`${e.primary}+j`)
  const emptyCopy = await says('There are no pixels in the selected area')
  step('Layer via Copy on empty pixels says the selected area of the layer has none (no “image engine” message)', emptyCopy && !(await shown()).includes('image engine') && (await details()).layers.length === layersBefore, await shown())
  await shot('stage8-empty-copy')
  await page.keyboard.press(`${e.primary}+d`)
  await until(async () => !(await details()).hasSelection, 30)
  const crop = await menuRow('Image', 'image.crop')
  const cropDisabled = (await crop.isDisabled()) || (await crop.getAttribute('aria-disabled')) === 'true'
  await page.keyboard.press('Escape')
  step('Image › Crop is greyed out without a selection', cropDisabled)
  await (await menuRow('Select', 'select.findLayers')).click()
  const searching = await until(async () => (await focused()) === 'photo-layers-search', 30)
  await kit.getByTestId('photo-layers-search').fill('')
  await workspace.focus()
  await page.keyboard.press(`${e.primary}+Alt+Shift+KeyF`)
  const byKey = await until(async () => (await focused()) === 'photo-layers-search', 30)
  step('Select › Find Layers (and its key) puts the keyboard in the Layers panel’s name search', searching && byKey && !(await shown()).includes('out of range'), `${searching} ${byKey} ${await focused()}`)
  await workspace.focus()
  await choose('CloneStamp')
  await e.drag([300, 300], [400, 300])
  step('the Clone Stamp without a source says to Alt-click a source first', await says('Alt-click'), await shown())
  await shot('stage8-clone-no-source')
  for (const layer of (await details()).layers) await e.query('layer.setProps', { layer: layer.id, visible: false })
  await (await menuRow('Select', 'select.subject')).click()
  step('Select › Subject on a picture with nothing in it says no subject was found', await says('No subject was found'), await shown())
  for (const layer of (await details()).layers) await e.query('layer.setProps', { layer: layer.id, visible: true })
  await workspace.focus()

  // ---------------------------------------------------------------- 7. Quick Selection's outline stays
  // The ants (the workspace's data-ants while the canvas draws them) show from the drag through the
  // release without a gap, while the stroke's last step runs again with every point.
  await page.keyboard.press(`${e.primary}+d`)
  await until(async () => !(await details()).hasSelection, 30)
  // The error cards above sit over the Layers panel's last rows: closed first.
  while (await kit.getByTestId('photo-error-close').count()) await kit.getByTestId('photo-error-close').first().click()
  await selectLayer(doc.layers.find((layer) => layer.name === 'Background').id)
  await choose('QuickSelection')
  // A brush as the walkthrough's (the shared size grew with ] above).
  await kit.getByTestId('photo-option-size').locator('input').fill('30')
  await kit.getByTestId('photo-option-size').locator('input').press('Enter')
  await workspace.focus()
  const quickFrom = (await details()).history.length
  const a = await e.at(870, 240)
  const b = await e.at(980, 330)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  await page.mouse.move(b.x, b.y, { steps: 25 })
  await shot('stage8-quick-mid')
  const antsMid = await until(async () => (await workspace.getAttribute('data-ants')) !== null, 60)
  await page.mouse.up()
  const gaps = []
  const started = Date.now()
  while (Date.now() - started < 2500) {
    if ((await workspace.getAttribute('data-ants')) === null) gaps.push(Date.now() - started)
    await page.waitForTimeout(40)
  }
  const quick = await details()
  step('Quick Selection’s outline shows during the drag and stays through the release (no gap)', antsMid && gaps.length === 0 && quick.hasSelection && quick.history.length === quickFrom + 1, JSON.stringify({ antsMid, gaps: gaps.slice(0, 5), last: quick.history.at(-1) }))
  await page.keyboard.press(`${e.primary}+d`)
  await workspace.focus()

  // ---------------------------------------------------------------- 8. the Layers panel
  // Alt over the line between two rows shows the clipping cursor; a click clips the layer above
  // the line to the one below, another releases it.
  while (await kit.getByTestId('photo-error-close').count()) await kit.getByTestId('photo-error-close').first().click()
  await until(async () => !(await details()).hasSelection, 30)
  const order = await kit.locator('[data-layer-row]').evaluateAll((rows) => rows.map((row) => Number(row.getAttribute('data-layer-row'))))
  const upperId = order[0]
  const lowerRow = await kit.locator(`[data-layer-row="${order[1]}"]`).boundingBox()
  const line = { x: lowerRow.x + lowerRow.width * 0.6, y: lowerRow.y }
  await page.mouse.move(line.x, line.y + 12)
  await page.keyboard.down('Alt')
  await page.mouse.move(line.x, line.y + 1, { steps: 3 })
  const rowsList = kit.getByTestId('photo-layers-rows')
  const cursorShown = await until(async () => (await rowsList.getAttribute('data-clip-line')) !== null && (await rowsList.evaluate((element) => getComputedStyle(element).cursor)).startsWith('url('), 20)
  await page.mouse.down()
  await page.mouse.up()
  const clipped = await until(async () => (await details()).layers.find((layer) => layer.id === upperId)?.clipped === true, 40)
  await page.mouse.down()
  await page.mouse.up()
  const released = await until(async () => (await details()).layers.find((layer) => layer.id === upperId)?.clipped === false, 40)
  await page.keyboard.up('Alt')
  step('Alt-click on the line between two rows clips the layer above to the one below (with the clipping cursor); again releases it', cursorShown && clipped && released, JSON.stringify({ cursorShown, clipped, released }))
  // Delete with no selection deletes the selected layer; with a selection it clears pixels as before.
  await selectLayer(pixel.id)
  const layersNow = (await details()).layers.length
  await page.keyboard.press('Delete')
  const deleted = await until(async () => (await details()).layers.length === layersNow - 1 && !(await details()).layers.some((layer) => layer.id === pixel.id), 40)
  step('Delete with no selection deletes the selected layer (one step)', deleted && (await details()).history.at(-1) === 'Delete Layer', (await details()).history.at(-1))
  await page.keyboard.press(`${e.primary}+z`)
  await until(async () => (await details()).layers.length === layersNow, 40)
  await selectLayer(pixel.id)
  await page.keyboard.press(`${e.primary}+a`)
  await until(async () => (await details()).hasSelection, 30)
  await page.keyboard.press('Backspace')
  const cleared = await until(async () => (await details()).history.at(-1) === 'Clear', 40)
  step('Backspace with a selection clears the pixels as before, and keeps the layer', cleared && (await details()).layers.length === layersNow, (await details()).history.at(-1))
  await page.keyboard.press(`${e.primary}+d`)

  // ---------------------------------------------------------------- 10. the dock resizes
  // The divider under a panel group drags its height (the filler below gives way), the column's
  // left edge drags its width; both are kept (checked after the reload below).
  const groupHeight = async (group) => Number(await kit.getByTestId(`photo-dock-area-group-${group}-group`).getAttribute('data-height'))
  const colorBefore = await groupHeight('color')
  const divider = await kit.getByTestId('photo-dock-area-handle-color-group').boundingBox()
  await page.mouse.move(divider.x + divider.width / 2, divider.y + divider.height / 2)
  await page.mouse.down()
  await page.mouse.move(divider.x + divider.width / 2, divider.y + divider.height / 2 + 50, { steps: 6 })
  await page.mouse.up()
  const colorAfter = await groupHeight('color')
  const dockBefore = (await kit.getByTestId('photo-dock').boundingBox()).width
  const edge = await kit.getByTestId('photo-dock-edge').boundingBox()
  await page.mouse.move(edge.x + edge.width / 2, edge.y + 300)
  await page.mouse.down()
  await page.mouse.move(edge.x + edge.width / 2 - 80, edge.y + 300, { steps: 6 })
  await page.mouse.up()
  const dockAfter = (await kit.getByTestId('photo-dock').boundingBox()).width
  await shot('stage8-dock')
  step('the divider under the Color group drags its height, the dock’s edge its width', Math.abs(colorAfter - colorBefore - 50) <= 2 && Math.abs(dockAfter - dockBefore - 80) <= 2, JSON.stringify({ colorBefore, colorAfter, dockBefore, dockAfter }))

  // A placed (object) layer's kind is in the interface's language: Chinese here.
  await page.goto(page.url().replace('locale=en-US', 'locale=zh-CN'))
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  const keptHeight = await groupHeight('color')
  const keptWidth = (await kit.getByTestId('photo-dock').boundingBox()).width
  step('the group’s height and the column’s width are kept when the editor opens again', Math.abs(keptHeight - colorAfter) <= 1 && Math.abs(keptWidth - dockAfter) <= 1, JSON.stringify({ keptHeight, keptWidth }))
  const object = (await details()).layers.find((layer) => layer.kind === 'Shape') ?? (await details()).layers[0]
  await e.query('layer.select', { layer: object.id, mode: 'replace' })
  await e.query('layer.objectLayers.convertToObjectLayer', {})
  const placed = await until(async () => (await details()).layers.some((layer) => layer.kind === 'Object'), 40)
  const objectId = (await details()).layers.find((layer) => layer.kind === 'Object')?.id
  const kindText = placed ? ((await kit.getByTestId(`photo-layer-kind-${objectId}`).textContent()) ?? '').trim() : ''
  step('an object layer’s kind under its name is translated (对象 in Chinese, not “Object”)', placed && kindText === '对象', kindText)

  // ---------------------------------------------------------------- 11. File › New… standalone
  // The development host opens the document File › New… writes, so a blank canvas is a click away.
  await kit.getByTestId('photo-menu-File').click()
  await kit.getByTestId('photo-file-new-document').click()
  await kit.getByTestId('photo-dialog').waitFor()
  await kit.getByTestId('photo-dialog-ok').click()
  const opened = await page
    .waitForFunction(() => /\.psd open in /.test(document.getElementById('status')?.textContent ?? '') && !(document.getElementById('status')?.textContent ?? '').includes('Sample.psd'), null, { timeout: 30_000 })
    .then(() => true, () => false)
  await page.waitForTimeout(800)
  const blank = opened ? await details() : null
  await shot('stage8-standalone-new')
  step('standalone, File › New… opens the new blank document in the page', opened && blank?.width === 1920 && blank?.height === 1080 && blank.layers.length === 1, JSON.stringify({ opened, status: await page.locator('#status').textContent(), size: blank && [blank.width, blank.height] }))

  await e.finish()
} catch (error) {
  await e.fail(error)
}
