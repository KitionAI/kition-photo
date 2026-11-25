// The browser harness the stage 7 tests share (plan 2026-11-24): dev.html in chromium or webkit,
// real pointer and key input, the engine's state through the development host, PASS/FAIL per step.
// Screenshots go to the folder in KITION_SHOTS when it is set.
import { mkdirSync } from 'node:fs'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

export async function openEditor(which) {
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
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  const kit = page.frameLocator('#kit')
  const surface = kit.getByTestId('kit-canvas-surface')
  const workspace = kit.getByTestId('photo-workspace')
  // The editor takes Cmd or Ctrl from navigator.platform (kit-editor's isMac).
  const primary = (await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control'
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const query = async (command, params = {}) => {
    try {
      return (await page.evaluate(([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])).completed?.[0]?.result ?? null
    } catch {
      return null
    }
  }
  const view = async () => {
    const [zoom, x, y] = (await workspace.getAttribute('data-view')).split(' ').map(Number)
    return { zoom, x, y, box: await surface.boundingBox() }
  }
  /** Where document point (x, y) is on the page. */
  const at = async (x, y) => {
    const v = await view()
    return { x: v.box.x + (x - v.x) * v.zoom, y: v.box.y + (y - v.y) * v.zoom }
  }
  const drag = async (from, to, { keys = [], later = [], steps = 10 } = {}) => {
    const a = await at(...from)
    const b = await at(...to)
    for (const key of keys) await page.keyboard.down(key)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(a.x + (b.x - a.x) * 0.2, a.y + (b.y - a.y) * 0.2, { steps: 2 })
    for (const key of later) await page.keyboard.down(key)
    await page.mouse.move(b.x, b.y, { steps })
    await page.mouse.up()
    for (const key of [...later, ...keys].reverse()) await page.keyboard.up(key)
  }
  const click = async (x, y, options = {}) => {
    const p = await at(x, y)
    await page.mouse.click(p.x, p.y, options)
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
  const layerNamed = async (name) => (await details()).layers.find((layer) => layer.name === name)
  const tool = () => kit.getByTestId('photo-options-tool').getAttribute('data-tool')
  /** Chooses a tool from its slot or the slot's flyout, as a person does. */
  const choose = async (id) => {
    const direct = kit.locator(`.kui-tool-slot > [data-testid="photo-tool-${id}"]`)
    if (!(await direct.count())) {
      const slot = kit.locator('.kui-tool-slot').filter({ has: kit.locator(`[data-testid="photo-tool-${id}"]`) })
      if (await slot.count()) await slot.locator('> [data-testid^="photo-tool-"]').first().click({ button: 'right' })
      else {
        // The tool is in a flyout only: open each slot's flyout until it shows.
        for (const button of await kit.locator('.kui-tool-slot > [data-testid^="photo-tool-"]').all()) {
          await button.click({ button: 'right' })
          if (await kit.locator(`.kui-tool-flyout [data-testid="photo-tool-${id}"]`).count()) break
          await page.keyboard.press('Escape')
        }
      }
      await kit.locator(`.kui-tool-flyout [data-testid="photo-tool-${id}"]`).click()
    } else await direct.click()
    await until(async () => (await tool()) === id, 30)
    await workspace.focus()
  }
  const selectLayer = async (id) => {
    await kit.getByTestId(`photo-layer-${id}`).click()
    await until(async () => (await details()).activeLayer === id)
    await workspace.focus()
  }
  /** The canvas's right-click menu at document point (x, y) (with `keys` held), once it shows. */
  const menuAt = async (x, y, keys = []) => {
    for (const key of keys) await page.keyboard.down(key)
    await click(x, y, { button: 'right' })
    for (const key of [...keys].reverse()) await page.keyboard.up(key)
    await until(async () => (await kit.getByTestId('photo-canvas-menu').count()) === 1, 30)
    return kit.getByTestId('photo-canvas-menu')
  }
  const menuLabels = async () => (await kit.getByTestId('photo-canvas-menu').locator('[role^="menuitem"]').allTextContents()).map((text) => text.trim())
  const finish = async () => {
    step('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
    await browser.close()
    await server.close()
    process.exit(failures ? 1 : 0)
  }
  const fail = async (error) => {
    console.log(`FAIL ${which} ${error?.stack ?? error}`)
    await browser.close().catch(() => undefined)
    await server.close().catch(() => undefined)
    process.exit(1)
  }
  return { page, kit, surface, workspace, primary, inspect, details, query, view, at, drag, click, settle, layerNamed, tool, choose, selectLayer, menuAt, menuLabels, step, until, shot, finish, fail }
}
