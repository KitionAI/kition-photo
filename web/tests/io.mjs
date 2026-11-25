// Files in and out of the editor (plan 2026-11-24, stage 6), through the development host:
// node tests/io.mjs [chromium|webkit]. Export As writes the chosen format at the chosen size beside
// the document, Quick Export a PNG; an image on the clipboard pastes as a new layer; a file dropped
// on the canvas is placed where it drops, one dropped on the Layers panel in the middle; the File
// menu offers what the host can do; an engine error shows in the editor's words. SHOTS=<folder>
// saves screenshots.
import { mkdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const shots = process.env.SHOTS ? fileURLToPath(new URL(process.env.SHOTS, `file://${process.cwd()}/`)) : null
if (shots) mkdirSync(shots, { recursive: true })
const web = fileURLToPath(new URL('..', import.meta.url))
const server = await createServer({ root: web, configFile: fileURLToPath(new URL('../vite.config.ts', import.meta.url)), server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const origin = `http://127.0.0.1:${server.httpServer.address().port}`
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
await context.tracing.start({ screenshots: true, snapshots: true })
if (which === 'chromium') await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin })
const page = await context.newPage()
const primaryKey = async () => ((await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform))) ? 'Meta' : 'Control')
const errors = []
let failures = 0
page.on('pageerror', (error) => errors.push(error.message))
const step = (name, ok, detail = '') => {
  if (!ok) failures += 1
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` - ${detail}` : ''}`)
}
const until = async (check, tries = 80) => {
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
const flat = (layers) => (layers ?? []).flatMap((layer) => [layer, ...flat(layer.children)])
const layerCount = async () => flat((await details()).layers).length
const activeLayer = async () => {
  const now = await details()
  return flat(now.layers).find((layer) => layer.id === now.activeLayer) ?? null
}
/** A written file of the development host: its first bytes as text, its size, and its decoded size in pixels. */
const written = (name) =>
  page.evaluate(async (path) => {
    const file = window.devHost.files.get(path)
    if (!file) return null
    const bytes = new Uint8Array(file.bytes)
    const type = path.endsWith('.jpg') ? 'image/jpeg' : path.endsWith('.webp') ? 'image/webp' : 'image/png'
    const bitmap = await createImageBitmap(new Blob([bytes], { type })).catch(() => null)
    return { head: Array.from(bytes.slice(0, 12)), length: bytes.length, width: bitmap?.width ?? null, height: bitmap?.height ?? null }
  }, name)
const filesNamed = (pattern) => page.evaluate((source) => [...window.devHost.files.keys()].filter((path) => new RegExp(source).test(path)), pattern)

try {
  await page.goto(`${origin}/dev.html?locale=zh-CN`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  const kit = page.frameLocator('#kit')
  const frame = await (await page.$('#kit')).contentFrame()
  const count = (testId) => kit.getByTestId(testId).count()
  const dialog = kit.getByTestId('photo-dialog')
  await page.waitForTimeout(1000)
  const size = { width: (await inspect()).details.width, height: (await inspect()).details.height }

  // The File menu shows what the development host can do: New, Save, Export As and Quick Export;
  // it cannot open a second document (no `open-file`), so Open is not there.
  await kit.getByTestId('photo-menu-File').click()
  const fileRows = { exportAs: await count('photo-file-export-as'), quick: await count('photo-file-quick-export'), save: await count('photo-file-save'), open: await count('photo-file-open'), placeRow: await count('photo-cmd-file.placeEmbedded'), upstreamExport: await count('photo-cmd-file.export.exportAs') }
  step('the File menu has New, Save, Export As, Quick Export and Place, and no Open where the host opens nothing', fileRows.exportAs === 1 && fileRows.quick === 1 && fileRows.save === 1 && fileRows.open === 0 && fileRows.placeRow === 1 && fileRows.upstreamExport === 0, JSON.stringify(fileRows))
  await shot('file-menu')

  // Export As: PNG at full size by default, with a preview of the result and its file size.
  await kit.getByTestId('photo-file-export-as').click()
  await dialog.waitFor()
  const preview = kit.getByTestId('photo-export-preview')
  await until(async () => (await preview.getAttribute('data-width')) === String(size.width))
  const sizeText = async () => (await kit.getByTestId('photo-export-size').textContent()) ?? ''
  await until(async () => /文件大小：[\d.,]+ (?:B|KB|MB)/.test(await sizeText()))
  step('Export As opens on PNG at the image’s size, previewing the file and its size', (await kit.getByTestId('photo-export-format').inputValue()) === 'png' && /文件大小/.test(await sizeText()), await sizeText())
  // JPEG, 300 px wide: the height follows, and the preview is the 300 x 200 file.
  await kit.getByTestId('photo-export-format').selectOption('jpg')
  await kit.getByTestId('photo-export-sizing').selectOption('pixels')
  await kit.getByTestId('photo-export-width').fill('300')
  const height = Math.round((300 * size.height) / size.width)
  await until(async () => (await kit.getByTestId('photo-export-height').inputValue()) === String(height))
  await until(async () => (await preview.getAttribute('data-width')) === '300' && (await preview.getAttribute('data-height')) === String(height))
  const jpegBytes = Number(await kit.getByTestId('photo-export-size').getAttribute('data-bytes'))
  step('JPEG at 300 px wide keeps the proportions and previews that file', (await kit.getByTestId('photo-export-quality').count()) === 1 && jpegBytes > 0, `${height} px high, ${jpegBytes} bytes`)
  await shot('export-dialog-jpeg')
  const before = await filesNamed('^Sample.*\\.jpg$')
  await kit.getByTestId('photo-dialog-ok').click()
  await dialog.waitFor({ state: 'detached' })
  await until(async () => (await filesNamed('^Sample.*\\.jpg$')).length > before.length)
  const jpegName = (await filesNamed('^Sample.*\\.jpg$')).find((name) => !before.includes(name))
  const jpeg = jpegName ? await written(jpegName) : null
  step('Export writes the JPEG beside the document, through the host, at 300 px wide', jpeg != null && jpeg.head[0] === 0xff && jpeg.head[1] === 0xd8 && jpeg.width === 300 && jpeg.height === height && jpeg.length === jpegBytes, JSON.stringify({ jpegName, ...jpeg, head: jpeg?.head.slice(0, 3) }))

  // Cmd+Alt+Shift+W opens it again with the last settings; WebP at half size.
  await kit.getByTestId('photo-workspace').focus()
  await page.keyboard.press(`${await primaryKey()}+Alt+Shift+KeyW`)
  await dialog.waitFor({ timeout: 10000 })
  const remembered = await kit.getByTestId('photo-export-format').inputValue()
  await kit.getByTestId('photo-export-format').selectOption('webp')
  await kit.getByTestId('photo-export-sizing').selectOption('scale')
  await kit.getByTestId('photo-export-scale').fill('50')
  await until(async () => (await preview.getAttribute('data-width')) === String(Math.round(size.width / 2)))
  await kit.getByTestId('photo-dialog-ok').click()
  await dialog.waitFor({ state: 'detached' })
  await until(async () => (await filesNamed('^Sample.*\\.webp$')).length > 0)
  const webp = await written((await filesNamed('^Sample.*\\.webp$'))[0])
  const riff = webp ? String.fromCharCode(...webp.head.slice(0, 4)) + String.fromCharCode(...webp.head.slice(8, 12)) : ''
  step('Cmd+Alt+Shift+W opens Export As with the last format; WebP at 50% writes a half-size WebP', remembered === 'jpg' && riff === 'RIFFWEBP' && (webp.width === null || webp.width === Math.round(size.width / 2)), JSON.stringify({ remembered, riff, width: webp?.width }))

  // Quick Export as PNG: the image's size, no dialog.
  const pngsBefore = await filesNamed('^Sample.*\\.png$')
  await kit.getByTestId('photo-menu-File').click()
  await kit.getByTestId('photo-file-quick-export').click()
  await until(async () => (await filesNamed('^Sample.*\\.png$')).length > pngsBefore.length)
  const pngName = (await filesNamed('^Sample.*\\.png$')).find((name) => !pngsBefore.includes(name))
  const png = pngName ? await written(pngName) : null
  step('Quick Export as PNG writes the whole image as a PNG', png != null && png.head[1] === 0x50 && png.width === size.width && png.height === size.height && (await dialog.count()) === 0, JSON.stringify({ pngName, width: png?.width }))

  // An image on the clipboard pastes as a new pixel layer, in the middle of the view, in one step.
  await kit.getByTestId('photo-workspace').focus()
  const layersBefore = await layerCount()
  const historyBefore = (await details()).history.length
  await frame.evaluate(async () => {
    const canvas = new OffscreenCanvas(40, 30)
    const context = canvas.getContext('2d')
    context.fillStyle = '#ff0000'
    context.fillRect(0, 0, 40, 30)
    const blob = await canvas.convertToBlob({ type: 'image/png' })
    const data = new DataTransfer()
    data.items.add(new File([blob], 'image.png', { type: 'image/png' }))
    const event = new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: data })
    if (!event.clipboardData) Object.defineProperty(event, 'clipboardData', { value: data })
    document.body.dispatchEvent(event)
  })
  await until(async () => (await layerCount()) === layersBefore + 1)
  const pasted = await activeLayer()
  const view = (await frame.evaluate(() => document.querySelector('[data-testid="photo-workspace"]').getAttribute('data-view'))).split(' ').map(Number)
  const box = await frame.evaluate(() => {
    const rect = document.querySelector('[data-testid="kit-canvas-surface"]').getBoundingClientRect()
    return { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
  })
  const middle = { x: view[1] + box.width / 2 / view[0], y: view[2] + box.height / 2 / view[0] }
  const pastedCenter = pasted?.bounds ? { x: pasted.bounds[0] + pasted.bounds[2] / 2, y: pasted.bounds[1] + pasted.bounds[3] / 2 } : null
  step(
    'an image on the clipboard pastes as a new pixel layer in the middle of the view, in one history step',
    pasted?.kind === 'Pixel' && pasted.name === '粘贴的图像' && pasted.bounds?.[2] === 40 && pasted.bounds?.[3] === 30 && Math.abs(pastedCenter.x - Math.min(Math.max(middle.x, 0), size.width)) <= 1.5 && (await details()).history.length === historyBefore + 1,
    JSON.stringify({ kind: pasted?.kind, name: pasted?.name, bounds: pasted?.bounds, middle, history: (await details()).history.slice(-2) }),
  )
  await shot('pasted')

  if (which === 'chromium') {
    // The real keys and the system clipboard (Chromium lets the test grant it): Cmd+C puts the
    // layer on it as a PNG; Cmd+V of another app's image pastes that image.
    await kit.getByTestId('photo-workspace').focus()
    await page.keyboard.press(`${await primaryKey()}+KeyC`)
    const copied = await until(async () =>
      frame.evaluate(async () => {
        const items = await navigator.clipboard.read()
        const item = items.find((entry) => entry.types.includes('image/png'))
        if (!item) return false
        const bitmap = await createImageBitmap(await item.getType('image/png'))
        return bitmap.width === 40 && bitmap.height === 30
      }),
    )
    step('Cmd+C puts the layer on the system clipboard as a PNG too', copied)
    await frame.evaluate(async () => {
      const canvas = new OffscreenCanvas(24, 16)
      const context = canvas.getContext('2d')
      context.fillStyle = '#0000ff'
      context.fillRect(0, 0, 24, 16)
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': await canvas.convertToBlob({ type: 'image/png' }) })])
    })
    const keyed = await layerCount()
    await kit.getByTestId('photo-workspace').focus()
    await page.keyboard.press(`${await primaryKey()}+KeyV`)
    await until(async () => (await layerCount()) === keyed + 1)
    const keyedLayer = await activeLayer()
    step('Cmd+V pastes the system clipboard’s image as a new layer', keyedLayer?.bounds?.[2] === 24 && keyedLayer?.bounds?.[3] === 16, JSON.stringify(keyedLayer?.bounds))
  }

  // Cmd+C then Cmd+V: the editor's own copy comes back where it was, as Photoshop's paste does
  // (in Chromium through the system clipboard, which holds it; elsewhere from the engine's).
  const copiedLayer = await activeLayer()
  await kit.getByTestId('photo-workspace').focus()
  await page.keyboard.press(`${await primaryKey()}+KeyC`)
  await page.waitForTimeout(300)
  const beforeRoundTrip = await layerCount()
  await kit.getByTestId('photo-workspace').focus()
  await page.keyboard.press(`${await primaryKey()}+KeyV`)
  await until(async () => (await layerCount()) === beforeRoundTrip + 1)
  const roundTrip = await activeLayer()
  step('Cmd+C then Cmd+V pastes the copy as a new layer in its place', roundTrip?.id !== copiedLayer?.id && JSON.stringify(roundTrip?.bounds) === JSON.stringify(copiedLayer?.bounds), JSON.stringify({ from: copiedLayer?.bounds, to: roundTrip?.bounds }))
  while ((await count('photo-error-close')) > 0) await kit.getByTestId('photo-error-close').first().click()

  // A file dragged from the computer: the canvas shows it is a drop target, and places the image
  // with its middle at the drop point.
  const dropAt = { x: box.left + box.width * 0.3, y: box.top + box.height * 0.35 }
  const viewNow = (await frame.evaluate(() => document.querySelector('[data-testid="photo-workspace"]').getAttribute('data-view'))).split(' ').map(Number)
  const expected = { x: viewNow[1] + (dropAt.x - box.left) / viewNow[0], y: viewNow[2] + (dropAt.y - box.top) / viewNow[0] }
  const drag = (type, at, files) =>
    frame.evaluate(
      async ({ type, at, files }) => {
        const data = new DataTransfer()
        for (const file of files) {
          let blob
          if (file.color) {
            const canvas = new OffscreenCanvas(file.width, file.height)
            const context = canvas.getContext('2d')
            context.fillStyle = file.color
            context.fillRect(0, 0, file.width, file.height)
            blob = await canvas.convertToBlob({ type: 'image/png' })
          } else blob = new Blob([file.text], { type: file.type })
          data.items.add(new File([blob], file.name, { type: file.type }))
        }
        const target = document.elementFromPoint(at.x, at.y) ?? document.body
        const event = new DragEvent(type, { bubbles: true, cancelable: true, clientX: at.x, clientY: at.y, dataTransfer: data })
        if (!event.dataTransfer) Object.defineProperty(event, 'dataTransfer', { value: data })
        target.dispatchEvent(event)
        return event.defaultPrevented
      },
      { type, at, files },
    )
  const logo = [{ name: 'Logo.png', type: 'image/png', color: '#00aa00', width: 20, height: 20 }]
  const allowed = await drag('dragover', dropAt, logo)
  const marked = await kit.getByTestId('photo-workspace').getAttribute('data-drop')
  await shot('drop-target')
  step('a file dragged over the canvas marks it as the drop target', allowed && marked === 'true', JSON.stringify({ allowed, marked }))
  const beforeDrop = await layerCount()
  await drag('drop', dropAt, logo)
  await until(async () => (await layerCount()) === beforeDrop + 1)
  const placed = await activeLayer()
  const placedCenter = placed?.bounds ? { x: placed.bounds[0] + placed.bounds[2] / 2, y: placed.bounds[1] + placed.bounds[3] / 2 } : null
  step(
    'a dropped image is placed as a layer with its middle at the drop point',
    placed?.name === 'Logo' && placed?.bounds?.[2] === 20 && Math.abs(placedCenter.x - expected.x) <= 1.5 && Math.abs(placedCenter.y - expected.y) <= 1.5 && (await kit.getByTestId('photo-workspace').getAttribute('data-drop')) === null,
    JSON.stringify({ name: placed?.name, kind: placed?.kind, bounds: placed?.bounds, expected }),
  )
  // On the Layers panel: placed in the middle of the image.
  const panel = await frame.evaluate(() => {
    const rect = document.querySelector('[data-testid="photo-kit-layers"]').getBoundingClientRect()
    return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }
  })
  const badge = [{ name: 'Badge.png', type: 'image/png', color: '#ffaa00', width: 10, height: 10 }]
  await drag('dragover', panel, badge)
  const panelMarked = await kit.getByTestId('photo-kit-layers').getAttribute('data-drop')
  const beforePanel = await layerCount()
  await drag('drop', panel, badge)
  await until(async () => (await layerCount()) === beforePanel + 1)
  const badgeLayer = await activeLayer()
  step('a file dropped on the Layers panel is placed in the middle of the image', panelMarked === 'true' && badgeLayer?.name === 'Badge' && Math.abs(badgeLayer.bounds[0] + 5 - size.width / 2) <= 1 && Math.abs(badgeLayer.bounds[1] + 5 - size.height / 2) <= 1, JSON.stringify({ panelMarked, bounds: badgeLayer?.bounds }))

  // An induced engine error (a dropped "PNG" that is not an image) shows in the editor's words, not the engine's.
  const beforeBroken = await layerCount()
  await drag('drop', dropAt, [{ name: 'broken.png', type: 'image/png', text: 'this is not an image' }])
  await kit.getByTestId('photo-error').first().waitFor({ timeout: 10000 })
  const message = await kit.getByTestId('photo-error-message').first().textContent()
  const card = await kit.getByTestId('photo-error').first().textContent()
  await shot('error-translated')
  step('an engine error shows translated, without command ids or the engine’s English', message === '此文件不是 Kition Photo 能读取的图像。' && !/file\.placeEmbedded|`|invalid parameters|decode/i.test(card) && (await layerCount()) === beforeBroken, card)
  await kit.getByTestId('photo-error-close').first().click()
  // A file that is not an image at all is refused by name, before the engine sees it.
  await drag('drop', dropAt, [{ name: 'notes.txt', type: 'text/plain', text: 'hello' }])
  await until(async () => /notes\.txt/.test((await kit.getByTestId('photo-error-message').first().textContent().catch(() => '')) ?? ''))
  step('a dropped file that is not an image is refused by name', /notes\.txt 不是 Kition Photo 能置入的图像/.test(await kit.getByTestId('photo-error-message').first().textContent()) && (await layerCount()) === beforeBroken)
  while ((await count('photo-error-close')) > 0) await kit.getByTestId('photo-error-close').first().click()

  // A long command (Surface Blur at a radius of about 60 again, by Last Filter's key) shows the progress bar
  // after half a second; the editor keeps answering meanwhile; Cancel leaves the image as it was.
  const background = flat((await details()).layers).find((layer) => layer.name === 'Background' && layer.kind === 'Pixel')
  await kit.getByTestId(`photo-layer-${background.id}`).click()
  await until(async () => (await details()).activeLayer === background.id)
  await kit.getByTestId('photo-menu-Filter').click()
  await kit.locator('[data-kit-ui-id="menu:Filter/Blur"]').first().hover()
  await kit.getByTestId('photo-cmd-filter.blur.surfaceBlur').click()
  await dialog.waitFor()
  const track = await kit.getByTestId('photo-dialog-param-radius-track').boundingBox()
  await page.mouse.click(track.x + track.width * 0.65, track.y + track.height / 2)
  const radiusSet = Number(await kit.getByTestId('photo-dialog-param-radius-track').getAttribute('aria-valuenow'))
  await page.waitForTimeout(400)
  await kit.getByTestId('photo-dialog-ok').click()
  await dialog.waitFor({ state: 'detached', timeout: 60000 })
  await page.waitForTimeout(300)
  const beforeLong = await details()
  await kit.getByTestId('photo-workspace').focus()
  await page.keyboard.press(`${await primaryKey()}+Alt+KeyF`)
  const shown = await until(async () => (await count('photo-busy')) === 1, 50)
  const label = shown ? await kit.getByTestId('photo-busy').getAttribute('data-label') : null
  await shot('busy')
  // The editor answers while the engine works: the File menu opens.
  await kit.getByTestId('photo-menu-File').click()
  const answering = (await count('photo-file-export-as')) === 1
  await page.keyboard.press('Escape')
  if (shown) await kit.getByTestId('photo-busy-cancel').click()
  const gone = await until(async () => (await count('photo-busy')) === 0, 50)
  const restored = await until(async () => JSON.stringify((await details()).history) === JSON.stringify(beforeLong.history) && (await details()).historyIndex === beforeLong.historyIndex, 200)
  step('a long command shows its progress with Cancel, the editor keeps answering, and Cancel leaves the image as it was', shown && label === '上次滤镜' && answering && gone && restored, JSON.stringify({ radiusSet, shown, label, answering, gone, restored, history: (await details()).history.slice(-2) }))

  // Where the host opens files (`open-file`, dev.html?open=1), File › Open… is there: the host's
  // picker, then `host/open` with the file picked.
  await page.goto(`${origin}/dev.html?locale=zh-CN&open=1`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  await page.waitForTimeout(800)
  await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(8, 8)
    canvas.getContext('2d').fillRect(0, 0, 8, 8)
    const bytes = await (await canvas.convertToBlob({ type: 'image/png' })).arrayBuffer()
    window.devHost.files.set('Other.png', { bytes, version: 1 })
  })
  await kit.getByTestId('photo-menu-File').click()
  const openRow = await count('photo-file-open')
  if (openRow) await kit.getByTestId('photo-file-open').click()
  await until(async () => (await page.evaluate(() => window.devHost.opens.length)) > 0, 50)
  const opened = await page.evaluate(() => window.devHost.opens.map((request) => request.params?.link?.path ?? null))
  step('where the host opens files, File › Open… picks one through the host and asks the host to open it', openRow === 1 && opened.length === 1 && /Other\.png$/.test(opened[0] ?? ''), JSON.stringify({ openRow, opened }))

  step('no page errors', errors.length === 0, errors.join(' | '))
} catch (error) {
  step('the IO flow ran to its end', false, error instanceof Error ? error.stack?.split('\n').slice(0, 3).join(' ') : String(error))
} finally {
  if (failures) await context.tracing.stop({ path: `test-results/io-${which}-trace.zip` }).catch(() => undefined)
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} failed` : 'all passed')
process.exit(failures ? 1 : 0)
