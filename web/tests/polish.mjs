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

  await e.finish()
} catch (error) {
  await e.fail(error)
}
