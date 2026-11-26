// The canvas's handles and held keys in a real browser (plan 2026-11-24, stage 2): node tests/handles.mjs [chromium|webkit]
// Real pointer drags and keys on dev.html; every check reads the engine's state (layers, selection,
// history, the document's size) or the view, and prints PASS or FAIL per step. Screenshots of what a
// person sees go to the folder in KITION_SHOTS when it is set.
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
// The editor takes Cmd or Ctrl from navigator.platform (kit-editor's isMac); Playwright's
// ControlOrMeta follows the operating system running the test, and the two can disagree.
const primaryKey = async () => ((await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control')

try {
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  const kit = page.frameLocator('#kit')
  const surface = kit.getByTestId('kit-canvas-surface')
  const workspace = kit.getByTestId('photo-workspace')
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const runAgent = (command, params) => page.evaluate(([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])
  const layerNamed = async (name) => (await details()).layers.find((layer) => layer.name === name)
  const layerById = async (id) => (await details()).layers.find((layer) => layer.id === id)
  const tool = () => kit.getByTestId('photo-options-tool').getAttribute('data-tool')
  const history = async () => (await details()).history.length
  const view = async () => {
    const [zoom, x, y] = (await workspace.getAttribute('data-view')).split(' ').map(Number)
    return { zoom, x, y, box: await surface.boundingBox() }
  }
  /** Where document point (x, y) is on the page. */
  const at = async (x, y) => {
    const v = await view()
    return { x: v.box.x + (x - v.x) * v.zoom, y: v.box.y + (y - v.y) * v.zoom }
  }
  /** A real drag between document points, with keys held from before the press (`keys`) or pressed after it (`later`). */
  const drag = async (from, to, { keys = [], later = [], steps = 10, pause = 0 } = {}) => {
    const a = await at(...from)
    const b = await at(...to)
    for (const key of keys) await page.keyboard.down(key)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(a.x + (b.x - a.x) * 0.2, a.y + (b.y - a.y) * 0.2, { steps: 2 })
    for (const key of later) await page.keyboard.down(key)
    await page.mouse.move(b.x, b.y, { steps })
    if (pause) await page.waitForTimeout(pause)
    await page.mouse.up()
    for (const key of [...later, ...keys].reverse()) await page.keyboard.up(key)
  }
  const selectLayer = async (id) => {
    await kit.getByTestId(`photo-layer-${id}`).click()
    await until(async () => (await details()).activeLayer === id)
    // The keys are the canvas's again.
    await workspace.focus()
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
  // The person's Select › Deselect (an agent's edit.deselectAll keeps to the agent's own selection).
  const deselect = async () => {
    await kit.getByTestId('photo-menu-Select').click()
    await kit.getByTestId('photo-cmd-edit.deselectAll').click()
    await until(async () => !(await details()).hasSelection, 40)
    await workspace.focus()
  }
  const fitView = async () => {
    await kit.getByTestId('photo-menu-View').click()
    await kit.getByTestId('photo-control-view.fitOnScreen').click()
    await page.waitForTimeout(300)
    await workspace.focus()
  }
  const overlayPixels = () =>
    kit.locator('.kui-canvas-overlay').evaluate((canvas) => {
      const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      let visible = 0
      for (let i = 3; i < data.length; i += 4) if (data[i]) visible += 1
      return visible
    })

  // ---------------------------------------------------------------- focus and the view
  // Keys work right after the picture opens, with no click first.
  await page.keyboard.press('v')
  step('the editor has the keyboard on open: V picks the Move tool without a click', await until(async () => (await tool()) === 'Move', 20), await tool())
  const fitted = await view()
  step('no scrollbar shows while the whole picture is in view', (await kit.getByTestId('photo-scrollbars').count()) === 0 || (await kit.getByTestId('photo-scrollbars').getAttribute('data-fits')) !== null)

  // ---------------------------------------------------------------- Move
  const ellipse = await layerNamed('Ellipse 1')
  await selectLayer(ellipse.id)
  await kit.getByTestId('photo-option-autoSelect').uncheck()
  await workspace.focus()
  const [ex, ey, ew, eh] = ellipse.bounds
  const centre = [ex + ew / 2, ey + eh / 2]
  // Mid-drag: the layer's pixels show at the pointer, and nothing has reached the engine yet.
  const revisionBefore = (await inspect()).revision
  const historyBefore = await history()
  {
    const a = await at(...centre)
    const b = await at(centre[0] - 300, centre[1] + 150)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 12 })
    await page.waitForTimeout(400)
    const drawn = await kit.locator('.kui-canvas-overlay').evaluate((canvas, [fx, fy]) => {
      const x = Math.round(fx * canvas.width), y = Math.round(fy * canvas.height)
      const pixel = canvas.getContext('2d').getImageData(x, y, 1, 1).data
      return [...pixel]
    }, [(b.x - (await surface.boundingBox()).x) / (await surface.boundingBox()).width, (b.y - (await surface.boundingBox()).y) / (await surface.boundingBox()).height])
    await shot('move-mid-drag')
    step('mid-drag the layer itself shows under the pointer (its pixels, not a dashed box), and the engine is untouched', drawn[3] > 200 && drawn[0] > 200 && drawn[2] < 160 && (await inspect()).revision === revisionBefore, JSON.stringify(drawn))
    await page.mouse.up()
  }
  await settle()
  const moved = (await layerById(ellipse.id)).bounds
  const zoom = (await view()).zoom
  step('the release moves the layer by the drag, in one history step', Math.abs(moved[0] - (ex - 300)) <= 2 && Math.abs(moved[1] - (ey + 150)) <= 2 && (await history()) === historyBefore + 1, `${JSON.stringify(ellipse.bounds)} -> ${JSON.stringify(moved)} (zoom ${zoom})`)

  // Shift holds the drag to an axis: a drag of (100, 12) moves (100, 0) exactly.
  await drag([moved[0] + ew / 2, moved[1] + eh / 2], [moved[0] + ew / 2 + 100, moved[1] + eh / 2 + 12], { later: ['Shift'] })
  await settle()
  const locked = (await layerById(ellipse.id)).bounds
  step('Shift keeps a move to one axis exactly', locked[1] === moved[1] && Math.abs(locked[0] - moved[0] - 100) <= 2, `${JSON.stringify(moved)} -> ${JSON.stringify(locked)}`)

  // Alt-drag copies the layer: the original stays, the copy moves.
  const countBefore = (await details()).layers.length
  await drag([locked[0] + ew / 2, locked[1] + eh / 2], [locked[0] + ew / 2, locked[1] + eh / 2 + 120], { keys: ['Alt'] })
  await settle()
  const afterCopy = await details()
  const original = afterCopy.layers.find((layer) => layer.id === ellipse.id)
  const copy = afterCopy.layers.find((layer) => layer.id === afterCopy.activeLayer)
  step('Alt-drag copies the layer and moves the copy, leaving the original', afterCopy.layers.length === countBefore + 1 && JSON.stringify(original.bounds) === JSON.stringify(locked) && copy.id !== ellipse.id && Math.abs(copy.bounds[1] - locked[1] - 120) <= 2, `${countBefore} -> ${afterCopy.layers.length}; copy ${JSON.stringify(copy.bounds)}`)

  // Arrow keys nudge the layer one pixel (Shift: 10); quick presses are one history step.
  await selectLayer(ellipse.id)
  const nudgeFrom = (await layerById(ellipse.id)).bounds
  const nudgeHistory = await history()
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('Shift+ArrowDown')
  await page.waitForTimeout(1400)
  await settle()
  const nudged = (await layerById(ellipse.id)).bounds
  step('arrows nudge the layer 1 px and Shift+arrows 10 px, bundled into one history step', nudged[0] === nudgeFrom[0] + 3 && nudged[1] === nudgeFrom[1] + 10 && (await history()) === nudgeHistory + 1, `${JSON.stringify(nudgeFrom)} -> ${JSON.stringify(nudged)}, history +${(await history()) - nudgeHistory}`)

  // The locked Background: a drag says why and moves nothing, leaving nothing drawn.
  const background = await layerNamed('Background')
  await selectLayer(background.id)
  const lockedRevision = (await inspect()).revision
  await drag([100, 100], [300, 200])
  await page.waitForTimeout(400)
  const toastShown = await until(async () => ((await kit.locator('.kui-toast, [role="status"], [data-sonner-toast]').allTextContents()).join(' ').includes('locked')), 20)
  step('dragging the locked Background says it is locked, changes nothing and leaves no box', toastShown && (await inspect()).revision === lockedRevision && (await overlayPixels()) === 0, `toast ${toastShown}, overlay ${await overlayPixels()}`)

  // The selected layer has a mark on the canvas (its bounds, in the accent).
  await selectLayer(ellipse.id)
  await page.waitForTimeout(200)
  const markAt = await at(nudged[0], nudged[1] + nudged[3] / 2)
  const marked = await surface.evaluate((canvas, [fx, fy]) => {
    const context = canvas.getContext('2d')
    const x = Math.round(fx * canvas.width), y = Math.round(fy * canvas.height)
    const data = context.getImageData(x - 2, y - 2, 5, 5).data
    // An accent (blue-green) pixel near the layer's left edge.
    for (let i = 0; i < data.length; i += 4) if (data[i + 2] > data[i] + 40 && data[i + 1] > 60) return true
    return false
  }, [(markAt.x - (await surface.boundingBox()).x) / (await surface.boundingBox()).width, (markAt.y - (await surface.boundingBox()).y) / (await surface.boundingBox()).height])
  await shot('move-layer-mark')
  step('the selected layer is marked on the canvas', marked)

  // With a selection, Move drags the selected pixels (select.float), not the whole layer.
  await selectLayer(background.id)
  await kit.getByTestId('photo-tool-RectMarquee').click()
  await drag([40, 40], [240, 200])
  await until(async () => (await details()).hasSelection)
  await kit.getByTestId('photo-tool-Move').click()
  await workspace.focus()
  const floatHistory = await history()
  const before = (await runAgent('file.sampleColor', { x: 400, y: 300, size: 'point', sampleLayer: 'current' })).completed[0].result
  await drag([140, 120], [440, 320])
  await settle()
  const after = (await runAgent('file.sampleColor', { x: 400, y: 300, size: 'point', sampleLayer: 'current' })).completed[0].result
  const last = (await details()).history.at(-1)
  step('with a selection, Move drags the selected pixels (one step), even on the Background', (await history()) === floatHistory + 1 && JSON.stringify(before) !== JSON.stringify(after), `${JSON.stringify(before)} -> ${JSON.stringify(after)}; ${JSON.stringify(last)}`)
  await page.keyboard.press(`${await primaryKey()}+z`)
  await settle()
  await deselect()

  // ---------------------------------------------------------------- Free Transform
  await selectLayer(ellipse.id)
  const box0 = (await layerById(ellipse.id)).bounds
  const ftHistory = await history()
  await page.keyboard.press(`${await primaryKey()}+t`)
  const opened = await until(async () => (await tool()) === 'FreeTransform', 20)
  await shot('free-transform-open')
  step('Cmd+T opens Free Transform: the options bar shows its X, Y, W, H and angle', opened && (await kit.getByTestId('photo-transform-w').count()) === 1 && (await kit.getByTestId('photo-transform-angle').count()) === 1)
  // The bottom-right handle, dragged out: the layer follows live (mid-drag) and stays one step.
  {
    const a = await at(box0[0] + box0[2], box0[1] + box0[3])
    const b = await at(box0[0] + box0[2] + 100, box0[1] + box0[3] + 50)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 10 })
    await page.waitForTimeout(700)
    const live = (await layerById(ellipse.id)).bounds
    await shot('free-transform-drag')
    step('dragging a corner handle scales the layer live, during the drag', live[2] > box0[2] + 60 && live[3] > box0[3] + 30, `${JSON.stringify(box0)} -> ${JSON.stringify(live)}`)
    await page.mouse.up()
  }
  await settle()
  const w1 = Number(await kit.getByTestId('photo-transform-w').locator('input').inputValue())
  const h1 = Number(await kit.getByTestId('photo-transform-h').locator('input').inputValue())
  step('the bar follows the box (W and H in %)', Math.abs(w1 - ((box0[2] + 100) / box0[2]) * 100) < 3 && Math.abs(h1 - ((box0[3] + 50) / box0[3]) * 100) < 3, `W ${w1} H ${h1}`)
  // Shift on a corner keeps the proportions.
  {
    const now = (await layerById(ellipse.id)).bounds
    await drag([now[0] + now[2], now[1] + now[3]], [now[0] + now[2] + 80, now[1] + now[3] + 10], { later: ['Shift'] })
    await settle()
    const w = Number(await kit.getByTestId('photo-transform-w').locator('input').inputValue())
    const h = Number(await kit.getByTestId('photo-transform-h').locator('input').inputValue())
    const ratio = (w / 100) * box0[2] / ((h / 100) * box0[3])
    const before = (box0[2] + 100) / (box0[3] + 50)
    step('Shift on a corner keeps the proportions', Math.abs(ratio - before) < 0.03, `W ${w} H ${h}`)
  }
  // Outside the box rotates; the bar shows the angle.
  {
    const now = (await layerById(ellipse.id)).bounds
    const cx = now[0] + now[2] / 2, cy = now[1] + now[3] / 2
    await drag([now[0] + now[2] + 40, cy], [cx + 10, now[1] + now[3] + 120])
    await settle()
    const angle = Number(await kit.getByTestId('photo-transform-angle').locator('input').inputValue())
    await shot('free-transform-rotated')
    step('a drag outside the box rotates it (the bar shows the angle)', Math.abs(angle) > 20, `angle ${angle}`)
  }
  // Enter commits: one history step for the whole box.
  await page.keyboard.press('Enter')
  await until(async () => (await tool()) === 'Move', 20)
  await settle()
  const committed = (await layerById(ellipse.id)).bounds
  step('Enter commits Free Transform as one history step', (await history()) === ftHistory + 1 && committed[2] > box0[2] + 60, `history +${(await history()) - ftHistory}, ${JSON.stringify(box0)} -> ${JSON.stringify(committed)}`)
  // Escape takes an open box back: nothing changes.
  const escHistory = await history()
  await page.keyboard.press(`${await primaryKey()}+t`)
  await until(async () => (await tool()) === 'FreeTransform', 20)
  await drag([committed[0] + committed[2] / 2, committed[1] + committed[3] / 2], [committed[0] + committed[2] / 2 + 60, committed[1] + committed[3] / 2])
  await settle()
  await page.keyboard.press('Escape')
  await until(async () => (await tool()) === 'Move', 20)
  await settle()
  step('Escape cancels Free Transform: the layer and the history are as before', JSON.stringify((await layerById(ellipse.id)).bounds) === JSON.stringify(committed) && (await history()) === escHistory, `${JSON.stringify(committed)} -> ${JSON.stringify((await layerById(ellipse.id)).bounds)}`)
  // The bar's W field scales the box; Edit › Free Transform opens it from the menu.
  await kit.getByTestId('photo-menu-Edit').click()
  await kit.getByTestId('photo-edit-free-transform').click()
  await until(async () => (await tool()) === 'FreeTransform', 20)
  const field = kit.getByTestId('photo-transform-w').locator('input')
  await field.fill('50')
  await field.press('Enter')
  await settle()
  await kit.getByTestId('photo-transform-commit').click()
  await until(async () => (await tool()) === 'Move', 20)
  await settle()
  const halved = (await layerById(ellipse.id)).bounds
  step('Edit › Free Transform opens the box and its W field scales the layer', Math.abs(halved[2] - committed[2] / 2) <= 3, `${committed[2]} -> ${halved[2]}`)

  // Plan 2026-11-24, stage 8: after a field of the bar, Enter applies it and the next Enter commits;
  // Escape puts a field being typed in back, and the next Escape cancels the box.
  const fieldHistory = await history()
  await workspace.focus()
  await page.keyboard.press(`${await primaryKey()}+t`)
  await until(async () => (await tool()) === 'FreeTransform', 20)
  await kit.getByTestId('photo-transform-w').locator('input').fill('80')
  await page.keyboard.press('Enter')
  await settle()
  const stillOpen = (await tool()) === 'FreeTransform'
  await page.keyboard.press('Enter')
  await until(async () => (await tool()) === 'Move', 20)
  await settle()
  const fieldCommitted = (await layerById(ellipse.id)).bounds
  step('a W typed in the bar: the first Enter applies it, the second commits the box (one step)', stillOpen && (await tool()) === 'Move' && Math.abs(fieldCommitted[2] - halved[2] * 0.8) <= 3 && (await history()) === fieldHistory + 1, `${halved[2]} -> ${fieldCommitted[2]}, open after one Enter ${stillOpen}, history +${(await history()) - fieldHistory}`)
  await page.keyboard.press(`${await primaryKey()}+t`)
  await until(async () => (await tool()) === 'FreeTransform', 20)
  const wField = kit.getByTestId('photo-transform-w').locator('input')
  await wField.fill('50')
  await page.keyboard.press('Escape')
  const reverted = (await tool()) === 'FreeTransform' && Number(await wField.inputValue()) === 100
  await page.keyboard.press('Escape')
  await until(async () => (await tool()) === 'Move', 20)
  await page.keyboard.press(`${await primaryKey()}+t`)
  await until(async () => (await tool()) === 'FreeTransform', 20)
  await kit.getByTestId('photo-transform-w').locator('input').fill('50')
  await page.keyboard.press('Tab')
  await settle()
  await page.keyboard.press('Escape')
  await until(async () => (await tool()) === 'Move', 20)
  await settle()
  step('Escape in a field puts it back, the next Escape cancels; after Tab one Escape cancels', reverted && (await tool()) === 'Move' && JSON.stringify((await layerById(ellipse.id)).bounds) === JSON.stringify(fieldCommitted) && (await history()) === fieldHistory + 1, `${reverted} ${JSON.stringify((await layerById(ellipse.id)).bounds)}`)
  // Back to the halved layer the next steps start from.
  await kit.getByTestId('photo-undo').click()
  await until(async () => JSON.stringify((await layerById(ellipse.id)).bounds) === JSON.stringify(halved), 40)
  await workspace.focus()

  // Show Transform Controls: a handle of the Move tool's box starts Free Transform.
  await kit.getByTestId('photo-option-showTransform').check()
  await workspace.focus()
  await page.waitForTimeout(200)
  await drag([halved[0] + halved[2], halved[1] + halved[3] / 2], [halved[0] + halved[2] + 60, halved[1] + halved[3] / 2])
  await settle()
  const fromControls = (await tool()) === 'FreeTransform'
  await page.keyboard.press('Enter')
  await until(async () => (await tool()) === 'Move', 20)
  await settle()
  step("Show Transform Controls' handles open Free Transform and drag it", fromControls && (await layerById(ellipse.id)).bounds[2] > halved[2] + 30, JSON.stringify((await layerById(ellipse.id)).bounds))
  await kit.getByTestId('photo-option-showTransform').uncheck()
  await kit.getByTestId('photo-option-autoSelect').check()

  // ---------------------------------------------------------------- Crop
  await fitView()
  const size = await details()
  await workspace.focus()
  await page.keyboard.press('c')
  await until(async () => (await tool()) === 'Crop', 20)
  await page.waitForTimeout(200)
  await shot('crop-frame')
  step('the Crop tool starts with a frame round the whole canvas (nothing to apply yet)', await kit.getByTestId('photo-crop-apply').isDisabled())
  // The right edge's handle, dragged in by 200 document pixels.
  await drag([size.width, size.height / 2], [size.width - 200, size.height / 2])
  await page.waitForTimeout(200)
  await shot('crop-handle')
  const applyOn = await kit.getByTestId('photo-crop-apply').isEnabled()
  await page.keyboard.press('Enter')
  await until(async () => (await details()).width !== size.width, 40)
  const cropped = await details()
  step('a crop handle resizes the frame; Enter crops to it', applyOn && Math.abs(cropped.width - (size.width - 200)) <= 2 && cropped.height === size.height, `${size.width}x${size.height} -> ${cropped.width}x${cropped.height}`)
  await kit.getByTestId('photo-undo').click()
  await until(async () => (await details()).width === size.width, 40)
  await fitView()
  // A drag outside the frame turns it; a double click inside crops (the canvas turns to fit).
  await kit.getByTestId('photo-tool-Move').click()
  await kit.getByTestId('photo-tool-Crop').click()
  await page.waitForTimeout(200)
  await drag([100, 200], [1100, 600])
  await drag([1150, 400], [1150, 520], { later: ['Shift'] })
  await page.waitForTimeout(200)
  await shot('crop-rotated')
  const inside = await at(600, 400)
  await page.mouse.dblclick(inside.x, inside.y)
  await until(async () => (await details()).width !== size.width, 40)
  const turned = await details()
  step('a drag outside turns the frame (Shift: 15° steps) and a double click inside crops', turned.width !== size.width && Math.abs(turned.width - 1000) <= 3 && Math.abs(turned.height - 400) <= 3, `${turned.width}x${turned.height}`)
  await kit.getByTestId('photo-undo').click()
  await until(async () => (await details()).width === size.width, 40)

  // Plan 2026-11-24, stage 8: the frame's edges are grabbed anywhere along them (with a resize
  // cursor), and Cmd+Z while the frame is open takes back its last adjustment, not a document step.
  await fitView()
  await kit.getByTestId('photo-tool-Move').click()
  await kit.getByTestId('photo-tool-Crop').click()
  await workspace.focus()
  await page.waitForTimeout(200)
  const statesBefore = (await details()).history.join('|')
  const onRightEdge = await at(size.width, size.height * 0.2)
  await page.mouse.move(onRightEdge.x - 2, onRightEdge.y)
  await page.waitForTimeout(150)
  const edgeCursor = await surface.evaluate((element) => getComputedStyle(element).cursor)
  await drag([size.width, size.height * 0.2], [size.width - 150, size.height * 0.2])
  await drag([size.width * 0.3, 0], [size.width * 0.3, 100])
  await page.waitForTimeout(200)
  await shot('crop-edges')
  await page.keyboard.press(`${await primaryKey()}+z`)
  await page.waitForTimeout(200)
  const sameDocument = (await details()).history.join('|') === statesBefore
  await page.keyboard.press('Enter')
  await until(async () => (await details()).width !== size.width, 40)
  const edged = await details()
  step('a crop edge drags anywhere along it (resize cursor); Cmd+Z takes back the frame’s last change, not a document step', edgeCursor === 'ew-resize' && sameDocument && Math.abs(edged.width - (size.width - 150)) <= 2 && edged.height === size.height, `${edgeCursor}, ${edged.width}x${edged.height}, document untouched ${sameDocument}`)
  await kit.getByTestId('photo-undo').click()
  await until(async () => (await details()).width === size.width, 40)
  await kit.getByTestId('photo-tool-Move').click()
  await kit.getByTestId('photo-tool-Crop').click()
  await workspace.focus()
  await page.waitForTimeout(200)
  const statesNow = (await details()).history.join('|')
  await drag([size.width * 0.5, size.height], [size.width * 0.5, size.height - 120])
  await page.waitForTimeout(200)
  const adjusted = await kit.getByTestId('photo-crop-apply').isEnabled()
  await page.keyboard.press(`${await primaryKey()}+z`)
  await page.waitForTimeout(200)
  step('Cmd+Z on the only adjustment gives back the untouched frame (nothing to apply) and leaves the history', adjusted && (await kit.getByTestId('photo-crop-apply').isDisabled()) && (await details()).history.join('|') === statesNow)

  // ---------------------------------------------------------------- selections
  await fitView()
  await kit.getByTestId('photo-tool-RectMarquee').click()
  await workspace.focus()
  await drag([100, 100], [300, 250])
  await until(async () => (await details()).hasSelection)
  const sel0 = (await details()).selectionBounds
  const selHistory = await history()
  await drag([200, 175], [260, 215])
  await settle()
  const sel1 = (await details()).selectionBounds
  step('dragging inside the selection with a selection tool moves its outline (one step), not a new selection', sel1 && Math.abs(sel1[0] - sel0[0] - 60) <= 1 && Math.abs(sel1[1] - sel0[1] - 40) <= 1 && sel1[2] === sel0[2] && sel1[3] === sel0[3] && (await history()) === selHistory + 1, `${JSON.stringify(sel0)} -> ${JSON.stringify(sel1)}`)
  await page.keyboard.press('ArrowLeft')
  await page.keyboard.press('ArrowLeft')
  await page.waitForTimeout(1300)
  await settle()
  const sel2 = (await details()).selectionBounds
  step('arrows nudge the selection outline', sel2 && sel2[0] === sel1[0] - 2 && sel2[1] === sel1[1], JSON.stringify(sel2))
  // Alt pressed after the press draws the marquee from its centre; the drag shows its size.
  await deselect()
  {
    const a = await at(600, 400)
    const b = await at(700, 460)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(a.x + 10, a.y + 10, { steps: 2 })
    await page.keyboard.down('Alt')
    await page.mouse.move(b.x, b.y, { steps: 8 })
    await page.waitForTimeout(150)
    await shot('marquee-from-centre')
    await page.mouse.up()
    await page.keyboard.up('Alt')
  }
  await settle()
  const centred = (await details()).selectionBounds
  step('Alt after the press draws the marquee from its centre', centred && Math.abs(centred[0] - 500) <= 2 && Math.abs(centred[1] - 340) <= 2 && Math.abs(centred[2] - 200) <= 3, JSON.stringify(centred))
  // The Polygonal Lasso: Backspace takes the last point back.
  await deselect()
  await kit.getByTestId('photo-tool-Lasso').click({ button: 'right' })
  await kit.getByTestId('photo-tool-PolygonLasso').click()
  await workspace.focus()
  for (const [x, y] of [[100, 100], [400, 100], [400, 300], [900, 700]]) {
    const p = await at(x, y)
    await page.mouse.click(p.x, p.y)
    await page.waitForTimeout(80)
  }
  await page.keyboard.press('Backspace')
  {
    const p = await at(250, 500)
    await page.mouse.move(p.x, p.y, { steps: 3 })
  }
  await shot('polygon-lasso')
  await page.keyboard.press('Enter')
  await until(async () => (await details()).hasSelection)
  const polygon = (await details()).selectionBounds
  step('Backspace takes the Polygonal Lasso’s last point back', polygon && polygon[0] + polygon[2] <= 402 && polygon[1] + polygon[3] <= 302, JSON.stringify(polygon))
  await deselect()
  // Quick Selection grows the selection during the drag.
  await kit.getByTestId('photo-tool-ObjectSelection').click({ button: 'right' })
  await kit.getByTestId('photo-tool-QuickSelection').click()
  await workspace.focus()
  {
    const a = await at(150, 150)
    const b = await at(450, 250)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 20 })
    const during = await until(async () => (await details()).hasSelection, 30)
    await shot('quick-selection-drag')
    await page.mouse.up()
    await settle()
    step('Quick Selection shows its selection during the drag', during && (await details()).hasSelection)
  }
  await deselect()

  // ---------------------------------------------------------------- held keys and the view
  await fitView()
  // Cmd held turns a Brush press into the Move tool's.
  await kit.getByTestId('photo-tool-Brush').click()
  await selectLayer(ellipse.id)
  const cmdFrom = (await layerById(ellipse.id)).bounds
  const cmdRevision = (await inspect()).revision
  await drag([cmdFrom[0] + cmdFrom[2] / 2, cmdFrom[1] + cmdFrom[3] / 2], [cmdFrom[0] + cmdFrom[2] / 2 + 50, cmdFrom[1] + cmdFrom[3] / 2], { keys: [await primaryKey()] })
  await settle()
  const cmdTo = (await layerById(ellipse.id)).bounds
  step('holding Cmd with the Brush moves the layer (the temporary Move tool)', Math.abs(cmdTo[0] - cmdFrom[0] - 50) <= 2 && (await inspect()).revision > cmdRevision && (await tool()) === 'Brush', `${JSON.stringify(cmdFrom)} -> ${JSON.stringify(cmdTo)}`)
  // Space held pans, also mid-stroke: the view moves and the stroke goes on (on the pixel layer).
  await selectLayer((await layerNamed('Layer 1')).id)
  const panHistory = await history()
  {
    const a = await at(300, 500)
    const viewBefore = await view()
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(a.x + 30, a.y, { steps: 3 })
    await page.keyboard.down('Space')
    await page.mouse.move(a.x + 130, a.y + 40, { steps: 6 })
    await page.keyboard.up('Space')
    const viewMid = await view()
    await page.mouse.move(a.x + 180, a.y + 40, { steps: 3 })
    await page.mouse.up()
    await settle()
    step('Space held mid-stroke pans the view, and the stroke goes on', (viewMid.x !== viewBefore.x || viewMid.y !== viewBefore.y) && (await history()) === panHistory + 1, `${JSON.stringify([viewBefore.x, viewBefore.y])} -> ${JSON.stringify([viewMid.x, viewMid.y])}`)
  }
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.fitOnScreen').click()
  await page.waitForTimeout(300)
  await workspace.focus()
  // Cmd+Space and a click zooms in.
  {
    const before = (await view()).zoom
    const p = await at(600, 400)
    await page.keyboard.down(await primaryKey())
    await page.keyboard.down('Space')
    await page.mouse.click(p.x, p.y)
    await page.keyboard.up('Space')
    await page.keyboard.up(await primaryKey())
    await page.waitForTimeout(200)
    step('Cmd+Space and a click zooms in', (await view()).zoom > before * 1.5, `${before} -> ${(await view()).zoom}`)
  }
  // A mouse wheel notch with Ctrl zooms 1.2 times; Alt and the wheel zooms too.
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.fitOnScreen').click()
  await page.waitForTimeout(300)
  {
    const p = await at(600, 400)
    await page.mouse.move(p.x, p.y)
    const before = (await view()).zoom
    await page.keyboard.down('Control')
    await page.mouse.wheel(0, -100)
    await page.keyboard.up('Control')
    await page.waitForTimeout(200)
    const ctrl = (await view()).zoom
    await page.keyboard.down('Alt')
    await page.mouse.wheel(0, -100)
    await page.keyboard.up('Alt')
    await page.waitForTimeout(200)
    const alt = (await view()).zoom
    step('a Ctrl+wheel notch zooms 1.2 times (not 2.7), and Alt+wheel zooms as well', Math.abs(ctrl / before - 1.2) < 0.02 && Math.abs(alt / ctrl - 1.2) < 0.02, `${before} -> ${ctrl} -> ${alt}`)
  }
  // The zoom field gives the keyboard back on Enter: the next key picks a tool.
  {
    const zoomField = kit.getByTestId('photo-status-zoom').locator('input')
    await zoomField.fill('50')
    await zoomField.press('Enter')
    await page.waitForTimeout(150)
    await page.keyboard.press('m')
    const picked = await until(async () => (await tool()) === 'RectMarquee', 20)
    step('the zoom field gives the keyboard back on Enter', picked && (await zoomField.inputValue()) === '50', `${await tool()} ${await zoomField.inputValue()}`)
  }
  // Shift and a tool's key cycles its slot.
  await page.keyboard.press('Shift+M')
  step("Shift and a tool's key cycles its slot", await until(async () => (await tool()) === 'EllipseMarquee', 20), await tool())
  // Double-clicking the Hand fits the picture; the Zoom tool shows it at 100 %.
  await kit.getByTestId('photo-tool-Zoom').dblclick()
  const actual = await until(async () => (await view()).zoom === 1, 20)
  await kit.getByTestId('photo-tool-Hand').dblclick()
  const fit = await until(async () => Math.abs((await view()).zoom - fitted.zoom) < 1e-6, 20)
  step('double-clicking the Zoom tool shows 100 %, the Hand fits the picture', actual && fit, `${(await view()).zoom} vs ${fitted.zoom}`)
  // A press on the canvas leaves no focus ring around it.
  await kit.getByTestId('photo-tool-Brush').click()
  {
    const p = await at(600, 760)
    await page.mouse.click(p.x, p.y)
    const ring = await workspace.evaluate((element) => {
      const style = getComputedStyle(element)
      return { focused: document.activeElement === element, outline: style.outlineStyle, width: style.outlineWidth }
    })
    step('a press on the canvas takes the keyboard but draws no focus ring', ring.focused && (ring.outline === 'none' || ring.width === '0px'), JSON.stringify(ring))
  }
  step('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} FAILED (${which})` : `ALL PASSED (${which})`)
process.exit(failures ? 1 : 0)
