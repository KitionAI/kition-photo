// The command dialogs in a real browser, through the development host: node tests/dialogs.mjs [chromium|webkit]
// Starts the Vite dev server itself, opens dev.html in Chinese, and prints PASS or FAIL per step
// (plan 2026-11-24, stage 1: a "…" menu row asks first, previews, and Cancel and OK keep history exact).
import { mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
// Where screenshots go when SHOTS names a folder (the walkthrough's), else nowhere.
const shots = process.env.SHOTS ? fileURLToPath(new URL(process.env.SHOTS, `file://${process.cwd()}/`)) : null
if (shots) mkdirSync(shots, { recursive: true })
const web = fileURLToPath(new URL('..', import.meta.url))
const server = await createServer({ root: web, configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)), server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
await context.tracing.start({ screenshots: true, snapshots: true })
const page = await context.newPage()
const primaryKey = async () => ((await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control')
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 60) => {
  for (let i = 0; i < tries; i += 1) {
    if (await check().catch(() => false)) return true
    await page.waitForTimeout(100)
  }
  return false
}
const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
const details = async () => (await inspect()).details
const shot = async (name) => {
  if (shots) await page.screenshot({ path: `${shots}/${name}.png` })
}

try {
  await page.goto(`${origin}/dev.html?locale=zh-CN`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const count = (testId) => kit.getByTestId(testId).count()
  const dialog = kit.getByTestId('photo-dialog')
  await page.waitForTimeout(1000)

  /** A fingerprint of the picture on the canvas (every 7th byte, hashed), and the history as the History panel lists it. */
  const picture = () =>
    kit.getByTestId('kit-canvas-surface').evaluate((canvas) => {
      const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      let hash = 2166136261
      for (let i = 0; i < data.length; i += 7) hash = Math.imul(hash ^ data[i], 16777619) >>> 0
      return hash
    })
  const history = async () => {
    const now = await details()
    return JSON.stringify({ states: now.history, index: now.historyIndex, redo: now.historyRedo ?? [] })
  }
  /** Waits until the dialog's preview has run (no "updating" status for 300 ms): a slow machine may still be blurring. */
  const previewIdle = async () => {
    for (let quiet = 0, i = 0; quiet < 2 && i < 100; i += 1) {
      await page.waitForTimeout(150)
      quiet = (await count('photo-dialog-status')) === 0 ? quiet + 1 : 0
    }
  }
  /** Waits for the canvas to settle (two equal fingerprints in a row) and answers it. */
  const settled = async () => {
    let last = await picture()
    for (let i = 0; i < 40; i += 1) {
      await page.waitForTimeout(150)
      const next = await picture()
      if (next === last) return next
      last = next
    }
    return last
  }
  const openRow = async (menu, submenu, command) => {
    await kit.getByTestId(`photo-menu-${menu}`).click()
    if (submenu) await kit.locator(`[data-kit-ui-id="menu:${menu}/${submenu}"]`).first().hover()
    await kit.getByTestId(`photo-cmd-${command}`).click()
  }

  // A pixel layer with pixels (the sample's Background), so a blur shows.
  const layers = (await details()).layers
  const painted = layers.find((layer) => layer.kind === 'Pixel' && layer.name === 'Background') ?? layers.find((layer) => layer.kind === 'Pixel')
  await kit.getByTestId(`photo-layer-${painted.id}`).click()
  await until(async () => (await details()).activeLayer === painted.id)
  const before = await settled()
  const historyBefore = await history()

  // Filter › Blur › Gaussian Blur… opens its dialog; the click itself runs nothing.
  await openRow('Filter', 'Blur', 'filter.blur.gaussianBlur')
  const opened = await until(async () => (await dialog.count()) === 1, 40)
  const radius = kit.getByTestId('photo-dialog-param-radius')
  const label = opened ? ((await radius.locator('.kui-gradient-slider-label').textContent()) ?? '') : ''
  step('Gaussian Blur… opens a dialog with its Radius labelled in Chinese, not by its key', opened && label === '半径', JSON.stringify({ opened, label }))
  await shot('dialog-gaussian-blur')
  // Preview off shows the picture as it was: the menu click committed nothing.
  await kit.getByTestId('photo-dialog-preview').uncheck()
  await until(async () => (await history()) === historyBefore, 40)
  const previewOff = await settled()
  step('with Preview off the picture and the history are as before the click', previewOff === before && (await history()) === historyBefore, JSON.stringify({ before, previewOff }))
  await kit.getByTestId('photo-dialog-preview').check()
  const previewed = await until(async () => (await settled()) !== before, 40)
  step('Preview on shows the filter on the canvas', previewed)
  const atDefault = await settled()
  // Moving the slider changes the picture (the preview runs again with the new radius).
  const track = kit.getByTestId('photo-dialog-param-radius-track')
  const box = await track.boundingBox()
  await page.mouse.click(box.x + box.width * 0.2, box.y + box.height / 2)
  const moved = await until(async () => (await settled()) !== atDefault, 40)
  const radiusNow = Number(await track.getAttribute('aria-valuenow'))
  step('moving the Radius slider changes the picture', moved && radiusNow > 1, `radius ${radiusNow}`)
  await shot('dialog-gaussian-blur-moved')
  // Cancel restores the picture and leaves no extra history step.
  await kit.getByTestId('photo-dialog-cancel').click()
  await until(async () => (await dialog.count()) === 0, 20)
  await until(async () => (await history()) === historyBefore, 40)
  const cancelled = await settled()
  step('Cancel restores the picture and adds no history step', (await dialog.count()) === 0 && cancelled === before && (await history()) === historyBefore, JSON.stringify({ before, cancelled, history: await history(), historyBefore }))

  // OK leaves exactly one step: the filter with the dialog's values.
  await openRow('Filter', 'Blur', 'filter.blur.gaussianBlur')
  await until(async () => (await dialog.count()) === 1, 40)
  const again = kit.getByTestId('photo-dialog-param-radius-track')
  const box2 = await again.boundingBox()
  await page.mouse.click(box2.x + box2.width * 0.3, box2.y + box2.height / 2)
  await page.waitForTimeout(400)
  await previewIdle()
  const shown = await settled()
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await dialog.count()) === 0, 40)
  const afterOk = await settled()
  const states = (await details()).history
  const was = JSON.parse(historyBefore).states
  const oneStep = states.length === was.length + 1 ? states.slice(0, -1).join('|') === was.join('|') : states.slice(0, -1).join('|') === was.slice(1).join('|')
  step('OK adds exactly one history step and keeps what the preview showed', oneStep && afterOk === shown && afterOk !== before, JSON.stringify({ last: states.at(-1), was: was.length, now: states.length }))
  // The dialog remembers its values: it opens again at the radius last OK'd.
  await openRow('Filter', 'Blur', 'filter.blur.gaussianBlur')
  await until(async () => (await dialog.count()) === 1, 40)
  const kept = Number(await kit.getByTestId('photo-dialog-param-radius-track').getAttribute('aria-valuenow'))
  await kit.getByTestId('photo-dialog-cancel').click()
  await until(async () => (await dialog.count()) === 0, 20)
  step('Gaussian Blur opens again with the radius last OK’d', kept > 1, `radius ${kept}`)

  // Levels by its key (Cmd+L): the dialog with the image's histogram, its sliders moving the picture.
  await kit.getByTestId(`photo-layer-${painted.id}`).click()
  const levelsFrom = await history()
  const levelsBefore = await settled()
  await page.keyboard.press(`${await primaryKey()}+KeyL`)
  const levelsOpen = await until(async () => (await dialog.count()) === 1 && (await count('photo-dialog-levels')) === 1, 40)
  const histogram = await count('photo-dialog-histogram')
  step('Cmd+L opens the Levels dialog with a histogram, not a bare run', levelsOpen && histogram === 1, JSON.stringify({ levelsOpen, histogram }))
  const inBlack = kit.getByTestId('photo-dialog-inBlack').locator('input')
  await inBlack.fill('60')
  await inBlack.press('Tab')
  const levelsMoved = await until(async () => (await settled()) !== levelsBefore, 40)
  step('a Levels value changes the picture', levelsMoved)
  await shot('dialog-levels')
  await kit.getByTestId('photo-dialog-cancel').click()
  await until(async () => (await history()) === levelsFrom, 40)
  step('Cancel in Levels leaves the history as it was', (await history()) === levelsFrom && (await settled()) === levelsBefore)

  // Curves (Cmd+M), Hue/Saturation (Cmd+U) and Color Balance (Cmd+B) open their own dialogs.
  const ownDialog = async (key, testId, name) => {
    await page.keyboard.press(`${await primaryKey()}+${key}`)
    const shown = await until(async () => (await dialog.count()) === 1 && (await count(testId)) === 1, 40)
    await shot(name)
    await kit.getByTestId('photo-dialog-cancel').click()
    await until(async () => (await dialog.count()) === 0, 20)
    return shown
  }
  const curves = await ownDialog('KeyM', 'photo-dialog-curve', 'dialog-curves')
  const hueSat = await ownDialog('KeyU', 'photo-dialog-hue-bars', 'dialog-hue-saturation')
  const balance = await ownDialog('KeyB', 'photo-dialog-tone', 'dialog-color-balance')
  step('Curves, Hue/Saturation and Color Balance open their own dialogs by their keys', curves && hueSat && balance, JSON.stringify({ curves, hueSat, balance }))
  await until(async () => (await history()) === levelsFrom, 40)
  step('…and Cancel leaves the history as it was', (await history()) === levelsFrom)

  // Every other "…" filter gets the generic dialog: Add Noise's Amount, Distribution and Monochromatic, in Chinese.
  await openRow('Filter', 'Noise', 'filter.noise.addNoise')
  await until(async () => (await dialog.count()) === 1, 40)
  const noiseLabels = await kit.locator('[data-testid="photo-dialog"] .kui-gradient-slider-label, [data-testid="photo-dialog"] .kui-choice > span, [data-testid="photo-dialog"] .photo-dialog .kui-checkbox').allTextContents()
  await shot('dialog-add-noise')
  await kit.getByTestId('photo-dialog-cancel').click()
  await until(async () => (await dialog.count()) === 0, 20)
  step('Add Noise… gets the generic dialog with Chinese labels', noiseLabels.length === 3 && noiseLabels.every((text) => /[\u4e00-\u9fff]/.test(text)), JSON.stringify(noiseLabels))

  // Plan 2026-11-24, stage 8: Enter is OK from anywhere in a dialog (right after it opens, after Tab
  // took the focus off the field), the Enter that closed it reaches nothing else, and a press or a
  // drag on the canvas outside the dialog neither cancels it nor paints.
  const addedOne = (was, now) => (now.length === was.length + 1 ? now.slice(0, -1).join('|') === was.join('|') : now.slice(0, -1).join('|') === was.slice(1).join('|'))
  await kit.getByTestId('photo-workspace').focus()
  const enterFrom = (await details()).history
  await page.keyboard.press(`${await primaryKey()}+KeyL`)
  await until(async () => (await dialog.count()) === 1 && (await count('photo-dialog-levels')) === 1, 40)
  await previewIdle()
  await page.keyboard.press('Enter')
  const levelsClosed = await until(async () => (await dialog.count()) === 0, 40)
  const levelsStates = (await details()).history
  step('Enter right after Cmd+L confirms Levels with one history step', levelsClosed && addedOne(enterFrom, levelsStates), JSON.stringify({ levelsClosed, last: levelsStates.at(-1) }))
  await openRow('Filter', 'Blur', 'filter.blur.gaussianBlur')
  await until(async () => (await dialog.count()) === 1, 40)
  await kit.getByTestId('photo-dialog-param-radius-value-value').click()
  await kit.getByTestId('photo-dialog-param-radius-value-input').fill('4')
  await page.keyboard.press('Tab')
  const tabbedTo = await kit.locator('body').evaluate(() => document.activeElement?.getAttribute('data-testid') ?? document.activeElement?.tagName)
  await previewIdle()
  await page.keyboard.press('Enter')
  const blurClosed = await until(async () => (await dialog.count()) === 0, 40)
  const blurStates = (await details()).history
  step('after Tab moved the focus off the Radius field, Enter is still OK (one step), not Cancel', blurClosed && addedOne(levelsStates, blurStates), JSON.stringify({ tabbedTo, blurClosed, last: blurStates.at(-1) }))
  await page.keyboard.press('Enter')
  await page.waitForTimeout(800)
  const second = await details()
  const menusOpen = await kit.locator('[role="menu"]').count()
  const focusHome = await kit.locator('body').evaluate(() => document.activeElement?.getAttribute('data-testid'))
  step('a second Enter after the dialog closed runs nothing and opens no menu; the keys are the canvas’s', second.history.join('|') === blurStates.join('|') && (await dialog.count()) === 0 && menusOpen === 0 && focusHome === 'photo-workspace', JSON.stringify({ last: second.history.at(-1), menusOpen, focusHome }))
  await page.keyboard.press('b')
  await until(async () => (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === 'Brush', 20)
  await openRow('Filter', 'Blur', 'filter.blur.gaussianBlur')
  await until(async () => (await dialog.count()) === 1, 40)
  const radiusKept = Number(await kit.getByTestId('photo-dialog-param-radius-track').getAttribute('aria-valuenow'))
  step('the Radius typed before Tab and Enter is the one applied (the dialog opens again at 4)', radiusKept === 4, `radius ${radiusKept}`)
  await previewIdle()
  const surface = await kit.getByTestId('kit-canvas-surface').boundingBox()
  await page.mouse.click(surface.x + surface.width * 0.3, surface.y + surface.height * 0.7)
  await page.mouse.move(surface.x + surface.width * 0.3, surface.y + surface.height * 0.75)
  await page.mouse.down()
  await page.mouse.move(surface.x + surface.width * 0.5, surface.y + surface.height * 0.75, { steps: 8 })
  await page.mouse.up()
  await page.waitForTimeout(600)
  const stillOpen = (await dialog.count()) === 1
  await kit.getByTestId('photo-dialog-cancel').click()
  await until(async () => (await dialog.count()) === 0, 20)
  await until(async () => (await details()).history.join('|') === blurStates.join('|'), 40)
  const unpainted = (await details()).history
  step('a click and a drag on the canvas leave the dialog open and paint nothing', stillOpen && unpainted.join('|') === blurStates.join('|'), JSON.stringify({ stillOpen, last: unpainted.at(-1) }))
  await shot('stage8-dialog-modal')

  // A Levels adjustment layer: Properties shows the same Levels editor (histogram and handles); a value is one step.
  await kit.getByTestId('photo-dock-area-tab-adjustments').click()
  const layerCount = (await details()).layers.length
  await kit.getByTestId('photo-adjustments-levels').click()
  await until(async () => (await details()).layers.length === layerCount + 1, 40)
  const adjustment = (await details()).activeLayer
  const editorShown = await until(async () => (await count('photo-adjustment-levels')) === 1 && (await count('photo-adjustment-histogram')) === 1, 40)
  const field = kit.getByTestId('photo-adjustment-inBlack').locator('input')
  await field.fill('30')
  await field.press('Enter')
  const findLayer = (list, id) => list.find((layer) => layer.id === id)
  const setTo30 = await until(async () => findLayer((await details()).layers, adjustment)?.adjustmentParams?.inBlack === 30, 40)
  await shot('properties-levels')
  step('a Levels adjustment layer is edited in Properties with the Levels editor', editorShown && setTo30, JSON.stringify({ editorShown, setTo30 }))
  await page.evaluate((id) => window.devHost.connection().ai.run([{ command: 'layer.delete', params: { layer: id } }], false), adjustment)
  await kit.getByTestId('photo-dock-area-tab-layers').click()
  await kit.getByTestId(`photo-layer-${painted.id}`).click()
  const levelsFrom2 = await history()

  // Image Size: a new width keeps the proportions; OK resizes in one step.
  const size = { width: (await details()).width, height: (await details()).height }
  await openRow('Image', null, 'image.imageSize')
  await until(async () => (await dialog.count()) === 1, 40)
  await shot('dialog-image-size')
  const widthField = kit.getByTestId('photo-dialog-width')
  await widthField.fill(String(Math.round(size.width / 2)))
  const heightShown = Number(await kit.getByTestId('photo-dialog-height').inputValue())
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await details()).width === Math.round(size.width / 2), 60)
  const resized = await details()
  step('Image Size keeps the proportions and resizes in one step', resized.width === Math.round(size.width / 2) && Math.abs(resized.height - size.height / 2) <= 1 && Math.abs(heightShown - size.height / 2) <= 1 && resized.history.at(-1) !== JSON.parse(levelsFrom2).states.at(-1), JSON.stringify({ size, now: [resized.width, resized.height], heightShown, last: resized.history.at(-1) }))

  // Canvas Size anchors the new area; Fill asks what to fill with.
  await openRow('Image', null, 'image.canvasSize')
  await until(async () => (await dialog.count()) === 1, 40)
  await kit.getByTestId('photo-dialog-anchor-topLeft').click()
  const anchored = (await kit.getByTestId('photo-dialog-anchor-topLeft').getAttribute('aria-checked')) === 'true'
  await shot('dialog-canvas-size')
  await kit.getByTestId('photo-dialog-cancel').click()
  await openRow('Edit', null, 'edit.fill')
  const fillOpen = await until(async () => (await count('photo-dialog-contents')) === 1, 40)
  await kit.getByTestId('photo-dialog-cancel').click()
  step('Canvas Size has a nine-way anchor and Fill asks for its contents', anchored && fillOpen)

  // Rows with nothing behind them are gone: the Preferences pages but General (the editor's own
  // settings, plan 2026-11-24 stage 2) and the keyboard shortcut editor.
  await kit.getByTestId('photo-menu-Edit').click()
  await kit.locator('[data-kit-ui-id="menu:Edit/Preferences"]').first().hover()
  await until(async () => (await count('photo-cmd-edit.preferences.general')) === 1, 20)
  const preferences = await kit.locator('[data-testid^="photo-cmd-edit.preferences."]').evaluateAll((rows) => rows.map((row) => row.getAttribute('data-testid')))
  const shortcuts = await count('photo-cmd-edit.keyboardShortcuts')
  await page.keyboard.press('Escape')
  await page.keyboard.press('Escape')
  step('the Edit menu leaves out the Preferences pages but General, and the shortcut editor', JSON.stringify(preferences) === JSON.stringify(['photo-cmd-edit.preferences.general']) && shortcuts === 0, JSON.stringify({ preferences, shortcuts }))

  // Preferences › General (Cmd+K): Zoom with Scroll Wheel makes the plain wheel zoom, and is kept.
  await kit.getByTestId('photo-workspace').focus()
  await page.keyboard.press(`${await primaryKey()}+k`)
  const prefsOpen = await until(async () => (await count('photo-dialog-pref-zoomWithWheel')) === 1, 40)
  await kit.getByTestId('photo-dialog-pref-zoomWithWheel').check()
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await dialog.count()) === 0, 40)
  const zoomOf = async () => Number((await kit.getByTestId('photo-workspace').getAttribute('data-view')).split(' ')[0])
  const surfaceBox = await kit.getByTestId('kit-canvas-surface').boundingBox()
  await page.mouse.move(surfaceBox.x + surfaceBox.width / 2, surfaceBox.y + surfaceBox.height / 2)
  const zoomBefore = await zoomOf()
  await page.mouse.wheel(0, -100)
  const zoomed = await until(async () => (await zoomOf()) > zoomBefore * 1.1, 30)
  const stored = await kit.locator('body').evaluate(() => Object.keys(localStorage).filter((key) => key.includes('preferences')).map((key) => localStorage.getItem(key)).join(''))
  step('Preferences › General opens on Cmd+K, and Zoom with Scroll Wheel makes the plain wheel zoom and is kept', prefsOpen && zoomed && stored.includes('"zoomWithWheel":true'), JSON.stringify({ prefsOpen, zoomBefore, after: await zoomOf(), stored }))
  // Back to the default, for the next run in this browser profile.
  await page.keyboard.press(`${await primaryKey()}+k`)
  await until(async () => (await count('photo-dialog-pref-zoomWithWheel')) === 1, 40)
  await kit.getByTestId('photo-dialog-pref-zoomWithWheel').uncheck()
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await dialog.count()) === 0, 40)

  step('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
} catch (error) {
  step('the dialog checks ran to the end', false, error instanceof Error ? error.message.split('\n')[0] : String(error))
} finally {
  if (failures) {
    mkdirSync(new URL('../test-results/dialogs/', import.meta.url), { recursive: true })
    await context.tracing.stop({ path: fileURLToPath(new URL(`../test-results/dialogs/trace-${which}.zip`, import.meta.url)) })
  } else await context.tracing.stop()
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} step(s) failed` : 'all dialog steps passed')
process.exit(failures ? 1 : 0)
