// Move with Auto-Select, as upstream: a press on another layer's pixels makes that layer active, and
// the same drag moves it. node tests/auto-select.mjs [chromium|webkit]
import assert from 'node:assert/strict'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
const kit = page.frameLocator('#kit')
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const errors = []
page.on('pageerror', (error) => errors.push(error.message))
const details = async () => (await page.evaluate(async () => window.devHost.connection().ai.inspect())).details
const runAgent = (command, params) => page.evaluate(([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])
const layerOf = async (id) => (await details()).layers.find((layer) => layer.id === id)
const until = async (name, check) => {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    if (await check().catch(() => false)) {
      console.log(`PASS ${which} ${name}`)
      return
    }
    await page.waitForTimeout(100)
  }
  assert.fail(`${which} ${name}`)
}

try {
  await page.goto(`${origin}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  // A rectangle in the middle of the image, on a layer of its own: a press at the canvas's centre
  // lands on its pixels.
  const { width, height } = await details()
  const made = await runAgent('shape.add', { kind: 'rect', rect: [Math.round(width / 3), Math.round(height / 3), Math.round(width / 3), Math.round(height / 3)], fill: '#2a9d8f' })
  const target = made.completed[0].result.layer
  await until('the rectangle layer exists', async () => (await layerOf(target)) != null)
  const background = (await details()).layers.find((layer) => layer.name === 'Background')
  await kit.getByTestId(`photo-layer-${background.id}`).click()
  await until('the Background is the active layer', async () => (await details()).activeLayer === background.id)

  await kit.locator('.kui-app').focus()
  await page.keyboard.press('v')
  await until('V picks the move tool', async () => (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === 'Move')
  assert.equal(await kit.getByTestId('photo-option-autoSelect').isChecked(), true, 'Auto-Select is on by default')

  const from = JSON.stringify((await layerOf(target)).bounds)
  const backgroundFrom = JSON.stringify(background.bounds)
  const region = await kit.getByTestId('photo-canvas-region').boundingBox()
  const press = { x: region.x + region.width / 2, y: region.y + region.height / 2 }
  await page.mouse.move(press.x, press.y)
  await page.mouse.down()
  await page.mouse.move(press.x + 60, press.y + 40, { steps: 8 })
  await page.mouse.up()
  await until('the press on the rectangle makes its layer active', async () => (await details()).activeLayer === target)
  // Down and to the right, as dragged, its size unchanged (bounds are [x, y, width, height]).
  await until('the same drag moves the rectangle layer', async () => {
    const [x, y, w, h] = (await layerOf(target)).bounds
    const [x0, y0, w0, h0] = JSON.parse(from)
    return x > x0 && y > y0 && w === w0 && h === h0
  })
  assert.equal(JSON.stringify((await layerOf(background.id)).bounds), backgroundFrom, 'the Background stays where it was')
  console.log(`PASS ${which} the Background stays where it was`)
  assert.deepEqual(errors, [])
  console.log(`PASS ${which} no page errors`)
} finally {
  await browser.close()
  await server.close()
}
