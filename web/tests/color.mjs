// Colours, brushes and gradients chosen by hand (plan 2026-11-24, stage 5):
//   node tests/color.mjs [chromium|webkit]
// The Color Picker from the tool rail's chip sets the foreground (live while it moves, back on
// Cancel), its hex, RGB, HSB and Lab fields follow each other, its eyedropper picks from the
// canvas, and a brush stroke paints with the colour it chose; Escape in a picker over Fill closes
// only the picker; the Color panel's HSB sliders set the colour; the brush picker (the options
// bar's chip, a right click on the canvas) sets the size and picks a preset; Brush Settings (F5)
// sets the engine's brush; the gradient editor adds a stop and the Gradient tool's drag fills
// with it, showing it before the release. Real pointer input on dev.html; engine state read back
// through the development host. Prints PASS or FAIL per step.
import { mkdirSync } from 'node:fs'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const shots = process.env.KITION_SHOTS ?? null
if (shots) mkdirSync(shots, { recursive: true })
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
const errors = []
page.on('pageerror', (error) => errors.push(error.message))
let failures = 0
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${which} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 50) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}
const shot = async (name) => {
  if (shots) await page.screenshot({ path: `${shots}/${name}.png` })
}

try {
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  const kit = page.frameLocator('#kit')
  const count = (testId) => kit.getByTestId(testId).count()
  const surface = kit.getByTestId('kit-canvas-surface')
  const workspace = kit.getByTestId('photo-workspace')
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const agent = async (command, params = {}) => (await page.evaluate(async ([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])).completed[0]?.result
  const hex = (rgba) => `#${rgba.slice(0, 3).map((value) => Math.round(value * 255).toString(16).padStart(2, '0')).join('')}`
  const engineForeground = async () => hex((await agent('session.inspect')).foreground)
  const sample = async (x, y) => (await agent('document.sampleColor', { x, y, size: 'point', sampleLayer: 'all' })).slice(0, 3).map((value) => Math.round(value * 255))
  const near = (a, b, tolerance = 12) => a.every((value, index) => Math.abs(value - b[index]) <= tolerance)
  const rgbOf = (text) => [1, 3, 5].map((index) => parseInt(text.slice(index, index + 2), 16))
  const view = async () => {
    const [zoom, x, y] = (await workspace.getAttribute('data-view')).split(' ').map(Number)
    return { zoom, x, y, box: await surface.boundingBox() }
  }
  const at = async (x, y) => {
    const v = await view()
    return { x: v.box.x + (x - v.x) * v.zoom, y: v.box.y + (y - v.y) * v.zoom }
  }
  const pick = async (key, tool) => {
    await workspace.focus()
    await page.keyboard.press(key)
    return until(async () => (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === tool, 20)
  }
  const settle = async () => {
    let before = (await inspect()).revision
    for (let i = 0; i < 20; i += 1) {
      await page.waitForTimeout(150)
      const now = (await inspect()).revision
      if (now === before) return
      before = now
    }
  }
  const picker = kit.getByTestId('photo-color-dialog')
  const field = (name) => kit.getByTestId(`photo-color-dialog-${name}`)
  const typeIn = async (name, value) => {
    await field(name).fill(String(value))
    await field(name).press('Tab')
  }

  // The foreground chip opens the Color Picker on the foreground, not the browser's own.
  await kit.getByTestId('photo-colors-reset').click()
  await until(async () => (await engineForeground()) === '#000000')
  await kit.getByTestId('photo-foreground').click()
  const opened = await until(async () => (await count('photo-color-dialog')) === 1, 30)
  step('the foreground chip opens the Color Picker on the foreground', opened && (await field('hex').inputValue()) === '000000' && (await field('current').getAttribute('data-color')) === '#000000', await field('hex').inputValue().catch(() => '-'))

  // The hex field round-trips: typed, it moves RGB, HSB and Lab; the foreground follows at once.
  await field('hex').fill('ff8000')
  const typed = await until(async () => (await field('r').inputValue()) === '255' && (await field('g').inputValue()) === '128' && (await field('bl').inputValue()) === '0', 30)
  const hsb = [await field('h').inputValue(), await field('s').inputValue(), await field('b').inputValue()]
  const lab = [await field('l').inputValue(), await field('la').inputValue(), await field('lb').inputValue()]
  step('a typed hex moves the RGB, HSB and Lab fields', typed && hsb.join() === '30,100,100' && Number(lab[0]) > 60 && Number(lab[1]) > 30 && Number(lab[2]) > 60, `HSB ${hsb}, Lab ${lab}`)
  const live = await until(async () => (await engineForeground()) === '#ff8000' && (await kit.getByTestId('photo-foreground').inputValue()) === '#ff8000', 30)
  step('the foreground follows the picker while it is open', live, await engineForeground())
  await typeIn('r', 0)
  const fromRgb = await until(async () => (await field('hex').inputValue()) === '008000', 30)
  await typeIn('h', 240)
  const fromHue = await until(async () => (await field('hex').inputValue()) === '000080', 30)
  step('typed RGB and HSB values move the hex field', fromRgb && fromHue, await field('hex').inputValue())
  await shot('color-picker')
  await kit.getByTestId('photo-color-dialog-cancel').click()
  const cancelled = await until(async () => (await count('photo-color-dialog')) === 0 && (await engineForeground()) === '#000000', 30)
  step('Cancel puts the foreground back', cancelled, await engineForeground())

  // The square and the strip, by pointer; OK keeps the colour.
  await kit.getByTestId('photo-foreground').click()
  await until(async () => (await count('photo-color-dialog')) === 1)
  const hue = await field('hue').boundingBox()
  await page.mouse.click(hue.x + hue.width / 2, hue.y + hue.height * (1 - 30 / 360))
  const square = await field('square').boundingBox()
  await page.mouse.move(square.x + 5, square.y + square.height - 5)
  await page.mouse.down()
  await page.mouse.move(square.x + square.width - 1, square.y + 1, { steps: 6 })
  await page.mouse.up()
  const dragged = await until(async () => {
    const [h, s, b] = [Number(await field('h').inputValue()), Number(await field('s').inputValue()), Number(await field('b').inputValue())]
    return Math.abs(h - 30) <= 2 && s >= 98 && b >= 98
  }, 30)
  const chosen = await field('hex').inputValue()
  await kit.getByTestId('photo-color-dialog-ok').click()
  const kept = await until(async () => (await count('photo-color-dialog')) === 0 && (await engineForeground()) === `#${chosen}`, 30)
  step('the hue strip and a drag in the square choose a colour, and OK keeps it', dragged && kept, `#${chosen} / ${await engineForeground()}`)

  // A brush stroke paints with the colour the picker chose.
  await pick('b', 'Brush')
  {
    const layersBefore = (await details()).layers.length
    await kit.getByTestId('photo-layer-new').click()
    await until(async () => (await details()).layers.length === layersBefore + 1)
  }
  await kit.getByTestId('photo-option-size').locator('input').fill('30')
  await kit.getByTestId('photo-option-size').locator('input').press('Enter')
  await workspace.focus()
  {
    const from = await at(200, 120)
    const to = await at(500, 120)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 12 })
    await page.mouse.up()
    await settle()
    const painted = await sample(350, 120)
    step('a brush stroke paints with the chosen colour', near(painted, rgbOf(`#${chosen}`)), `${painted} vs #${chosen}`)
  }

  // The eyedropper: with the picker open, a press on the canvas picks the picture's colour there.
  // A point of the picture the picker does not cover (its top left corner).
  const centre = [40, 40]
  const truth = await sample(centre[0], centre[1])
  await kit.getByTestId('photo-foreground').click()
  await until(async () => (await count('photo-color-dialog')) === 1)
  const historyBefore = (await details()).history.length
  const point = await at(centre[0], centre[1])
  await page.mouse.click(point.x, point.y)
  const sampled = await until(async () => near(rgbOf(`#${await field('hex').inputValue()}`), truth, 3), 30)
  step('a press on the canvas while the picker is open picks the colour there, and paints nothing', sampled && (await count('photo-color-dialog')) === 1 && (await details()).history.length === historyBefore, `${await field('hex').inputValue()} vs ${truth}`)
  await shot('color-picker-eyedropper')
  await kit.getByTestId('photo-color-dialog-ok').click()
  await until(async () => (await count('photo-color-dialog')) === 0)
  const fromEyedropper = await engineForeground()
  step('OK makes the picked colour the foreground', near(rgbOf(fromEyedropper), truth, 3), fromEyedropper)

  // Over Fill: the colour row opens the picker; Escape closes the picker only.
  await kit.getByTestId('photo-menu-Edit').click()
  await kit.getByTestId('photo-cmd-edit.fill').click()
  await until(async () => (await count('photo-dialog')) === 1)
  await kit.getByTestId('photo-dialog-contents').selectOption('color').catch(async () => {
    await kit.getByTestId('photo-dialog-contents').click()
    await kit.getByRole('option', { name: 'Color…' }).click()
  })
  await until(async () => (await count('photo-dialog-color')) === 1)
  await kit.getByTestId('photo-dialog-color').click()
  const overFill = await until(async () => (await count('photo-color-dialog')) === 1, 30)
  await page.keyboard.press('Escape')
  const onlyPicker = await until(async () => (await count('photo-color-dialog')) === 0, 30)
  await page.waitForTimeout(200)
  step('Escape in the picker over Fill closes the picker and leaves Fill open', overFill && onlyPicker && (await count('photo-dialog')) === 1)
  await kit.getByTestId('photo-dialog-cancel').click()
  await until(async () => (await count('photo-dialog')) === 0)

  // The Color panel's Brightness slider.
  await kit.getByTestId('photo-dock-area-tab-color').click()
  await kit.getByTestId('photo-color-hsb').scrollIntoViewIfNeeded()
  const before = await engineForeground()
  const brightness = await kit.getByTestId('photo-color-hsb-b-track').boundingBox()
  await page.mouse.click(brightness.x + brightness.width * 0.25, brightness.y + brightness.height / 2)
  const darker = await until(async () => {
    const now = await engineForeground()
    return now !== before && Math.max(...rgbOf(now)) <= 80
  }, 30)
  step("the Color panel's Brightness slider sets the foreground", darker, `${before} -> ${await engineForeground()}`)

  // The brush picker from the options bar's chip: its Size slider and a preset.
  await pick('b', 'Brush')
  await kit.getByTestId('photo-brush-chip').click()
  const popped = await until(async () => (await count('photo-brush-picker')) === 1, 30)
  const sizeTrack = await kit.getByTestId('photo-brush-picker-size-track').boundingBox()
  await page.mouse.move(sizeTrack.x + 2, sizeTrack.y + sizeTrack.height / 2)
  await page.mouse.down()
  await page.mouse.move(sizeTrack.x + sizeTrack.width * 0.2, sizeTrack.y + sizeTrack.height / 2, { steps: 5 })
  await page.mouse.up()
  const sized = await until(async () => {
    const value = Number(await kit.getByTestId('photo-option-size').locator('input').inputValue())
    return value > 60 && value < 140
  }, 30)
  step("the brush chip opens the picker and its Size slider sets the brush's size", popped && sized, await kit.getByTestId('photo-option-size').locator('input').inputValue())
  const soft = kit.locator('[data-testid^="photo-brush-picker-preset-"][data-name="Soft Round"]')
  await soft.click()
  const softPicked = await until(async () => (await kit.getByTestId('photo-option-size').locator('input').inputValue()) === '45' && (await kit.getByTestId('photo-option-hardness').locator('input').inputValue()) === '0', 30)
  // The strokes are painted by a scratch engine of their own, which takes a moment to start.
  await until(async () => (await kit.locator('[data-testid^="photo-brush-picker-preset-"] img').count()) > 5, 200)
  const thumbs = await kit.locator('[data-testid^="photo-brush-picker-preset-"] img').count()
  step('a preset sets the size and hardness, and the presets show their strokes', softPicked && thumbs > 5, `size ${await kit.getByTestId('photo-option-size').locator('input').inputValue()}, ${thumbs} strokes`)
  await shot('brush-picker')
  await page.keyboard.press('Escape')
  await until(async () => (await count('photo-brush-picker')) === 0)
  // A right click on the canvas with a brush opens it at the pointer, and paints nothing.
  const strokeHistory = (await details()).history.length
  const right = await at(600, 600)
  await page.mouse.click(right.x, right.y, { button: 'right' })
  const there = await until(async () => {
    const box = await kit.getByTestId('photo-brush-picker').boundingBox()
    return box !== null && Math.abs(box.x - right.x) < 4 && Math.abs(box.y - right.y) < 4
  }, 30)
  step('a right click on the canvas with the Brush opens the picker there, painting nothing', there && (await details()).history.length === strokeHistory)
  await page.mouse.click(right.x - 300, right.y - 300)
  await until(async () => (await count('photo-brush-picker')) === 0)

  // Brush Settings (F5): its Roundness changes the engine's brush.
  await workspace.focus()
  await page.keyboard.press('F5')
  const settingsShown = await until(async () => (await count('photo-brush-settings')) === 1, 30)
  const roundness = await kit.getByTestId('photo-brush-settings-roundness-track').boundingBox()
  await page.mouse.click(roundness.x + roundness.width * 0.5, roundness.y + roundness.height / 2)
  const round = await until(async () => Math.abs((await agent('brush.get')).roundness - 0.5) < 0.06, 30)
  step('Brush Settings (F5) sets the brush’s roundness in the engine', settingsShown && round, String((await agent('brush.get')).roundness))
  await shot('brush-settings')
  await workspace.focus()
  await page.keyboard.press('F5')
  await until(async () => (await count('photo-brush-settings')) === 0)

  // The gradient editor: a stop added and coloured, then the Gradient's drag fills with it.
  await pick('g', 'Gradient')
  await kit.getByTestId('photo-gradient-swatch').click()
  await until(async () => (await count('photo-gradient-editor')) === 1)
  const colors = await kit.getByTestId('photo-gradient-editor-colors').boundingBox()
  await page.mouse.click(colors.x + colors.width * 0.5, colors.y + colors.height / 2)
  const added = await until(async () => (await kit.locator('[data-testid^="photo-gradient-editor-color-"]').count()) === 3, 30)
  await kit.getByTestId('photo-gradient-editor-stop-color').click()
  await until(async () => (await count('photo-color-dialog')) === 1)
  await field('hex').fill('ff0000')
  await kit.getByTestId('photo-color-dialog-ok').click()
  await until(async () => (await count('photo-color-dialog')) === 0)
  await shot('gradient-editor')
  await kit.getByTestId('photo-gradient-editor-ok').click()
  await until(async () => (await count('photo-gradient-editor')) === 0)
  const current = await until(async () => {
    const list = await agent('gradient.presets.list')
    return list.current.stops.length === 3 && list.current.stops.some(([location, colour]) => Math.abs(location - 0.5) < 0.02 && colour === '#ff0000')
  }, 30)
  step('the gradient editor adds a stop, colours it, and OK makes it the current gradient', added && current, JSON.stringify((await agent('gradient.presets.list')).current.stops))
  {
    const layersBefore = (await details()).layers.length
    const historyStart = (await details()).history.length
    const a = await at(100, 400)
    const b = await at(1100, 400)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 16 })
    const duringDrag = await until(async () => near(await sample(600, 400), [255, 0, 0], 40), 40)
    await shot('gradient-drag')
    await page.mouse.up()
    await settle()
    const after = await details()
    step('the Gradient shows the gradient under the drag before the release', duringDrag)
    step('the release leaves one gradient fill layer and one history step, red in the middle', after.layers.length === layersBefore + 1 && after.history.length === historyStart + 1 && near(await sample(600, 400), [255, 0, 0], 40), `${await sample(600, 400)}, history +${after.history.length - historyStart}`)
  }

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  failures += 1
  console.log(`FAIL ${which} the run stopped - ${error instanceof Error ? error.stack : error}`)
} finally {
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
