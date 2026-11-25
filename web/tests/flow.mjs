// The editor's flow in a real browser, through the development host: node tests/flow.mjs [chromium|webkit]
// Starts the Vite dev server itself, opens dev.html in Chinese, and prints PASS or FAIL per step.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { checkUiIds } from '@kition/sdk/dev-host/check-ui-ids'
import { TOOL_FIELDS } from '../src/tools/options.ts'

const which = process.argv[2] ?? 'chromium'
const UPSTREAM_TOOLS = JSON.parse(readFileSync(new URL('../parity/photo-upstream.json', import.meta.url), 'utf8')).tools
const KIT_BRAND = JSON.parse(readFileSync(new URL('../../kit.json', import.meta.url), 'utf8')).ui.brand
// Vite's root and config come from this file's place, so the flow runs from any directory.
const web = fileURLToPath(new URL('..', import.meta.url))
const server = await createServer({ root: web, configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)), server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
await context.tracing.start({ screenshots: true, snapshots: true })
if (which === 'chromium') await context.grantPermissions(['clipboard-read', 'clipboard-write'])
const page = await context.newPage()
// The editor takes Cmd or Ctrl from navigator.platform (kit-editor's isMac); Playwright's
// ControlOrMeta follows the operating system running the test, and the two can disagree.
const primaryKey = async () => (await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control'
const errors = []
let failures = 0
const results = []
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  results.push({ name, ok, detail })
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
// Runs a customize-dialog interaction on its own: a missing element (expected before Photo
// declares its UI) fails just this step, with the error's message, instead of aborting the rest.
const attempt = async (name, fn) => {
  try {
    step(name, await fn())
  } catch (error) {
    step(name, false, error instanceof Error ? error.message.split('\n')[0] : String(error))
  }
}
// A "…" command opens its dialog (plan 2026-11-24, stage 1): wait for it and press OK.
const okDialog = async () => {
  const kit = page.frameLocator('#kit')
  await kit.getByTestId('photo-dialog').waitFor({ timeout: 10000 })
  await kit.getByTestId('photo-dialog-ok').click()
  await kit.getByTestId('photo-dialog').waitFor({ state: 'detached', timeout: 10000 })
}
const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
const details = async () => (await inspect()).details

try {
  await page.goto(`${origin}/dev.html?locale=zh-CN`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const count = (testId) => kit.getByTestId(testId).count()
  await page.waitForTimeout(1000)
  step('Chinese menu names', (await kit.getByTestId('photo-menu-Image').textContent()) === '图像')
  step('status describes the image', (await kit.getByTestId('photo-status').textContent())?.includes('4 个图层'))
  // Upstream's rail: two columns by default; its chevrons switch to one and back, a choice the kit store keeps.
  const rail = kit.getByTestId('photo-tool-rail')
  const twoByDefault = (await rail.getAttribute('data-columns')) === '2'
  const railWidth = (await rail.boundingBox()).width
  await kit.getByTestId('photo-tool-rail-columns').click()
  const oneColumn = (await rail.getAttribute('data-columns')) === '1'
  const keptColumns = await kit.locator('body').evaluate(() => localStorage.getItem('kit:kit.photo:railColumns'))
  await kit.getByTestId('photo-tool-rail-columns').click()
  step('the rail draws two columns by default and its chevrons switch to one and back', twoByDefault && railWidth > 70 && oneColumn && keptColumns === '1' && (await rail.getAttribute('data-columns')) === '2', JSON.stringify({ twoByDefault, railWidth, oneColumn, keptColumns }))
  // Layers rows: the kind under the name (none for a pixel layer), the Background's name in italics.
  const opened = (await details()).layers
  const kindOf = async (layer) => ((await count(`photo-layer-kind-${layer.id}`)) ? await kit.getByTestId(`photo-layer-kind-${layer.id}`).textContent() : null)
  const typeKind = await kindOf(opened.find((layer) => layer.kind === 'Type'))
  const shapeKind = await kindOf(opened.find((layer) => layer.kind === 'Shape'))
  const pixelKind = await kindOf(opened.find((layer) => layer.kind === 'Pixel' && layer.name !== 'Background'))
  const backgroundRow = opened.find((layer) => layer.name === 'Background')
  const italic = await kit.getByTestId(`photo-layer-${backgroundRow.id}`).evaluate((element) => getComputedStyle(element).fontStyle)
  const thumbSide = (await kit.getByTestId(`photo-layer-thumb-${backgroundRow.id}`).boundingBox()).height
  step("Layers rows show the kind under the name, the Background in italics, and upstream's larger thumbnails", Boolean(typeKind) && Boolean(shapeKind) && typeKind !== shapeKind && pixelKind === null && italic === 'italic' && thumbSide >= 34, JSON.stringify({ typeKind, shapeKind, pixelKind, italic, thumbSide }))
  // The rail's Quick Mask button: on (pressed), then off, as Select › Edit in Quick Mask Mode.
  const quickMask = kit.getByTestId('photo-quick-mask')
  await quickMask.click()
  const maskOn = await until(async () => (await details()).channels?.quickMask != null && (await quickMask.getAttribute('aria-pressed')) === 'true', 40)
  await quickMask.click()
  const maskOff = await until(async () => (await details()).channels?.quickMask == null && (await quickMask.getAttribute('aria-pressed')) === 'false', 40)
  step("the rail's Quick Mask button enters and leaves Quick Mask", maskOn && maskOff, JSON.stringify({ maskOn, maskOff }))
  const region = await kit.getByTestId('photo-canvas-region').boundingBox()
  const at = (fx, fy) => ({ x: region.x + region.width * fx, y: region.y + region.height * fy })

  const before = (await inspect()).revision
  await kit.getByTestId('photo-layer-new').click()
  await until(async () => (await details()).layers.length === 5)
  const a = at(0.3, 0.4)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  await page.mouse.move(a.x + 120, a.y + 40, { steps: 8 })
  await page.mouse.up()
  await until(async () => (await inspect()).revision > before + 1)
  step('paints a brush stroke', (await inspect()).revision > before + 1 && (await inspect()).canUndo)

  await kit.getByTestId('photo-tool-RectMarquee').click()
  const s = at(0.2, 0.2)
  await page.mouse.move(s.x, s.y)
  await page.mouse.down()
  await page.mouse.move(s.x + 150, s.y + 100, { steps: 5 })
  await page.mouse.up()
  await until(async () => (await details()).hasSelection === true)
  step('selects with the marquee', (await details()).hasSelection === true)

  await kit.getByTestId('photo-tool-Gradient').click({ button: 'right' })
  await kit.getByTestId('photo-tool-PaintBucket').click()
  step('chooses a tool from a flyout', (await kit.getByTestId('photo-tool-PaintBucket').getAttribute('aria-pressed')) === 'true')

  await kit.getByTestId('photo-tool-Type').click()
  const t = at(0.4, 0.75)
  await page.mouse.click(t.x, t.y)
  await kit.getByTestId('photo-text-field').waitFor()
  if (which === 'chromium') {
    const cdp = await page.context().newCDPSession(page)
    await cdp.send('Input.imeSetComposition', { text: 'ji', selectionStart: 2, selectionEnd: 2 })
    await cdp.send('Input.insertText', { text: '极光' })
  } else {
    await kit.getByTestId('photo-text-field').fill('极光')
  }
  await kit.getByTestId('photo-text-field').press('Enter')
  await until(async () => (await details()).layers.some((layer) => layer.name === '极光'))
  step('places Chinese text as a type layer', (await details()).layers.some((layer) => layer.name === '极光'))
  const typeInfo = async (layer) => (await page.evaluate((id) => window.devHost.connection().ai.run([{ command: 'type.info', params: { layer: id } }], false), layer)).completed[0].result
  const textLayer = (await details()).layers.find((layer) => layer.name === '极光')
  step("new text takes the bar's anti-aliasing (Sharp) in its one step", (await typeInfo(textLayer.id)).antialias === 'Sharp', (await typeInfo(textLayer.id)).antialias)

  // Upstream's 49 tools: each one from its slot or the slot's flyout, the options bar naming it.
  const slotButton = (id) => kit.locator(`.kui-tool-slot > [data-testid="photo-tool-${id}"]`)
  const slotOf = (id) => {
    const tool = UPSTREAM_TOOLS.find((item) => item.id === id)
    return UPSTREAM_TOOLS.filter((item) => item.section === tool.section && item.slot === tool.slot).map((item) => item.id)
  }
  const choose = async (id) => {
    if (await slotButton(id).count()) return slotButton(id).click()
    for (const mate of slotOf(id)) {
      if (await slotButton(mate).count()) {
        await slotButton(mate).click({ button: 'right' })
        break
      }
    }
    await kit.locator(`.kui-tool-flyout [data-testid="photo-tool-${id}"]`).click()
  }
  const activeTool = () => kit.getByTestId('photo-options-tool').getAttribute('data-tool')
  const unnamed = []
  const misdrawn = []
  // Each slot's tools last to first, so every slot ends on its first tool, as the later steps expect.
  for (const tool of [...UPSTREAM_TOOLS].reverse()) {
    await choose(tool.id)
    const header = kit.getByTestId('photo-options-tool')
    await until(async () => (await header.getAttribute('data-tool')) === tool.id, 20)
    const name = ((await header.textContent()) ?? '').trim()
    const label = (await slotButton(tool.id).getAttribute('aria-label').catch(() => null)) ?? ''
    if ((await header.getAttribute('data-tool')) !== tool.id || !name || !label.startsWith(name)) unnamed.push(`${tool.id}: "${name}" vs "${label}"`)
    const drawn = [...new Set(await kit.locator('[data-testid="photo-options-bar"] [data-option]').evaluateAll((fields) => fields.map((field) => field.getAttribute('data-option'))))].sort()
    const listed = [...TOOL_FIELDS[tool.id]].sort()
    if (JSON.stringify(drawn) !== JSON.stringify(listed)) misdrawn.push(`${tool.id}: drawn ${drawn.join(',')} vs listed ${listed.join(',')}`)
  }
  step('selects each of the 49 tools from its slot or flyout, and the options bar names it', unnamed.length === 0, unnamed.slice(0, 4).join('; '))
  step("each tool's bar draws exactly its listed options (TOOL_FIELDS)", misdrawn.length === 0, misdrawn.slice(0, 4).join('; '))

  // Tool keys as upstream: by default the slot's key again cycles, Shift or not; with Preferences ›
  // Tools › Use Shift Key for Tool Switch, only Shift and the key cycles.
  const shiftSwitch = (value) => page.evaluate((on) => window.devHost.connection().ai.run([{ command: 'prefs.set', params: { path: 'tools.useShiftKeyForToolSwitch', value: on } }], false), value)
  const press = async (key, expected) => {
    await page.keyboard.press(key)
    await until(async () => (await activeTool()) === expected, 10)
    return activeTool()
  }
  await choose('Move')
  await press('b', 'Brush')
  step('B selects the Brush and B again cycles to the Pencil (the default)', (await press('b', 'Pencil')) === 'Pencil', await activeTool())
  step('Shift+B cycles as B does by default', (await press('Shift+B', 'MixerBrush')) === 'MixerBrush', await activeTool())
  await shiftSwitch(true)
  // The editor reads the preference again after the change; until it has, B B still cycles.
  const keeps = await until(async () => {
    await choose('Move')
    await press('b', 'Brush')
    await page.keyboard.press('b')
    await page.waitForTimeout(50)
    return (await activeTool()) === 'Brush'
  }, 20)
  step('with Use Shift Key for Tool Switch, B keeps the tool and Shift+B cycles', keeps && (await press('Shift+B', 'Pencil')) === 'Pencil', await activeTool())
  await shiftSwitch(false)

  // Painting tools work on the painted pixel layer, over the brush stroke above.
  const pixel = (await details()).layers.find((layer) => layer.kind === 'Pixel')
  await kit.getByTestId(`photo-layer-${pixel.id}`).click()
  await until(async () => (await details()).activeLayer === pixel.id)
  const stroke = async (fx, fy) => {
    const from = at(fx, fy)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(from.x + 90, from.y + 20, { steps: 6 })
    await page.mouse.up()
  }
  const changes = async (name, id, act) => {
    await choose(id)
    const start = (await inspect()).revision
    await act()
    step(name, await until(async () => (await inspect()).revision > start, 40), `${id}, revision ${start} -> ${(await inspect()).revision}`)
  }
  await changes('paints with the Pencil', 'Pencil', () => stroke(0.3, 0.4))
  await changes('erases with the Eraser', 'Eraser', () => stroke(0.3, 0.41))
  await changes('dodges with the Dodge tool', 'Dodge', () => stroke(0.31, 0.4))
  await changes('blurs with the Blur tool', 'Blur', () => stroke(0.32, 0.4))
  await choose('CloneStamp')
  const source = at(0.3, 0.4)
  await page.keyboard.down('Alt')
  await page.mouse.click(source.x, source.y)
  await page.keyboard.up('Alt')
  await until(async () => (await count('photo-clone-hint')) === 0, 20)
  await changes('clones from an Alt-clicked source with the Clone Stamp', 'CloneStamp', () => stroke(0.5, 0.5))
  const bounds = JSON.stringify((await details()).selectionBounds)
  await changes('adds to the selection with Quick Selection', 'QuickSelection', () => stroke(0.6, 0.3))
  step('Quick Selection grows the selection', (await details()).hasSelection === true && JSON.stringify((await details()).selectionBounds) !== bounds, `${bounds} -> ${JSON.stringify((await details()).selectionBounds)}`)
  await choose('Pen')
  for (const [fx, fy] of [
    [0.55, 0.6],
    [0.7, 0.65],
  ]) {
    const point = at(fx, fy)
    await page.mouse.click(point.x, point.y)
  }
  await page.keyboard.press('Enter')
  const workPath = async () => (await page.evaluate(() => window.devHost.connection().ai.run([{ command: 'path.info', params: { name: 'work' } }], false)).catch(() => null))?.completed?.[0]?.result?.path
  await until(async () => (await workPath())?.subpaths?.[0]?.knots?.length === 2, 30)
  step('makes a work path with two Pen clicks and Enter', (await workPath())?.subpaths?.[0]?.knots?.length === 2)
  const shapes = (await details()).layers.length
  await choose('Rectangle')
  await stroke(0.6, 0.2)
  await until(async () => (await details()).layers.length === shapes + 1)
  step('draws a shape layer with the Rectangle tool', (await details()).layers.length === shapes + 1)
  // Back to each slot's first tool, which the later steps click.
  for (const tool of UPSTREAM_TOOLS.filter((item, index, all) => index === all.findIndex((other) => other.section === item.section && other.slot === item.slot))) await choose(tool.id)

  // The options bars (upstream's, per tool): a field's value reaches the tool's next command.
  const option = (field) => kit.locator(`[data-kit-ui-id="control:options.${field}"]`)
  const setNumber = async (field, value) => {
    const input = option(field).locator('input')
    await input.fill(String(value))
    await input.press('Enter')
  }
  const findLayer = (layers, id) => {
    for (const layer of layers ?? []) {
      if (layer.id === id) return layer
      const inner = findLayer(layer.children, id)
      if (inner) return inner
    }
    return null
  }
  const runAgent = (command, params) => page.evaluate(([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])
  // The person's Undo (the editor's button): an agent's edit.undo takes back only the agent's own
  // steps (KitAgent A7), so it would undo the agent's last layer.new.layer, not the person's edit.
  const undo = async () => {
    const before = (await inspect()).revision
    await kit.getByTestId('photo-undo').click()
    await until(async () => (await inspect()).revision !== before, 40)
  }
  const activeBounds = async () => {
    const now = await details()
    return findLayer(now.layers, now.activeLayer)?.bounds ?? null
  }
  const sampleCurrent = async (point) => (await runAgent('document.sampleColor', { ...point, sampleLayer: 'current', size: 'point' })).completed[0].result
  const selectLayer = async (id) => {
    await kit.getByTestId(`photo-layer-${id}`).click()
    await until(async () => (await details()).activeLayer === id)
  }
  // Agent calls keep their own editor state. Prepare the person's canvas through its UI.
  const createForPerson = async (command, params) => {
    const result = await runAgent(command, params)
    await selectLayer(result.completed[0].result.layer)
    return result
  }
  const deselectForPerson = async () => {
    await kit.getByTestId('photo-menu-Select').click()
    await kit.getByTestId('photo-cmd-select.deselect').click()
    await until(async () => !(await details()).hasSelection, 40)
  }
  const rectForPerson = async ({ x, y, width, height }) => {
    await kit.getByTestId('photo-menu-View').click()
    await kit.getByTestId('photo-control-view.fitOnScreen').click()
    await page.waitForTimeout(200)
    await choose('RectMarquee')
    await kit.getByTestId('photo-option-selectionMode-replace').click()
    const canvas = await kit.locator('.kui-canvas-overlay').boundingBox()
    const image = await details()
    const zoom = Number(await kit.getByTestId('photo-status-zoom').locator('input').inputValue()) / 100
    const left = canvas.x + canvas.width / 2 - image.width * zoom / 2
    const top = canvas.y + canvas.height / 2 - image.height * zoom / 2
    await page.mouse.move(left + x * zoom, top + y * zoom)
    await page.mouse.down()
    await page.mouse.move(left + (x + width) * zoom, top + (y + height) * zoom, { steps: 5 })
    await page.mouse.up()
    await until(async () => (await details()).hasSelection, 40)
  }
  // A stroke on a new layer: its height (the brush size plus the drag's drift) and the alpha at its middle.
  const strokeOnNewLayer = async (fx, fy) => {
    await createForPerson('layer.new.layer', {})
    const before = (await inspect()).revision
    await stroke(fx, fy)
    await until(async () => (await inspect()).revision > before && ((await activeBounds())?.[3] ?? 0) > 0, 40)
    const [x, y, w, h] = (await activeBounds()) ?? [0, 0, 0, 0]
    const middle = { x: Math.floor(x + w / 2), y: Math.floor(y + h / 2) }
    const rgba = await sampleCurrent(middle)
    return { h, alpha: Math.round(rgba[3] * 100) / 100, middle, layer: (await details()).activeLayer }
  }
  // Painting stays inside a selection: start from none.
  await deselectForPerson()
  await choose('Brush')
  await setNumber('size', 6)
  await setNumber('opacity', 100)
  const thin = await strokeOnNewLayer(0.35, 0.55)
  await setNumber('size', 40)
  await setNumber('opacity', 50)
  const thick = await strokeOnNewLayer(0.35, 0.7)
  // A mouse presses fully: Pressure for Size (on) leaves a mouse stroke at the bar's size.
  step("the bar's brush size and opacity change the next stroke", thick.h >= thin.h + 30 && thin.alpha > 0.9 && thick.alpha > 0.35 && thick.alpha < 0.65, JSON.stringify({ thin, thick }))
  await setNumber('size', 20)
  await setNumber('opacity', 100)
  // The brush's preview stays on the document: a drag past the canvas edge shows no paint there.
  const overlayAlpha = async (point) => {
    const box = await kit.locator('.kui-canvas-overlay').boundingBox()
    return kit.locator('.kui-canvas-overlay').evaluate((canvas, [fx, fy]) => {
      // The most opaque pixel near the point: a zoomed-out brush is a thin line.
      const x = Math.round(fx * canvas.width)
      const y = Math.round(fy * canvas.height)
      const { data } = canvas.getContext('2d').getImageData(x - 4, y - 4, 9, 9)
      return Math.max(...data.filter((_, index) => index % 4 === 3))
    }, [(point.x - box.x) / box.width, (point.y - box.y) / box.height])
  }
  {
    // Zoomed out, the canvas's corner is past the document's edge.
    const zoom = kit.getByTestId('photo-status-zoom').locator('input')
    await zoom.fill('10')
    await zoom.press('Enter')
    await until(async () => (await zoom.getAttribute('aria-valuenow')) === '10')
    const box = await kit.locator('.kui-canvas-overlay').boundingBox()
    const inside = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
    const outside = { x: box.x + 12, y: box.y + 12 }
    await page.mouse.move(inside.x, inside.y)
    await page.mouse.down()
    await page.mouse.move(outside.x, outside.y, { steps: 8 })
    const shown = { inside: await overlayAlpha(inside), outside: await overlayAlpha(outside) }
    const before = (await inspect()).revision
    await page.mouse.up()
    await until(async () => (await inspect()).revision > before, 40)
    await undo()
    await kit.getByTestId('photo-menu-View').click()
    await kit.getByTestId('photo-control-view.fitOnScreen').click()
    await until(async () => (await zoom.getAttribute('aria-valuenow')) !== '10')
    step('the brush preview stays inside the document', shown.inside > 0 && shown.outside === 0, JSON.stringify(shown))
  }

  // The editor's colours are the engine's: the Eraser on the Background paints the background colour.
  await kit.getByTestId('photo-foreground').fill('#ff0000')
  await choose('Eraser')
  await page.keyboard.press('x')
  const backgroundLayer = (await details()).layers.find((layer) => layer.name === 'Background')
  await selectLayer(backgroundLayer.id)
  const erasedBefore = (await inspect()).revision
  await stroke(0.35, 0.55)
  await until(async () => (await inspect()).revision > erasedBefore, 40)
  const erased = (await sampleCurrent(thin.middle)).map((channel) => Math.round(channel * 255))
  step('the Eraser on the Background paints the editor’s background colour', erased[0] === 255 && erased[1] === 0 && erased[2] === 0, JSON.stringify(erased))
  await choose('Eraser')
  await page.keyboard.press('d')
  await selectLayer(thick.layer)

  // Magic Wand: Tolerance 0 selects the thin stroke's black; 255 everything (all layers sampled).
  await choose('MagicWand')
  await kit.getByTestId('photo-option-sampleAllLayers').check()
  const area = async () => {
    const [, , w, h] = (await details()).selectionBounds ?? [0, 0, 0, 0]
    return w * h
  }
  const wandAt = async (tolerance) => {
    await setNumber('tolerance', tolerance)
    const before = (await inspect()).revision
    const mid = at(0.35, 0.55)
    await page.mouse.click(mid.x + 45, mid.y + 10)
    await until(async () => (await inspect()).revision > before, 40)
    return area()
  }
  const tight = await wandAt(0)
  const loose = await wandAt(255)
  step('Magic Wand tolerance changes the selected area', tight > 0 && loose > tight * 4, `${tight} -> ${loose}`)
  await kit.getByTestId('photo-option-sampleAllLayers').uncheck()
  await setNumber('tolerance', 32)

  // Marquee: the bar's New, Add, and Subtract buttons combine the selections without modifier keys.
  // From no selection: a drag inside one moves its outline (plan 2026-11-24, stage 2).
  await deselectForPerson()
  await choose('RectMarquee')
  const marquee = async (fx, fy, dx, dy) => {
    const before = (await inspect()).revision
    const from = at(fx, fy)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(from.x + dx, from.y + dy, { steps: 5 })
    await page.mouse.up()
    await until(async () => (await inspect()).revision > before, 40)
    return JSON.stringify((await details()).selectionBounds)
  }
  await kit.getByTestId('photo-option-selectionMode-replace').click()
  const first = await marquee(0.15, 0.15, 60, 40)
  await kit.getByTestId('photo-option-selectionMode-add').click()
  const both = await marquee(0.5, 0.45, 60, 40)
  await kit.getByTestId('photo-option-selectionMode-subtract').click()
  const back = await marquee(0.48, 0.43, 100, 80)
  await kit.getByTestId('photo-option-selectionMode-replace').click()
  step('selection mode buttons add and subtract', first !== both && JSON.parse(both)[2] > JSON.parse(first)[2] && back === first, `${first} + -> ${both} - -> ${back}`)

  // Move: the bar's align buttons run Layer › Align on the active layer (to the selection).
  await choose('Move')
  const unaligned = (await activeBounds())?.[0]
  const beforeAlign = (await inspect()).revision
  await kit.getByTestId('photo-option-layer.align.leftEdges').click()
  await until(async () => (await inspect()).revision > beforeAlign, 40)
  const aligned = (await activeBounds())?.[0]
  step("Move's align buttons move the layer", aligned !== unaligned && aligned === JSON.parse(first)[0], `${unaligned} -> ${aligned}`)

  // Crop: a 1 : 1 ratio holds the dragged box square.
  await choose('Crop')
  await kit.getByTestId('photo-option-ratio').selectOption('1:1')
  const size = await details()
  const box = at(0.4, 0.4)
  await page.mouse.move(box.x, box.y)
  await page.mouse.down()
  await page.mouse.move(box.x + 100, box.y + 40, { steps: 5 })
  await page.mouse.up()
  await kit.getByTestId('photo-crop-apply').click()
  await until(async () => (await details()).width !== size.width)
  const cropped = await details()
  step('the crop ratio holds the crop square', cropped.width === cropped.height && cropped.width < size.width, `${size.width}x${size.height} -> ${cropped.width}x${cropped.height}`)
  await undo()
  await until(async () => (await details()).width === size.width)
  await kit.getByTestId('photo-option-ratio').selectOption('')

  // Type: with a type layer selected, the bar shows and changes that layer (type.setStyle).
  await choose('Type')
  await selectLayer(textLayer.id)
  // The bar shows the layer's size once its style is read (a tenth of a point).
  await until(async () => (await option('fontSize').locator('input').inputValue()) === '48.0', 40)
  const shownSize = await option('fontSize').locator('input').inputValue()
  await setNumber('fontSize', 30)
  const resized = await until(async () => {
    const info = await typeInfo(textLayer.id)
    return Math.abs(info.runs[0].style.size_pt - 30) < 0.01
  }, 40)
  step('the Type bar shows and changes the selected type layer', shownSize === '48.0' && resized, `shown ${shownSize}`)

  // A drag through the text colour picker (several input events) is one history step.
  const historyLength = async () => (await details()).history.length
  const coloursBefore = await historyLength()
  await option('textColor')
    .locator('input')
    .evaluate((input) => {
      const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      for (const value of ['#ff0000', '#00ff00', '#0000ff']) {
        setValue.call(input, value)
        input.dispatchEvent(new Event('input', { bubbles: true }))
      }
      input.dispatchEvent(new Event('change', { bubbles: true }))
    })
  const recoloured = await until(async () => (await typeInfo(textLayer.id)).runs[0].style.color.c.slice(0, 3).join() === '0,0,1', 40)
  await page.waitForTimeout(300)
  step('a drag through the text colour picker is one history step', recoloured && (await historyLength()) === coloursBefore + 1, `${coloursBefore} -> ${await historyLength()}`)

  // While new text is typed, the bar's changes are for it, not for the selected type layer.
  const typedAt = at(0.15, 0.85)
  await page.mouse.click(typedAt.x, typedAt.y)
  await kit.getByTestId('photo-text-field').waitFor()
  await kit.getByTestId('photo-text-field').fill('AB')
  await setNumber('fontSize', 24)
  const stillTyping = (await count('photo-text-field')) === 1
  await kit.getByTestId('photo-option-commit').click()
  await until(async () => (await details()).layers.some((layer) => layer.name === 'AB'))
  const typedLayer = (await details()).layers.find((layer) => layer.name === 'AB')
  const oldSize = (await typeInfo(textLayer.id)).runs[0].style.size_pt
  const newSize = typedLayer ? (await typeInfo(typedLayer.id)).runs[0].style.size_pt : null
  step('while text is typed the Type bar changes it, not the selected type layer', stillTyping && oldSize === 30 && newSize === 24, JSON.stringify({ stillTyping, oldSize, newSize }))

  await kit.getByTestId('photo-tool-Brush').click()
  await page.keyboard.press('x')
  await until(async () => (await kit.getByTestId('photo-foreground').inputValue()) === '#ffffff')
  step('X swaps the colors', (await kit.getByTestId('photo-foreground').inputValue()) === '#ffffff')
  const layers = (await details()).layers.length
  await kit.getByTestId('photo-menu-Layer').click()
  await kit.getByTestId('photo-cmd-layer.duplicate').click()
  // "Duplicate Layer…" asks first (plan 2026-11-24, stage 1): OK duplicates with the dialog's values.
  await okDialog()
  await until(async () => (await details()).layers.length === layers + 1)
  step('Layer > Duplicate from the menu bar', (await details()).layers.length === layers + 1)

  // The dock's Layers panel (upstream's): blend mode, Opacity, locks, rename, reorder, masks, eyes.
  const tab = (id) => kit.getByTestId(`photo-dock-area-tab-${id}`)
  const topLayer = async (index) => (await details()).layers[index]
  const panelLayer = await topLayer(0)
  await selectLayer(panelLayer.id)
  await kit.getByTestId('photo-layers-blend').selectOption('Multiply')
  const opacityInput = kit.locator('[data-kit-ui-id="control:layers.opacity"] input')
  await opacityInput.fill('40')
  await opacityInput.press('Enter')
  const blended = await until(async () => {
    const layer = findLayer((await details()).layers, panelLayer.id)
    return layer.blend === 'Multiply' && Math.abs(layer.opacity - 0.4) < 0.01
  }, 40)
  step('the Layers panel sets the blend mode and Opacity', blended, JSON.stringify(findLayer((await details()).layers, panelLayer.id), ['blend', 'opacity']))
  await kit.getByTestId('photo-layers-blend').selectOption('Normal')
  await opacityInput.fill('100')
  await opacityInput.press('Enter')
  await until(async () => findLayer((await details()).layers, panelLayer.id).opacity === 1, 40)

  // A painted layer whose position is locked refuses the Move tool.
  await selectLayer(thick.layer)
  await kit.getByTestId('photo-layers-lock-position').click()
  const locked = await until(async () => findLayer((await details()).layers, thick.layer).locks.position === true, 40)
  await choose('Move')
  // Auto-Select would pick the layer under the pointer: the drag is for the locked one.
  await kit.getByTestId('photo-option-autoSelect').uncheck()
  const lockedFrom = JSON.stringify(await activeBounds())
  const lockedRevision = (await inspect()).revision
  const m = at(0.35, 0.7)
  await page.mouse.move(m.x, m.y)
  await page.mouse.down()
  await page.mouse.move(m.x + 80, m.y + 30, { steps: 6 })
  await page.mouse.up()
  await page.waitForTimeout(400)
  step('a lock from the Layers panel refuses a move', locked && JSON.stringify(await activeBounds()) === lockedFrom && (await inspect()).revision === lockedRevision, `${lockedFrom} -> ${JSON.stringify(await activeBounds())}`)
  await kit.getByTestId('photo-option-autoSelect').check()
  await kit.getByTestId('photo-layers-lock-position').click()
  await until(async () => findLayer((await details()).layers, thick.layer).locks.position === false, 40)

  await kit.getByTestId(`photo-layer-${thick.layer}`).dblclick()
  await kit.getByTestId('photo-layer-rename').fill('笔触')
  await kit.getByTestId('photo-layer-rename').press('Enter')
  await until(async () => findLayer((await details()).layers, thick.layer).name === '笔触', 40)
  step('double-clicking a layer name renames it', findLayer((await details()).layers, thick.layer).name === '笔触')

  // Drag the top row onto the second row's lower half: the two swap (the whole row: its name is the upper of two lines).
  const [upper, lower] = (await details()).layers
  const rowBox = async (id) => kit.locator(`[data-layer-row="${id}"]`).boundingBox()
  const from = await rowBox(upper.id)
  const onto = await rowBox(lower.id)
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
  await page.mouse.down()
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2 + 10, { steps: 3 })
  await page.mouse.move(onto.x + onto.width / 2, onto.y + onto.height * 0.8, { steps: 8 })
  await page.mouse.up()
  const swapped = await until(async () => {
    const now = (await details()).layers
    return now[0].id === lower.id && now[1].id === upper.id
  }, 40)
  step('dragging a layer row reorders the layers', swapped, (await details()).layers.slice(0, 3).map((layer) => layer.name).join(', '))

  // The footer's mask button gives the active layer a mask; its thumbnail and the layer's draw.
  await selectLayer(thick.layer)
  await kit.getByTestId('photo-layers-mask').click()
  await until(async () => findLayer((await details()).layers, thick.layer).hasMask === true, 40)
  const drawn = (testId) => kit.locator(`[data-testid="${testId}"] .kui-thumbnail`).getAttribute('data-drawn')
  const thumbsDrawn = await until(async () => (await drawn(`photo-layer-mask-${thick.layer}`)) === 'true' && (await drawn(`photo-layer-thumb-${thick.layer}`)) === 'true', 40)
  step('the footer adds a mask, and the layer and mask thumbnails draw', findLayer((await details()).layers, thick.layer).hasMask === true && thumbsDrawn)

  // With the mask's thumbnail targeted, the Brush paints the mask, not the layer's pixels.
  await kit.getByTestId(`photo-layer-mask-${thick.layer}`).click()
  await choose('Brush')
  const pixelsBefore = JSON.stringify(await activeBounds())
  const maskedBefore = (await inspect()).revision
  await stroke(0.6, 0.62)
  await until(async () => (await inspect()).revision > maskedBefore, 40)
  step('the mask thumbnail targets the mask for painting', (await inspect()).revision > maskedBefore && JSON.stringify(await activeBounds()) === pixelsBefore, `${pixelsBefore} -> ${JSON.stringify(await activeBounds())}`)

  // With the mask targeted, Image › Adjustments › Invert from its key (Cmd+I) inverts the mask, not the pixels.
  const frameNow = () => page.frames().find((candidate) => candidate.url().includes('index.html'))
  /** The sum of a thumbnail's pixels, as drawn: it changes when the thumbnail does. */
  const thumbSum = (testId) =>
    frameNow().evaluate((id) => {
      const canvas = document.querySelector(`[data-testid="${id}"] canvas`)
      if (!canvas) return -1
      const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
      let sum = 0
      for (let index = 0; index < data.length; index += 1) sum += data[index]
      return sum
    }, testId)
  await kit.getByTestId(`photo-layer-mask-${thick.layer}`).click()
  const maskSum = await thumbSum(`photo-layer-mask-${thick.layer}`)
  const pixelColour = JSON.stringify(await sampleCurrent(thick.middle))
  const invertFrom = (await inspect()).revision
  await kit.getByTestId('photo-tool-Brush').click()
  await page.keyboard.press(`${await primaryKey()}+KeyI`)
  await until(async () => (await inspect()).revision > invertFrom, 40)
  const maskInverted = await until(async () => (await thumbSum(`photo-layer-mask-${thick.layer}`)) !== maskSum, 40)
  step('Cmd+I with the mask targeted inverts only the mask', maskInverted && JSON.stringify(await sampleCurrent(thick.middle)) === pixelColour, `${pixelColour} -> ${JSON.stringify(await sampleCurrent(thick.middle))}`)

  // In Quick Mask mode the mask target gives way: a stroke paints the Quick Mask, not the layer mask.
  await tab('channels').click()
  await rectForPerson({ x: 40, y: 40, width: 300, height: 200 })
  await kit.getByTestId('photo-quick-mask').click()
  await until(async () => (await count('photo-channel-thumb-quickMask')) === 1, 40)
  await until(async () => (await thumbSum('photo-channel-thumb-quickMask')) > 0, 40)
  const quickBefore = await thumbSum('photo-channel-thumb-quickMask')
  const layerMaskBefore = await thumbSum('photo-channel-thumb-mask')
  await choose('Brush')
  await setNumber('size', 80)
  // Mid gray differs from masked and unmasked alike, so the stroke shows wherever it lands.
  await kit.getByTestId('photo-foreground').fill('#808080')
  await stroke(0.25, 0.25)
  const quickPainted = await until(async () => (await thumbSum('photo-channel-thumb-quickMask')) !== quickBefore, 40)
  step('with Quick Mask on and the mask targeted, a stroke paints the Quick Mask', quickPainted && (await thumbSum('photo-channel-thumb-mask')) === layerMaskBefore, `quick ${quickBefore} -> ${await thumbSum('photo-channel-thumb-quickMask')}, mask ${layerMaskBefore} -> ${await thumbSum('photo-channel-thumb-mask')}`)
  await kit.getByTestId('photo-quick-mask').click()
  await deselectForPerson()
  await setNumber('size', 20)
  // Back to the default colors (D), black over white.
  await kit.getByTestId('photo-tool-Brush').click()
  await page.keyboard.press('d')
  await tab('layers').click()
  await kit.getByTestId(`photo-layer-thumb-${thick.layer}`).click()

  // The keyboard: Down selects the next row, Alt+Up moves the active layer up the stack.
  await kit.getByTestId(`photo-layer-${thick.layer}`).focus()
  const order = (await details()).layers.map((layer) => layer.id)
  await page.keyboard.press('ArrowDown')
  const below = order[order.indexOf(thick.layer) + 1]
  await until(async () => (await details()).activeLayer === below, 40)
  const selectedByKey = (await details()).activeLayer === below
  await page.keyboard.press('Alt+ArrowUp')
  const raised = await until(async () => {
    const now = (await details()).layers.map((layer) => layer.id)
    return now.indexOf(below) === order.indexOf(below) - 1
  }, 40)
  step('Down selects the next row and Alt+Up raises it', selectedByKey && raised, `${(await details()).layers.slice(0, 4).map((layer) => layer.name).join(', ')}`)
  await undo()
  await selectLayer(thick.layer)

  await kit.getByTestId(`photo-layer-visibility-${thick.layer}`).click()
  await until(async () => findLayer((await details()).layers, thick.layer).visible === false, 40)
  step("a row's eye hides its layer", findLayer((await details()).layers, thick.layer).visible === false)
  await kit.getByTestId(`photo-layer-visibility-${thick.layer}`).click()
  await until(async () => findLayer((await details()).layers, thick.layer).visible === true, 40)

  // A layer style lists its effects under the row; the Effects row's eye turns them all off.
  await runAgent('layer.layerStyle.dropShadow', { add: true })
  await until(async () => (await count(`photo-effect-visibility-${thick.layer}-all`)) === 1, 40)
  await kit.getByTestId(`photo-effect-visibility-${thick.layer}-all`).click()
  await until(async () => findLayer((await details()).layers, thick.layer).effects?.enabled === false, 40)
  step("an effect row's eye turns the layer's effects off", findLayer((await details()).layers, thick.layer).effects?.enabled === false && (await count(`photo-effect-visibility-${thick.layer}-0`)) === 1)
  await kit.getByTestId(`photo-layer-fx-${thick.layer}`).click()
  await until(async () => (await count(`photo-effect-visibility-${thick.layer}-all`)) === 0, 40)
  step('the fx mark folds the effects rows', (await count(`photo-effect-visibility-${thick.layer}-all`)) === 0)

  // The kind filter keeps the type layers; the context menu offers the layer commands and Rename.
  await kit.getByTestId('photo-layers-filter-type').click()
  const typeRows = await kit.locator('[data-layer-row]').count()
  const typeLayers = (await details()).layers.filter((layer) => layer.kind === 'Type').length
  await kit.getByTestId('photo-layers-filter-type').click()
  step('the kind filter shows only type layers', typeRows === typeLayers && typeLayers > 0, `${typeRows} rows, ${typeLayers} type layers`)
  await kit.getByTestId(`photo-layer-${thick.layer}`).click({ button: 'right' })
  const offered = (await count('photo-layers-menu-layer.mergeDown')) === 1 && (await count('photo-layers-menu-layer.quickExportAsPng')) === 0
  await kit.getByTestId('photo-layers-menu-rename').click()
  const renameShown = (await count('photo-layer-rename')) === 1
  await kit.getByTestId('photo-layer-rename').press('Escape')
  step('the row menu offers the layer commands the web runs, and Rename', offered && renameShown && (await count('photo-layers-menu')) === 0)

  // Channels: a color channel's eye, and the selection saved as an alpha channel.
  await tab('channels').click()
  await kit.getByTestId('photo-channel-visibility-color-0').click()
  await until(async () => (await details()).channels.colors[0].visible === false, 40)
  step("a color channel's eye hides it", (await details()).channels.colors[0].visible === false)
  await kit.getByTestId('photo-channel-visibility-color-0').click()
  await until(async () => (await details()).channels.colors[0].visible === true, 40)
  await rectForPerson({ x: 40, y: 40, width: 200, height: 120 })
  const alphas = (await details()).channels.alpha.length
  await kit.getByTestId('photo-channels-save').click()
  await until(async () => (await details()).channels.alpha.length === alphas + 1, 40)
  step('Save selection as channel adds an alpha channel row', (await details()).channels.alpha.length === alphas + 1 && (await count(`photo-channel-alpha-${alphas}`)) === 1)

  // Paths: a work path made from the selection.
  await runAgent('path.delete', { name: 'work' }).catch(() => null)
  await tab('paths').click()
  await kit.getByTestId('photo-paths-work').click()
  await until(async () => (await count('photo-path-work')) === 1, 40)
  step('Make work path from selection lists the work path', (await count('photo-path-work')) === 1 && ((await workPath())?.subpaths?.length ?? 0) > 0)
  await tab('layers').click()

  // Adjustments: a Levels layer from the grid, its settings then in Properties (upstream's tab switch).
  await tab('adjustments').click()
  const adjustedFrom = (await details()).layers.length
  await kit.getByTestId('photo-adjustments-levels').click()
  await until(async () => (await details()).layers.length === adjustedFrom + 1, 40)
  const afterAdd = await details()
  const levels = findLayer(afterAdd.layers, afterAdd.activeLayer)
  await until(async () => (await count('photo-props-adjustment')) === 1, 40)
  step('Adjustments adds a Levels adjustment layer and Properties shows its settings', levels?.kind === 'Adjustment' && levels?.adjustmentKind === 'levels' && (await count('photo-props-adjustment')) === 1, `${levels?.kind} ${levels?.adjustmentKind}`)
  // The history holds a limited number of states, so one new step shows as the last state before it moving up one.
  const levelsStep = (await details()).history.at(-1)
  const inBlack = kit.getByTestId('photo-adjustment-inBlack').locator('input')
  await inBlack.fill('30')
  await inBlack.press('Enter')
  await until(async () => findLayer((await details()).layers, levels.id)?.adjustmentParams?.inBlack === 30, 40)
  const levelsHistory = (await details()).history
  step("Properties sets an adjustment's value in one history step", findLayer((await details()).layers, levels.id)?.adjustmentParams?.inBlack === 30 && levelsHistory.at(-2) === levelsStep && levelsHistory.at(-1) === 'Modify Adjustment', `${JSON.stringify(findLayer((await details()).layers, levels.id)?.adjustmentParams?.inBlack)}, ${levelsHistory.slice(-3).join(' | ')}`)
  await runAgent('layer.delete', { layer: levels.id })

  // Color Lookup's Load picks a look file through the host (the dev host answers with a workspace file).
  const cube = 'TITLE "Tiny"\nLUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n'
  await page.evaluate((text) => window.devHost.files.set('looks/tiny.cube', { bytes: new TextEncoder().encode(text).buffer, version: 1 }), cube)
  await tab('adjustments').click()
  await kit.getByTestId('photo-adjustments-colorLookup').click()
  await kit.getByTestId('photo-adjustment-lutFile').click()
  const lookupName = async () => {
    const now = await details()
    return findLayer(now.layers, now.activeLayer)?.adjustment?.ColorLookup?.name ?? null
  }
  const loaded = await until(async () => (await lookupName()) === 'tiny.cube', 40)
  step("Color Lookup loads a look file the host picks", loaded && (await kit.getByTestId('photo-adjustment-lut').inputValue()) === 'custom', String(await lookupName()))
  await runAgent('layer.delete', {})

  // Properties, in upstream's order: a pixel layer's X, Y, W, and H (read only), its Opacity slider; a type layer's Character.
  await selectLayer(thick.layer)
  await tab('properties').click()
  const propsBox = await activeBounds()
  await until(async () => (await kit.getByTestId('photo-props-bounds-x').textContent()) === `${propsBox[0]} px`, 40)
  const shownBounds = await Promise.all(['x', 'y', 'w', 'h'].map((name) => kit.getByTestId(`photo-props-bounds-${name}`).textContent()))
  const boundsInputs = await kit.getByTestId('photo-props-bounds').locator('input').count()
  step("Properties shows a layer's X, Y, W, and H, read only", JSON.stringify(shownBounds) === JSON.stringify(propsBox.map((value) => `${value} px`)) && boundsInputs === 0, `${JSON.stringify(shownBounds)} for ${JSON.stringify(propsBox)}`)
  const opacityBefore = (await details()).history.length
  const opacityStep = (await details()).history.at(-1)
  await kit.getByTestId('photo-props-opacity-track').scrollIntoViewIfNeeded()
  const opacityTrack = await kit.getByTestId('photo-props-opacity-track').boundingBox()
  await page.mouse.move(opacityTrack.x + opacityTrack.width * 0.9, opacityTrack.y + opacityTrack.height / 2)
  await page.mouse.down()
  await page.mouse.move(opacityTrack.x + opacityTrack.width * 0.6, opacityTrack.y + opacityTrack.height / 2, { steps: 4 })
  await page.mouse.move(opacityTrack.x + opacityTrack.width * 0.5, opacityTrack.y + opacityTrack.height / 2, { steps: 2 })
  await page.mouse.up()
  const thickOpacity = async () => findLayer((await details()).layers, thick.layer)?.opacity
  await until(async () => Math.abs((await thickOpacity()) - 0.5) < 0.02, 40)
  await page.waitForTimeout(200)
  const opacityHistory = (await details()).history
  // One step for the drag: the history grew by one, or (when full) its last state before moved up one.
  const oneStep = opacityHistory.length === opacityBefore + 1 || opacityHistory.at(-2) === opacityStep
  step("Properties' Opacity slider sets the layer's opacity in one history step", Math.abs((await thickOpacity()) - 0.5) < 0.02 && oneStep, `${await thickOpacity()}, ${opacityHistory.slice(-3).join(' | ')}`)
  await runAgent('layer.setProps', { layer: thick.layer, opacity: 1 })
  const typeLayer = (await details()).layers.find((layer) => layer.kind === 'Type')
  await selectLayer(typeLayer.id)
  const sizeField = kit.getByTestId('photo-props-type-size').locator('input')
  await sizeField.waitFor()
  await sizeField.fill('30')
  await sizeField.press('Enter')
  const sized = await until(async () => Math.abs((await typeInfo(typeLayer.id)).runs[0].style.size_pt - 30) < 0.05, 40)
  step("Properties sets a type layer's size", sized, String((await typeInfo(typeLayer.id)).runs[0].style.size_pt))
  // Upstream's type screenshot: with the Color group hidden, Character shows under X, Y, W, H, Opacity, and Fill, unscrolled.
  const colorShown = (await count('photo-dock-area-tab-color')) === 1
  const toggleColor = async () => {
    await kit.getByTestId('photo-menu-Window').click()
    await kit.getByTestId('photo-control-window.panel.color').click()
  }
  if (colorShown) await toggleColor()
  await until(async () => (await count('photo-dock-area-tab-color')) === 0)
  // As the panel opens for a newly chosen layer: at its top (the Character size field above scrolled it).
  await kit.getByTestId('photo-properties').evaluate((element) => {
    element.scrollTop = 0
  })
  // Hiding a dock group triggers layout/ResizeObserver updates. Read one coherent frame and
  // wait for the required layout, rather than mixing boxes from separate browser round trips.
  const propertyLayouts = []
  const characterVisible = await until(async () => {
    const layout = await kit.getByTestId('photo-properties').evaluate((element) => {
      const panelBox = element.getBoundingClientRect().toJSON()
      const characterHead = element.querySelector('[data-testid="photo-props-type-character-head"]').getBoundingClientRect().toJSON()
      const sectionTops = ['photo-props-bounds', 'photo-props-opacity', 'photo-props-fill', 'photo-props-type-character'].map((id) => element.querySelector(`[data-testid="${id}"]`)?.getBoundingClientRect().top ?? -1)
      return { panelBox, characterHead, sectionTops, scrolled: element.scrollTop }
    })
    propertyLayouts.push(layout)
    const { panelBox, characterHead, sectionTops, scrolled } = layout
    const inOrder = sectionTops.every((top, index) => top >= 0 && (index === 0 || top > sectionTops[index - 1]))
    return scrolled === 0 && characterHead.y + characterHead.height <= panelBox.y + panelBox.height && inOrder
  })
  step("a type layer's Properties show Character without scrolling, after X, Y, W, H, Opacity, and Fill", characterVisible, JSON.stringify({ first: propertyLayouts[0], last: propertyLayouts.at(-1), observations: propertyLayouts.length }))
  if (colorShown) await toggleColor()
  await tab('layers').click()

  // History: a click on a state undoes to it, a click on a later (dimmed) state redoes to it.
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.history').click()
  await until(async () => (await count('photo-history')) === 1)
  const stepNow = (await details()).historyIndex
  const changesBefore = await page.evaluate(() => window.devHost.changes.length)
  await kit.getByTestId(`photo-history-state-${stepNow - 2}`).click()
  const wentBack = await until(async () => (await details()).historyIndex === stepNow - 2, 40)
  await page.waitForTimeout(300)
  // One command (history.goTo) for the two steps: the host hears of one change.
  const oneChange = (await page.evaluate(() => window.devHost.changes.length)) - changesBefore === 1
  const redoRows = (await kit.locator('[data-testid^="photo-history-state-"][data-redo="true"]').count()) === 2
  await kit.getByTestId(`photo-history-state-${stepNow - 1}`).click()
  const wentForward = await until(async () => (await details()).historyIndex === stepNow - 1, 40)
  const current = (await kit.getByTestId(`photo-history-state-${stepNow - 1}`).getAttribute('aria-current')) === 'step'
  step('History goes back two states and forward one, each in one command', wentBack && redoRows && wentForward && current && oneChange, `index ${stepNow} -> ${(await details()).historyIndex}, one change: ${oneChange}`)
  await kit.getByTestId(`photo-history-state-${stepNow}`).click()
  await until(async () => (await details()).historyIndex === stepNow, 40)
  step('History leaves out New Document from State, which would open a second document', (await count('photo-history-newDocument')) === 0 && (await count('photo-history-delete')) === 1)
  // Delete current state asks first: Cancel keeps it, Delete drops it (and it can't be redone).
  const statesBefore = (await details()).history.length
  await kit.getByTestId('photo-history-delete').click()
  const asked = (await count('photo-history-delete-question')) === 1
  await kit.getByTestId('photo-history-delete-cancel').click()
  const keptState = (await details()).history.length === statesBefore
  await kit.getByTestId('photo-history-delete').click()
  await kit.getByTestId('photo-history-delete-confirm').click()
  const deleted = await until(async () => (await details()).history.length === statesBefore - 1 && (await details()).historyRedo.length === 0, 40)
  step('Delete current state asks, then deletes it', asked && keptState && deleted && (await count('photo-history-delete-question')) === 0, `${statesBefore} -> ${(await details()).history.length}`)
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.history').click()
  await until(async () => (await count('photo-history')) === 0)

  // Color, Swatches, Navigator, Histogram, Info, Character, Paragraph (upstream's panels).
  const kitFrame = () => page.frames().find((candidate) => candidate.url().includes('index.html'))
  const idProblems = []
  const checkIds = async () => {
    const elements = await page.evaluate(() => window.devHost.connection().elements())
    idProblems.push(...(await checkUiIds(kitFrame(), elements)))
  }
  const engineColors = async () => {
    const answer = (await runAgent('session.inspect', {})).completed[0].result
    const hex = (rgba) => `#${rgba.slice(0, 3).map((value) => Math.round(value * 255).toString(16).padStart(2, '0')).join('')}`
    return { foreground: hex(answer.foreground), background: hex(answer.background) }
  }
  const railColor = (which) => kit.getByTestId(`photo-${which}`).inputValue()
  await tab('color').click()
  await kit.getByTestId('photo-color-panel').waitFor()
  await checkIds()
  const historyBeforeColors = (await details()).history.length
  const hexField = kit.getByTestId('photo-color-picker-hex')
  await hexField.fill('3366cc')
  await hexField.press('Enter')
  const typedColor = await until(async () => (await railColor('foreground')) === '#3366cc' && (await engineColors()).foreground === '#3366cc', 40)
  const square = kit.getByTestId('photo-color-picker-square')
  const squareBox = await square.boundingBox()
  await square.click({ position: { x: squareBox.width - 3, y: squareBox.height / 2 } })
  const pickedColor = await until(async () => {
    const now = await engineColors()
    return now.foreground !== '#3366cc' && now.foreground === (await railColor('foreground'))
  }, 40)
  step(
    'picking a color in the Color panel sets the engine’s foreground, in no history step',
    typedColor && pickedColor && (await details()).history.length === historyBeforeColors,
    `${await railColor('foreground')} / ${JSON.stringify(await engineColors())}, history ${historyBeforeColors} -> ${(await details()).history.length}`,
  )
  // The background chip shows below and right of the foreground's, as upstream draws them.
  await kit.getByTestId('photo-color-chip-background').click({ position: { x: 18, y: 18 } })
  await hexField.fill('00ff00')
  await hexField.press('Enter')
  const backgroundSet = await until(async () => (await engineColors()).background === '#00ff00' && (await railColor('background')) === '#00ff00', 40)
  await kit.getByTestId('photo-color-chip-foreground').click()
  step("the Color panel's background chip makes the field edit the background", backgroundSet, JSON.stringify(await engineColors()))

  await tab('swatches').click()
  await kit.getByTestId('photo-swatch-0').waitFor()
  await checkIds()
  const swatchColor = (index) => kit.getByTestId(`photo-swatch-${index}`).getAttribute('data-color')
  await kit.getByTestId('photo-swatch-3').click()
  const swatchForeground = await until(async () => (await engineColors()).foreground === (await swatchColor(3)) && (await railColor('foreground')) === (await swatchColor(3)), 40)
  await kit.getByTestId('photo-swatch-5').click({ button: 'right' })
  const swatchBackground = await until(async () => (await engineColors()).background === (await swatchColor(5)), 40)
  step('clicking a swatch sets the foreground, right-clicking the background', swatchForeground && swatchBackground, `${await swatchColor(3)} ${await swatchColor(5)} -> ${JSON.stringify(await engineColors())}`)
  const swatchCount = () => kit.locator('[data-testid^="photo-swatch-"][data-color]').count()
  const swatchesBefore = await swatchCount()
  await kit.getByTestId('photo-swatches-new').click()
  const added = await until(async () => (await swatchCount()) === swatchesBefore + 1, 40)
  await kit.getByTestId(`photo-swatch-${swatchesBefore}`).click()
  await kit.getByTestId('photo-swatches-delete').click()
  const removed = await until(async () => (await swatchCount()) === swatchesBefore, 40)
  step('the Swatches footer adds the foreground as a swatch and deletes the one clicked', added && removed, `${swatchesBefore} swatches`)
  await tab('color').click()

  // Navigator: its own group, off in upstream's default workspace.
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.navigator').click()
  await kit.getByTestId('photo-navigator').waitFor()
  await until(async () => (await kit.getByTestId('photo-navigator-frame').getAttribute('data-drawn')) === 'true', 60)
  await checkIds()
  const thumbInk = await kit.getByTestId('photo-navigator-image').evaluate((canvas) => {
    const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
    let opaque = 0
    for (let i = 3; i < data.length; i += 4) if (data[i] > 0) opaque += 1
    return opaque
  })
  await kit.getByTestId('photo-navigator-zoom-200').click()
  await until(async () => (await kit.getByTestId('photo-navigator-zoom-200').getAttribute('aria-pressed')) === 'true', 40)
  // The group is at its compact height (upstream's 140): its content scrolls, the frame's top shows.
  await kit.getByTestId('photo-navigator-frame').evaluate((frame) => frame.scrollIntoView({ block: 'start' }))
  const navFrame = await kit.getByTestId('photo-navigator-frame').boundingBox()
  const viewBox = () => kit.getByTestId('photo-navigator-view').boundingBox()
  const viewBefore = await viewBox()
  const target = { x: navFrame.x + navFrame.width * 0.3, y: navFrame.y + 40 }
  await page.mouse.move(navFrame.x + navFrame.width / 2, navFrame.y + 60)
  await page.mouse.down()
  await page.mouse.move(target.x, target.y, { steps: 6 })
  await page.mouse.up()
  const centredOnPointer = await until(async () => {
    const box = await viewBox()
    return box && Math.abs(box.x + box.width / 2 - target.x) < 3 && Math.abs(box.y + box.height / 2 - target.y) < 3
  }, 40)
  step('Navigator draws the image, and a drag pans the canvas to the pointer', thumbInk > 0 && centredOnPointer && JSON.stringify(viewBefore) !== JSON.stringify(await viewBox()), `ink ${thumbInk}, ${JSON.stringify(viewBefore)} -> ${JSON.stringify(await viewBox())}`)
  await kit.getByTestId('photo-navigator-fit').click()

  await tab('histogram').click()
  await kit.getByTestId('photo-histogram-stats').waitFor()
  await checkIds()
  const histogramChannel = () => kit.getByTestId('photo-histogram').getAttribute('data-channel')
  const rgbShown = await until(async () => (await histogramChannel()) === 'rgb' && (await kit.getByTestId('photo-histogram').getAttribute('data-stale')) === 'false', 40)
  const rgbStats = await kit.getByTestId('photo-histogram-stats').textContent()
  await kit.getByTestId('photo-histogram-channel').selectOption('red')
  const redShown = await until(async () => (await histogramChannel()) === 'red' && (await kit.getByTestId('photo-histogram').getAttribute('data-stale')) === 'false', 40)
  const docSize = await details()
  step('Histogram shows for RGB and Red, with its statistics', rgbShown && redShown && (await count('photo-histogram-bins')) === 1 && rgbStats.includes(String(docSize.width * docSize.height)), rgbStats)

  await tab('info').click()
  await kit.getByTestId('photo-info').waitFor()
  await checkIds()
  const over = at(0.45, 0.4)
  await page.mouse.move(over.x, over.y)
  await page.mouse.move(over.x + 3, over.y + 2)
  const readout = (id) => kit.getByTestId(`photo-info-${id}`).textContent()
  const sampled = await until(async () => (await readout('r')) !== '—' && (await readout('x')) !== '—', 40)
  const px = Number(await readout('x'))
  const py = Number(await readout('y'))
  const truth = (await runAgent('document.sampleColor', { x: px + 0.5, y: py + 0.5 })).completed[0].result.slice(0, 3).map((value) => Math.round(value * 255))
  const shownRgb = [Number(await readout('r')), Number(await readout('g')), Number(await readout('b'))]
  step('Info shows the pixel under the pointer, its CMYK, and the document size', sampled && JSON.stringify(shownRgb) === JSON.stringify(truth) && (await readout('black')).endsWith('%') && (await readout('doc')).includes(String(docSize.width)), `${px},${py}: ${shownRgb} vs ${truth}`)
  await tab('navigator').click()
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.navigator').click()
  await until(async () => (await count('photo-navigator')) === 0)

  // Character and Paragraph: the active type layer, or the Type tool's options for new type.
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.character').click()
  await kit.getByTestId('photo-character').waitFor()
  // Four groups shown: Color, Properties, and Character give way (upstream's compact heights) so the Layers rows stay in reach.
  const groupHeight = async (group) => Number(await kit.getByTestId(`photo-dock-area-group-${group}-group`).getAttribute('data-height'))
  const heights = { color: await groupHeight('color'), properties: await groupHeight('properties'), character: await groupHeight('character'), layers: await groupHeight('layers') }
  step(
    "with four groups shown, the others give way to upstream's compact heights and Layers keeps room for its rows",
    heights.color === 130 && heights.properties === 160 && heights.character === 160 && heights.layers >= 280,
    JSON.stringify(heights),
  )
  const styledLayer = (await details()).layers.find((layer) => layer.kind === 'Type')
  await selectLayer(styledLayer.id)
  await until(async () => (await kit.getByTestId('photo-character').getAttribute('data-target')) === 'layer', 40)
  await checkIds()
  const typeField = (id) => kit.getByTestId(`photo-${id}`).locator('input')
  const setTypeField = async (id, value) => {
    await typeField(id).fill(String(value))
    await typeField(id).press('Enter')
  }
  const styleOf = async () => (await typeInfo(styledLayer.id)).runs[0].style
  const scaleOf = async () => {
    const m = (await typeInfo(styledLayer.id)).transform
    return Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2]))
  }
  const typeScale = await scaleOf()
  await setTypeField('character-size', 36)
  const characterSized = await until(async () => Math.abs((await styleOf()).size_pt * typeScale - 36) < 0.05, 40)
  await setTypeField('character-tracking', 50)
  const tracked = await until(async () => (await styleOf()).tracking === 50, 40)
  await kit.getByTestId('photo-character-fauxBold').click()
  const bolded = await until(async () => (await styleOf()).faux_bold === true, 40)
  await kit.getByTestId('photo-character-kerning-input').fill('optical')
  await kit.getByTestId('photo-character-kerning-input').press('Enter')
  const kerned = await until(async () => (await styleOf()).kerning === 'Optical', 40)
  step('Character sets size, tracking, kerning, and Faux Bold on a type layer', characterSized && tracked && bolded && kerned, JSON.stringify(await styleOf(), ['size_pt', 'tracking', 'faux_bold', 'kerning']))
  await tab('paragraph').click()
  await kit.getByTestId('photo-paragraph').waitFor()
  await checkIds()
  await kit.getByTestId('photo-paragraph-align-center').click()
  await setTypeField('paragraph-spaceAfter', 6)
  const paragraphed = await until(async () => {
    const style = (await typeInfo(styledLayer.id)).paragraphs[0].style
    return style.align === 'Center' && style.space_after_pt === 6
  }, 40)
  step('Paragraph sets alignment and space after on a type layer', paragraphed, JSON.stringify((await typeInfo(styledLayer.id)).paragraphs[0].style))
  // Without a type layer, upstream's hint shows: the Type tool's own options (font, style, size,
  // alignment) and the colour can change, the rest is disabled.
  const pixelLayer = (await details()).layers.find((layer) => layer.kind === 'Pixel' && layer.id != null)
  await selectLayer(pixelLayer.id)
  await tab('character').click()
  await until(async () => (await kit.getByTestId('photo-character').getAttribute('data-target')) === 'tool', 40)
  const hinted = (await kit.locator('[data-testid="photo-character"] .photo-type-hint').count()) === 1
  const trackingLocked = await typeField('character-tracking').isDisabled()
  await setTypeField('character-size', 30)
  await choose('Type')
  const toolSized = await until(async () => (await option('fontSize').locator('input').inputValue()) === '30', 40)
  step("without a type layer, Character shows upstream's hint, sets the Type tool's size, and disables the rest", hinted && trackingLocked && toolSized, JSON.stringify({ hinted, trackingLocked, toolSized }))
  // New type takes the tool's font, style, size, and alignment, and the rest from Type › Save
  // Default Type Styles (the styled layer's tracking 50 and Faux Bold), as upstream's does.
  await selectLayer(styledLayer.id)
  await kit.getByTestId('photo-menu-Type').click()
  await kit.getByTestId('photo-cmd-type.saveDefaultTypeStyles').click()
  await selectLayer(pixelLayer.id)
  await until(async () => (await kit.getByTestId('photo-character').getAttribute('data-target')) === 'tool', 40)
  const newTypeAt = at(0.6, 0.3)
  await page.mouse.click(newTypeAt.x, newTypeAt.y)
  await kit.getByTestId('photo-text-field').waitFor()
  await kit.getByTestId('photo-text-field').fill('NT')
  await kit.getByTestId('photo-text-field').press('Enter')
  await until(async () => (await details()).layers.some((layer) => layer.name === 'NT'))
  const newType = (await details()).layers.find((layer) => layer.name === 'NT')
  const newStyle = newType ? (await typeInfo(newType.id)).runs[0].style : null
  step('new type takes the tool’s size and the saved default type styles for the rest', newStyle?.tracking === 50 && newStyle?.faux_bold === true && Math.abs(newStyle?.size_pt - 30) < 0.05, JSON.stringify(newStyle, ['size_pt', 'tracking', 'faux_bold']))
  await kit.getByTestId('photo-tool-Brush').click()
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.character').click()
  await until(async () => (await count('photo-character')) === 0)
  step('the new panels give every control a declared id', idProblems.length === 0, idProblems.slice(0, 3).join('; '))

  // Window toggles show the dock's groups: History is off in upstream's default workspace.
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.history').click()
  await until(async () => (await count('photo-dock-area-group-history-group')) === 1)
  step('Window > History shows the History group in the dock', (await count('photo-dock-area-group-history-group')) === 1 && (await count('photo-kit-layers')) === 1)
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.history').click()
  await until(async () => (await count('photo-dock-area-group-history-group')) === 0)

  // Upstream's menu catalog: web-hidden rows left out, engine paths, Kition's Window toggles and Help.
  await kit.getByTestId('photo-menu-File').click()
  step('leaves out commands a browser cannot run', (await count('photo-cmd-file.saveACopy')) === 0 && (await count('photo-cmd-file.new')) === 0 && (await count('photo-cmd-file.placeEmbedded')) === 1)
  await kit.getByTestId('photo-menu-File').click()
  await kit.getByTestId('photo-menu-Window').click()
  const layersRow = kit.getByTestId('photo-control-window.panel.layers')
  step('Window > Layers is checked and shows F7', (await layersRow.getAttribute('aria-checked')) === 'true' && (await layersRow.textContent()).includes('F7'))
  await layersRow.click()
  await until(async () => (await count('photo-kit-layers')) === 0)
  step('Window > Layers hides the Layers panel', (await count('photo-kit-layers')) === 0)
  // Keys reach the editor from inside it: a tool button takes the focus first.
  await kit.getByTestId('photo-tool-Brush').click()
  await page.keyboard.press('F7')
  await until(async () => (await count('photo-kit-layers')) === 1)
  step('F7 shows the Layers panel again', (await count('photo-kit-layers')) === 1)
  // A key of a command hidden on the web does nothing: Close All's (Cmd+Alt+W) would close the only document.
  await kit.getByTestId('photo-tool-Brush').click()
  await page.keyboard.press(`${await primaryKey()}+Alt+KeyW`)
  await page.waitForTimeout(300)
  const layerCount = async () => (await details().catch(() => null))?.layers?.length ?? -1
  const kept = await layerCount()
  await kit.getByTestId('photo-menu-Layer').click()
  await kit.getByTestId('photo-cmd-layer.duplicate').click()
  await okDialog()
  await until(async () => (await layerCount()) === kept + 1, 30)
  step("ignores Close All's key, hidden on the web, and keeps the document open", kept > 0 && (await layerCount()) === kept + 1, `layers ${kept} -> ${await layerCount()}`)
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.status').click()
  await until(async () => !(await kit.getByTestId('photo-status').isVisible()))
  step('Window > Status Bar hides the status bar', !(await kit.getByTestId('photo-status').isVisible()))
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.status').click()
  await until(async () => kit.getByTestId('photo-status').isVisible())
  // Upstream's status bar: its zoom field moves the canvas, and it shows the active tool's hint.
  const zoomInput = kit.getByTestId('photo-status-zoom').locator('input')
  await zoomInput.fill('50')
  await zoomInput.press('Enter')
  await until(async () => (await zoomInput.getAttribute('aria-valuenow')) === '50')
  step('the status bar zoom field zooms the canvas', (await zoomInput.getAttribute('aria-valuenow')) === '50', await zoomInput.getAttribute('aria-valuenow'))
  await kit.getByTestId('photo-menu-View').click()
  await kit.getByTestId('photo-control-view.fitOnScreen').click()
  await until(async () => (await zoomInput.getAttribute('aria-valuenow')) !== '50')
  await kit.getByTestId('photo-tool-RectMarquee').click()
  step("the status bar shows the active tool's hint", ((await kit.getByTestId('photo-status-hint').textContent()) ?? '').includes('Shift'), await kit.getByTestId('photo-status-hint').textContent())
  // Type › Panels › Paragraph shows the Character group and brings Paragraph forward.
  await kit.getByTestId('photo-menu-Type').click()
  const submenus = kit.locator('.kui-submenu')
  for (let index = 0; index < (await submenus.count()) && (await count('photo-control-type.panels.paragraph')) === 0; index += 1) await submenus.nth(index).hover()
  await kit.getByTestId('photo-control-type.panels.paragraph').click()
  await until(async () => (await count('photo-paragraph')) === 1)
  step('Type > Panels > Paragraph shows the Character group on its Paragraph tab', (await count('photo-paragraph')) === 1)
  await kit.getByTestId('photo-menu-Window').click()
  await kit.getByTestId('photo-control-window.panel.character').click()
  await until(async () => (await count('photo-paragraph')) === 0)
  await kit.getByTestId('photo-menu-Help').click()
  await kit.getByTestId('photo-control-help.shortcuts').click()
  const shortcuts = (await kit.getByTestId('photo-shortcuts').textContent()) ?? ''
  step(
    "Help > Keyboard Shortcuts lists Photo's tool keys and the shortcuts of rows it runs, once each",
    shortcuts.includes('F7') && shortcuts.includes('Cmd+Alt+I') && shortcuts.includes('矩形选框') && (shortcuts.match(/Cmd\+K(?![A-Za-z])/g) ?? []).length === 1,
    shortcuts.slice(0, 80),
  )
  await kit.getByTestId('photo-help-close').click()
  await kit.getByTestId('photo-menu-Help').click()
  await kit.getByTestId('photo-control-help.about').click()
  const version = JSON.parse(readFileSync(new URL('../../kit.json', import.meta.url), 'utf8')).version
  step('Help > About shows the version', ((await kit.getByTestId('photo-about').textContent()) ?? '').includes(version))
  await kit.getByTestId('photo-help-close').click()
  await until(async () => (await count('photo-about')) === 0)
  // Menus by keyboard: into the Image menu, down to a submenu and into it, back out with Escape,
  // and the focus is back on the menu bar, where the editor's keys (X) still work.
  const focusedIn = () =>
    frameNow().evaluate(() => {
      const element = document.activeElement
      return { id: element?.getAttribute('data-kit-ui-id') ?? null, testId: element?.getAttribute('data-testid') ?? null, submenu: element?.getAttribute('aria-haspopup') === 'menu', inSubmenu: Boolean(element?.closest('.kui-submenu-popover')) }
    })
  await kit.getByTestId('photo-foreground').fill('#ff0000')
  await kit.getByTestId('photo-menu-Image').click()
  let row = await focusedIn()
  for (let index = 0; index < 40 && !row.submenu; index += 1) {
    await page.keyboard.press('ArrowDown')
    row = await focusedIn()
  }
  await page.keyboard.press('ArrowRight')
  const inside = await focusedIn()
  await page.keyboard.press('Escape')
  const backOnRow = await focusedIn()
  await page.keyboard.press('Escape')
  await until(async () => (await kit.locator('.kui-popover').count()) === 0, 20)
  const onBar = await focusedIn()
  const swapFrom = await kit.getByTestId('photo-foreground').inputValue()
  await page.keyboard.press('x')
  const keysWork = await until(async () => (await kit.getByTestId('photo-foreground').inputValue()) !== swapFrom, 20)
  step(
    'the keyboard walks a menu into a submenu and back, and the editor keys work after',
    row.submenu && inside.inSubmenu && backOnRow.id === row.id && onBar.testId === 'photo-menu-Image' && keysWork,
    JSON.stringify({ row, inside, backOnRow, onBar, keysWork }),
  )
  await page.keyboard.press('x')
  await page.keyboard.press('d')
  // Feather's key is upstream's (the engine binds none): the editor runs it as the menu row does.
  const unfeathered = (await inspect()).revision
  await kit.getByTestId('photo-tool-Brush').click()
  await page.keyboard.press('Shift+F6')
  // Feather… asks for its radius first (plan 2026-11-24, stage 1): OK feathers.
  await okDialog()
  const feathered = await until(async () => (await inspect()).revision > unfeathered, 30)
  step("Shift+F6 feathers the selection (upstream's key)", feathered && (await details()).hasSelection === true, `selection: ${(await details()).hasSelection}`)

  const run = await page.evaluate(async () => window.devHost.connection().ai.run([{ command: 'layer.new.layer', params: {} }], false))
  await page.waitForTimeout(300)
  step('the agent edits the live image', run.completed[0].changed && (await page.evaluate(() => window.devHost.changes.at(-1).by)) === 'agent')
  await kit.getByTestId('photo-save').click()
  await until(async () => (await page.evaluate(() => window.devHost.files.get('Sample.psd').version)) > 1, 100)
  const header = await page.evaluate(() => new TextDecoder().decode(new Uint8Array(window.devHost.files.get('Sample.psd').bytes).slice(0, 4)))
  step('saves a PSD through the host', header === '8BPS', header)

  // Painting a type or shape layer's pixels asks to rasterize it first, as Photoshop does: Cancel
  // changes nothing; OK rasterizes, then paints at the press, in two history states.
  await deselectForPerson()
  const activeKind = async () => {
    const now = await details()
    return findLayer(now.layers, now.activeLayer)?.kind
  }
  const middle = at(0.5, 0.5)
  const pressWith = async (tool) => {
    if (tool === 'Brush') {
      await page.mouse.move(middle.x, middle.y)
      await page.mouse.down()
      await page.mouse.move(middle.x + 30, middle.y + 10, { steps: 4 })
      await page.mouse.up()
    } else await page.mouse.click(middle.x, middle.y)
  }
  const asksToRasterize = async (name, tool, kind, make) => {
    await make()
    await until(async () => (await activeKind()) === kind, 40)
    await choose(tool)
    const before = await details()
    await pressWith(tool)
    const asked = await until(async () => (await count('photo-rasterize-question')) === 1, 40)
    const question = asked ? await kit.getByTestId('photo-rasterize-question').textContent() : ''
    const title = asked ? await kit.locator('.kui-dialog h2').textContent() : ''
    const waited = (await details()).history.length === before.history.length && (await activeKind()) === kind
    step(`${name}: the press asks to rasterize the layer first`, asked && question.includes('栅格化') && title === 'Kition Photo' && waited, JSON.stringify({ asked, question, title, waited }))
    await kit.getByTestId('photo-rasterize-cancel').click()
    await page.waitForTimeout(300)
    const after = await details()
    step(`${name}: Cancel changes nothing`, (await count('photo-rasterize')) === 0 && after.revision === before.revision && (await activeKind()) === kind, `${before.revision} -> ${after.revision}, ${await activeKind()}`)
    await pressWith(tool)
    await kit.getByTestId('photo-rasterize-confirm').waitFor()
    await kit.getByTestId('photo-rasterize-confirm').click()
    // Two new states: the one current before the press is now third from the end.
    const twoSteps = await until(async () => {
      const now = await details()
      return now.history.at(-3) === before.history.at(-1) && now.history.at(-2) !== before.history.at(-1) && now.historyRedo.length === 0
    }, 60)
    const states = (await details()).history.slice(-3)
    step(`${name}: OK rasterizes, then paints, in two history states`, twoSteps && (await activeKind()) === 'Pixel' && (await count('photo-rasterize')) === 0, `${JSON.stringify(states)}, ${await activeKind()}`)
  }
  await asksToRasterize('Brush on a type layer', 'Brush', 'Type', () => createForPerson('type.create', { x: 40, y: 60, text: 'Raster' }))
  await asksToRasterize('Paint Bucket on a shape layer', 'PaintBucket', 'Shape', () => createForPerson('shape.create', { kind: 'rect', rect: [20, 20, 120, 90], fill: '#ff0000' }))

  // UI configuration (Kit UI protocol section 9).
  const frame = page.frames().find((candidate) => candidate.url().includes('index.html'))
  const configure = (ui) => page.evaluate((next) => window.devHost.configure(next), ui)

  await configure({ hide: ['menu:Filter', 'panel:properties', 'control:window.panel.layers', 'control:options.opacity'], disable: ['tool:Brush', 'tool:HistoryBrush'] })
  await until(async () => (await count('photo-menu-Filter')) === 0)
  step('hides a menu the host hides', (await count('photo-menu-Filter')) === 0 && (await count('photo-menu-Image')) === 1)
  // A hidden tab goes; its group stays while another of its tabs shows (Adjustments).
  step('hides a panel the host hides', (await count('photo-properties')) === 0 && (await count('photo-kit-layers')) === 1 && (await count('photo-dock-area-tab-adjustments')) === 1 && (await count('photo-dock-area-group-properties-group')) === 1)
  // A slot whose tool the host disables shows its first allowed tool, the disabled one greyed in its flyout.
  await slotButton('Pencil').click({ button: 'right' })
  const flyoutBrush = kit.locator('.kui-tool-flyout [data-testid="photo-tool-Brush"]')
  step('disables a tool the host disables', (await slotButton('HistoryBrush').isDisabled()) && (await flyoutBrush.isDisabled()))
  await kit.getByTestId('photo-tool-Lasso').click()
  await page.keyboard.press('v')
  await until(async () => (await kit.getByTestId('photo-tool-Move').getAttribute('aria-pressed')) === 'true')
  step('selects an allowed tool by its key', (await kit.getByTestId('photo-tool-Move').getAttribute('aria-pressed')) === 'true')
  await kit.getByTestId('photo-tool-Lasso').click()
  await page.keyboard.press('y')
  // History Brush is disabled and alone in its slot, so 'y' should select nothing: there is no
  // state change to wait on, so a short settle covers this negative assertion.
  await page.waitForTimeout(200)
  step('ignores the key of a disabled tool', (await activeTool()) === 'Lasso', await activeTool())
  await page.keyboard.press('b')
  await until(async () => (await activeTool()) === 'Pencil', 20)
  step("passes a tool key over a disabled tool to the slot's first allowed one", (await activeTool()) === 'Pencil', await activeTool())
  step("hides an options-bar field the host hides, for every tool", (await option('opacity').count()) === 0 && (await option('smoothing').count()) === 1)
  await page.keyboard.press('F7')
  await page.waitForTimeout(200)
  step('ignores the key of a Window row the host hides', (await count('photo-kit-layers')) === 1)
  const refused = await page.evaluate(() =>
    window.devHost.connection().ai.run([{ command: 'filter.blur.gaussianBlur', params: { radius: 2 } }], false).then(
      () => 'ran',
      (error) => error?.data?.reason ?? String(error),
    ),
  )
  step('refuses the agent a hidden command', refused === 'not-allowed', refused)

  // A group goes only when all its tabs are hidden.
  await configure({ hide: ['panel:layers', 'panel:channels'] })
  await until(async () => (await count('photo-dock-area-tab-layers')) === 0)
  const lastTab = (await count('photo-dock-area-group-layers-group')) === 1 && (await count('photo-dock-area-tab-paths')) === 1
  await configure({ hide: ['panel:layers', 'panel:channels', 'panel:paths'] })
  await until(async () => (await count('photo-dock-area-group-layers-group')) === 0)
  step('hides a dock group only when the host hides all its tabs', lastTab && (await count('photo-dock-area-group-layers-group')) === 0 && (await count('photo-dock-area-group-properties-group')) === 1)

  await configure({ add: [{ id: 'submit', place: 'action', label: { 'en-US': 'Submit', 'zh-CN': '提交' } }] })
  await kit.locator('[data-kit-ui-id="host:submit"]').click()
  await until(async () => (await page.evaluate(() => window.devHost.actions.length)) > 0)
  const action = await page.evaluate(() => window.devHost.actions.at(-1))
  step('reports a host button press', action?.id === 'submit' && action?.document?.path === 'Sample.psd', JSON.stringify(action))

  await configure({ hide: ['panel:nope'] })
  const elements = await page.evaluate(() => window.devHost.connection().elements()).then(
    (found) => found,
    (error) => (error?.message ?? String(error)).split('\n')[0],
  )
  if (Array.isArray(elements)) {
    const problems = await checkUiIds(frame, elements)
    step('gives every control a declared id', problems.length === 0, problems.slice(0, 3).join('; '))
  } else {
    step('gives every control a declared id', false, elements)
  }
  await configure({})

  // The swatch library and the colours are the kit's, not the document's: kept across a reopen (upstream saves them).
  await tab('swatches').click()
  await kit.getByTestId('photo-swatch-0').waitFor()
  const keptSwatches = (await kit.locator('[data-testid^="photo-swatch-"][data-color]').count()) + 1
  await kit.getByTestId('photo-swatches-new').click()
  await until(async () => (await kit.locator('[data-testid^="photo-swatch-"][data-color]').count()) === keptSwatches, 40)
  const keptForeground = await kit.getByTestId('photo-foreground').inputValue()

  await attempt('hides a panel the person hides', async () => {
    await kit.getByTestId('photo-more').click()
    await kit.getByTestId('photo-customize').click()
    await kit.locator('input[data-element="panel:layers"]').uncheck()
    await kit.locator('[data-action="save"]').click()
    await until(async () => (await count('photo-kit-layers')) === 0)
    // Its siblings, Channels and Paths, keep the group on screen.
    return (await count('photo-kit-layers')) === 0 && (await count('photo-dock-area-tab-channels')) === 1 && (await count('photo-dock-area-tab-paths')) === 1
  })
  await attempt("keeps the person's hides when reopened", async () => {
    const bytes = await page.evaluate(() => Array.from(new Uint8Array(window.devHost.files.get('Sample.psd').bytes)))
    // Does not await `open` inside the page: it navigates the kit iframe, which would destroy
    // this very `page.evaluate`'s execution context while it is still pending (WebKit hits this).
    await page.evaluate((data) => {
      void window.devHost.open('Sample.psd', new Uint8Array(data).buffer)
    }, bytes)
    await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
    await until(async () => (await count('photo-kit-layers')) === 0)
    return (await count('photo-kit-layers')) === 0 && (await count('photo-dock-area-tab-channels')) === 1 && (await count('photo-menu-Image')) === 1
  })
  await attempt('keeps a new swatch and the colours when the document is reopened', async () => {
    await kit.getByTestId('photo-dock-area-tab-swatches').click()
    const kept = await until(async () => (await kit.locator('[data-testid^="photo-swatch-"][data-color]').count()) === keptSwatches, 60)
    const colour = await until(async () => (await kit.getByTestId('photo-foreground').inputValue()) === keptForeground, 40)
    return kept && colour
  })
  await attempt('restores the defaults', async () => {
    await kit.getByTestId('photo-more').click()
    await kit.getByTestId('photo-customize').click()
    await kit.locator('[data-action="restore"]').click()
    await kit.locator('[data-action="save"]').click()
    await until(async () => (await count('photo-kit-layers')) === 1)
    return (await count('photo-kit-layers')) === 1
  })

  const reopened = page.frames().find((candidate) => candidate.url().includes('index.html'))
  const primary = () => reopened.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--kit-primary').trim())
  await page.evaluate(() => window.devHost.setScheme('dark'))
  await until(async () => (await primary()) === KIT_BRAND.dark)
  const darkPrimary = await primary()
  await page.evaluate(() => window.devHost.setScheme('light'))
  await until(async () => (await primary()) === KIT_BRAND.light)
  const lightPrimary = await primary()
  step('uses the manifest brand colors when switching schemes', darkPrimary === KIT_BRAND.dark && lightPrimary === KIT_BRAND.light, JSON.stringify({ dark: darkPrimary, light: lightPrimary, expected: KIT_BRAND }))

  // Two Photo tabs share the colours (Kit UI's kit store and its storage events): B follows A, and
  // an engine change in B right after A's newer colour never writes B's older colour back.
  const pageB = await context.newPage()
  pageB.on('pageerror', (error) => errors.push(error.message))
  // B's frames can hear `storage` late: while `__holdStorage` is set, each storage listener gets its event that many milliseconds later.
  await pageB.addInitScript(() => {
    const add = EventTarget.prototype.addEventListener
    const remove = EventTarget.prototype.removeEventListener
    const gates = new WeakMap()
    window.__holdStorage = 0
    EventTarget.prototype.addEventListener = function (type, listener, options) {
      if (type !== 'storage' || typeof listener !== 'function') return add.call(this, type, listener, options)
      const gate = function (event) {
        if (window.__holdStorage > 0) setTimeout(() => listener.call(this, event), window.__holdStorage)
        else listener.call(this, event)
      }
      gates.set(listener, gate)
      return add.call(this, type, gate, options)
    }
    EventTarget.prototype.removeEventListener = function (type, listener, options) {
      return remove.call(this, type, gates.get(listener) ?? listener, options)
    }
  })
  await pageB.goto(`${origin}/dev.html?locale=zh-CN`)
  await pageB.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kitB = pageB.frameLocator('#kit')
  await kitB.getByTestId('photo-foreground').waitFor()
  const engineForeground = async (which) => {
    const answer = (await which.evaluate(() => window.devHost.connection().ai.run([{ command: 'session.inspect', params: {} }], false))).completed[0].result
    return `#${answer.foreground.slice(0, 3).map((value) => Math.round(value * 255).toString(16).padStart(2, '0')).join('')}`
  }
  await kit.getByTestId('photo-foreground').fill('#aa0000')
  const followed = await until(async () => (await kitB.getByTestId('photo-foreground').inputValue()) === '#aa0000' && (await engineForeground(pageB)) === '#aa0000', 60)
  step('a second Photo tab follows the colour the first one sets', followed, await kitB.getByTestId('photo-foreground').inputValue())
  // B hears of A's next colour late (its storage event held back 1.5 s), and its engine changes meanwhile.
  const frameB = pageB.frames().find((candidate) => candidate.url().includes('index.html'))
  await frameB.evaluate(() => {
    window.__holdStorage = 1500
  })
  const storedForeground = () => page.evaluate(() => JSON.parse(document.getElementById('kit').contentWindow.localStorage.getItem('kit:kit.photo:colors') ?? 'null')?.foreground)
  await kit.getByTestId('photo-foreground').fill('#00bb00')
  await until(async () => (await storedForeground()) === '#00bb00', 40)
  await pageB.evaluate(() => window.devHost.connection().ai.run([{ command: 'layer.new.layer', params: {} }], false))
  const heldBack = (await kitB.getByTestId('photo-foreground').inputValue()) === '#aa0000'
  await page.waitForTimeout(2500)
  await frameB.evaluate(() => {
    window.__holdStorage = 0
  })
  const stored = await storedForeground()
  const newest = [await kit.getByTestId('photo-foreground').inputValue(), await kitB.getByTestId('photo-foreground').inputValue(), await engineForeground(page), await engineForeground(pageB), stored]
  step("an engine change in the second tab never writes its older colour over the first tab's newer one", heldBack && newest.every((value) => value === '#00bb00'), JSON.stringify({ heldBack, newest }))
  await pageB.close()
} catch (error) {
  failures += 1
  results.push({ name: 'unhandled flow failure', ok: false, detail: String(error) })
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n')[0] : String(error)}`)
} finally {
  if (errors.length) {
    failures += errors.length
    console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  }
  const artifacts = new URL(`../test-results/legacy-${which}/`, import.meta.url)
  mkdirSync(artifacts, { recursive: true })
  writeFileSync(new URL('results.json', artifacts), JSON.stringify({ browser: which, failures, errors, results }, null, 2))
  if (failures) {
    await page.screenshot({ path: fileURLToPath(new URL('failure.png', artifacts)) }).catch(() => {})
    await context.tracing.stop({ path: fileURLToPath(new URL('trace.zip', artifacts)) })
  } else await context.tracing.stop()
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
