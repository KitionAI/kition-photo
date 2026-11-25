// Brush strokes drawn by the engine while the pointer moves (plan 2026-11-24, stage 3):
//   node tests/live-stroke.mjs [chromium|webkit]
// The Brush, the Eraser and the Clone Stamp change the pixels under the pointer before the pointer
// is released; the release leaves one history step; Escape mid-stroke puts the picture back and
// leaves the document unchanged; the Move tool's drag no longer shows the layer at its old place.
// Pixels are read from the canvas the document is drawn on (the tools' own marks are on another
// layer above it), through real pointer input on dev.html. Prints PASS or FAIL per step.
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
  const frame = await (await page.$('#kit')).contentFrame()
  const surface = kit.getByTestId('kit-canvas-surface')
  const workspace = kit.getByTestId('photo-workspace')
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const steps = async () => (await details()).historyIndex
  const option = (name) => kit.locator(`[data-option="${name}"] input`)
  const setOption = async (name, value) => {
    await option(name).fill(String(value))
    await option(name).press('Enter')
  }
  const view = async () => {
    const [zoom, x, y] = (await workspace.getAttribute('data-view')).split(' ').map(Number)
    return { zoom, x, y, box: await surface.boundingBox() }
  }
  const at = async (x, y) => {
    const v = await view()
    return { x: v.box.x + (x - v.x) * v.zoom, y: v.box.y + (y - v.y) * v.zoom }
  }
  /** The colour the canvas shows at document point (x, y). */
  const pixel = async (x, y) => {
    const v = await view()
    return frame.evaluate(([cx, cy]) => {
      const canvas = document.querySelector('[data-testid="kit-canvas-surface"]')
      const ratio = canvas.width / canvas.clientWidth
      return Array.from(canvas.getContext('2d').getImageData(Math.round(cx * ratio), Math.round(cy * ratio), 1, 1).data).slice(0, 3)
    }, [(x - v.x) * v.zoom, (y - v.y) * v.zoom])
  }
  const near = (a, b, tolerance = 24) => a.every((value, index) => Math.abs(value - b[index]) <= tolerance)
  const MAGENTA = [255, 0, 255]
  const settle = async () => {
    let before = (await inspect()).revision
    for (let i = 0; i < 20; i += 1) {
      await page.waitForTimeout(150)
      const now = (await inspect()).revision
      if (now === before) return
      before = now
    }
  }
  const press = async (x, y) => {
    const p = await at(x, y)
    await page.mouse.move(p.x, p.y)
    await page.mouse.down()
  }
  const drawTo = async (x, y, moves = 12) => {
    const p = await at(x, y)
    await page.mouse.move(p.x, p.y, { steps: moves })
  }
  const pick = async (key, tool) => {
    await workspace.focus()
    await page.keyboard.press(key)
    return until(async () => (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === tool, 20)
  }

  await page.keyboard.press('v')
  await pick('b', 'Brush')
  await kit.getByTestId('photo-foreground').fill('#ff00ff')
  await setOption('size', 40)
  await setOption('hardness', 100)
  await setOption('opacity', 100)
  await setOption('flow', 100)
  {
    const count = (await details()).layers.length
    await kit.getByTestId('photo-layer-new').click()
    await until(async () => (await details()).layers.length === count + 1)
  }
  await workspace.focus()

  // The Brush: the stroke shows under the pointer before the release.
  {
    const start = await inspect()
    const history = await steps()
    await press(300, 300)
    await drawTo(700, 300)
    const shown = await until(async () => near(await pixel(690, 300), MAGENTA) && near(await pixel(500, 300), MAGENTA), 50)
    await shot('live-brush-held')
    const held = await inspect()
    step('the Brush paints under the pointer before the release', shown, `${await pixel(690, 300)}`)
    step('while the Brush is held the document is unchanged', held.revision === start.revision && held.details.historyIndex === history, `revision ${start.revision} -> ${held.revision}`)
    await page.mouse.up()
    await settle()
    step('releasing the Brush leaves one history step and the stroke', (await steps()) === history + 1 && near(await pixel(500, 300), MAGENTA), `history +${(await steps()) - history}`)
  }

  // Escape mid-stroke puts the picture back and changes nothing.
  {
    const start = await inspect()
    // Read the picture there once the canvas has painted it (after the last release it can still
    // be drawing): the document's own colour at that point, as the engine composites it.
    const composite = (await page.evaluate(async () => (await window.devHost.connection().ai.run([{ command: 'document.sampleColor', params: { x: 500, y: 450, size: 'point', sampleLayer: 'all' } }], false)).completed[0].result)).slice(0, 3).map((value) => Math.round(value * 255))
    await until(async () => near(await pixel(500, 450), composite, 6), 50)
    const before = await pixel(500, 450)
    await press(300, 450)
    await drawTo(700, 450)
    const shown = await until(async () => near(await pixel(500, 450), MAGENTA), 50)
    await page.keyboard.press('Escape')
    const back = await until(async () => near(await pixel(500, 450), before, 2), 50)
    await page.mouse.up()
    await settle()
    const after = await inspect()
    step('Escape mid-stroke puts the picture back', shown && back, `${before} -> ${await pixel(500, 450)}`)
    step('an Escaped stroke leaves the revision and history unchanged', after.revision === start.revision && after.details.historyIndex === start.details.historyIndex, `revision ${start.revision} -> ${after.revision}`)
  }

  // The Eraser: the crossing goes before the release.
  await pick('e', 'Eraser')
  {
    const history = await steps()
    await press(600, 240)
    await drawTo(600, 360)
    const erased = await until(async () => !near(await pixel(600, 300), MAGENTA, 60), 50)
    await shot('live-eraser-held')
    step('the Eraser erases under the pointer before the release', erased && (await steps()) === history, `${await pixel(600, 300)}`)
    await page.mouse.up()
    await settle()
    step('releasing the Eraser leaves one history step', (await steps()) === history + 1 && !near(await pixel(600, 300), MAGENTA, 60), `history +${(await steps()) - history}`)
  }

  // The Clone Stamp: Alt-click the stroke as the source, then paint it below.
  await pick('s', 'CloneStamp')
  {
    const source = await at(400, 300)
    await page.keyboard.down('Alt')
    await page.mouse.click(source.x, source.y)
    await page.keyboard.up('Alt')
    await page.waitForTimeout(200)
    const history = await steps()
    const before = await pixel(320, 560)
    await press(250, 560)
    await drawTo(400, 560)
    const cloned = await until(async () => near(await pixel(320, 560), MAGENTA, 40), 50)
    await shot('live-clone-held')
    step('the Clone Stamp paints its source under the pointer before the release', cloned && (await steps()) === history, `${before} -> ${await pixel(320, 560)}`)
    await page.mouse.up()
    await settle()
    step('releasing the Clone Stamp leaves one history step', (await steps()) === history + 1 && near(await pixel(320, 560), MAGENTA, 40), `history +${(await steps()) - history}`)
  }

  // The Move tool: while the layer follows the pointer, its old place shows what is under it.
  await pick('v', 'Move')
  {
    const start = await inspect()
    await press(400, 300)
    await drawTo(400, 380)
    const gone = await until(async () => !near(await pixel(400, 300), MAGENTA, 60), 50)
    await shot('live-move-held')
    step('a moved layer is gone from its old place during the drag', gone, `${await pixel(400, 300)}`)
    await page.keyboard.press('Escape')
    const back = await until(async () => near(await pixel(400, 300), MAGENTA), 50)
    await page.mouse.up()
    await settle()
    step('Escape puts the layer back where it was, unchanged', back && (await inspect()).revision === start.revision, `${await pixel(400, 300)}`)
  }
  step('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} FAILED (${which})` : `ALL PASSED (${which})`)
process.exit(failures ? 1 : 0)
