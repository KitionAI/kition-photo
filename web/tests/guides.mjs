// Rulers, guides, the grid and snapping in a real browser (plan 2026-11-24, stage 7): node tests/guides.mjs [chromium|webkit]
// Real drags out of the rulers, on guides and on the canvas, real keys; checks read the engine's
// guides (the binding's `guides`, which the editor shows on its workspace), its history, the
// layers and the selection, and the canvas's pixels; PASS or FAIL per step.
import { openEditor } from './canvas-harness.mjs'

const which = process.argv[2] ?? 'chromium'
const e = await openEditor(which)
const { page, kit, step, until, details, inspect, choose, selectLayer, layerNamed, drag, at, shot, workspace, surface, primary, view } = e
const guides = async () => JSON.parse((await workspace.getAttribute('data-guides')) ?? '{}')
const lastStep = async () => (await details()).history.at(-1)
/** The canvas's colour at page point (x, y) (the page's box of the canvas: the editor is in an iframe below the host's bar). */
const pixel = async (x, y) => {
  const box = await surface.boundingBox()
  return surface.evaluate((canvas, [lx, ly, width]) => {
    const ratio = canvas.width / width
    return [...canvas.getContext('2d').getImageData(Math.round(lx * ratio), Math.round(ly * ratio), 1, 1).data]
  }, [x - box.x, y - box.y, box.width])
}
/** Whether `test` holds for the canvas within a pixel of (x, y) (a one-pixel line lands on either side of a rounded point). */
const near = async (x, y, test) => {
  for (const [dx, dy] of [[0, 0], [0, -1], [0, 1], [-1, 0], [1, 0]]) if (test(await pixel(x + dx, y + dy))) return true
  return false
}
const cyan = ([r, g, b]) => b > 200 && g > 160 && r < 120
const magenta = ([r, g, b]) => r > 200 && b > 160 && g < 120
try {
  const pageBox = async () => surface.boundingBox()
  await workspace.focus()

  // ---------------------------------------------------------------- rulers
  const without = await pageBox()
  await page.keyboard.press(`${primary}+r`)
  await until(async () => (await kit.getByTestId('photo-ruler-top').count()) === 1, 30)
  await page.waitForTimeout(300)
  const withRulers = await pageBox()
  step('Cmd+R shows rulers along the top and the left, and the canvas moves in by their thickness', (await kit.getByTestId('photo-ruler-left').count()) === 1 && Math.round(withRulers.x - without.x) === 18 && Math.round(withRulers.y - without.y) === 18, JSON.stringify([withRulers.x - without.x, withRulers.y - without.y]))
  const ruler = kit.getByTestId('photo-ruler-top')
  const rulerImage = () => ruler.evaluate((canvas) => canvas.toDataURL())
  const before = await rulerImage()
  const centre = await at(600, 400)
  await page.mouse.move(centre.x, centre.y)
  await page.waitForTimeout(200)
  const marked = await ruler.evaluate((canvas, x) => {
    const box = canvas.getBoundingClientRect()
    const ratio = canvas.width / box.width
    return [...canvas.getContext('2d').getImageData(Math.round((x - box.left) * ratio), Math.round(4 * ratio), 1, 1).data]
  }, centre.x)
  step('the pointer’s place is marked on the ruler', marked[3] > 0 && marked[2] > marked[0], JSON.stringify(marked))
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.actualPixels').click()
  await page.waitForTimeout(400)
  step('the ruler follows the zoom', (await rulerImage()) !== before)
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.fitOnScreen').click()
  await page.waitForTimeout(400)
  await workspace.focus()
  await shot('rulers')

  // ---------------------------------------------------------------- a guide out of a ruler
  const steps = (await details()).history.length
  {
    const top = await ruler.boundingBox()
    const target = await at(600, 200)
    await page.mouse.move(target.x, top.y + 9)
    await page.mouse.down()
    await page.mouse.move(target.x, target.y, { steps: 12 })
    await page.waitForTimeout(200)
    const drawn = await near(target.x - 100, target.y, cyan)
    await shot('guide-drag')
    step('mid-drag the new guide is drawn across the canvas', drawn)
    await page.mouse.up()
  }
  await until(async () => (await guides()).horizontal?.length === 1, 30)
  step('dragging out of the top ruler makes a horizontal guide where it is let go, in one step', Math.abs((await guides()).horizontal[0] - 200) <= 1 && (await lastStep()) === 'New Guide' && (await details()).history.length === steps + 1, JSON.stringify(await guides()))
  {
    const left = await kit.getByTestId('photo-ruler-left').boundingBox()
    const target = await at(300, 400)
    await page.mouse.move(left.x + 9, target.y)
    await page.mouse.down()
    await page.mouse.move(target.x, target.y, { steps: 12 })
    await page.mouse.up()
  }
  await until(async () => (await guides()).vertical?.length === 1, 30)
  step('dragging out of the left ruler makes a vertical guide', Math.abs((await guides()).vertical[0] - 300) <= 1, JSON.stringify(await guides()))
  {
    const top = await ruler.boundingBox()
    const target = await at(900, 300)
    await page.keyboard.down('Alt')
    await page.mouse.move(target.x, top.y + 9)
    await page.mouse.down()
    await page.mouse.move(target.x, target.y, { steps: 8 })
    await page.mouse.up()
    await page.keyboard.up('Alt')
  }
  await until(async () => (await guides()).vertical?.length === 2, 30)
  step('Alt turns a guide from the top ruler vertical', Math.abs((await guides()).vertical[1] - 900) <= 1, JSON.stringify(await guides()))
  {
    // Let go over the ruler: no guide.
    const top = await ruler.boundingBox()
    await page.mouse.move(top.x + 200, top.y + 9)
    await page.mouse.down()
    await page.mouse.move(top.x + 200, top.y + 60, { steps: 4 })
    await page.mouse.move(top.x + 200, top.y + 6, { steps: 4 })
    await page.mouse.up()
    await page.waitForTimeout(300)
  }
  step('a guide let go back over the ruler is not made', (await guides()).horizontal.length === 1 && (await guides()).vertical.length === 2)

  // ---------------------------------------------------------------- moving and deleting guides
  await choose('Move')
  {
    const p = await at(450, 200)
    await page.mouse.move(p.x, p.y)
    await page.waitForTimeout(150)
    const cursor = await surface.evaluate((canvas) => canvas.style.cursor)
    step('over a guide the Move tool shows the guide’s cursor', cursor === 'row-resize', cursor)
  }
  const layersBefore = JSON.stringify((await details()).layers.map((layer) => layer.bounds))
  await drag([450, 200], [450, 260])
  await until(async () => Math.abs((await guides()).horizontal[0] - 260) <= 1, 30)
  step('the Move tool drags a guide (and no layer), one step', Math.abs((await guides()).horizontal[0] - 260) <= 1 && (await lastStep()) === 'Move Guide' && JSON.stringify((await details()).layers.map((layer) => layer.bounds)) === layersBefore, JSON.stringify(await guides()))
  await choose('Brush')
  await drag([300, 600], [340, 600], { keys: [primary] })
  await until(async () => Math.abs((await guides()).vertical[0] - 340) <= 1, 30)
  step('with another tool, Cmd held drags a guide', Math.abs((await guides()).vertical[0] - 340) <= 1, JSON.stringify(await guides()))
  await choose('Move')
  {
    const p = await at(450, 260)
    const top = await ruler.boundingBox()
    await page.mouse.move(p.x, p.y)
    await page.mouse.down()
    await page.mouse.move(p.x, top.y + 6, { steps: 10 })
    await page.mouse.up()
  }
  await until(async () => (await guides()).horizontal.length === 0, 30)
  step('a guide dragged back onto the ruler is deleted', (await guides()).horizontal.length === 0 && (await lastStep()) === 'Delete Guide', JSON.stringify(await guides()))

  // Lock Guides (Cmd+Alt+;): a press on a guide no longer takes it.
  await page.keyboard.press(`${primary}+Alt+Semicolon`)
  await page.waitForTimeout(200)
  await drag([340, 100], [380, 100])
  await page.waitForTimeout(400)
  step('Lock Guides keeps the guides where they are', Math.abs((await guides()).vertical[0] - 340) <= 1, JSON.stringify(await guides()))
  if ((await details()).history.at(-1) === 'Move') {
    await kit.getByTestId('photo-undo').click()
    await e.settle()
  }
  await page.keyboard.press(`${primary}+Alt+Semicolon`)
  await workspace.focus()

  // ---------------------------------------------------------------- showing and hiding
  const guideAt = await at(340, 700)
  step('the guides are drawn on the canvas', await near(guideAt.x, guideAt.y, cyan))
  await page.keyboard.press(`${primary}+Semicolon`)
  await page.waitForTimeout(300)
  step('Cmd+; hides them', !(await near(guideAt.x, guideAt.y, cyan)))
  await page.keyboard.press(`${primary}+Semicolon`)
  await page.waitForTimeout(300)
  step('and shows them again', await near(guideAt.x, guideAt.y, cyan))

  // The grid (Cmd+'): lines every 18 px of a 72 ppi document.
  const gridPoint = await at(18 * 20, 18 * 5 + 9)
  const plain = await pixel(gridPoint.x, gridPoint.y)
  await page.keyboard.press(`${primary}+Quote`)
  await page.waitForTimeout(300)
  const gridded = await pixel(gridPoint.x, gridPoint.y)
  await shot('grid')
  step("Cmd+' draws the grid", JSON.stringify(plain) !== JSON.stringify(gridded), `${plain} -> ${gridded}`)
  await page.keyboard.press(`${primary}+Quote`)
  await page.waitForTimeout(200)

  // The pixel grid at high zoom.
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.actualPixels').click()
  for (let i = 0; i < 5; i += 1) {
    await workspace.focus()
    await page.keyboard.press(`${primary}+Equal`)
    await page.waitForTimeout(250)
  }
  const zoomed = await view()
  // A strip across a few document pixels, with the pixel grid and without (View › Show › Pixel Grid).
  const strip = async () => {
    const out = []
    for (let dx = 0; dx < zoomed.zoom * 3; dx += 1) out.push((await pixel(zoomed.box.x + zoomed.box.width / 2 + dx, zoomed.box.y + zoomed.box.height / 2)).join(','))
    return out.join(' ')
  }
  const gridOn = await strip()
  await shot('pixel-grid')
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByText('Show', { exact: true }).hover()
  await kit.getByTestId('photo-cmd-view.show.pixelGrid').click()
  await page.waitForTimeout(300)
  const gridOff = await strip()
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByText('Show', { exact: true }).hover()
  await kit.getByTestId('photo-cmd-view.show.pixelGrid').click()
  step('from 600 % the pixel grid lines every pixel', zoomed.zoom >= 6 && gridOn !== gridOff, String(zoomed.zoom))
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.fitOnScreen').click()
  await page.waitForTimeout(400)
  await workspace.focus()

  // ---------------------------------------------------------------- snapping
  await choose('RectMarquee')
  await drag([200, 450], [337, 560])
  await until(async () => (await details()).hasSelection, 30)
  // The guide is where the Cmd-drag left it (WebKit's mouse may have rounded it a pixel off 340).
  const guideX = (await guides()).vertical[0]
  step('a marquee drag snaps its corner to a guide', (await details()).selectionBounds?.[0] + (await details()).selectionBounds?.[2] === guideX, JSON.stringify({ selection: (await details()).selectionBounds, guideX }))
  await drag([200, 450], [337, 560], { later: ['Control'] })
  await page.waitForTimeout(300)
  step('with Ctrl held it does not snap', (await details()).selectionBounds?.[0] + (await details()).selectionBounds?.[2] !== guideX, JSON.stringify((await details()).selectionBounds))
  await kit.getByTestId('photo-menu-Select').click()
  await kit.getByTestId('photo-cmd-edit.deselectAll').click()
  await until(async () => !(await details()).hasSelection, 30)
  await workspace.focus()

  const ellipse = await layerNamed('Ellipse 1')
  await selectLayer(ellipse.id)
  await choose('Move')
  await kit.getByTestId('photo-option-autoSelect').uncheck()
  await workspace.focus()
  const [ex, ey, ew, eh] = ellipse.bounds
  // Drag the ellipse so its left edge comes 3 px short of the guide at 340.
  await drag([ex + ew / 2, ey + eh / 2], [ex + ew / 2 + (343 - ex), ey + eh / 2 + 40])
  await until(async () => (await layerNamed('Ellipse 1')).bounds[0] !== ex, 40)
  step('a Move drag snaps the layer’s edge to a guide', (await layerNamed('Ellipse 1')).bounds[0] === Math.round(guideX), JSON.stringify((await layerNamed('Ellipse 1')).bounds))

  // Smart guides: the layer's centre to the document's, with a magenta line during the drag. Smart
  // Guides and View › Snap To › Layers and Document Bounds are on by default, as Photoshop's (stage 9).
  const viewRow = async (submenu, command, click = false) => {
    await kit.getByTestId('photo-menu-View').click()
    await kit.getByText(submenu, { exact: true }).hover()
    const row = kit.getByTestId(`photo-cmd-${command}`)
    const checked = (await row.getAttribute('aria-checked')) === 'true'
    if (click) await row.click()
    else await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
    await workspace.focus()
    return checked
  }
  const smartOn = await viewRow('Show', 'view.show.smartGuides')
  const layersOn = await viewRow('Snap To', 'view.snapTo.layers')
  const boundsOn = await viewRow('Snap To', 'view.snapTo.documentBounds')
  step('Smart Guides and View › Snap To › Layers and Document Bounds are on by default', smartOn && layersOn && boundsOn, JSON.stringify({ smartOn, layersOn, boundsOn }))
  {
    const [bx, by, bw, bh] = (await layerNamed('Ellipse 1')).bounds
    const from = await at(bx + bw / 2, by + bh / 2)
    // The centre ends 4 px right of the document's centre (x 600), away from the guides.
    const to = await at(604, by + bh / 2 + 30)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 14 })
    await page.waitForTimeout(250)
    const line = await at(600, 20)
    const drawn = await pixel(line.x, line.y)
    await shot('smart-guides')
    step('mid-drag a smart guide shows the centres meeting, in magenta', magenta(drawn), JSON.stringify(drawn))
    await page.mouse.up()
  }
  await until(async () => true, 5)
  await e.settle()
  {
    const [bx, , bw] = (await layerNamed('Ellipse 1')).bounds
    step('with Smart Guides the layer’s centre snaps to the document’s', Math.abs(bx + bw / 2 - 600) <= 0.5, JSON.stringify((await layerNamed('Ellipse 1')).bounds))
  }
  // Turned off in the View menu, the same drag lands where it was dragged.
  await viewRow('Show', 'view.show.smartGuides', true)
  await viewRow('Snap To', 'view.snapTo.layers', true)
  await viewRow('Snap To', 'view.snapTo.documentBounds', true)
  {
    const [bx, by, bw, bh] = (await layerNamed('Ellipse 1')).bounds
    const from = await at(bx + bw / 2, by + bh / 2)
    const to = await at(bx + bw / 2 + 4, by + bh / 2 + 30)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 14 })
    await page.mouse.up()
    await e.settle()
    const [ax] = (await layerNamed('Ellipse 1')).bounds
    step('with Smart Guides and snapping to layers and the document off, a drag lands where it was dragged', Math.abs(ax - (bx + 4)) <= 1, `${bx} -> ${ax}`)
  }

  // ---------------------------------------------------------------- View › New Guide…, Clear Guides
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-cmd-view.newGuide').click()
  await until(async () => (await kit.getByTestId('photo-dialog-ok').count()) === 1, 30)
  const dialog = kit.getByTestId('photo-dialog')
  await dialog.locator('select').first().selectOption('vertical')
  const position = dialog.locator('input[type="number"], input[inputmode="decimal"], input').last()
  await position.fill('777')
  await position.press('Tab')
  await kit.getByTestId('photo-dialog-ok').click()
  await until(async () => (await guides()).vertical.includes(777), 30)
  step('View › New Guide… asks for the orientation and position, and adds that guide', (await guides()).vertical.includes(777), JSON.stringify(await guides()))
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-cmd-view.clearGuides').click()
  await until(async () => (await guides()).vertical.length === 0, 30)
  step('View › Clear Guides deletes them all', (await guides()).vertical.length === 0 && (await guides()).horizontal.length === 0)
  await e.finish()
} catch (error) {
  await e.fail(error)
}
