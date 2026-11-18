// Escape cancels first in Focus: it drops a marquee or a move mid-drag (the release then commits
// nothing) and a proposed crop, and stays in Focus; only an Escape with nothing to drop leaves
// Focus. node tests/focus.mjs [chromium|webkit]
import assert from 'node:assert/strict'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
const kit = page.frameLocator('#kit')
const app = kit.locator('.kui-app')
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const errors = []
page.on('pageerror', (error) => errors.push(error.message))
const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
const mode = () => app.getAttribute('data-mode')
const tool = async () => (await kit.locator('.kui-focus-tool').textContent())?.trim()
const until = async (name, check) => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await check().catch(() => false)) {
      console.log(`PASS ${which} ${name}`)
      return
    }
    await page.waitForTimeout(100)
  }
  assert.fail(`${which} ${name}`)
}
const stays = async (what) => {
  await page.waitForTimeout(200)
  assert.equal(await mode(), 'focus', `${what} does not leave Focus`)
  console.log(`PASS ${which} ${what} stays in Focus`)
}

try {
  await page.goto(`${origin}/dev.html?modes=classic,line,menus,focus&locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  await app.focus()
  await page.keyboard.press('ControlOrMeta+Backslash')
  await until('Cmd+\\ enters Focus', async () => (await mode()) === 'focus')
  const region = await kit.getByTestId('photo-canvas-region').boundingBox()
  const at = (fx, fy) => ({ x: region.x + region.width * fx, y: region.y + region.height * fy })
  const drag = async (from, to) => {
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 6 })
  }
  const start = await inspect()
  assert.equal(start.details.hasSelection, false)

  // A marquee mid-drag: Escape drops it, and the release selects nothing.
  await page.keyboard.press('m')
  await until('M picks the marquee', async () => (await tool())?.includes('Marquee'))
  await drag(at(0.3, 0.3), at(0.5, 0.5))
  await page.keyboard.press('Escape')
  await stays('an Escape that dropped a marquee')
  await page.mouse.up()
  await page.waitForTimeout(400)
  assert.equal((await inspect()).details.hasSelection, false, 'the dropped marquee selects nothing')
  console.log(`PASS ${which} the dropped marquee selects nothing`)

  // A move mid-drag: Escape drops it, and the release moves nothing. Cmd held at the press keeps
  // the active layer (it turns Auto-Select off for the press).
  await page.keyboard.press('v')
  await until('V picks the move tool', async () => (await tool())?.includes('Move'))
  const active = (await inspect()).details.activeLayer
  await page.keyboard.down('Meta')
  await drag(at(0.5, 0.5), at(0.6, 0.6))
  await page.keyboard.up('Meta')
  assert.equal((await inspect()).details.activeLayer, active, 'the press kept the active layer')
  await page.keyboard.press('Escape')
  await stays('an Escape that dropped a move')
  await page.mouse.up()
  await page.waitForTimeout(400)
  assert.equal((await inspect()).revision, start.revision, 'the dropped move changes nothing')
  console.log(`PASS ${which} the dropped move changes nothing`)

  // A proposed crop: Escape drops it.
  await page.keyboard.press('c')
  await until('C picks the crop tool', async () => (await tool())?.includes('Crop'))
  await drag(at(0.2, 0.2), at(0.6, 0.6))
  await page.mouse.up()
  await page.waitForTimeout(300)
  await page.keyboard.press('Escape')
  await stays('an Escape that dropped a proposed crop')

  // Nothing left to drop: Escape leaves Focus, and no crop is proposed there.
  await page.keyboard.press('Escape')
  await until('Escape with nothing to drop leaves Focus', async () => (await mode()) === 'classic')
  await until('the crop was dropped', async () => await kit.getByTestId('photo-crop-apply').isDisabled())
  assert.equal((await inspect()).revision, start.revision, 'nothing was changed')
  assert.deepEqual(errors, [])
  console.log(`PASS ${which} no page errors`)
} finally {
  await browser.close()
  await server.close()
}
