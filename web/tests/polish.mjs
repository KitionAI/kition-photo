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
  // Off (the default, as upstream): the key again cycles. On (Preferences › General): the key again
  // keeps the tool and Shift with it cycles; the engine's tool keys follow at once.
  await choose('Move')
  await pressFor('b', 'Brush')
  const cycles = (await pressFor('b', 'Pencil')) === 'Pencil'
  const prefsDialog = kit.getByTestId('photo-dialog')
  await page.keyboard.press(`${e.primary}+k`)
  await until(async () => (await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').count()) === 1, 40)
  const offAtFirst = !(await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').isChecked())
  await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').check()
  await page.keyboard.press('Enter')
  await until(async () => (await prefsDialog.count()) === 0, 40)
  await choose('Move')
  await pressFor('b', 'Brush')
  const kept = await until(async () => {
    await page.keyboard.press('b')
    await page.waitForTimeout(100)
    return (await tool()) === 'Brush'
  }, 10)
  const shifted = (await pressFor('Shift+B', 'Pencil')) === 'Pencil'
  step('Preferences › General’s Use Shift Key for Tool Switch: off by default (B B cycles); on, B keeps the Brush and Shift+B cycles', cycles && offAtFirst && kept && shifted, JSON.stringify({ cycles, offAtFirst, kept, shifted }))
  await page.keyboard.press(`${e.primary}+k`)
  await until(async () => (await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').count()) === 1, 40)
  const shownOn = await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').isChecked()
  await kit.getByTestId('photo-dialog-pref-shiftToolSwitch').uncheck()
  await page.keyboard.press('Enter')
  await until(async () => (await prefsDialog.count()) === 0, 40)
  await choose('Move')
  await pressFor('b', 'Brush')
  step('the preference shows as set when Preferences opens again, and off again B cycles', shownOn && (await pressFor('b', 'Pencil')) === 'Pencil', `${shownOn} ${await tool()}`)
  await workspace.focus()

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

  await e.finish()
} catch (error) {
  await e.fail(error)
}
