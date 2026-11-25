// The Layer Style dialog, Properties' X, Y, W and H, and its Masks section (plan 2026-11-24, stage 5):
//   node tests/layer-style.mjs [chromium|webkit]
// Layer Style opens from the Layers panel's fx menu, a double click on a layer row and Layer ›
// Layer Style; its preview shows on the canvas, Cancel restores the layer, OK leaves exactly one
// history step with the effects and the blending options; the effects' eyes switch one off; a
// gradient overlay's gradient comes from the gradient editor. Properties' X and W are typed and
// move and scale the layer; a mask's Density changes the composite and its Feather is kept. Real
// pointer input on dev.html; engine state read back through the development host.
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
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const agent = async (command, params = {}) => (await page.evaluate(async ([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])).completed[0]?.result
  const sample = async (x, y) => (await agent('document.sampleColor', { x, y, size: 'point', sampleLayer: 'all' })).slice(0, 3).map((value) => Math.round(value * 255))
  const near = (a, b, tolerance = 10) => a.every((value, index) => Math.abs(value - b[index]) <= tolerance)
  const layerOf = async (id) => (await details()).layers.find((layer) => layer.id === id)
  /** A fingerprint of the picture on the canvas (every 7th byte, hashed). */
  const picture = () =>
    kit.getByTestId('kit-canvas-surface').evaluate((canvas) => {
      const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      let hash = 2166136261
      for (let i = 0; i < data.length; i += 7) hash = Math.imul(hash ^ data[i], 16777619) >>> 0
      return hash
    })
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
  /** Waits until the dialog's preview is idle (no "updating" status twice in a row). */
  const previewIdle = async () => {
    for (let quiet = 0, i = 0; quiet < 2 && i < 100; i += 1) {
      await page.waitForTimeout(150)
      quiet = (await count('photo-layer-style-status')) === 0 ? quiet + 1 : 0
    }
  }
  const clickTrack = async (testId, fraction) => {
    const track = await kit.getByTestId(`${testId}-track`).boundingBox()
    await page.mouse.click(track.x + track.width * fraction, track.y + track.height / 2)
  }
  const dialog = 'photo-layer-style'
  const selected = (row) => kit.getByTestId(`${dialog}-row-${row}`).getAttribute('aria-selected')

  // The dock at 1440 × 900 and 1165 × 900 (findings.md item 12): Color shows its whole square,
  // Properties its X, Y, W, H, Opacity and Fill above Layers, and Layers its four rows.
  const dockLayout = () =>
    kit.getByTestId('photo-dock-area').evaluate((dock) => {
      const box = (selector) => dock.querySelector(selector)?.getBoundingClientRect() ?? null
      const color = box('[data-testid="photo-color-panel"]')
      const square = box('[data-testid="photo-color-picker-square"]')
      const fill = box('[data-testid="photo-props-fill"]')
      const layers = box('[data-testid="photo-dock-area-group-layers-group"]')
      const props = box('[data-testid="photo-properties"]')
      const rows = [...dock.querySelectorAll('[data-layer-row]')].map((row) => row.getBoundingClientRect())
      const list = box('[data-testid="photo-layers-rows"]') ?? layers
      return {
        square: Boolean(color && square && square.top >= color.top && square.bottom <= color.bottom + 0.5),
        props: Boolean(fill && layers && props && fill.bottom <= Math.min(layers.top, props.bottom) + 0.5),
        rows: rows.filter((row) => list && row.bottom <= list.bottom + 0.5 && row.top >= list.top - 0.5).length,
      }
    })
  for (const width of [1440, 1165]) {
    await page.setViewportSize({ width, height: 900 })
    let layout = null
    const fits = await until(async () => {
      layout = await dockLayout()
      return layout.square && layout.props && layout.rows >= 4
    }, 30)
    step(`at ${width} × 900 the dock shows Color's whole square, Properties' X to Fill, and four layer rows`, fits, JSON.stringify(layout))
    if (width === 1440) await shot('dock-1440')
  }
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.waitForTimeout(300)

  const ellipse = (await details()).layers.find((layer) => layer.kind === 'Shape')
  await kit.getByTestId(`photo-layer-${ellipse.id}`).click()
  await until(async () => (await details()).activeLayer === ellipse.id)
  const before = await settled()
  const historyBefore = (await details()).history.length

  // The fx menu's Drop Shadow… opens Layer Style on Drop Shadow, switched on, previewing.
  await kit.getByTestId('photo-layers-style').click()
  await kit.getByTestId('photo-layers-style-dropShadow').click()
  const opened = await until(async () => (await count(dialog)) === 1, 40)
  const onShadow = opened && (await selected('dropShadow')) === 'true' && (await kit.getByTestId(`${dialog}-check-dropShadow`).isChecked())
  await clickTrack(`${dialog}-field-distance`, 0.2)
  await previewIdle()
  const previewed = await settled()
  step('the fx menu opens Layer Style on Drop Shadow, and the canvas shows it', onShadow && previewed !== before, `${before} -> ${previewed}`)
  await shot('layer-style-drop-shadow')
  await kit.getByTestId(`${dialog}-cancel`).click()
  await until(async () => (await count(dialog)) === 0)
  const restored = await settled()
  const afterCancel = await layerOf(ellipse.id)
  step('Cancel restores the layer: no effects, the same picture, no history step', restored === before && !afterCancel.effects && (await details()).history.length === historyBefore, `${restored} vs ${before}, history ${historyBefore} -> ${(await details()).history.length}`)

  // A double click on the row's empty part opens Layer Style on Blending Options.
  const row = await kit.locator(`[data-layer-row="${ellipse.id}"]`).boundingBox()
  await page.mouse.dblclick(row.x + row.width - 30, row.y + row.height / 2)
  const onBlending = (await until(async () => (await count(dialog)) === 1, 40)) && (await selected('blending')) === 'true'
  step('a double click on a layer row opens Layer Style on Blending Options', onBlending)
  // Stroke by its check box, Color Overlay by its name, a colour from the Color Picker, the opacity halved.
  await kit.getByTestId(`${dialog}-check-stroke`).check()
  await until(async () => (await selected('stroke')) === 'true')
  await clickTrack(`${dialog}-field-size`, 0.04)
  await kit.getByTestId(`${dialog}-row-colorOverlay`).click()
  await until(async () => (await selected('colorOverlay')) === 'true')
  await kit.getByTestId(`${dialog}-field-color`).click()
  await until(async () => (await count('photo-color-dialog')) === 1)
  await kit.getByTestId('photo-color-dialog-hex').fill('00ff00')
  await kit.getByTestId('photo-color-dialog-ok').click()
  await until(async () => (await count('photo-color-dialog')) === 0)
  await kit.getByTestId(`${dialog}-row-blending`).click()
  await clickTrack(`${dialog}-field-opacity`, 0.5)
  await previewIdle()
  await shot('layer-style-dialog')
  await kit.getByTestId(`${dialog}-ok`).click()
  await until(async () => (await count(dialog)) === 0)
  await settled()
  const styled = await layerOf(ellipse.id)
  const style = await agent('layer.layerStyle.get', { layer: ellipse.id })
  const overlay = style.items.find((item) => item.kind === 'colorOverlay')
  const stroke = style.items.find((item) => item.kind === 'stroke')
  step(
    'OK leaves one history step with the stroke, the colour overlay and the opacity',
    (await details()).history.length === historyBefore + 1 && overlay?.params.color === '#00ff00' && stroke?.enabled === true && Math.abs(styled.opacity - 0.5) < 0.03,
    `history +${(await details()).history.length - historyBefore}, ${JSON.stringify(style.items.map((item) => item.kind))}, opacity ${styled.opacity}`,
  )
  const centre = (await agent('shape.info', { layer: ellipse.id }))?.live?.rect ?? null
  const middle = centre ? [Math.round(centre[0] + centre[2] / 2), Math.round(centre[1] + centre[3] / 2)] : null
  if (middle) {
    const green = await sample(...middle)
    step('the colour overlay shows on the canvas at half opacity', green[1] > green[0] + 40 && green[1] < 240, String(green))
  }
  // One Undo takes the whole style and the opacity back; Redo brings them again.
  await kit.getByTestId('photo-workspace').focus()
  await page.keyboard.press(process.platform === 'darwin' ? 'Meta+z' : 'Control+z')
  const undone = await until(async () => {
    const layer = await layerOf(ellipse.id)
    return !layer.effects && Math.abs((layer.opacity ?? 1) - 1) < 0.01
  }, 40)
  step('one Undo takes the effects and the opacity back together', undone)
  await kit.getByTestId('photo-menu-Edit').click()
  await kit.getByTestId('photo-cmd-edit.redo').click()
  const redone = await until(async () => (await layerOf(ellipse.id)).effects?.items.length === 2, 40)
  step('Redo brings them back', redone)

  // The effects listed under the row: an eye switches one effect off.
  const items = (await layerOf(ellipse.id)).effects.items
  const strokeIndex = items.findIndex((item) => item.key === 'stroke')
  await kit.getByTestId(`photo-effect-visibility-${ellipse.id}-${strokeIndex}`).click()
  const strokeOff = await until(async () => (await agent('layer.layerStyle.get', { layer: ellipse.id })).items.find((item) => item.kind === 'stroke')?.enabled === false, 40)
  step("an effect's eye under the layer row switches that effect off", strokeOff)

  // Layer › Layer Style › Stroke… opens on the stroke, switched on again, with its size as it was.
  await kit.getByTestId('photo-menu-Layer').click()
  await kit.locator('[data-kit-ui-id="menu:Layer/Layer Style"]').first().hover()
  await kit.getByTestId('photo-cmd-layer.layerStyle.stroke').click()
  await until(async () => (await count(dialog)) === 1, 40)
  const strokeSize = Number(await kit.getByTestId(`${dialog}-field-size-track`).getAttribute('aria-valuenow'))
  step('Layer › Layer Style › Stroke… opens on the stroke with its own size, switched on', (await selected('stroke')) === 'true' && (await kit.getByTestId(`${dialog}-check-stroke`).isChecked()) && strokeSize === stroke.params.size, `${strokeSize} vs ${stroke.params.size}`)
  // A gradient overlay from the gradient editor: a stop added in the middle.
  await kit.getByTestId(`${dialog}-check-gradientOverlay`).check()
  await until(async () => (await selected('gradientOverlay')) === 'true')
  await kit.getByTestId(`${dialog}-field-gradient`).click()
  await until(async () => (await count('photo-gradient-editor')) === 1)
  const colors = await kit.getByTestId('photo-gradient-editor-colors').boundingBox()
  await page.mouse.click(colors.x + colors.width * 0.5, colors.y + colors.height / 2)
  await until(async () => (await kit.locator('[data-testid^="photo-gradient-editor-color-"]').count()) === 3)
  // Escape in the editor over Layer Style closes the editor only; open it again and keep the stop.
  await page.keyboard.press('Escape')
  const editorOnly = (await until(async () => (await count('photo-gradient-editor')) === 0)) && (await count(dialog)) === 1
  await kit.getByTestId(`${dialog}-field-gradient`).click()
  await until(async () => (await count('photo-gradient-editor')) === 1)
  await page.mouse.click(colors.x + colors.width * 0.5, colors.y + colors.height / 2)
  await until(async () => (await kit.locator('[data-testid^="photo-gradient-editor-color-"]').count()) === 3)
  await kit.getByTestId('photo-gradient-editor-ok').click()
  await until(async () => (await count('photo-gradient-editor')) === 0)
  await previewIdle()
  await kit.getByTestId(`${dialog}-ok`).click()
  await until(async () => (await count(dialog)) === 0)
  const withGradient = await until(async () => (await agent('layer.layerStyle.get', { layer: ellipse.id })).items.find((item) => item.kind === 'gradientOverlay')?.fx?.GradientOverlay?.gradient?.stops?.length === 3, 40)
  step('a gradient overlay takes its gradient from the gradient editor; Escape there leaves Layer Style open', editorOnly && withGradient)

  // Properties: X typed moves the layer, W typed scales it (the Move tool's and Free Transform's commands).
  await kit.getByTestId('photo-dock-area-tab-properties').click()
  const boundsOf = async () => {
    // inspect's bounds: [x, y, width, height].
    const [x, y, w, h] = (await layerOf(ellipse.id)).bounds
    return { x0: x, y0: y, x1: x + w, y1: y + h }
  }
  const box = await boundsOf()
  await kit.getByTestId('photo-props-bounds-x-edit').click()
  await kit.getByTestId('photo-props-bounds-x-edit-input').fill(String(box.x0 + 40))
  await kit.getByTestId('photo-props-bounds-x-edit-input').press('Enter')
  const moved = await until(async () => (await boundsOf()).x0 === box.x0 + 40 && (await kit.getByTestId('photo-props-bounds-x').textContent()) === `${box.x0 + 40} px`, 40)
  step("Properties' X typed moves the layer", moved, JSON.stringify(await boundsOf()))
  const wide = await boundsOf()
  const width = wide.x1 - wide.x0
  await kit.getByTestId('photo-props-bounds-w-edit').click()
  await kit.getByTestId('photo-props-bounds-w-edit-input').fill(String(Math.round(width / 2)))
  await kit.getByTestId('photo-props-bounds-w-edit-input').press('Enter')
  const scaled = await until(async () => {
    const now = await boundsOf()
    return Math.abs(now.x1 - now.x0 - Math.round(width / 2)) <= 2 && Math.abs(now.x0 - wide.x0) <= 1
  }, 40)
  step("Properties' W typed scales the layer from its left edge", scaled, `${JSON.stringify(wide)} -> ${JSON.stringify(await boundsOf())}`)

  // Masks: a filled layer hidden by its mask; Density 50 shows half of it; Feather is kept.
  const backdrop = await sample(60, 60)
  {
    const layersBefore = (await details()).layers.length
    await kit.getByTestId('photo-layer-new').click()
    await until(async () => (await details()).layers.length === layersBefore + 1)
  }
  await kit.getByTestId('photo-colors-reset').click()
  await kit.getByTestId('photo-menu-Edit').click()
  await kit.getByTestId('photo-cmd-edit.fill').click()
  await until(async () => (await count('photo-dialog')) === 1)
  await kit.getByTestId('photo-dialog-contents').selectOption('black')
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await count('photo-dialog')) === 0)
  await until(async () => near(await sample(60, 60), [0, 0, 0], 2), 40)
  await kit.getByTestId('photo-layers-mask').click({ modifiers: ['Alt'] })
  const hidden = await until(async () => near(await sample(60, 60), backdrop, 2), 40)
  await kit.getByTestId('photo-dock-area-tab-properties').click()
  const masks = (await until(async () => (await count('photo-props-masks')) === 1, 40)) && hidden
  const density = kit.getByTestId('photo-props-mask-density').locator('input')
  await density.fill('50')
  await density.press('Enter')
  const half = backdrop.map((value) => Math.round(value / 2))
  const halfShown = await until(async () => near(await sample(60, 60), half, 6), 40)
  const activeLayer = async () => {
    const now = await details()
    return now.layers.find((layer) => layer.id === now.activeLayer)
  }
  const masked = await activeLayer()
  step("a hidden mask's Density 50 shows half of the layer", masks && halfShown && masked.maskDensity === 50, `${await sample(60, 60)} vs ${half}, density ${masked.maskDensity}`)
  const feather = kit.getByTestId('photo-props-mask-feather').locator('input')
  await feather.fill('12')
  await feather.press('Enter')
  const feathered = await until(async () => (await activeLayer())?.maskFeather === 12, 40)
  await kit.getByTestId('photo-props-mask-enabled').click()
  const disabled = await until(async () => near(await sample(60, 60), [0, 0, 0], 2), 40)
  step("the mask's Feather is kept, and Disable shows the whole layer", feathered && disabled, String(await sample(60, 60)))
  await shot('properties-masks')

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  failures += 1
  console.log(`FAIL ${which} the run stopped - ${error instanceof Error ? error.stack : error}`)
} finally {
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
