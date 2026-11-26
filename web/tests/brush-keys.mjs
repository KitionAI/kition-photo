// The brush's cursor and keys in a real browser (plan 2026-11-24, stage 2): node tests/brush-keys.mjs [chromium|webkit]
// The outline cursor (size and hardness at the zoom, the crosshair below 4 px), [ and ] (Shift:
// hardness), number keys (Shift: flow), Shift's straight line and Shift-click's connection, and the
// Alt eyedropper, through real keys and pointer drags on dev.html. Checks read the options bar and
// the engine's state; prints PASS or FAIL per step.
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
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=zh-CN`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  const kit = page.frameLocator('#kit')
  const surface = kit.getByTestId('kit-canvas-surface')
  const workspace = kit.getByTestId('photo-workspace')
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const history = async () => (await details()).history.length
  const option = (name) => kit.locator(`[data-option="${name}"] input`)
  const value = async (name) => Number(await option(name).inputValue())
  const cursor = kit.getByTestId('photo-brush-cursor')
  const view = async () => {
    const [zoom, x, y] = (await workspace.getAttribute('data-view')).split(' ').map(Number)
    return { zoom, x, y, box: await surface.boundingBox() }
  }
  const at = async (x, y) => {
    const v = await view()
    return { x: v.box.x + (x - v.x) * v.zoom, y: v.box.y + (y - v.y) * v.zoom }
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
  const activeBounds = async () => {
    const now = await details()
    return now.layers.find((layer) => layer.id === now.activeLayer)?.bounds ?? null
  }
  const newLayer = async () => {
    const count = (await details()).layers.length
    await kit.getByTestId('photo-layer-new').click()
    await until(async () => (await details()).layers.length === count + 1)
    await workspace.focus()
  }
  const hover = async (x, y) => {
    const p = await at(x, y)
    await page.mouse.move(p.x - 3, p.y)
    await page.mouse.move(p.x, p.y, { steps: 2 })
  }

  // The keys work on open, with no click first: V, then B picks the Brush (B again would cycle to the Pencil).
  await page.keyboard.press('v')
  await page.keyboard.press('b')
  await until(async () => (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === 'Brush', 20)
  await hover(300, 300)
  const zoom = (await view()).zoom
  const diameter = Number(await cursor.getAttribute('data-diameter'))
  const hidden = await surface.evaluate((canvas) => getComputedStyle(canvas).cursor)
  await shot('brush-outline')
  step('the Brush shows its outline at the brush size × zoom, instead of the system cursor', Math.abs(diameter - (await value('size')) * zoom) <= 1 && hidden === 'none', `diameter ${diameter}, size ${await value('size')}, zoom ${zoom}, cursor ${hidden}`)

  // [ and ] step through Photoshop's sizes; Shift with them steps the hardness by quarters.
  // Each key's step lands before the next is read (the options bar follows the engine a frame later).
  await page.keyboard.press(']')
  await until(async () => (await value('size')) === 25, 20)
  await page.keyboard.press(']')
  await until(async () => (await value('size')) === 30, 20)
  const bigger = await value('size')
  await page.keyboard.press('[')
  await until(async () => (await value('size')) === 25, 20)
  const smaller = await value('size')
  step('] and [ step the size (20 → 25 → 30 → 25)', bigger === 30 && smaller === 25, `${bigger}, ${smaller}`)
  await page.keyboard.press('Shift+BracketLeft')
  const softer = await value('hardness')
  await page.waitForTimeout(100)
  const ring = await kit.getByTestId('photo-brush-cursor-hardness').count()
  await page.keyboard.press('Shift+BracketRight')
  step('Shift+[ and Shift+] step the hardness by 25 %, and the outline shows it', softer === 75 && (await value('hardness')) === 100 && ring === 1, `${softer} -> ${await value('hardness')}, ring ${ring}`)

  // Below 4 screen pixels the crosshair shows instead.
  await option('size').fill('3')
  await option('size').press('Enter')
  await workspace.focus()
  await hover(320, 300)
  const crosshair = await surface.evaluate((canvas) => getComputedStyle(canvas).cursor)
  step('below 4 screen pixels the crosshair shows instead of the outline', (await cursor.count()) === 0 && crosshair === 'crosshair', `outline ${await cursor.count()}, cursor ${crosshair}`)
  await option('size').fill('20')
  await option('size').press('Enter')
  await workspace.focus()

  // Number keys: one is tens, two quick ones an exact value, 0 is 100 %; Shift sets Flow.
  await page.keyboard.press('5')
  const fifty = await value('opacity')
  // A second key within 0.7 s pairs with the first: wait before the next value.
  await page.waitForTimeout(800)
  await page.keyboard.press('4')
  await page.keyboard.press('5')
  const exact = await value('opacity')
  await page.waitForTimeout(800)
  await page.keyboard.press('Shift+Digit3')
  const flow = await value('flow')
  await page.waitForTimeout(800)
  await page.keyboard.press('0')
  step('number keys set the opacity (5: 50 %, 4 5: 45 %, 0: 100 %) and Shift+number the flow', fifty === 50 && exact === 45 && flow === 30 && (await value('opacity')) === 100, JSON.stringify({ fifty, exact, flow, opacity: await value('opacity') }))
  await page.waitForTimeout(800)
  await page.keyboard.press('Shift+Digit0')

  // Shift keeps a stroke on one axis: a drag that drifts 40 px down paints a level line.
  await option('size').fill('10')
  await option('size').press('Enter')
  await newLayer()
  {
    const a = await at(300, 300)
    const b = await at(700, 340)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(a.x + 20, a.y + 2, { steps: 2 })
    await page.keyboard.down('Shift')
    await page.mouse.move(b.x, b.y, { steps: 10 })
    await page.mouse.up()
    await page.keyboard.up('Shift')
  }
  await settle()
  const level = await activeBounds()
  step('Shift keeps a brush stroke level', level && level[3] <= 16 && level[2] >= 380, JSON.stringify(level))

  // Shift-click draws a straight stroke from where the last one ended.
  await newLayer()
  {
    const a = await at(300, 500)
    const b = await at(700, 600)
    await page.mouse.click(a.x, a.y)
    await settle()
    await page.keyboard.down('Shift')
    await page.mouse.click(b.x, b.y)
    await page.keyboard.up('Shift')
  }
  await settle()
  const joined = await activeBounds()
  await shot('brush-shift-click')
  step('Shift-click joins the last stroke’s end with a straight stroke', joined && joined[2] >= 390 && joined[3] >= 90, JSON.stringify(joined))

  // Alt with the Brush picks the foreground colour, even over a shape layer, and paints nothing.
  const ellipse = (await details()).layers.find((layer) => layer.kind === 'Shape')
  await kit.getByTestId(`photo-layer-${ellipse.id}`).click()
  await workspace.focus()
  const pickHistory = await history()
  {
    const [x, y, w, h] = ellipse.bounds
    const p = await at(x + w / 2, y + h / 2)
    await page.mouse.move(p.x, p.y)
    await page.keyboard.down('Alt')
    await page.mouse.down()
    await page.mouse.up()
    await page.keyboard.up('Alt')
  }
  const picked = await until(async () => (await kit.getByTestId('photo-foreground').inputValue()) === '#ffd166', 30)
  await settle()
  step('Alt-click with the Brush picks the colour under it, asks nothing and paints nothing', picked && (await kit.getByTestId('photo-rasterize-confirm').count()) === 0 && (await history()) === pickHistory, `${await kit.getByTestId('photo-foreground').inputValue()}, history +${(await history()) - pickHistory}`)
  step('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} FAILED (${which})` : `ALL PASSED (${which})`)
process.exit(failures ? 1 : 0)
