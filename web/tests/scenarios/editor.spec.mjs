import { test as base, expect } from '@playwright/test'
import { createFixture, sdk, inspect as engineInspect, execute as engineExecute, structure, loadFont } from './engine.mjs'

// Every test gets a new browser context and document. Agent calls only set up fixtures or read
// state; the actions under test are real editor clicks, typing and pointer gestures.
const test = base.extend({
  editor: async ({ page, context }, use, testInfo) => {
    const errors = []
    const watch = (target) => target.on('pageerror', (error) => errors.push(error.message))
    watch(page)
    context.on('page', watch)
    const editor = makeEditor(page)
    await use(editor)
    if (testInfo.status !== testInfo.expectedStatus) {
      await testInfo.attach('document.json', { body: JSON.stringify(await editor.state().catch(() => null), null, 2), contentType: 'application/json' })
    }
    expect(errors, 'Uncaught browser errors').toEqual([])
  },
})

// The editor takes Cmd or Ctrl from navigator.platform (kit-editor's isMac), while Playwright's
// ControlOrMeta follows the operating system running the test, and the two can disagree (WebKit
// on Linux). Shortcuts press the modifier the editor reads.
const primaryKeys = new WeakMap()
async function shortcut(page, keys) {
  if (!primaryKeys.has(page)) primaryKeys.set(page, await page.evaluate(() => /Mac|iPhone|iPad/i.test(navigator.platform)) ? 'Meta' : 'Control')
  return keys.replace('ControlOrMeta', primaryKeys.get(page))
}

function makeEditor(page) {
  const kit = page.frameLocator('#kit')
  const state = () => page.evaluate(async () => (await window.devHost.connection().ai.inspect()).details)
  const run = async (command, params = {}) => {
    const answer = await page.evaluate(([command, params]) => window.devHost.connection().ai.run([{ command, params }], false), [command, params])
    expect(answer.completed, JSON.stringify(answer)).toHaveLength(1)
    return answer.completed[0].result
  }
  const ready = async (name) => {
    await expect(page.locator('#status')).toContainText(`${name} open in`, { timeout: 60_000 })
    await expect(kit.getByTestId('photo-save')).toBeVisible()
  }
  // Pixel assertions use document coordinates at 100%; every host reopen starts at fit zoom.
  const actualSize = async () => {
    const zoom = kit.getByTestId('photo-status-zoom').locator('input')
    await zoom.fill('100')
    await zoom.press('Enter')
    await expect(zoom).toHaveValue('100')
  }
  const open = async (name = 'Scenario.psd', commands = [], locale = 'en-US', mode) => {
    await page.goto(`/dev.html?locale=${locale}${mode ? `&modes=${mode}` : ''}`)
    await ready('Sample.psd')
    const bytes = createFixture(name, commands)
    await page.locator('#file').setInputFiles({ name, mimeType: 'image/vnd.adobe.photoshop', buffer: bytes })
    await ready(name)
    await actualSize()
    await shortcut(page, '')
  }
  const point = async (x, y) => {
    const box = await kit.getByTestId('kit-canvas-surface').boundingBox()
    const doc = await state()
    return { x: box.x + box.width / 2 + x - doc.width / 2, y: box.y + box.height / 2 + y - doc.height / 2 }
  }
  const drag = async (from, to) => {
    const a = await point(...from), b = await point(...to)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 12 })
    await page.mouse.up()
  }
  const click = async (x, y) => { const p = await point(x, y); await page.mouse.click(p.x, p.y) }
  // A release can carry a position newer than the last move (e.g. sparse/coalesced input).
  // Press through the real mouse, then deliver that release to the actual canvas handler.
  const sparseDrag = async (from, to) => {
    const start = await point(...from)
    await page.mouse.move(start.x, start.y)
    await page.mouse.down()
    const doc = await state()
    await kit.getByTestId('kit-canvas-surface').evaluate((canvas, { to, width, height }) => {
      const box = canvas.getBoundingClientRect()
      canvas.dispatchEvent(new PointerEvent('pointerup', {
        bubbles: true, pointerId: 1, pointerType: 'mouse', button: 0, buttons: 0,
        clientX: box.left + box.width / 2 + to[0] - width / 2,
        clientY: box.top + box.height / 2 + to[1] - height / 2,
      }))
    }, { to, width: doc.width, height: doc.height })
    await page.mouse.up()
  }
  const sample = async (x, y) => (await run('document.sampleColor', { x, y, size: 'point', sampleLayer: 'all' })).map((value) => Math.round(value * 255))
  const color = async (x, y, rgba) => {
    // Check the display first: sampling via an agent command can trigger a full repaint.
    await expect.poll(async () => {
      const doc = await state()
      return kit.getByTestId('kit-canvas-surface').evaluate((canvas, { x, y, width, height, rgba }) => {
        const actual = canvas.getContext('2d').getImageData(Math.floor(canvas.width / 2 + x - width / 2), Math.floor(canvas.height / 2 + y - height / 2), 1, 1).data
        return [...actual].every((value, i) => Math.abs(value - rgba[i]) <= 1)
      }, { x, y, width: doc.width, height: doc.height, rgba })
    }, { message: `Displayed canvas pixel (${x},${y}) should be ${rgba}` }).toBe(true)
    await expect.poll(async () => {
      const actual = await sample(x, y)
      return actual.every((value, i) => Math.abs(value - rgba[i]) <= 1)
    }, { message: `Document pixel (${x},${y}) should be ${rgba}` }).toBe(true)
  }
  const saved = async (name = 'Scenario.psd') => {
    const version = await page.evaluate((name) => window.devHost.files.get(name).version, name)
    await kit.getByTestId('photo-save').click()
    await expect.poll(() => page.evaluate((name) => window.devHost.files.get(name).version, name)).toBeGreaterThan(version)
    await expect.poll(() => page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
    return Buffer.from(await page.evaluate((name) => [...new Uint8Array(window.devHost.files.get(name).bytes)], name))
  }
  const reopen = async (name = 'Scenario.psd', savedBytes) => {
    const before = structure(await state())
    const bytes = savedBytes ?? await saved(name)
    const decoded = sdk.PhotoDocument.open(name, bytes)
    try { expect(structure(engineInspect(decoded))).toEqual(before) } finally { decoded.free() }
    await page.locator('#file').setInputFiles({ name, mimeType: 'image/vnd.adobe.photoshop', buffer: bytes })
    await ready(name)
    await actualSize()
    expect(structure(await state())).toEqual(before)
  }
  const undo = async () => page.keyboard.press(await shortcut(page, 'ControlOrMeta+z'))
  const redo = async () => page.keyboard.press(await shortcut(page, 'ControlOrMeta+Shift+z'))
  return { page, kit, state, run, open, drag, sparseDrag, click, sample, color, saved, reopen, undo, redo }
}

const layer = (name = 'Artwork') => ({ command: 'layer.new.layer', params: { name } })
const fill = (color) => ({ command: 'edit.fill', params: { color } })
const shape = { command: 'shape.create', params: { kind: 'rect', rect: [16, 16, 96, 64], fill: '#ff0000', name: 'Banner' } }

test('UI01 / SC05: draw, undo, redo and save a brush stroke', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer('Ink')])
  await e.kit.getByTestId('photo-tool-Brush').click()
  await e.kit.getByTestId('photo-foreground').fill('#000000')
  await e.drag([24, 48], [100, 48])
  await e.color(64, 48, [0, 0, 0, 255])
  await e.color(64, 5, [255, 255, 255, 255])
  await e.undo()
  await e.color(64, 48, [255, 255, 255, 255])
  await e.redo()
  await e.color(64, 48, [0, 0, 0, 255])
  await e.reopen()
})

test('UI02 / SC03: marquee constrains painting and leaves outside untouched', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer()])
  await e.kit.getByTestId('photo-tool-RectMarquee').click()
  await e.drag([32, 24], [96, 72])
  await expect.poll(async () => (await e.state()).selectionBounds).toEqual([32, 24, 64, 48])
  await e.kit.getByTestId('photo-tool-Brush').click()
  await e.kit.getByTestId('photo-foreground').fill('#ff0000')
  await e.drag([10, 48], [118, 48])
  await e.color(64, 48, [255, 0, 0, 255])
  await e.color(20, 48, [255, 255, 255, 255])
  await e.color(108, 48, [255, 255, 255, 255])
  await e.reopen()
})

test('UI03 / SC04: rename, blend, opacity and mask survive save/reopen', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer(), fill('#ff0000')])
  const id = (await e.state()).activeLayer
  await e.kit.getByTestId(`photo-layer-${id}`).dblclick()
  await e.kit.getByTestId('photo-layer-rename').fill('Red overlay')
  await e.kit.getByTestId('photo-layer-rename').press('Enter')
  await e.kit.getByTestId('photo-layers-blend').selectOption('Multiply')
  const opacity = e.kit.locator('[data-kit-ui-id="control:layers.opacity"] input')
  await opacity.fill('40')
  await opacity.press('Enter')
  await e.color(64, 48, [255, 153, 153, 255])
  await e.kit.getByTestId('photo-layers-mask').click()
  await expect.poll(async () => (await e.state()).layers.find((item) => item.id === id)?.hasMask).toBe(true)
  await e.reopen()
  expect((await e.state()).layers[0]).toMatchObject({ name: 'Red overlay', blend: 'Multiply', hasMask: true })
})

test('UI04 / SC02: cancel crop, apply square crop, undo and save', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer(), fill('#204060')])
  const before = await e.state()
  await e.kit.getByTestId('photo-tool-Crop').click()
  await e.kit.getByTestId('photo-option-ratio').selectOption('1:1')
  await e.drag([24, 24], [72, 60])
  await expect(e.kit.getByTestId('photo-crop-apply')).toBeEnabled()
  await e.page.keyboard.press('Escape')
  await expect(e.kit.getByTestId('photo-crop-apply')).toBeDisabled()
  expect((await e.state()).revision).toBe(before.revision)
  await e.drag([24, 24], [72, 60])
  await e.kit.getByTestId('photo-crop-apply').click()
  await expect.poll(async () => (await e.state()).width).toBeLessThan(128)
  const cropped = await e.state()
  expect(cropped.width).toBe(cropped.height)
  await e.undo()
  await expect.poll(async () => (await e.state()).width).toBe(128)
  expect((await e.state()).height).toBe(96)
  await e.reopen()
})

test('UI05 / SC01, SC06: compose a poster with Chinese editable text', async ({ editor: e, browserName }) => {
  loadFont()
  await e.open('Scenario.psd', [shape], 'zh-CN')
  await e.kit.getByTestId('photo-tool-Type').click()
  await e.click(24, 50)
  const field = e.kit.getByTestId('photo-text-field')
  await expect(field).toBeVisible()
  if (browserName === 'chromium') {
    const cdp = await e.page.context().newCDPSession(e.page)
    await cdp.send('Input.imeSetComposition', { text: 'ji', selectionStart: 2, selectionEnd: 2 })
    await cdp.send('Input.insertText', { text: '极光' })
  } else await field.fill('极光')
  await field.press('Enter')
  await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
  expect(await e.run('type.info')).toMatchObject({ text: '极光' })
  await e.reopen()
  const typeLayer = (await e.state()).layers.find((item) => item.kind === 'Type')
  await e.kit.getByTestId(`photo-layer-${typeLayer.id}`).click()
  expect(await e.run('type.info')).toMatchObject({ text: '极光' })
})

test('UI06 / SC08: rasterization cancel is lossless, confirm paints, undo restores shape', async ({ editor: e }) => {
  await e.open('Scenario.psd', [shape])
  const before = await e.state()
  await e.kit.getByTestId('photo-tool-Brush').click()
  await e.kit.getByTestId('photo-foreground').fill('#000000')
  await e.click(64, 48)
  await expect(e.kit.getByTestId('photo-rasterize-question')).toBeVisible()
  await e.kit.getByTestId('photo-rasterize-cancel').click()
  await expect(e.kit.getByTestId('photo-rasterize-question')).toHaveCount(0)
  expect((await e.state()).revision).toBe(before.revision)
  await e.color(64, 48, [255, 0, 0, 255])
  await e.click(64, 48)
  await e.kit.getByTestId('photo-rasterize-confirm').click()
  await e.color(64, 48, [0, 0, 0, 255])
  expect((await e.state()).layers[0].kind).toBe('Pixel')
  await e.undo()
  await e.color(64, 48, [255, 0, 0, 255])
  await e.undo()
  await expect.poll(async () => (await e.state()).layers[0].kind).toBe('Shape')
  await e.reopen()
})

test('UI07 / SC11: two open tabs keep documents and undo histories isolated', async ({ editor: a, context }) => {
  await a.open('A.psd', [layer('A ink')])
  const pageB = await context.newPage()
  const b = makeEditor(pageB)
  await b.open('B.psd', [layer('B ink'), fill('#0000ff')])
  const originalB = await b.state()
  await a.kit.getByTestId('photo-tool-Brush').click()
  await a.kit.getByTestId('photo-foreground').fill('#ff0000')
  await a.drag([24, 48], [100, 48])
  await a.color(64, 48, [255, 0, 0, 255])
  expect(await b.state()).toEqual(originalB)
  await b.color(64, 48, [0, 0, 255, 255])
  await a.undo()
  await a.color(64, 48, [255, 255, 255, 255])
  expect(await b.state()).toEqual(originalB)
  await b.kit.getByTestId(`photo-layer-${originalB.activeLayer}`).dblclick()
  await b.kit.getByTestId('photo-layer-rename').fill('B saved')
  await b.kit.getByTestId('photo-layer-rename').press('Enter')
  await expect.poll(async () => (await b.state()).layers[0].name).toBe('B saved')
  await b.reopen('B.psd')
  await a.kit.getByRole('button', { name: 'Redo', exact: true }).click()
  await a.color(64, 48, [255, 0, 0, 255])
  await a.reopen('A.psd')
  await pageB.close()
})

for (const [key, isComposing, keyCode] of [['Enter', true, 13], ['Escape', true, 27], ['Enter', false, 229]]) {
  test(`UI08: IME ${key} (composing=${isComposing}, code=${keyCode}) keeps the text draft`, async ({ editor: e }) => {
    await e.open()
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('ji')
    // Reproduce the browser events while accepting/cancelling an IME candidate, including
    // WebKit's legacy 229 signal when compositionend precedes the confirming keydown.
    await field.dispatchEvent('keydown', { key, isComposing, keyCode, bubbles: true })
    await expect(field).toBeVisible()
    expect((await e.state()).layers.filter((item) => item.kind === 'Type')).toHaveLength(0)
    await field.fill('极光')
    await field.press('Enter')
    await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
    expect(await e.run('type.info')).toMatchObject({ text: '极光' })
    await e.reopen()
  })
}

test('UI09: a fixed-size marquee click selects the configured rectangle', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer()])
  await e.kit.getByTestId('photo-tool-RectMarquee').click()
  await e.kit.getByTestId('photo-option-style').selectOption('fixedSize')
  for (const [name, value] of [['width', '40'], ['height', '24']]) {
    const input = e.kit.getByTestId(`photo-option-${name}`).locator('input')
    await input.fill(value)
    await input.press('Enter')
  }
  await e.click(32, 24)
  await expect.poll(async () => (await e.state()).selectionBounds).toEqual([32, 24, 40, 24])
  await e.kit.getByTestId('photo-tool-Brush').click()
  await e.kit.getByTestId('photo-foreground').fill('#ff0000')
  await e.drag([10, 36], [118, 36])
  await e.color(52, 36, [255, 0, 0, 255])
  await e.color(20, 36, [255, 255, 255, 255])
  await e.color(90, 36, [255, 255, 255, 255])
  await e.reopen()
})

for (const commit of ['Enter', 'toolbar']) {
  test(`UI10: committing text with ${commit} keeps immediate keyboard undo available`, async ({ editor: e }) => {
    await e.open()
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('Undo this title')
    if (commit === 'Enter') await field.press('Enter')
    else await e.kit.getByTestId('photo-option-commit').click()
    await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
    await e.undo()
    await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(0)
    await e.redo()
    await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
    await e.reopen()
  })
}

test('UI11: clicking another field commits text without stealing its focus', async ({ editor: e }) => {
  await e.open()
  await e.kit.getByTestId('photo-tool-Type').click()
  await e.click(24, 50)
  await e.kit.getByTestId('photo-text-field').fill('Keep the focus')
  const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
  await zoom.click()
  await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
  await expect(zoom).toBeFocused()
  await zoom.fill('75')
  await zoom.press('Enter')
  await expect(zoom).toHaveValue('75')
  await e.reopen()
})

test('UI12: cancelling text discards the draft and keeps tool shortcuts available', async ({ editor: e }) => {
  await e.open()
  const before = await e.state()
  await e.kit.getByTestId('photo-tool-Type').click()
  await e.click(24, 50)
  const field = e.kit.getByTestId('photo-text-field')
  await field.fill('Discard this title')
  await field.press('Escape')
  await expect(field).toHaveCount(0)
  await e.page.keyboard.press('b')
  await expect(e.kit.getByTestId('photo-tool-Brush')).toHaveAttribute('aria-pressed', 'true')
  expect((await e.state()).revision).toBe(before.revision)
  expect((await e.state()).layers.filter((item) => item.kind === 'Type')).toHaveLength(0)
})

test('UI13: thick shape strokes repaint beyond the dragged rectangle', async ({ editor: e }) => {
  await e.open()
  await e.kit.getByTestId('photo-tool-Rectangle').click()
  await e.kit.getByTestId('photo-foreground').fill('#ff0000')
  await e.kit.getByTestId('photo-background').fill('#000000')
  const stroke = e.kit.getByTestId('photo-option-stroke').locator('input')
  await stroke.fill('20')
  await stroke.press('Enter')
  await e.drag([40, 30], [80, 60])
  await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Shape').length).toBe(1)
  await e.color(34, 45, [0, 0, 0, 255])
  await e.color(86, 45, [0, 0, 0, 255])
  await e.color(60, 45, [255, 0, 0, 255])
  await e.undo()
  await e.color(34, 45, [255, 255, 255, 255])
  await e.redo()
  await e.color(34, 45, [0, 0, 0, 255])
  await e.reopen()
  await e.color(34, 45, [0, 0, 0, 255])
  await e.color(60, 45, [255, 0, 0, 255])
})

test('UI14: a brush stroke includes the final release position', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer('Ink')])
  await e.kit.getByTestId('photo-tool-Brush').click()
  await e.kit.getByTestId('photo-foreground').fill('#000000')
  const smoothing = e.kit.getByTestId('photo-option-smoothing').locator('input')
  await smoothing.fill('0')
  await smoothing.press('Enter')
  await e.sparseDrag([24, 48], [100, 48])
  await e.color(64, 48, [0, 0, 0, 255])
  await e.color(96, 48, [0, 0, 0, 255])
  await e.undo()
  await e.color(64, 48, [255, 255, 255, 255])
  await e.redo()
  await e.color(64, 48, [0, 0, 0, 255])
  await e.reopen()
  await e.color(64, 48, [0, 0, 0, 255])
  await e.color(96, 48, [0, 0, 0, 255])
})

test('UI15: moving a layer uses the final release position', async ({ editor: e }) => {
  await e.open('Scenario.psd', [{ command: 'shape.create', params: { kind: 'rect', rect: [24, 24, 24, 24], fill: '#ff0000' } }])
  await e.kit.getByTestId('photo-tool-Move').click()
  await e.sparseDrag([32, 32], [64, 48])
  await e.color(36, 36, [255, 255, 255, 255])
  await e.color(68, 52, [255, 0, 0, 255])
  await e.undo()
  await e.color(36, 36, [255, 0, 0, 255])
  await e.redo()
  await e.color(68, 52, [255, 0, 0, 255])
  await e.reopen()
  await e.color(36, 36, [255, 255, 255, 255])
  await e.color(68, 52, [255, 0, 0, 255])
})

for (const linked of [false, true]) {
  test(`UI16: moving ${linked ? 'linked' : 'selected'} layers repaints every moved layer`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [
      { command: 'shape.create', params: { kind: 'rect', rect: [16, 16, 16, 16], fill: '#0000ff', name: 'Blue' } },
      { command: 'shape.create', params: { kind: 'rect', rect: [80, 16, 16, 16], fill: '#ff0000', name: 'Red' } },
    ])
    const doc = await e.state()
    const blue = doc.layers.find((item) => item.name === 'Blue').id
    const red = doc.layers.find((item) => item.name === 'Red').id
    await e.kit.getByTestId(`photo-layer-${blue}`).click()
    await e.kit.getByTestId(`photo-layer-${red}`).click({ modifiers: [await shortcut(e.page, 'ControlOrMeta')] })
    if (linked) {
      await e.kit.getByTestId('photo-layers-link').click()
      await e.kit.getByTestId(`photo-layer-${red}`).click()
    }
    await e.kit.getByTestId('photo-tool-Move').click()
    await e.kit.getByTestId('photo-option-autoSelect').uncheck()
    await e.drag([88, 24], [88, 56])
    // Observe the non-active layer first, before any sampling command can repaint it.
    await e.color(24, 24, [255, 255, 255, 255])
    await e.color(24, 56, [0, 0, 255, 255])
    await e.color(88, 24, [255, 255, 255, 255])
    await e.color(88, 56, [255, 0, 0, 255])
    await e.undo()
    await e.color(24, 24, [0, 0, 255, 255])
    await e.color(88, 24, [255, 0, 0, 255])
    await e.redo()
    await e.color(24, 56, [0, 0, 255, 255])
    await e.reopen()
    await e.color(24, 24, [255, 255, 255, 255])
    await e.color(24, 56, [0, 0, 255, 255])
    await e.color(88, 56, [255, 0, 0, 255])
  })
}

for (const grouped of [false, true]) {
  test(`UI17: painting with ${grouped ? 'group' : 'layer'} styles repaints the outer stroke`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [
      layer('Styled ink'),
      ...(grouped ? [{ command: 'layer.new.groupFromLayers', params: { name: 'Styled group' } }] : []),
    ])
    await e.kit.getByTestId('photo-tool-Brush').click()
    // Apply style after the tool exists; document styles must not be captured by a stale tool.
    await e.run('layer.layerStyle.stroke', { size: 12, position: 'outside', color: '#0000ff', opacity: 100, blend: 'Normal' })
    if (grouped) {
      const doc = await e.state()
      const ink = doc.layers.flatMap((item) => [item, ...(item.children ?? [])]).find((item) => item.name === 'Styled ink')
      await e.kit.getByTestId(`photo-layer-${ink.id}`).click()
    }
    await e.kit.getByTestId('photo-foreground').fill('#ff0000')
    const smoothing = e.kit.getByTestId('photo-option-smoothing').locator('input')
    await smoothing.fill('0')
    await smoothing.press('Enter')
    await e.drag([48, 48], [80, 48])
    await e.color(64, 32, [0, 0, 255, 255])
    await e.color(64, 48, [255, 0, 0, 255])
    await e.color(64, 15, [255, 255, 255, 255])
    await e.undo()
    await e.color(64, 32, [255, 255, 255, 255])
    await e.redo()
    await e.color(64, 32, [0, 0, 255, 255])
    await e.reopen()
    await e.color(64, 32, [0, 0, 255, 255])
    await e.color(64, 48, [255, 0, 0, 255])
  })
}

for (const tool of ['RectMarquee', 'EllipseMarquee']) {
  for (const style of ['normal', 'fixedRatio']) {
    test(`UI18: ${tool} ${style} click at 200% clears selection before painting`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer()])
      if (tool === 'EllipseMarquee') await e.kit.getByTestId('photo-tool-RectMarquee').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.kit.getByTestId('photo-option-style').selectOption(style)
      await e.drag([32, 24], [80, 72])
      await expect.poll(async () => (await e.state()).selectionBounds).not.toBeNull()
      const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
      await zoom.fill('200')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('200')
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      // Center + one screen pixel maps to a fractional document point at 200%.
      await e.page.mouse.click(box.x + box.width / 2 + 1, box.y + box.height / 2 + 1)
      await expect.poll(async () => (await e.state()).selectionBounds).toBeNull()
      await zoom.fill('100')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('100')
      await e.kit.getByTestId('photo-tool-Brush').click()
      await e.kit.getByTestId('photo-foreground').fill('#ff0000')
      await e.drag([16, 48], [112, 48])
      await e.color(24, 48, [255, 0, 0, 255])
      await e.color(100, 48, [255, 0, 0, 255])
      await e.undo()
      await e.color(24, 48, [255, 255, 255, 255])
      await e.redo()
      await e.reopen()
      await e.color(100, 48, [255, 0, 0, 255])
    })
  }
}

test('UI19: scrubby zoom uses a newer release position', async ({ editor: e }) => {
  await e.open()
  const before = await e.state()
  await e.kit.getByTestId('photo-tool-Zoom').click()
  await e.kit.getByTestId('photo-option-scrubbyZoom').check()
  await e.sparseDrag([14, 48], [114, 48])
  await expect(e.kit.getByTestId('photo-status-zoom').locator('input')).toHaveValue('200')
  expect((await e.state()).revision).toBe(before.revision)
})

test('UI20: type properties show the Character heading without scrolling', async ({ editor: e }) => {
  await e.open('Scenario.psd', [], 'zh-CN')
  await e.kit.getByTestId('photo-tool-Type').click()
  await e.click(24, 48)
  const field = e.kit.getByTestId('photo-text-field')
  await field.fill('AB')
  await field.press('Enter')
  await expect.poll(async () => (await e.state()).layers.some((layer) => layer.kind === 'Type')).toBe(true)
  await e.kit.getByTestId('photo-tool-Brush').click()
  await e.kit.getByTestId('photo-menu-Window').click()
  await e.kit.getByTestId('photo-control-window.panel.color').click()
  await expect(e.kit.getByTestId('photo-dock-area-tab-color')).toHaveCount(0)
  const properties = e.kit.getByTestId('photo-properties')
  await expect.poll(() => properties.evaluate((element) => {
    const head = element.querySelector('[data-testid="photo-props-type-character-head"]')
    if (!head) return null
    const panelBox = element.getBoundingClientRect(), heading = head.getBoundingClientRect()
    return { scrollTop: element.scrollTop, clippedAbove: Math.max(0, panelBox.top - heading.top), clippedBelow: Math.max(0, heading.bottom - panelBox.bottom) }
  }), { message: 'Character heading should be fully visible in unscrolled type properties' }).toEqual({ scrollTop: 0, clippedAbove: 0, clippedBelow: 0 })
})

for (const tool of ['RectMarquee', 'EllipseMarquee']) {
  for (const gesture of ['click', 'reverse drag']) {
    test(`UI21: ${tool} fixed-size ${gesture} at 200% keeps exactly 40 by 24 pixels`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer()])
      if (tool === 'EllipseMarquee') await e.kit.getByTestId('photo-tool-RectMarquee').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.kit.getByTestId('photo-option-style').selectOption('fixedSize')
      for (const [name, value] of [['width', '40'], ['height', '24']]) {
        const field = e.kit.getByTestId(`photo-option-${name}`).locator('input')
        await field.fill(value)
        await field.press('Enter')
      }
      if (tool === 'EllipseMarquee') await e.kit.getByTestId('photo-option-antiAlias').uncheck()
      const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
      await zoom.fill('200')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('200')
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      const x = box.x + box.width / 2 + 1, y = box.y + box.height / 2 + 1
      // This screen position is (64.5, 48.5) in the document. Size must not grow on rounding.
      await e.page.mouse.move(x, y)
      await e.page.mouse.down()
      if (gesture === 'reverse drag') await e.page.mouse.move(x - 10, y - 10, { steps: 4 })
      await e.page.mouse.up()
      const [left, top] = gesture === 'click' ? [64, 48] : [24, 24]
      await expect.poll(async () => (await e.state()).selectionBounds).toEqual([left, top, 40, 24])
      await zoom.fill('100')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('100')
      await e.kit.getByTestId('photo-tool-Brush').click()
      await e.kit.getByTestId('photo-foreground').fill('#ff0000')
      await e.drag([left - 8, top + 12], [left + 48, top + 12])
      await e.color(left + 40, top + 12, [255, 255, 255, 255])
      await e.color(left + 20, top + 12, [255, 0, 0, 255])
      await e.color(left - 1, top + 12, [255, 255, 255, 255])
      await e.undo()
      await e.color(left + 20, top + 12, [255, 255, 255, 255])
      await e.redo()
      await e.reopen()
      await e.color(left + 40, top + 12, [255, 255, 255, 255])
      await e.color(left + 20, top + 12, [255, 0, 0, 255])
    })
  }
}


for (const tool of ['RectMarquee', 'EllipseMarquee']) {
  for (const constraint of [
    { name: 'fixed 1:1', style: 'fixedRatio', ratio: [1, 1], sx: 1, sy: 1, size: [18, 18] },
    { name: 'fixed 3:2 reverse', style: 'fixedRatio', ratio: [3, 2], sx: -1, sy: -1, size: [18, 12] },
    { name: 'Shift square', style: 'normal', sx: -1, sy: 1, size: [18, 18] },
  ]) {
    test(`UI22: ${tool} ${constraint.name} at 200% preserves constrained dimensions`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer()])
      if (tool === 'EllipseMarquee') await e.kit.getByTestId('photo-tool-RectMarquee').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.kit.getByTestId('photo-option-style').selectOption(constraint.style)
      if (constraint.ratio) {
        for (const [i, name] of ['width', 'height'].entries()) {
          const field = e.kit.getByTestId(`photo-option-${name}`).locator('input')
          await field.fill(String(constraint.ratio[i]))
          await field.press('Enter')
        }
      }
      if (tool === 'EllipseMarquee') await e.kit.getByTestId('photo-option-antiAlias').uncheck()
      const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
      await zoom.fill('200')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('200')
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      // Unequal fractional parts expose corner rounding that deforms a square or fixed ratio.
      const x = box.x + box.width / 2 + 1, y = box.y + box.height / 2
      await e.page.mouse.move(x, y)
      await e.page.mouse.down()
      if (!constraint.ratio) await e.page.keyboard.down('Shift')
      await e.page.mouse.move(x + constraint.sx * 36, y + constraint.sy * 24, { steps: 4 })
      await e.page.mouse.up()
      if (!constraint.ratio) await e.page.keyboard.up('Shift')
      const [width, height] = constraint.size
      const left = 64 - (constraint.sx < 0 ? width : 0), top = 48 - (constraint.sy < 0 ? height : 0)
      await expect.poll(async () => (await e.state()).selectionBounds).toEqual([left, top, width, height])
      await zoom.fill('100')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('100')
      await e.kit.getByTestId('photo-tool-Brush').click()
      await e.kit.getByTestId('photo-foreground').fill('#ff0000')
      const middle = top + height / 2
      await e.drag([left - 8, middle], [left + width + 8, middle])
      await e.color(left + width, middle, [255, 255, 255, 255])
      await e.color(left + width / 2, middle, [255, 0, 0, 255])
      await e.color(left - 1, middle, [255, 255, 255, 255])
      await e.undo()
      await e.color(left + width / 2, middle, [255, 255, 255, 255])
      await e.redo()
      await e.color(left + width / 2, middle, [255, 0, 0, 255])
      await e.reopen()
      await e.color(left + width, middle, [255, 255, 255, 255])
      await e.color(left + width / 2, middle, [255, 0, 0, 255])
    })
  }
}


for (const tool of ['RectMarquee', 'EllipseMarquee', 'Lasso']) {
  for (const mode of ['add', 'subtract']) {
    test(`UI23: ${tool} empty ${mode} gestures preserve the selection and paint boundary`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer()])
      await e.kit.getByTestId('photo-tool-RectMarquee').click()
      await e.drag([32, 24], [80, 72])
      await expect.poll(async () => (await e.state()).selectionBounds).toEqual([32, 24, 48, 48])
      if (tool === 'EllipseMarquee') await e.kit.getByTestId('photo-tool-RectMarquee').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
      await zoom.fill('200')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('200')
      const before = await e.state()
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      // An empty gesture outside the selection at document point (100.5, 80.5).
      const x = box.x + box.width / 2 + 73, y = box.y + box.height / 2 + 65
      await e.kit.getByTestId(`photo-option-selectionMode-${mode}`).click()
      await e.page.mouse.click(x, y)
      // Verify toolbar and modifier paths separately; modifiers override Replace.
      await e.kit.getByTestId('photo-option-selectionMode-replace').click()
      await expect.poll(async () => (await e.state()).selectionBounds).toEqual([32, 24, 48, 48])
      const key = mode === 'add' ? 'Shift' : 'Alt'
      await e.page.keyboard.down(key)
      await e.page.mouse.click(x, y)
      await e.page.keyboard.up(key)
      await zoom.fill('100')
      await zoom.press('Enter')
      await expect(zoom).toHaveValue('100')
      await e.kit.getByTestId('photo-tool-Brush').click()
      expect((await e.state()).revision).toBe(before.revision)
      await expect.poll(async () => (await e.state()).selectionBounds).toEqual([32, 24, 48, 48])
      // Empty add/subtract must not add an undo step.
      await e.undo()
      await expect.poll(async () => (await e.state()).selectionBounds).toBeNull()
      await e.redo()
      await expect.poll(async () => (await e.state()).selectionBounds).toEqual([32, 24, 48, 48])
      await e.kit.getByTestId('photo-foreground').fill('#ff0000')
      await e.drag([16, 48], [112, 48])
      await e.color(24, 48, [255, 255, 255, 255])
      await e.color(56, 48, [255, 0, 0, 255])
      await e.color(100, 48, [255, 255, 255, 255])
      await e.undo()
      await e.color(56, 48, [255, 255, 255, 255])
      await e.redo()
      await e.reopen()
      await e.color(24, 48, [255, 255, 255, 255])
      await e.color(56, 48, [255, 0, 0, 255])
      await e.color(100, 48, [255, 255, 255, 255])
    })
  }
}

for (const scenario of [
  { mode: 'add', bounds: [24, 24, 72, 48], red: [true, true, true] },
  { mode: 'subtract', bounds: [24, 24, 48, 48], red: [true, false, false] },
  { mode: 'intersect', bounds: [48, 36, 24, 24], red: [false, true, false] },
]) {
  test(`UI24: ${scenario.mode} overlapping selections constrains paint and survives PSD reopen`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [layer()])
    await e.kit.getByTestId('photo-tool-RectMarquee').click()
    await e.drag([24, 24], [72, 72])
    await expect.poll(async () => (await e.state()).selectionBounds).toEqual([24, 24, 48, 48])
    await e.kit.getByTestId(`photo-option-selectionMode-${scenario.mode}`).click()
    await e.drag([48, 36], [96, 60])
    await expect.poll(async () => (await e.state()).selectionBounds).toEqual(scenario.bounds)
    await e.undo()
    await expect.poll(async () => (await e.state()).selectionBounds).toEqual([24, 24, 48, 48])
    await e.redo()
    await expect.poll(async () => (await e.state()).selectionBounds).toEqual(scenario.bounds)
    await e.kit.getByTestId('photo-tool-Brush').click()
    await e.kit.getByTestId('photo-foreground').fill('#ff0000')
    await e.drag([16, 48], [112, 48])
    const verify = async () => {
      for (const [i, x] of [32, 60, 84].entries()) {
        await e.color(x, 48, scenario.red[i] ? [255, 0, 0, 255] : [255, 255, 255, 255])
      }
      await e.color(108, 48, [255, 255, 255, 255])
    }
    await verify()
    await e.undo()
    for (const x of [32, 60, 84]) await e.color(x, 48, [255, 255, 255, 255])
    await e.redo()
    await verify()
    await e.reopen()
    await verify()
  })
}


for (const tool of ['Rectangle', 'EllipseShape', 'Crop', 'Slice', 'ObjectSelection']) {
  for (const finish of ['release at start', 'pointer cancel']) {
    test(`UI25: ${tool} ${finish} clears the drag preview without editing`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer()])
      if (tool === 'EllipseShape') await e.kit.getByTestId('photo-tool-Rectangle').click({ button: 'right' })
      if (tool === 'Slice') await e.kit.getByTestId('photo-tool-Crop').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      const before = await e.state()
      const overlay = e.kit.locator('.kui-canvas-overlay')
      const previewPixels = () => overlay.evaluate((canvas) => {
        const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
        let visible = 0
        for (let i = 3; i < pixels.length; i += 4) if (pixels[i]) visible++
        return visible
      })
      await expect.poll(previewPixels).toBe(0)
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      const x = box.x + box.width / 2 - 40, y = box.y + box.height / 2 - 24
      await e.page.mouse.move(x, y)
      await e.page.mouse.down()
      await e.page.mouse.move(x + 64, y + 40, { steps: 4 })
      await expect.poll(previewPixels, { message: 'A visible drag preview must exist before release/cancel' }).toBeGreaterThan(0)
      // Sparse pointerup can return to the press without a final move. Cancellation is delivered
      // to the real SDK canvas handler as well; neither event may submit the intermediate area.
      await e.kit.getByTestId('kit-canvas-surface').evaluate((canvas, { x, y, finish }) => {
        canvas.dispatchEvent(new PointerEvent(finish === 'pointer cancel' ? 'pointercancel' : 'pointerup', {
          bubbles: true, pointerId: 1, pointerType: 'mouse', button: 0, buttons: 0, clientX: x, clientY: y,
        }))
      }, { x, y, finish })
      await e.page.mouse.up()
      // Check the transient canvas before any tool switch, sample command or repaint can hide it.
      await expect.poll(previewPixels, { message: 'An abandoned drag must leave no transient pixels' }).toBe(0)
      expect((await e.state()).revision).toBe(before.revision)
      expect(structure(await e.state())).toEqual(structure(before))
      if (tool === 'Crop') await expect(e.kit.getByTestId('photo-crop-apply')).toBeDisabled()
      await e.color(56, 44, [255, 255, 255, 255])
      await e.kit.getByTestId('photo-tool-Brush').click()
      await e.kit.getByTestId('photo-foreground').fill('#ff0000')
      await e.drag([24, 48], [104, 48])
      await e.color(64, 48, [255, 0, 0, 255])
      await e.undo()
      await e.color(64, 48, [255, 255, 255, 255])
      await e.redo()
      await e.reopen()
      await e.color(64, 48, [255, 0, 0, 255])
    })
  }
}


for (const nextAction of ['undo', 'edit another option']) {
  test(`UI26: crop keeps keyboard focus while pending and then ${nextAction}`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [layer(), fill('#204060')])
    await e.kit.getByTestId('photo-tool-Crop').click()
    await e.drag([24, 24], [72, 60])
    const apply = e.kit.getByTestId('photo-crop-apply')
    // Hold only the crop request. The real worker and engine still execute it on release.
    await apply.evaluate(() => {
      const post = Worker.prototype.postMessage
      window.cropGate = { held: false, release: null }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'image.crop') {
          window.cropGate.held = true
          window.cropGate.release = () => {
            Worker.prototype.postMessage = post
            post.call(this, message, ...args)
          }
        } else post.call(this, message, ...args)
      }
    })
    // Explicit focus also covers keyboard users on browsers that do not focus on mouse clicks.
    await apply.focus()
    await apply.press('Enter')
    expect(await apply.evaluate(() => window.cropGate.held)).toBe(true)
    let shortcutsReachable
    if (nextAction === 'undo') {
      shortcutsReachable = await apply.evaluate(() => {
        const active = document.activeElement
        return !!active?.closest('.kui-app') && !active.matches(':disabled, input, select, textarea')
      })
    } else {
      await e.kit.getByTestId('photo-option-ratio').focus()
    }
    await apply.evaluate(() => window.cropGate.release())
    if (nextAction === 'undo') expect(shortcutsReachable, 'Disabling Apply must leave editor shortcuts reachable before the worker replies').toBe(true)
    await expect.poll(async () => (await e.state()).width).toBe(48)
    if (nextAction === 'edit another option') {
      // A later user focus choice must survive command completion and view fitting.
      await expect(e.kit.getByTestId('photo-option-ratio')).toBeFocused()
      await e.kit.getByTestId('photo-undo').click()
    } else await e.undo()
    await expect.poll(async () => (await e.state()).width).toBe(128)
    expect((await e.state()).height).toBe(96)
    if (nextAction === 'edit another option') await e.kit.getByTestId('photo-redo').click()
    else await e.redo()
    await expect.poll(async () => (await e.state()).width).toBe(48)
    await e.reopen()
  })
}

for (const cancel of ['click', 'Enter', 'Escape']) {
  test(`UI27: cancelling crop with ${cancel} keeps tool and history shortcuts available`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [layer()])
    await e.kit.getByTestId('photo-foreground').fill('#ff0000')
    await e.drag([24, 48], [104, 48])
    await e.color(64, 48, [255, 0, 0, 255])
    const before = await e.state()
    await e.kit.getByTestId('photo-tool-Crop').click()
    await e.drag([24, 24], [72, 60])
    const button = e.kit.getByTestId('photo-option-cancel')
    // Keyboard focus may be on Cancel even in browsers that do not focus buttons on click.
    await button.focus()
    if (cancel === 'click') await button.click()
    else await button.press(cancel)
    await expect(e.kit.getByTestId('photo-crop-apply')).toBeDisabled()
    expect((await e.state()).revision).toBe(before.revision)
    expect(structure(await e.state())).toEqual(structure(before))
    await e.page.keyboard.press('b')
    await expect(e.kit.getByTestId('photo-tool-Brush')).toHaveAttribute('aria-pressed', 'true')
    await e.undo()
    await e.color(64, 48, [255, 255, 255, 255])
    await e.redo()
    await e.color(64, 48, [255, 0, 0, 255])
    await e.reopen()
  })
}

for (const commit of ['Enter', 'toolbar']) {
  for (const nextAction of ['undo', 'edit another field']) {
    test(`UI28: text ${commit} preserves focus while pending and then ${nextAction}`, async ({ editor: e }) => {
      await e.open()
      await e.kit.getByTestId('photo-tool-Type').click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('One pending title')
      const surface = e.kit.getByTestId('kit-canvas-surface')
      await surface.evaluate(() => {
        const post = Worker.prototype.postMessage
        window.textGate = { requests: 0, pending: [], release: null }
        window.textGate.release = () => {
          Worker.prototype.postMessage = post
          for (const send of window.textGate.pending) send()
        }
        Worker.prototype.postMessage = function (message, ...args) {
          if (message?.request?.method === 'execute' && message.request.command === 'type.create') {
            window.textGate.requests++
            window.textGate.pending.push(() => post.call(this, message, ...args))
          } else post.call(this, message, ...args)
        }
      })
      if (commit === 'Enter') await field.press('Enter')
      else await e.kit.getByTestId('photo-option-commit').click()
      await expect(field).toHaveCount(0)
      const held = await surface.evaluate(() => ({
        requests: window.textGate.requests,
        reachable: !!document.activeElement?.closest('.kui-app') && !document.activeElement.matches(':disabled, input, select, textarea'),
      }))
      const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
      if (nextAction === 'edit another field') await zoom.click()
      // Release before asserting, so a failing assertion cannot leave teardown waiting on WASM.
      await surface.evaluate(() => window.textGate.release())
      expect(held.requests, 'Committing or moving focus must not create duplicate text').toBe(1)
      if (nextAction === 'undo') expect(held.reachable, 'Editor shortcuts must be reachable before the worker replies').toBe(true)
      await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
      if (nextAction === 'edit another field') {
        await expect(zoom).toBeFocused()
        await zoom.fill('75')
        await zoom.press('Enter')
        await expect(zoom).toHaveValue('75')
        await e.kit.getByTestId('photo-undo').click()
      } else await e.undo()
      await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(0)
      if (nextAction === 'edit another field') await e.kit.getByTestId('photo-redo').click()
      else await e.redo()
      await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
      expect(await e.run('type.info')).toMatchObject({ text: 'One pending title' })
      await e.reopen()
    })
  }
}

for (const tool of ['Type', 'VerticalType']) {
  for (const route of ['direct', 'via font size', 'empty via font size']) {
    test(`UI29: ${tool} draft ${route} finishes once when switching tools`, async ({ editor: e }) => {
      await e.open()
      const before = await e.state()
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      const empty = route.startsWith('empty')
      if (!empty) await field.fill('Keep this draft')
      if (route !== 'direct') {
        const size = e.kit.getByTestId('photo-option-fontSize').locator('input')
        await size.fill('30')
        await size.press('Enter')
        await expect(field).toBeVisible()
        expect((await e.state()).revision).toBe(before.revision)
      }
      await e.kit.getByTestId('photo-tool-Move').click()
      await expect(e.kit.getByTestId('photo-tool-Move')).toHaveAttribute('aria-pressed', 'true')
      await expect(field, 'Switching tools must finish the old text draft even after visiting its options').toHaveCount(0)
      if (empty) {
        expect((await e.state()).revision).toBe(before.revision)
        expect(structure(await e.state())).toEqual(structure(before))
      } else {
        await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
        const verify = async () => {
          const info = await e.run('type.info')
          expect(info).toMatchObject({ text: 'Keep this draft', orientation: tool === 'Type' ? 'Horizontal' : 'Vertical' })
          expect(info.runs[0].style.size_pt).toBe(route === 'direct' ? 48 : 30)
          return info
        }
        const text = await verify()
        expect(text.transform).toEqual([1, 0, 0, 1, 24, 50])
        // Only one history step: changing draft options and finishing must not duplicate text.
        await e.undo()
        await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(0)
        await e.redo()
        await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
        expect((await verify()).transform).toEqual(text.transform)
        await e.reopen()
        await e.kit.getByTestId(`photo-layer-${(await e.state()).layers.find((item) => item.kind === 'Type').id}`).click()
        expect((await verify()).transform).toEqual(text.transform)
      }
      // A fresh draft must start empty and cancelling it must not alter the committed document.
      const finished = await e.state()
      if (!(await e.kit.getByTestId(`photo-tool-${tool}`).isVisible())) await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(90, 70)
      await expect(field).toHaveValue('')
      await field.press('Escape')
      await expect(field).toHaveCount(0)
      expect((await e.state()).revision).toBe(finished.revision)
    })
  }
}

for (const tool of ['Type', 'VerticalType']) {
  test(`UI38: ${tool} pending text options never restyle the previously selected title`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [{ command: 'type.create', params: { x: 8, y: 20, text: 'Existing title', size: 18 } }])
    const originalId = (await e.state()).activeLayer
    const original = await e.run('type.info', { layer: originalId })
    if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
    await e.kit.getByTestId(`photo-tool-${tool}`).click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('Pending title')
    const size = e.kit.getByTestId('photo-option-fontSize').locator('input')
    await size.fill('30')
    await size.press('Enter')
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      const pending = []
      window.optionsGate = { requests: 0, release: () => {
        Worker.prototype.postMessage = post
        for (const send of pending.splice(0)) send()
      } }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'type.create') {
          window.optionsGate.requests++
          pending.push(() => post.call(this, message, ...args))
        } else post.call(this, message, ...args)
      }
    })
    try {
      await e.kit.getByTestId('photo-option-commit').click()
      await expect(field).toHaveCount(0)
      expect(await surface.evaluate(() => window.optionsGate.requests)).toBe(1)
      // The submitted command owns its 30 pt snapshot. Further options are for new text.
      await size.fill('42')
      await size.press('Enter')
    } finally { await surface.evaluate(() => window.optionsGate.release()) }
    const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
    await expect.poll(async () => (await texts()).length).toBe(2)
    expect(await e.run('type.info', { layer: originalId }), 'Editing options while text is pending must not mutate the older title').toEqual(original)
    const pendingId = (await texts()).find((item) => item.id !== originalId).id
    const content = { text: 'Pending title', orientation: tool === 'Type' ? 'Horizontal' : 'Vertical', transform: [1, 0, 0, 1, 24, 50], runs: [{ style: { size_pt: 30 } }] }
    expect(await e.run('type.info', { layer: pendingId })).toMatchObject(content)
    // The option change must not add a hidden history step to the older layer.
    await e.kit.getByTestId('photo-undo').click()
    await expect.poll(async () => (await texts()).length).toBe(1)
    expect(await e.run('type.info', { layer: originalId })).toEqual(original)
    await e.kit.getByTestId('photo-redo').click()
    await expect.poll(async () => (await texts()).length).toBe(2)
    await e.click(70, 80)
    await field.fill('Next title')
    await expect(size).toHaveValue('42')
    await field.press('Enter')
    await expect.poll(async () => (await texts()).length).toBe(3)
    const nextId = (await texts()).find((item) => item.id !== originalId && item.id !== pendingId).id
    expect(await e.run('type.info', { layer: nextId })).toMatchObject({ text: 'Next title', runs: [{ style: { size_pt: 42 } }] })
    const bytes = await e.saved()
    const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
    try {
      const infos = engineInspect(saved).layers.filter((item) => item.kind === 'Type').map((item) => engineExecute(saved, 'type.info', { layer: item.id }))
      expect(infos).toHaveLength(3)
      expect(infos.find((item) => item.text === 'Existing title').runs).toEqual(original.runs)
      expect(infos.find((item) => item.text === 'Pending title')).toMatchObject(content)
      expect(infos.find((item) => item.text === 'Next title')).toMatchObject({ runs: [{ style: { size_pt: 42 } }] })
    } finally { saved.free() }
    await e.reopen('Scenario.psd', bytes)
  })
}

for (const cancel of ['Escape', 'toolbar']) {
  test(`UI39: ${cancel} cancels a draft queued for Save and later saves still work`, async ({ editor: e }) => {
    await e.open()
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('Keep this title')
    const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
    await e.page.evaluate((version) => {
      const post = MessagePort.prototype.postMessage
      let delivered = false
      window.cancelSaveGate = { held: false, release: () => { MessagePort.prototype.postMessage = post } }
      MessagePort.prototype.postMessage = function (message, ...args) {
        if (!window.cancelSaveGate.held && message?.jsonrpc === '2.0' && message.result?.path === 'Scenario.psd' && message.result.version === String(version + 1)) {
          window.cancelSaveGate.held = true
          window.cancelSaveGate.release = () => {
            MessagePort.prototype.postMessage = post
            if (!delivered) { delivered = true; post.call(this, message, ...args) }
          }
        } else post.call(this, message, ...args)
      }
    }, version)
    try {
      await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await expect.poll(() => e.page.evaluate(() => window.cancelSaveGate.held)).toBe(true)
      await e.click(80, 80)
      await field.fill('Cancel this title')
      await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await expect(field).toHaveValue('Cancel this title')
      if (cancel === 'Escape') await field.press('Escape')
      else await e.kit.getByTestId('photo-option-cancel').click()
      await expect(field).toHaveCount(0)
    } finally { await e.page.evaluate(() => window.cancelSaveGate.release()) }
    await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 2)
    await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
    const checkStored = async (expected) => {
      const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
      const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
      try {
        const titles = engineInspect(saved).layers.filter((item) => item.kind === 'Type').map((item) => engineExecute(saved, 'type.info', { layer: item.id }).text).sort()
        expect(titles).toEqual([...expected].sort())
      } finally { saved.free() }
      return bytes
    }
    await checkStored(['Keep this title'])
    await e.click(80, 80)
    await field.fill('Save after cancellation')
    await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
    await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 3)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
    const bytes = await checkStored(['Keep this title', 'Save after cancellation'])
    await e.undo()
    await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
    await e.redo()
    await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(2)
    await e.reopen('Scenario.psd', bytes)
  })
}

for (const history of ['undo', 'redo']) {
  for (const releasedFirst of ['history', 'host']) {
    test(`UI37: Save includes pending ${history} with ${releasedFirst} completing first`, async ({ editor: e }) => {
      await e.open()
      await e.kit.getByTestId('photo-tool-Type').click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('History title')
      await field.press('Enter')
      const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await expect.poll(async () => (await texts()).length).toBe(1)
      if (history === 'redo') {
        await e.undo()
        await expect.poll(async () => (await texts()).length).toBe(0)
      }
      const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
      await e.page.evaluate((version) => {
        const post = MessagePort.prototype.postMessage
        window.historyHostGate = { held: false, first: null, release: () => { MessagePort.prototype.postMessage = post } }
        MessagePort.prototype.postMessage = function (message, ...args) {
          if (!window.historyHostGate.held && message?.jsonrpc === '2.0' && message.result?.path === 'Scenario.psd' && message.result.version === String(version + 1)) {
            window.historyHostGate.held = true
            window.historyHostGate.first = [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]
            let sent = false
            window.historyHostGate.release = () => {
              MessagePort.prototype.postMessage = post
              if (!sent) { sent = true; post.call(this, message, ...args) }
            }
          } else post.call(this, message, ...args)
        }
      }, version)
      const surface = e.kit.getByTestId('kit-canvas-surface')
      await surface.evaluate((_, command) => {
        const post = Worker.prototype.postMessage
        window.historyCommandGate = { requests: 0, pending: [], release: null }
        window.historyCommandGate.release = () => {
          Worker.prototype.postMessage = post
          for (const send of window.historyCommandGate.pending.splice(0)) send()
        }
        Worker.prototype.postMessage = function (message, ...args) {
          if (message?.request?.method === 'execute' && message.request.command === command) {
            window.historyCommandGate.requests++
            window.historyCommandGate.pending.push(() => post.call(this, message, ...args))
          } else post.call(this, message, ...args)
        }
      }, `edit.${history}`)
      try {
        await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
        await expect.poll(() => e.page.evaluate(() => window.historyHostGate.held)).toBe(true)
        await e.page.keyboard.press(await shortcut(e.page, history === 'undo' ? 'ControlOrMeta+z' : 'ControlOrMeta+Shift+z'))
        await expect.poll(() => surface.evaluate(() => window.historyCommandGate.requests)).toBe(1)
        // Save is requested before the history command has produced its new revision.
        await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
        await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
        if (releasedFirst === 'host') await e.page.evaluate(() => window.historyHostGate.release())
        expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
        await surface.evaluate(() => window.historyCommandGate.release())
        await expect.poll(async () => (await texts()).length).toBe(history === 'undo' ? 0 : 1)
        if (releasedFirst === 'history') await e.page.evaluate(() => window.historyHostGate.release())
      } finally {
        await surface.evaluate(() => window.historyCommandGate.release())
        await e.page.evaluate(() => window.historyHostGate.release())
      }
      const first = sdk.PhotoDocument.open('Scenario.psd', Buffer.from(await e.page.evaluate(() => window.historyHostGate.first)))
      try { expect(engineInspect(first).layers.filter((item) => item.kind === 'Type')).toHaveLength(history === 'undo' ? 1 : 0) } finally { first.free() }
      await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version), { message: 'Save must include history already requested, even before its revision arrives' }).toBe(version + 2)
      await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
      const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
      const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
      try {
        const titles = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
        expect(titles).toHaveLength(history === 'undo' ? 0 : 1)
        if (titles.length) expect(engineExecute(saved, 'type.info', { layer: titles[0].id })).toMatchObject({ text: 'History title', transform: [1, 0, 0, 1, 24, 50] })
      } finally { saved.free() }
      expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 2)
      if (history === 'undo') { await e.redo(); await expect.poll(async () => (await texts()).length).toBe(1); await e.undo() }
      else { await e.undo(); await expect.poll(async () => (await texts()).length).toBe(0); await e.redo() }
      await expect.poll(async () => (await texts()).length).toBe(history === 'undo' ? 0 : 1)
      await e.reopen('Scenario.psd', bytes)
    })
  }
}

for (const tool of ['Type', 'VerticalType']) {
  for (const entry of ['shortcut', 'toolbar']) {
    test(`UI35: ${tool} ${entry} Save retains edits after a host write failure and retries once`, async ({ editor: e }) => {
      await e.open()
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('Keep after write failure')
      const original = await e.page.evaluate(() => {
        const file = window.devHost.files.get('Scenario.psd')
        const set = window.devHost.files.set
        window.failedWrite = { attempts: 0, restore: () => { window.devHost.files.set = set } }
        window.devHost.files.set = function (path, value) {
          if (path === 'Scenario.psd') {
            window.failedWrite.attempts++
            throw new Error('Scenario host storage unavailable')
          }
          return set.call(this, path, value)
        }
        return { version: file.version, bytes: [...new Uint8Array(file.bytes)] }
      })
      try {
        if (entry === 'shortcut') await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
        else await e.kit.getByTestId('photo-save').click()
        await expect(e.kit.getByRole('alert')).toContainText('Scenario host storage unavailable')
        await expect(field).toHaveCount(0)
        await expect(e.kit.getByTestId('photo-save')).toBeEnabled()
        expect(await e.page.evaluate(() => window.failedWrite.attempts)).toBe(1)
        expect(await e.page.evaluate(() => {
          const file = window.devHost.files.get('Scenario.psd')
          return { version: file.version, bytes: [...new Uint8Array(file.bytes)] }
        })).toEqual(original)
        expect(await e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(true)
        const texts = (await e.state()).layers.filter((item) => item.kind === 'Type')
        expect(texts).toHaveLength(1)
        expect(await e.run('type.info', { layer: texts[0].id })).toMatchObject({ text: 'Keep after write failure', transform: [1, 0, 0, 1, 24, 50], orientation: tool === 'Type' ? 'Horizontal' : 'Vertical' })
      } finally { await e.page.evaluate(() => window.failedWrite.restore()) }
      await e.kit.getByTestId('photo-save').click()
      await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(original.version + 1)
      await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
      const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
      try {
        const texts = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
        expect(texts).toHaveLength(1)
        expect(engineExecute(saved, 'type.info', { layer: texts[0].id }).text).toBe('Keep after write failure')
      } finally { saved.free() }
      await e.kit.getByTestId('photo-undo').click()
      await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(0)
      await e.kit.getByTestId('photo-redo').click()
      await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(1)
      await e.reopen('Scenario.psd', bytes)
    })
  }
}

for (const next of ['draft save', 'committed save', 'committed without save']) {
  test(`UI36: ${next} during a pending host write preserves the newer save intent`, async ({ editor: e }) => {
    await e.open()
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('First write')
    const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
    // Hold the first real host/write acknowledgement after bytes reached host storage.
    await e.page.evaluate((version) => {
      const post = MessagePort.prototype.postMessage
      window.hostWriteGate = { held: false, release: () => {}, first: null }
      window.hostWriteGate.release = () => { MessagePort.prototype.postMessage = post }
      MessagePort.prototype.postMessage = function (message, ...args) {
        if (!window.hostWriteGate.held && message?.jsonrpc === '2.0' && message.result?.path === 'Scenario.psd' && message.result.version === String(version + 1)) {
          window.hostWriteGate.held = true
          window.hostWriteGate.first = [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]
          window.hostWriteGate.release = () => {
            MessagePort.prototype.postMessage = post
            post.call(this, message, ...args)
          }
        } else post.call(this, message, ...args)
      }
    }, version)
    try {
      await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await expect.poll(() => e.page.evaluate(() => window.hostWriteGate.held)).toBe(true)
      await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      // Requests for the already captured content must not themselves queue another write.
      await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await e.click(90, 80)
      await field.fill('Second write')
      if (next !== 'draft save') {
        await field.press('Enter')
        await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(2)
      }
      if (next !== 'committed without save') {
        await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
        await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
      }
      expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
    } finally { await e.page.evaluate(() => window.hostWriteGate.release()) }
    const first = sdk.PhotoDocument.open('Scenario.psd', Buffer.from(await e.page.evaluate(() => window.hostWriteGate.first)))
    try {
      const texts = engineInspect(first).layers.filter((item) => item.kind === 'Type')
      expect(texts).toHaveLength(1)
      expect(engineExecute(first, 'type.info', { layer: texts[0].id }).text).toBe('First write')
    } finally { first.free() }
    if (next === 'committed without save') {
      await expect(e.kit.getByTestId('photo-save')).toBeEnabled()
      expect(await e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(true)
      expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
      await e.kit.getByTestId('photo-save').click()
    }
    await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version), { message: 'A save requested for newer edits must run after the first write' }).toBe(version + 2)
    await expect(field).toHaveCount(0)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
    const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
    const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
    try {
      const texts = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
      expect(texts).toHaveLength(2)
      const content = texts.map((item) => engineExecute(saved, 'type.info', { layer: item.id })).map(({ text, transform }) => ({ text, transform })).sort((a, b) => a.text.localeCompare(b.text))
      expect(content).toEqual([{ text: 'First write', transform: [1, 0, 0, 1, 24, 50] }, { text: 'Second write', transform: [1, 0, 0, 1, 90, 80] }])
    } finally { saved.free() }
    expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 2)
    await e.reopen('Scenario.psd', bytes)
  })
}

for (const tool of ['Type', 'VerticalType']) {
  for (const existingEdit of [false, true]) {
    test(`UI34: ${tool} toolbar Save includes a draft after editing options in a ${existingEdit ? 'dirty' : 'clean'} document`, async ({ editor: e }) => {
      await e.open()
      if (existingEdit) await e.run('layer.new.layer', { name: 'Existing edit' })
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('Save from options')
      const size = e.kit.getByTestId('photo-option-fontSize').locator('input')
      await size.fill('30')
      await size.press('Enter')
      await expect(field).toHaveValue('Save from options')
      const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
      const save = e.kit.getByTestId('photo-save')
      await expect(save, 'An unsubmitted text draft must be saveable').toBeEnabled()
      await save.click()
      await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
      const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
      const content = { text: 'Save from options', transform: [1, 0, 0, 1, 24, 50], orientation: tool === 'Type' ? 'Horizontal' : 'Vertical', runs: [{ style: { size_pt: 30 } }] }
      const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
      try {
        const texts = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
        expect(texts, 'The first toolbar write must include the draft').toHaveLength(1)
        expect(engineExecute(saved, 'type.info', { layer: texts[0].id })).toMatchObject(content)
      } finally { saved.free() }
      await expect(field).toHaveCount(0)
      await expect(save).toBeDisabled()
      await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
      const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await e.kit.getByTestId('photo-undo').click()
      await expect.poll(async () => (await texts()).length).toBe(0)
      await e.kit.getByTestId('photo-redo').click()
      await expect.poll(async () => (await texts()).length).toBe(1)
      await e.reopen('Scenario.psd', bytes)
      expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject(content)
    })
  }
}

for (const tool of ['Type', 'VerticalType']) {
  test(`UI34: ${tool} toolbar Save refuses to write a rejected draft and can retry`, async ({ editor: e }) => {
    await e.open()
    if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
    await e.kit.getByTestId(`photo-tool-${tool}`).click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('Retry toolbar save')
    await e.kit.getByTestId('photo-option-fontSize').locator('input').click()
    const before = await e.state()
    const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
    await e.page.evaluate(() => window.devHost.configure({ disable: ['command:type.create'], hide: ['action:redo'] }))
    await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
    const save = e.kit.getByTestId('photo-save')
    await expect(save).toBeEnabled()
    await save.click()
    await expect(e.kit.getByRole('alert')).toContainText('type.create')
    await expect(field).toHaveValue('Retry toolbar save')
    await expect(save).toBeEnabled()
    expect(await e.state()).toEqual(before)
    expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version)
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
    await save.click()
    await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
    await expect(field).toHaveCount(0)
    await expect(save).toBeDisabled()
    const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
    const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
    try {
      const texts = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
      expect(texts).toHaveLength(1)
      expect(engineExecute(saved, 'type.info', { layer: texts[0].id })).toMatchObject({ text: 'Retry toolbar save', orientation: tool === 'Type' ? 'Horizontal' : 'Vertical' })
    } finally { saved.free() }
    await e.reopen('Scenario.psd', bytes)
  })
}

test('UI34: Save availability tracks nonblank draft text without editing the document', async ({ editor: e }) => {
  await e.open()
  const before = await e.state()
  const save = e.kit.getByTestId('photo-save')
  await expect(save).toBeDisabled()
  await e.kit.getByTestId('photo-tool-Type').click()
  await e.click(24, 50)
  const field = e.kit.getByTestId('photo-text-field')
  await expect(save).toBeDisabled()
  await field.fill('Unsaved title')
  await expect(save).toBeEnabled()
  expect((await e.state()).revision).toBe(before.revision)
  await field.fill('   ')
  await expect(save).toBeDisabled()
  await field.fill('Cancelled title')
  await expect(save).toBeEnabled()
  await field.press('Escape')
  await expect(field).toHaveCount(0)
  await expect(save).toBeDisabled()
  expect(await e.state()).toEqual(before)
})

test('UI34: toolbar Save waits for text and shares repeated save shortcuts', async ({ editor: e }) => {
  await e.open()
  await e.kit.getByTestId('photo-tool-Type').click()
  await e.click(24, 50)
  const field = e.kit.getByTestId('photo-text-field')
  await field.fill('One toolbar write')
  const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
  const surface = e.kit.getByTestId('kit-canvas-surface')
  await surface.evaluate(() => {
    const post = Worker.prototype.postMessage
    window.toolbarSaveGate = { requests: 0, pending: [], release: null }
    window.toolbarSaveGate.release = () => {
      Worker.prototype.postMessage = post
      for (const send of window.toolbarSaveGate.pending) send()
    }
    Worker.prototype.postMessage = function (message, ...args) {
      if (message?.request?.method === 'execute' && message.request.command === 'type.create') {
        window.toolbarSaveGate.requests++
        window.toolbarSaveGate.pending.push(() => post.call(this, message, ...args))
      } else post.call(this, message, ...args)
    }
  })
  try {
    const save = e.kit.getByTestId('photo-save')
    await expect(save).toBeEnabled()
    await save.click()
    await expect(field).toHaveCount(0)
    // A focused draft can blur on Save's pointerdown. Keep its ensuing click usable.
    await expect(save).toBeEnabled()
    await save.click()
    await save.focus()
    await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
    await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
    expect(await surface.evaluate(() => window.toolbarSaveGate.requests)).toBe(1)
    expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version)
  } finally { await surface.evaluate(() => window.toolbarSaveGate.release()) }
  await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
  await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
  const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
  try {
    const texts = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
    expect(texts).toHaveLength(1)
    expect(engineExecute(saved, 'type.info', { layer: texts[0].id })).toMatchObject({ text: 'One toolbar write' })
  } finally { saved.free() }
  expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
  await e.reopen('Scenario.psd', bytes)
})

for (const latest of ['tool switch', 'new canvas draft']) {
  test(`UI33: pending text keeps the latest ${latest} intent`, async ({ editor: e }) => {
    await e.open()
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('First')
    const surface = e.kit.getByTestId('kit-canvas-surface')
    // Read the document size before blocking the worker; the usual click helper inspects it. The
    // canvas box is read at the click, so the point follows the canvas wherever the commit and the
    // tool switch leave it.
    const doc = await e.state()
    const clickNext = async () => {
      const box = await surface.boundingBox()
      await e.page.mouse.click(box.x + box.width / 2 + 90 - doc.width / 2, box.y + box.height / 2 + 80 - doc.height / 2)
    }
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      window.textIntentGate = { requests: 0, pending: [], release: null }
      window.textIntentGate.release = () => {
        Worker.prototype.postMessage = post
        for (const send of window.textIntentGate.pending) send()
      }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'type.create') {
          window.textIntentGate.requests++
          window.textIntentGate.pending.push(() => post.call(this, message, ...args))
        } else post.call(this, message, ...args)
      }
    })
    let requests
    try {
      await e.page.keyboard.press('Enter')
      await expect(field).toHaveCount(0)
      if (latest === 'tool switch') {
        await clickNext()
        await e.kit.getByTestId('photo-tool-Brush').click()
      } else {
        await e.kit.getByTestId('photo-tool-Brush').click()
        await clickNext()
      }
      requests = await surface.evaluate(() => window.textIntentGate.requests)
    } finally { await surface.evaluate(() => window.textIntentGate.release()) }
    expect(requests).toBe(1)
    const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
    await expect.poll(async () => (await texts()).length).toBe(1)
    if (latest === 'tool switch') {
      await expect(e.kit.getByTestId('photo-tool-Brush')).toHaveAttribute('aria-pressed', 'true')
      await expect(field, 'A superseded canvas click must not reopen a draft under another tool').toHaveCount(0)
    } else {
      await expect(e.kit.getByTestId('photo-tool-Type')).toHaveAttribute('aria-pressed', 'true')
      await expect(field).toHaveValue('')
      await expect(field).toBeFocused()
      await e.page.keyboard.type('Second')
      await e.page.keyboard.press('Enter')
      await expect.poll(async () => (await texts()).length).toBe(2)
    }
    const titles = []
    for (const item of await texts()) {
      const info = await e.run('type.info', { layer: item.id })
      titles.push({ text: info.text, transform: info.transform })
    }
    expect(titles.sort((a, b) => a.text.localeCompare(b.text))).toEqual([
      { text: 'First', transform: [1, 0, 0, 1, 24, 50] },
      ...(latest === 'new canvas draft' ? [{ text: 'Second', transform: [1, 0, 0, 1, 90, 80] }] : []),
    ])
    await e.undo()
    await expect.poll(async () => (await texts()).length).toBe(titles.length - 1)
    await e.redo()
    await expect.poll(async () => (await texts()).length).toBe(titles.length)
    await e.reopen()
  })
}

for (const tool of ['Type', 'VerticalType']) {
  for (const trigger of ['Enter', 'save shortcut', 'tool switch', 'new canvas draft']) {
    test(`UI33: ${tool} retains a rejected draft from ${trigger} and can retry saving it`, async ({ editor: e }) => {
      await e.open()
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      const size = e.kit.getByTestId('photo-option-fontSize').locator('input')
      await size.fill('30')
      await size.press('Enter')
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('Preserve this text')
      const before = await e.state()
      const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
      // Reject the real command at the host policy boundary; hiding Redo confirms config arrival.
      await e.page.evaluate(() => window.devHost.configure({ disable: ['command:type.create'], hide: ['action:redo'] }))
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (trigger === 'Enter') await e.page.keyboard.press('Enter')
      else if (trigger === 'save shortcut') await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
      else if (trigger === 'tool switch') await e.kit.getByTestId('photo-tool-Brush').click()
      else await e.click(90, 80)
      await expect(e.kit.getByRole('alert')).toContainText('type.create')
      await expect(field, 'A rejected command must preserve the editable text draft').toHaveValue('Preserve this text')
      await expect(e.kit.getByTestId(`photo-tool-${tool}`)).toHaveAttribute('aria-pressed', 'true')
      if (trigger === 'Enter' || trigger === 'save shortcut') await expect(field).toBeFocused()
      expect((await e.state()).revision).toBe(before.revision)
      expect((await e.state()).layers.filter((item) => item.kind === 'Type')).toHaveLength(0)
      expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version)
      // Restore permission and keep editing the same draft; one retry creates one title/write.
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      await field.click()
      // End does not move the caret on macOS WebKit. Set a platform-independent append point.
      await field.evaluate((input) => input.setSelectionRange(input.value.length, input.value.length))
      await e.page.keyboard.type(' recovered')
      await expect(field).toHaveValue('Preserve this text recovered')
      await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
      await expect(field).toHaveCount(0)
      const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await expect.poll(async () => (await texts()).length).toBe(1)
      const content = { text: 'Preserve this text recovered', transform: [1, 0, 0, 1, 24, 50], orientation: tool === 'Type' ? 'Horizontal' : 'Vertical', runs: [{ style: { size_pt: 30 } }] }
      expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject(content)
      await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
      const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
      const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
      try {
        const layers = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
        expect(layers).toHaveLength(1)
        expect(engineExecute(saved, 'type.info', { layer: layers[0].id })).toMatchObject(content)
      } finally { saved.free() }
      await e.undo()
      await expect.poll(async () => (await texts()).length).toBe(0)
      await e.redo()
      await expect.poll(async () => (await texts()).length).toBe(1)
      await e.reopen('Scenario.psd', bytes)
      expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject(content)
    })
  }
}

for (const tool of ['Type', 'VerticalType']) {
  for (const text of ['Save this draft', '   ']) {
    test(`UI32: ${tool} save shortcut finishes a ${text.trim() ? 'nonempty' : 'blank'} draft before writing PSD`, async ({ editor: e }) => {
      await e.open()
      const before = await e.state()
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill(text)
      const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
      // Save from the actual focused input, without a blur, Enter, or save-button click first.
      await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version), { message: 'The save shortcut must write the file while a text draft is focused' }).toBe(version + 1)
      await expect(field).toHaveCount(0)
      await expect.poll(() => e.page.evaluate(async () => (await window.devHost.connection().ai.inspect()).dirty)).toBe(false)
      const expected = text.trim() ? 1 : 0
      const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await expect.poll(async () => (await texts()).length).toBe(expected)
      const content = { text, transform: [1, 0, 0, 1, 24, 50], orientation: tool === 'Type' ? 'Horizontal' : 'Vertical', runs: [{ style: { size_pt: 48 } }] }
      // Decode the first write: a later save/reopen must not conceal a commit/save ordering bug.
      const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
      const saved = sdk.PhotoDocument.open('Scenario.psd', bytes)
      try {
        const savedTexts = engineInspect(saved).layers.filter((item) => item.kind === 'Type')
        expect(savedTexts).toHaveLength(expected)
        if (expected) expect(engineExecute(saved, 'type.info', { layer: savedTexts[0].id })).toMatchObject(content)
      } finally { saved.free() }
      if (expected) {
        expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject(content)
        // One history step, with focus restored so the shortcut needs no extra click.
        await e.undo()
        await expect.poll(async () => (await texts()).length).toBe(0)
        await e.redo()
        await expect.poll(async () => (await texts()).length).toBe(1)
        expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject(content)
      } else expect((await e.state()).revision).toBe(before.revision)
      await e.reopen('Scenario.psd', bytes)
      expect(await texts()).toHaveLength(expected)
      if (expected) expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject(content)
    })
  }
}

for (const policy of ['hide', 'disable']) {
  test(`UI32: draft save shortcut respects host ${policy} and keeps the draft editable`, async ({ editor: e }) => {
    await e.open()
    await e.run('layer.new.layer', { name: 'Unsaved artwork' })
    await expect(e.kit.getByTestId('photo-save')).toBeEnabled()
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 50)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('Keep')
    const before = await e.state()
    const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
    await e.page.evaluate((policy) => window.devHost.configure({ [policy]: ['action:save'] }), policy)
    if (policy === 'hide') await expect(e.kit.getByTestId('photo-save')).toHaveCount(0)
    else await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
    await expect(field).toBeFocused()
    await e.page.keyboard.type(' typing')
    await expect(field).toHaveValue('Keep typing')
    expect((await e.state()).revision).toBe(before.revision)
    expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version)
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(e.kit.getByTestId('photo-save')).toBeEnabled()
    await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
    await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version + 1)
    await expect(field).toHaveCount(0)
    expect(await e.run('type.info')).toMatchObject({ text: 'Keep typing' })
  })
}

for (const tool of ['Type', 'VerticalType']) {
  for (const gesture of ['pan', 'zoom']) {
    test(`UI31: ${tool} draft follows a wheel ${gesture} without changing its document anchor`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [{ command: 'shape.create', params: { kind: 'rect', rect: [24, 50, 32, 58], fill: '#ff0000', name: 'Anchor marker' } }])
      const before = await e.state()
      const surface = e.kit.getByTestId('kit-canvas-surface')
      // Observe the rendered document, independently of the input's positioning code.
      const marker = () => surface.evaluate((canvas) => {
        const { width, height } = canvas
        const rgba = canvas.getContext('2d').getImageData(0, 0, width, height).data
        let x = width, y = height
        for (let row = 0; row < height; row++) for (let col = 0; col < width; col++) {
          const i = (row * width + col) * 4
          if (rgba[i] === 255 && rgba[i + 1] === 0 && rgba[i + 2] === 0 && rgba[i + 3] === 255) {
            x = Math.min(x, col)
            y = Math.min(y, row)
          }
        }
        return x === width ? null : { x: x * canvas.clientWidth / width, y: y * canvas.clientHeight / height }
      })
      await expect.poll(marker).not.toBeNull()
      const start = await marker()
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('Draft')
      const inputAnchor = () => field.evaluate((input) => ({ x: parseFloat(input.style.left), y: parseFloat(input.style.top) + parseFloat(input.style.fontSize) }))
      const originalAnchor = await inputAnchor()
      expect(Math.abs(originalAnchor.x - start.x)).toBeLessThanOrEqual(1)
      expect(Math.abs(originalAnchor.y - start.y)).toBeLessThanOrEqual(1)
      const box = await surface.boundingBox()
      await e.page.mouse.move(box.x + box.width - 100, box.y + box.height - 100)
      if (gesture === 'zoom') await e.page.keyboard.down('Control')
      try { await e.page.mouse.wheel(gesture === 'pan' ? 24 : 0, gesture === 'pan' ? 16 : -20) }
      finally { if (gesture === 'zoom') await e.page.keyboard.up('Control') }
      await expect.poll(async () => {
        const point = await marker()
        return point != null && Math.hypot(point.x - start.x, point.y - start.y) > 10
      }, { message: 'The wheel must move the actual document marker' }).toBe(true)
      await expect.poll(async () => {
        const point = await marker(), anchor = await inputAnchor()
        return point == null ? Infinity : Math.max(Math.abs(anchor.x - point.x), Math.abs(anchor.y - point.y))
      }, { message: 'The draft baseline must stay on its rendered document anchor' }).toBeLessThanOrEqual(2)
      const shownSize = await field.evaluate((input) => parseFloat(input.style.fontSize))
      expect(shownSize).toBeCloseTo(48 * (gesture === 'pan' ? 1 : Math.exp(0.2)), 1)
      await expect(field).toHaveValue('Draft')
      await expect(field).toBeFocused()
      expect((await e.state()).revision).toBe(before.revision)
      await e.page.keyboard.type(' retained')
      await e.page.keyboard.press('Enter')
      await expect(field).toHaveCount(0)
      const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await expect.poll(async () => (await texts()).length).toBe(1)
      const checkText = async () => expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject({ text: 'Draft retained', transform: [1, 0, 0, 1, 24, 50], orientation: tool === 'Type' ? 'Horizontal' : 'Vertical', runs: [{ style: { size_pt: 48 } }] })
      await checkText()
      await e.undo()
      await expect.poll(async () => (await texts()).length).toBe(0)
      await e.redo()
      await expect.poll(async () => (await texts()).length).toBe(1)
      await checkText()
      await e.reopen()
      await checkText()
    })
  }
}

for (const tool of ['Type', 'VerticalType']) {
  for (const route of ['direct', 'via font size', 'empty via font size']) {
    test(`UI30: ${tool} starts a fresh canvas draft ${route} without losing the previous text`, async ({ editor: e }) => {
      await e.open()
      const before = await e.state()
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 30)
      const field = e.kit.getByTestId('photo-text-field')
      const empty = route.startsWith('empty')
      if (!empty) await field.fill('First')
      if (route !== 'direct') {
        const size = e.kit.getByTestId('photo-option-fontSize').locator('input')
        await size.fill('30')
        await size.press('Enter')
        await expect(field).toBeVisible()
      }
      await e.click(90, 80)
      await expect(field, 'A new canvas draft must not inherit the previous draft text').toHaveValue('')
      await expect(field, 'A new canvas draft must accept typing without a second click').toBeFocused()
      await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(empty ? 0 : 1)
      if (empty) expect((await e.state()).revision).toBe(before.revision)
      const readTitles = async () => {
        const titles = []
        for (const item of (await e.state()).layers.filter((item) => item.kind === 'Type')) {
          const info = await e.run('type.info', { layer: item.id })
          titles.push({ text: info.text, transform: info.transform, size: info.runs[0].style.size_pt, orientation: info.orientation })
        }
        return titles.sort((a, b) => a.text.localeCompare(b.text))
      }
      const expectTitles = async (expected) => {
        // Keyboard history is asynchronous: wait for its layer list before querying IDs.
        await expect.poll(async () => (await e.state()).layers.filter((item) => item.kind === 'Type').length).toBe(expected.length)
        expect(await readTitles()).toEqual(expected)
      }
      const title = (text, x, y) => ({ text, transform: [1, 0, 0, 1, x, y], size: route === 'direct' ? 48 : 30, orientation: tool === 'Type' ? 'Horizontal' : 'Vertical' })
      const first = empty ? [] : [title('First', 24, 30)]
      expect(await readTitles()).toEqual(first)
      // Type through the keyboard, without focusing or filling the replacement input for it.
      await e.page.keyboard.type('Second')
      await e.page.keyboard.press('Enter')
      await expect(field).toHaveCount(0)
      const both = [...first, title('Second', 90, 80)]
      await expectTitles(both)
      await e.undo()
      await expectTitles(first)
      if (!empty) {
        await e.undo()
        await expectTitles([])
        await e.redo()
        await expectTitles(first)
      }
      await e.redo()
      await expectTitles(both)
      await e.reopen()
      expect(await readTitles()).toEqual(both)
    })
  }
}

for (const mode of ['menus', 'line', 'classic']) {
  test(`UI40: ${mode} menu commands execute and respect host policy`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [], 'en-US', mode)
    const app = e.kit.locator('.kui-app')
    await expect(app).toHaveAttribute('data-mode', mode)
    const openMenu = async (name) => {
      const tab = e.kit.getByTestId(mode === 'menus' ? `photo-menu-${name}` : `ribbon-tab-${name}`)
      if (mode !== 'menus' || await tab.getAttribute('aria-expanded') !== 'true') await tab.click()
    }
    await openMenu('Select')
    const all = e.kit.getByTestId('photo-cmd-select.all')
    await expect(all, 'Common commands must be directly available in Single line').toBeVisible()
    await all.click()
    await expect.poll(async () => (await e.state()).selectionBounds).toEqual([0, 0, 128, 96])
    await openMenu('Select')
    const deselect = e.kit.getByTestId('photo-cmd-select.deselect')
    await e.page.evaluate(() => window.devHost.configure({ disable: ['command:select.deselect'] }))
    await expect(deselect).toBeDisabled()
    expect((await e.state()).selectionBounds).toEqual([0, 0, 128, 96])
    await e.page.evaluate(() => window.devHost.configure({ hide: ['command:select.all'] }))
    await expect(all).toHaveCount(0)
    await expect(deselect).toBeEnabled()
    await deselect.click()
    await expect.poll(async () => (await e.state()).selectionBounds).toBeNull()
    await e.page.evaluate(() => window.devHost.configure({}))
    const zoom = e.kit.getByTestId('photo-status-zoom').locator('input')
    await zoom.fill('50')
    await zoom.press('Enter')
    await expect(zoom).toHaveValue('50')
    await openMenu('View')
    const actual = e.kit.getByTestId('photo-control-view.actualPixels')
    await expect(actual).toBeVisible()
    await e.page.evaluate(() => window.devHost.configure({ disable: ['control:view.actualPixels'] }))
    await expect(actual).toBeDisabled()
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(actual).toBeEnabled()
    // Zoom is a kit command, so also check dispatch through the kit's own entry point.
    await actual.click()
    await expect(e.kit.getByTestId('photo-status-zoom').locator('input')).toHaveValue('100')
    await e.page.evaluate(() => window.devHost.configure({ hide: ['menu:Select'] }))
    await expect(e.kit.getByTestId(mode === 'menus' ? 'photo-menu-Select' : 'ribbon-tab-Select')).toHaveCount(0)
  })
}

for (const mode of ['line', 'classic']) {
  test(`UI41: ${mode} nested commands obey menu policy, invert pixels and survive history/save`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [layer(), fill('#ff0000')], 'en-US', mode)
    await e.kit.getByTestId('ribbon-tab-Image').click()
    const levels = e.kit.getByTestId('photo-cmd-image.adjustments.levels')
    await expect(levels).toBeVisible()
    await e.page.evaluate(() => window.devHost.configure({ disable: ['menu:Image/Adjustments'] }))
    await expect(levels).toBeDisabled()
    await e.page.evaluate(() => window.devHost.configure({ hide: ['menu:Image/Adjustments'] }))
    await expect(levels).toHaveCount(0)
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(levels).toBeEnabled()
    const invert = e.kit.getByTestId('photo-cmd-image.adjustments.invert')
    if (mode === 'line') {
      await expect(invert).toHaveCount(0)
      await e.kit.getByRole('button', { name: 'More: Adjustments', exact: true }).click()
      await expect(levels, 'A primary action must mount once when its group opens').toHaveCount(1)
    }
    await expect(invert).toBeVisible()
    await invert.click()
    if (mode === 'line') await e.page.keyboard.press('Escape')
    await e.color(64, 48, [0, 255, 255, 255])
    await e.undo()
    await e.color(64, 48, [255, 0, 0, 255])
    await e.redo()
    await e.color(64, 48, [0, 255, 255, 255])
    await e.reopen()
    await e.color(64, 48, [0, 255, 255, 255])
  })
}

for (const policy of ['disable', 'hide']) {
  for (const toolbar of ['visible', 'closed']) {
    test(`UI42: ${policy} all tools blocks canvas edits with the toolbar ${toolbar}`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer('Ink')], 'en-US', 'classic')
      await e.kit.getByTestId('photo-home-tool-Brush').click()
      await e.kit.getByTestId('photo-foreground').fill('#000000')
      if (toolbar === 'closed') {
        await e.kit.getByTestId('ribbon-tab-Window').click()
        await e.kit.getByTestId('photo-control-window.toolbar').click()
        await expect(e.kit.getByTestId('photo-tool-Brush')).toHaveCount(0)
        await e.kit.getByTestId('ribbon-tab-home').click()
      }
      const before = await e.state()
      await e.page.evaluate((policy) => window.devHost.configure({ [policy]: ['tool:*'], hide: policy === 'hide' ? ['tool:*', 'action:redo'] : ['action:redo'] }), policy)
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      const homeBrush = e.kit.getByTestId('photo-home-tool-Brush')
      if (policy === 'disable') await expect(homeBrush).toBeDisabled()
      else await expect(homeBrush).toHaveCount(0)
      await e.click(64, 48)
      expect(await e.sample(64, 48), 'A denied custom tool must not paint through the canvas').toEqual([255, 255, 255, 255])
      expect((await e.state()).revision).toBe(before.revision)
      await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      await homeBrush.click()
      await e.click(64, 48)
      await e.color(64, 48, [0, 0, 0, 255])
      await e.undo()
      await e.color(64, 48, [255, 255, 255, 255])
      await e.redo()
      await e.color(64, 48, [0, 0, 0, 255])
      await e.reopen()
      await e.color(64, 48, [0, 0, 0, 255])
    })
  }
}

for (const tool of ['Brush', 'RectMarquee']) {
  for (const restore of ['before release', 'after release']) {
    test(`UI43: revoking ${tool} during a drag cancels it even when restored ${restore}`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer('Ink')], 'en-US', 'classic')
      await e.kit.getByTestId(`photo-home-tool-${tool}`).click()
      await e.kit.getByTestId('photo-foreground').fill('#000000')
      const before = await e.state()
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      const point = (x, y) => ({ x: box.x + box.width / 2 + x - before.width / 2, y: box.y + box.height / 2 + y - before.height / 2 })
      const start = point(24, 24), end = point(96, 72)
      await e.page.mouse.move(start.x, start.y)
      await e.page.mouse.down()
      try {
        await e.page.mouse.move(end.x, end.y, { steps: 4 })
        await e.page.evaluate(() => window.devHost.configure({ disable: ['tool:*'], hide: ['action:redo'] }))
        await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
        if (restore === 'before release') {
          await e.page.evaluate(() => window.devHost.configure({}))
          await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
        }
      } finally { await e.page.mouse.up() }
      expect((await e.state()).revision, 'Revoking a tool must discard its uncommitted drag').toBe(before.revision)
      expect((await e.state()).selectionBounds).toBeNull()
      await e.color(60, 48, [255, 255, 255, 255])
      await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      await e.kit.getByTestId(`photo-home-tool-${tool}`).click()
      await e.drag([24, 24], [96, 72])
      if (tool === 'Brush') {
        await e.color(60, 48, [0, 0, 0, 255])
        await e.undo()
        await e.color(60, 48, [255, 255, 255, 255])
        await e.redo()
        await e.color(60, 48, [0, 0, 0, 255])
        await e.reopen()
        await e.color(60, 48, [0, 0, 0, 255])
      } else {
        await expect.poll(async () => (await e.state()).selectionBounds).toEqual([24, 24, 72, 48])
      }
    })
  }
}

for (const tool of ['PolygonLasso', 'MagneticLasso', 'Pen']) {
  for (const restoreFirst of [false, true]) {
    test(`UI44: ${tool} discards pending clicks when ${restoreFirst ? 'hidden then restored before Enter' : 'disabled before Enter'}`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer('Ink')], 'en-US', 'classic')
      if (tool !== 'Pen') {
        await e.kit.getByTestId('photo-tool-Lasso').click({ button: 'right' })
        await e.kit.locator(`.kui-tool-flyout [data-testid="photo-tool-${tool}"]`).click()
      } else await e.kit.getByTestId('photo-tool-Pen').click()
      // Keep the selected tool without relying on the rail's fallback selection.
      await e.kit.getByTestId('ribbon-tab-Window').click()
      await e.kit.getByTestId('photo-control-window.toolbar').click()
      const before = await e.state()
      for (const point of [[16, 16], [48, 16], [48, 64]]) await e.click(...point)
      await e.page.evaluate((restoreFirst) => window.devHost.configure(restoreFirst
        ? { hide: ['tool:*', 'action:redo'] }
        : { disable: ['tool:*'], hide: ['action:redo'] }), restoreFirst)
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (restoreFirst) {
        await e.page.evaluate(() => window.devHost.configure({}))
        await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      }
      await e.page.keyboard.press('Enter')
      expect((await e.state()).revision, 'Revoked pending points must never be submitted by Enter').toBe(before.revision)
      expect((await e.state()).selectionBounds).toBeNull()
      await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      // Enter after restoration must not revive old points, either.
      await e.page.keyboard.press('Enter')
      expect((await e.state()).revision).toBe(before.revision)
      const points = [[64, 24], [104, 24], [104, 72]]
      for (const point of points) await e.click(...point)
      await e.page.keyboard.press('Enter')
      if (tool === 'Pen') {
        const path = () => e.page.evaluate(async () => {
          try {
            return (await window.devHost.connection().ai.run([{ command: 'path.info', params: { name: 'work' } }], false)).completed[0]?.result?.path
          } catch (error) {
            if (error.message.includes('no work path')) return undefined
            throw error
          }
        })
        // WebKit rounds native mouse coordinates when the canvas center is a half pixel.
        // Compare document pixels here; retain exact fractional anchors for the PSD check.
        await expect.poll(async () => (await path())?.subpaths?.[0]?.knots?.map((knot) => knot.anchor.map(Math.round))).toEqual(points)
        const expected = await path()
        await e.undo()
        await expect.poll(path).toBeUndefined()
        await e.redo()
        await expect.poll(path).toEqual(expected)
        await e.reopen()
        expect(await path()).toEqual(expected)
      } else {
        await expect.poll(async () => (await e.state()).selectionBounds).toEqual([64, 24, 40, 48])
        await e.undo()
        await expect.poll(async () => (await e.state()).selectionBounds).toBeNull()
        await e.redo()
        await expect.poll(async () => (await e.state()).selectionBounds).toEqual([64, 24, 40, 48])
      }
    })
  }
}

for (const policy of ['disable', 'hide']) {
  for (const restoreFirst of [false, true]) {
    test(`UI45: crop proposal is discarded on ${policy}, with Enter ${restoreFirst ? 'after restoration' : 'while denied'}`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer(), fill('#204060')], 'en-US', 'classic')
      await e.kit.getByTestId('photo-home-tool-Crop').click()
      await e.kit.getByTestId('ribbon-tab-Window').click()
      await e.kit.getByTestId('photo-control-window.toolbar').click()
      await e.drag([24, 24], [88, 72])
      await expect(e.kit.getByTestId('photo-crop-apply')).toBeEnabled()
      const before = await e.state()
      await e.page.evaluate((policy) => window.devHost.configure({ [policy]: ['tool:*'], hide: policy === 'hide' ? ['tool:*', 'action:redo'] : ['action:redo'] }), policy)
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (restoreFirst) {
        await e.page.evaluate(() => window.devHost.configure({}))
        await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      }
      await e.page.keyboard.press('Enter')
      expect((await e.state()).width).toBe(128)
      expect((await e.state()).height).toBe(96)
      expect((await e.state()).revision).toBe(before.revision)
      await expect(e.kit.getByTestId('photo-crop-apply')).toBeDisabled()
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      await e.page.keyboard.press('Enter')
      expect((await e.state()).width).toBe(128)
      await e.drag([24, 24], [88, 72])
      await e.page.keyboard.press('Enter')
      await expect.poll(async () => (await e.state()).width).toBe(64)
      expect((await e.state()).height).toBe(48)
      await e.undo()
      await expect.poll(async () => (await e.state()).width).toBe(128)
      await e.redo()
      await expect.poll(async () => (await e.state()).width).toBe(64)
      await e.reopen()
      expect((await e.state()).height).toBe(48)
    })
  }
}

for (const policy of ['disable', 'hide']) {
  test(`UI46: ${policy} dismisses a pending rasterize prompt without changing the layer`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [shape], 'en-US', 'classic')
    await e.kit.getByTestId('photo-home-tool-Brush').click()
    await e.kit.getByTestId('photo-foreground').fill('#000000')
    await e.click(64, 48)
    const confirm = e.kit.getByTestId('photo-rasterize-confirm')
    await expect(confirm).toBeVisible()
    const before = await e.state()
    await e.page.evaluate((policy) => window.devHost.configure({ [policy]: ['tool:*'], hide: policy === 'hide' ? ['tool:*', 'action:redo'] : ['action:redo'] }), policy)
    await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
    // If an obsolete prompt survives, exercise its real confirmation to expose the edit.
    if (await confirm.count()) await confirm.click()
    await e.color(64, 48, [255, 0, 0, 255])
    expect(structure(await e.state())).toEqual(structure(before))
    expect((await e.state()).revision).toBe(before.revision)
    await expect(confirm).toHaveCount(0)
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
    await e.kit.getByTestId('ribbon-tab-home').click()
    await e.kit.getByTestId('photo-home-tool-Brush').click()
    await e.click(64, 48)
    await confirm.click()
    await e.color(64, 48, [0, 0, 0, 255])
    await e.undo()
    await e.color(64, 48, [255, 0, 0, 255])
    await e.undo()
    await expect.poll(async () => (await e.state()).layers.find((item) => item.id === before.activeLayer)?.kind).toBe('Shape')
    await e.redo()
    await e.redo()
    await e.color(64, 48, [0, 0, 0, 255])
    await e.reopen()
    await e.color(64, 48, [0, 0, 0, 255])
  })
}

for (const tool of ['Type', 'VerticalType']) {
  for (const route of ['Enter', 'save shortcut', 'toolbar Save', 'blur']) {
    test(`UI47: denied ${tool} preserves its draft through ${route} and commits only after restoration`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [], 'en-US', 'classic')
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 50)
      const field = e.kit.getByTestId('photo-text-field')
      const title = `Keep ${tool} draft`
      await field.fill(title)
      const before = await e.state()
      const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
      await e.page.evaluate((vertical) => window.devHost.configure(vertical
        ? { hide: ['tool:*', 'action:redo'] }
        : { disable: ['tool:*'], hide: ['action:redo'] }), tool === 'VerticalType')
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (route === 'Enter') await field.press('Enter')
      else if (route === 'save shortcut') await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
      else if (route === 'toolbar Save') await e.kit.getByTestId('photo-save').click()
      else await e.kit.getByTestId('photo-status-zoom').locator('input').focus()
      await expect(field, 'Denied Type must retain the unsubmitted text').toHaveValue(title)
      await expect(field).toHaveAttribute('readonly', '')
      await expect(e.kit.getByTestId('photo-option-commit')).toBeDisabled()
      expect((await e.state()).revision).toBe(before.revision)
      expect((await e.state()).layers.filter((item) => item.kind === 'Type')).toHaveLength(0)
      expect(await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBe(version)
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      await expect(field).toBeEditable()
      await expect(e.kit.getByTestId('photo-option-commit')).toBeEnabled()
      await field.fill(`${title} restored`)
      await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
      await expect(field).toHaveCount(0)
      await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBeGreaterThan(version)
      const texts = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await expect.poll(async () => (await texts()).length).toBe(1)
      expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject({ text: `${title} restored`, orientation: tool === 'Type' ? 'Horizontal' : 'Vertical' })
      await e.undo()
      await expect.poll(async () => (await texts()).length).toBe(0)
      await e.redo()
      await expect.poll(async () => (await texts()).length).toBe(1)
      await e.reopen()
      expect(await e.run('type.info', { layer: (await texts())[0].id })).toMatchObject({ text: `${title} restored` })
    })
  }
}

for (const restoreFirst of [false, true]) {
  test(`UI48: rasterization completion cannot replay a revoked brush ${restoreFirst ? 'after restoration' : 'while denied'}`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [shape], 'en-US', 'classic')
    await e.kit.getByTestId('photo-home-tool-Brush').click()
    await e.kit.getByTestId('photo-foreground').fill('#000000')
    await e.click(64, 48)
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      window.rasterGate = { held: false, release: () => {} }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'layer.rasterize.shape') {
          window.rasterGate.held = true
          window.rasterGate.release = () => { Worker.prototype.postMessage = post; post.call(this, message, ...args) }
        } else post.call(this, message, ...args)
      }
    })
    try {
      await e.kit.getByTestId('photo-rasterize-confirm').click()
      await expect.poll(() => surface.evaluate(() => window.rasterGate.held)).toBe(true)
      await e.page.evaluate(() => window.devHost.configure({ disable: ['tool:*'], hide: ['action:redo'] }))
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (restoreFirst) {
        await e.page.evaluate(() => window.devHost.configure({}))
        await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      }
    } finally { await surface.evaluate(() => window.rasterGate.release()) }
    await expect.poll(async () => (await e.state()).layers.find((item) => item.name === 'Banner')?.kind).toBe('Pixel')
    await e.color(64, 48, [255, 0, 0, 255])
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
    await e.undo()
    await expect.poll(async () => (await e.state()).layers.find((item) => item.name === 'Banner')?.kind).toBe('Shape')
    await e.redo()
    await expect.poll(async () => (await e.state()).layers.find((item) => item.name === 'Banner')?.kind).toBe('Pixel')
    await e.click(64, 48)
    await e.color(64, 48, [0, 0, 0, 255])
    await e.reopen()
    await e.color(64, 48, [0, 0, 0, 255])
  })

  test(`UI49: revoked Move discards a released drag awaiting auto-select ${restoreFirst ? 'after restoration' : 'while denied'}`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [shape], 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Move').click()
    const before = await e.state()
    const originalBounds = before.layers.find((item) => item.id === before.activeLayer).bounds
    const bounds = async () => (await e.state()).layers.find((item) => item.id === before.activeLayer).bounds
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      window.pickGate = { held: false, release: () => {} }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'layer.pickAt' && message.request.params.select === false) {
          window.pickGate.held = true
          window.pickGate.release = () => { Worker.prototype.postMessage = post; post.call(this, message, ...args) }
        } else post.call(this, message, ...args)
      }
    })
    try {
      await e.drag([32, 32], [56, 40])
      await expect.poll(() => surface.evaluate(() => window.pickGate.held)).toBe(true)
      await e.page.evaluate(() => window.devHost.configure({ disable: ['tool:*'], hide: ['action:redo'] }))
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (restoreFirst) {
        await e.page.evaluate(() => window.devHost.configure({}))
        await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      }
    } finally { await surface.evaluate(() => window.pickGate.release()) }
    await e.state()
    await surface.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    expect(await bounds()).toEqual(originalBounds)
    expect((await e.state()).revision).toBe(before.revision)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
    await e.drag([32, 32], [56, 40])
    await expect.poll(async () => (await bounds())[0]).toBe(originalBounds[0] + 24)
    expect((await bounds())[1]).toBe(originalBounds[1] + 8)
    await e.undo()
    await expect.poll(bounds).toEqual(originalBounds)
    await e.redo()
    await expect.poll(async () => (await bounds())[0]).toBe(originalBounds[0] + 24)
    await e.reopen()
  })
}

for (const nextField of ['zoom', 'opacity']) {
  test(`UI50: completing delayed rasterization preserves a later ${nextField} focus choice`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [shape], 'en-US', 'classic')
    await e.kit.getByTestId('photo-home-tool-Brush').click()
    await e.kit.getByTestId('photo-foreground').fill('#000000')
    await e.click(64, 48)
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      window.focusGate = { held: false, release: () => {} }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'layer.rasterize.shape') {
          window.focusGate.held = true
          window.focusGate.release = () => { Worker.prototype.postMessage = post; post.call(this, message, ...args) }
        } else post.call(this, message, ...args)
      }
    })
    const next = nextField === 'zoom' ? e.kit.getByTestId('photo-status-zoom').locator('input') : e.kit.locator('[data-option="opacity"] input')
    try {
      await e.kit.getByTestId('photo-rasterize-confirm').click()
      await expect.poll(() => surface.evaluate(() => window.focusGate.held)).toBe(true)
      await next.focus()
    } finally { await surface.evaluate(() => window.focusGate.release()) }
    await e.color(64, 48, [0, 0, 0, 255])
    await expect(next).toBeFocused()
    await next.fill('50')
    await next.press('Enter')
    await expect(next).toHaveValue('50')
    await e.kit.getByTestId('photo-undo').click()
    await expect.poll(async () => await e.sample(64, 48)).toEqual([255, 0, 0, 255])
  })
}

for (const draft of ['crop', 'rasterize']) {
  test(`UI51: external file reload discards the old ${draft} proposal`, async ({ editor: e }) => {
    await e.open('Scenario.psd', draft === 'rasterize' ? [shape] : [], 'en-US', 'classic')
    if (draft === 'crop') {
      await e.kit.getByTestId('photo-home-tool-Crop').click()
      await e.drag([24, 24], [88, 72])
    } else {
      await e.kit.getByTestId('photo-foreground').fill('#000000')
      await e.click(64, 48)
      await expect(e.kit.getByTestId('photo-rasterize-confirm')).toBeVisible()
    }
    // A real file-stamp change reloads the engine in the existing iframe and App.
    // Opening through the picker replaces the whole iframe and would miss stale UI state.
    const bytes = createFixture('Scenario.psd', [{ command: 'shape.create', params: { ...shape.params, fill: '#0000ff', name: 'Replacement' } }])
    await e.page.evaluate((bytes) => {
      const old = window.devHost.files.get('Scenario.psd')
      window.devHost.files.set('Scenario.psd', { bytes: new Uint8Array(bytes).buffer, version: old.version + 1 })
    }, [...bytes])
    await expect.poll(async () => (await e.state()).layers.some((item) => item.name === 'Replacement')).toBe(true)
    const fresh = await e.state()
    if (draft === 'rasterize') {
      const confirm = e.kit.getByTestId('photo-rasterize-confirm')
      if (await confirm.count()) await confirm.click()
      await expect(confirm).toHaveCount(0)
      expect((await e.state()).layers.find((item) => item.name === 'Replacement')?.kind).toBe('Shape')
      expect(await e.sample(64, 48)).toEqual([0, 0, 255, 255])
    } else {
      await e.page.keyboard.press('Enter')
      expect((await e.state()).width).toBe(128)
      expect((await e.state()).height).toBe(96)
      await expect(e.kit.getByTestId('photo-crop-apply')).toBeDisabled()
    }
    expect((await e.state()).revision).toBe(fresh.revision)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  })
}

for (const controls of ['options', 'properties', 'cross controls']) {
  test(`UI52: ${controls} keep the latest font when an earlier family query finishes last`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [{ command: 'type.create', params: { x: 12, y: 40, text: 'Latest font wins', font: 'Inter', size: 22 } }], 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Type').click()
    const id = (await e.state()).activeLayer
    const options = e.kit.getByTestId('photo-option-font')
    const properties = e.kit.getByTestId('photo-props-type-font')
    const first = controls === 'properties' ? properties : options
    const second = controls === 'options' ? options : properties
    await expect(first).toHaveValue('Inter')
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      const sends = [], ids = new Set(), listened = new WeakSet()
      window.fontGate = { held: 0, replies: 0, release: () => {
        Worker.prototype.postMessage = post
        for (const send of sends.splice(0)) send()
      } }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'type.fonts' && message.request.params.family === 'JetBrains Mono') {
          if (!listened.has(this)) {
            listened.add(this)
            this.addEventListener('message', (event) => { if (ids.delete(event.data.id)) window.fontGate.replies++ })
          }
          ids.add(message.id)
          window.fontGate.held++
          sends.push(() => post.call(this, message, ...args))
        } else post.call(this, message, ...args)
      }
    })
    try {
      await first.selectOption('JetBrains Mono')
      await expect.poll(() => surface.evaluate(() => window.fontGate.held)).toBeGreaterThan(0)
      await second.selectOption('Droid Sans Fallback')
      await expect.poll(async () => (await e.run('type.info', { layer: id })).runs[0].style.font_family).toBe('Droid Sans Fallback')
    } finally { await surface.evaluate(() => window.fontGate.release()) }
    await expect.poll(() => surface.evaluate(() => window.fontGate.replies === window.fontGate.held)).toBe(true)
    await surface.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    expect((await e.run('type.info', { layer: id })).runs[0].style.font_family).toBe('Droid Sans Fallback')
    await expect(options).toHaveValue('Droid Sans Fallback')
    await expect(properties).toHaveValue('Droid Sans Fallback')
    await e.kit.getByTestId('photo-undo').click()
    await expect.poll(async () => (await e.run('type.info', { layer: id })).runs[0].style.font_family).toBe('Inter')
    await e.kit.getByTestId('photo-redo').click()
    await expect.poll(async () => (await e.run('type.info', { layer: id })).runs[0].style.font_family).toBe('Droid Sans Fallback')
    await e.reopen()
    expect((await e.run('type.info', { layer: (await e.state()).activeLayer })).runs[0].style.font_family).toBe('Droid Sans Fallback')
  })
}

for (const control of ['options', 'properties']) {
  test(`UI53: a pending ${control} font lookup cannot restyle an externally reloaded document`, async ({ editor: e }) => {
    const makeTitle = (text) => [{ command: 'type.create', params: { x: 12, y: 40, text, font: 'Inter', size: 22 } }]
    await e.open('Scenario.psd', makeTitle('Original title'), 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Type').click()
    const select = e.kit.getByTestId(control === 'options' ? 'photo-option-font' : 'photo-props-type-font')
    await expect(select).toHaveValue('Inter')
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage
      window.reloadFontGate = { held: false, restore: () => { Worker.prototype.postMessage = post } }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'type.fonts' && message.request.params.family === 'JetBrains Mono') {
          window.reloadFontGate.held = true
          // The old engine's disposal must settle this pending query without applying it.
        } else post.call(this, message, ...args)
      }
    })
    try {
      await select.selectOption('JetBrains Mono')
      await expect.poll(() => surface.evaluate(() => window.reloadFontGate.held)).toBe(true)
      await e.page.evaluate((bytes) => {
        const old = window.devHost.files.get('Scenario.psd')
        window.devHost.files.set('Scenario.psd', { bytes: new Uint8Array(bytes).buffer, version: old.version + 1 })
      }, [...createFixture('Scenario.psd', makeTitle('Replacement title'))])
      await expect.poll(async () => {
        try { return (await e.run('type.info', { layer: (await e.state()).activeLayer })).text } catch { return null }
      }).toBe('Replacement title')
    } finally { await surface.evaluate(() => window.reloadFontGate.restore()) }
    await surface.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    expect((await e.run('type.info', { layer: (await e.state()).activeLayer })).runs[0].style.font_family).toBe('Inter')
    await expect(select).toHaveValue('Inter')
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    await select.selectOption('JetBrains Mono')
    await expect.poll(async () => (await e.run('type.info', { layer: (await e.state()).activeLayer })).runs[0].style.font_family).toBe('JetBrains Mono')
    await e.reopen()
    expect((await e.run('type.info', { layer: (await e.state()).activeLayer })).text).toBe('Replacement title')
  })
}

// A file-stamp refresh preserves the editor instance, unlike opening through the picker.
async function replaceExternal(e, commands, marker) {
  await e.page.evaluate((bytes) => {
    const old = window.devHost.files.get('Scenario.psd')
    window.devHost.files.set('Scenario.psd', { bytes: new Uint8Array(bytes).buffer, version: old.version + 1 })
  }, [...createFixture('Scenario.psd', commands)])
  await expect.poll(async () => {
    try { return (await e.state()).layers.some((item) => item.name === marker) } catch { return false }
  }).toBe(true)
}

test('UI54: external reload clears Clone Stamp source before a new stroke', async ({ editor: e }) => {
  const patch = { command: 'paint.stroke', params: { points: [[32, 48, 1]], size: 24, hardness: 1, color: '#ff0000' } }
  await e.open('Scenario.psd', [layer('Original'), patch], 'en-US', 'classic')
  await e.kit.getByTestId('photo-tool-CloneStamp').click()
  await e.page.keyboard.down('Alt')
  try { await e.click(32, 48) } finally { await e.page.keyboard.up('Alt') }
  await replaceExternal(e, [layer('Replacement'), patch], 'Replacement')
  await e.click(96, 48)
  await e.color(96, 48, [255, 255, 255, 255])
  await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  await e.page.keyboard.down('Alt')
  try { await e.click(32, 48) } finally { await e.page.keyboard.up('Alt') }
  await e.click(96, 48)
  await e.color(96, 48, [255, 0, 0, 255])
  await e.undo()
  await e.color(96, 48, [255, 255, 255, 255])
  await e.redo()
  await e.color(96, 48, [255, 0, 0, 255])
  await e.reopen()
  await e.color(96, 48, [255, 0, 0, 255])
})

test('UI55: external reload starts painting on pixels instead of the previous document mask', async ({ editor: e }) => {
  const masked = (name) => [layer(name), fill('#ff0000'), { command: 'layer.layerMask.revealAll' }]
  await e.open('Scenario.psd', masked('Original'), 'en-US', 'classic')
  const id = (await e.state()).activeLayer
  await e.kit.getByTestId(`photo-layer-mask-${id}`).click()
  await expect(e.kit.getByTestId(`photo-layer-mask-${id}`)).toHaveAttribute('data-target', 'true')
  await replaceExternal(e, masked('Replacement'), 'Replacement')
  const fresh = (await e.state()).activeLayer
  await expect(e.kit.getByTestId(`photo-layer-thumb-${fresh}`)).toHaveAttribute('data-target', 'true')
  await e.kit.getByTestId('photo-foreground').fill('#000000')
  await e.click(64, 48)
  await e.color(64, 48, [0, 0, 0, 255])
  await e.undo()
  await e.color(64, 48, [255, 0, 0, 255])
  await e.redo()
  await e.color(64, 48, [0, 0, 0, 255])
  // A new mask selection still routes painting to the mask.
  await e.kit.getByTestId(`photo-layer-mask-${fresh}`).click()
  await expect(e.kit.getByTestId(`photo-layer-mask-${fresh}`)).toHaveAttribute('data-target', 'true')
  await e.click(96, 48)
  await e.color(96, 48, [255, 255, 255, 255])
  await e.reopen()
  await e.color(64, 48, [0, 0, 0, 255])
  await e.color(96, 48, [255, 255, 255, 255])
})

test('UI56: a brush waiting for mask metadata cannot edit an externally replaced document', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer('Initial')], 'en-US', 'classic')
  const surface = e.kit.getByTestId('kit-canvas-surface')
  await surface.evaluate(() => {
    const post = Worker.prototype.postMessage
    window.maskGate = { held: false, restore: () => { Worker.prototype.postMessage = post } }
    Worker.prototype.postMessage = function (message, ...args) {
      if (!window.maskGate.held && message?.request?.method === 'invoke' && message.request.name === 'maskTargetCommands') {
        window.maskGate.held = true
        // Engine disposal settles the pending metadata request; no manual rejection needed.
      } else post.call(this, message, ...args)
    }
  })
  try {
    await replaceExternal(e, [layer('Waiting')], 'Waiting')
    await expect.poll(() => surface.evaluate(() => window.maskGate.held)).toBe(true)
    await e.kit.getByTestId('photo-foreground').fill('#000000')
    await e.click(64, 48)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    await replaceExternal(e, [layer('Replacement')], 'Replacement')
  } finally { await surface.evaluate(() => window.maskGate.restore()) }
  await surface.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  await e.color(64, 48, [255, 255, 255, 255])
  await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  await e.click(64, 48)
  await e.color(64, 48, [0, 0, 0, 255])
  await e.undo()
  await e.color(64, 48, [255, 255, 255, 255])
  await e.redo()
  await e.reopen()
  await e.color(64, 48, [0, 0, 0, 255])
})

async function holdRedSample(e) {
  const surface = e.kit.getByTestId('kit-canvas-surface')
  await surface.evaluate(() => {
    const post = Worker.prototype.postMessage
    const sends = [], ids = new Set(), listened = new WeakSet()
    window.sampleGate = { held: 0, replies: 0, release: () => {
      Worker.prototype.postMessage = post
      for (const send of sends.splice(0)) send()
    } }
    Worker.prototype.postMessage = function (message, ...args) {
      if (message?.request?.method === 'execute' && message.request.command === 'document.sampleColor' && message.request.params.x === 32) {
        if (!listened.has(this)) {
          listened.add(this)
          this.addEventListener('message', (event) => { if (ids.delete(event.data.id)) window.sampleGate.replies++ })
        }
        ids.add(message.id)
        window.sampleGate.held++
        sends.push(() => post.call(this, message, ...args))
      } else post.call(this, message, ...args)
    }
  })
  return surface
}

const samplePatches = [layer('Colors'),
  { command: 'paint.stroke', params: { points: [[32, 48, 1]], size: 24, hardness: 1, color: '#ff0000' } },
  { command: 'paint.stroke', params: { points: [[96, 48, 1]], size: 24, hardness: 1, color: '#0000ff' } },
]

for (const target of ['foreground', 'background']) {
  test(`UI57: the latest Eyedropper ${target} sample wins when queries finish out of order`, async ({ editor: e }) => {
    await e.open('Scenario.psd', samplePatches, 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Eyedropper').click()
    const surface = await holdRedSample(e)
    if (target === 'background') await e.page.keyboard.down('Alt')
    try {
      await e.click(32, 48)
      await expect.poll(() => surface.evaluate(() => window.sampleGate.held)).toBeGreaterThan(0)
      await e.click(96, 48)
      await expect(e.kit.getByTestId(`photo-${target}`)).toHaveValue('#0000ff')
    } finally {
      await e.page.keyboard.up('Alt')
      await surface.evaluate(() => window.sampleGate.release())
    }
    await expect.poll(() => surface.evaluate(() => window.sampleGate.replies === window.sampleGate.held)).toBe(true)
    await surface.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    await expect(e.kit.getByTestId(`photo-${target}`)).toHaveValue('#0000ff')
    const colors = await e.run('session.inspect')
    expect(colors[target].slice(0, 3)).toEqual([0, 0, 1])
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  })
}

for (const restoreFirst of [false, true]) {
  test(`UI58: a revoked Eyedropper sample is discarded ${restoreFirst ? 'after restoration' : 'while denied'}`, async ({ editor: e }) => {
    await e.open('Scenario.psd', samplePatches, 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Eyedropper').click()
    await e.kit.getByTestId('photo-foreground').fill('#00ff00')
    const surface = await holdRedSample(e)
    try {
      await e.click(32, 48)
      await expect.poll(() => surface.evaluate(() => window.sampleGate.held)).toBeGreaterThan(0)
      await e.page.evaluate(() => window.devHost.configure({ disable: ['tool:*'], hide: ['action:redo'] }))
      await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
      if (restoreFirst) {
        await e.page.evaluate(() => window.devHost.configure({}))
        await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      }
    } finally { await surface.evaluate(() => window.sampleGate.release()) }
    await expect.poll(() => surface.evaluate(() => window.sampleGate.replies === window.sampleGate.held)).toBe(true)
    await surface.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
    await expect(e.kit.getByTestId('photo-foreground')).toHaveValue('#00ff00')
    await e.page.evaluate(() => window.devHost.configure({}))
    await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
    await e.click(96, 48)
    await expect(e.kit.getByTestId('photo-foreground')).toHaveValue('#0000ff')
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  })
}

for (const panel of ['navigator', 'histogram', 'info']) {
  test(`UI59: ${panel} refreshes when an external replacement has the same revision`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [layer('Original'), fill('#ff0000')], 'en-US', 'menus')
    await e.kit.getByTestId('photo-menu-Window').click()
    await e.kit.getByTestId(`photo-control-window.panel.${panel}`).click()
    await expect(e.kit.getByTestId(`photo-${panel}`)).toBeVisible()
    if (panel === 'histogram') await e.kit.getByTestId('photo-histogram-channel').selectOption('red')
    if (panel === 'info') {
      const box = await e.kit.getByTestId('kit-canvas-surface').boundingBox()
      await e.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
    }
    const shown = async () => {
      if (panel === 'navigator') return e.kit.getByTestId('photo-navigator-image').evaluate((canvas) => [...canvas.getContext('2d').getImageData(Math.floor(canvas.width / 2), Math.floor(canvas.height / 2), 1, 1).data].slice(0, 3))
      if (panel === 'histogram') return e.kit.getByTestId('photo-histogram-stats').locator('dd').first().textContent()
      return Promise.all(['r', 'g', 'b'].map(async (channel) => Number(await e.kit.getByTestId(`photo-info-${channel}`).textContent())))
    }
    await expect.poll(shown).toEqual(panel === 'histogram' ? '255.00' : [255, 0, 0])
    const revision = (await e.state()).revision
    await replaceExternal(e, [layer('Replacement'), fill('#0000ff')], 'Replacement')
    expect((await e.state()).revision).toBe(revision)
    await expect.poll(shown).toEqual(panel === 'histogram' ? '0.00' : [0, 0, 255])
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  })
}

async function startLayerReorder(e) {
  const layers = (await e.state()).layers
  const upper = layers.find((item) => item.name.endsWith('Top'))
  const lower = layers.find((item) => item.name.endsWith('Bottom'))
  const from = await e.kit.getByTestId(`photo-layer-${upper.id}`).boundingBox()
  const row = await e.kit.locator(`[data-layer-row="${lower.id}"]`).boundingBox()
  const to = { x: from.x + from.width / 2, y: row.y + row.height - 3 }
  await e.page.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
  await e.page.mouse.down()
  await e.page.mouse.move(to.x, to.y, { steps: 8 })
  return to
}

for (const policy of ['disable', 'hide']) {
  for (const restoreFirst of [false, true]) {
    test(`UI60: layer drag is cancelled when the list is ${policy === 'hide' ? 'hidden' : 'disabled'} ${restoreFirst ? 'and restored before release' : 'before release'}`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [layer('Bottom'), layer('Top')], 'en-US', 'classic')
      const before = await e.state()
      await startLayerReorder(e)
      try {
        await e.page.evaluate((policy) => window.devHost.configure({ [policy]: ['control:layers.list'], hide: policy === 'hide' ? ['control:layers.list', 'action:redo'] : ['action:redo'] }), policy)
        await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
        if (restoreFirst) {
          await e.page.evaluate(() => window.devHost.configure({}))
          await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
        }
      } finally { await e.page.mouse.up() }
      await e.kit.getByTestId('kit-canvas-surface').evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
      expect((await e.state()).layers.map((item) => item.name)).toEqual(before.layers.map((item) => item.name))
      expect((await e.state()).revision).toBe(before.revision)
      await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      await e.page.evaluate(() => window.devHost.configure({}))
      await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      await startLayerReorder(e)
      await e.page.mouse.up()
      await expect.poll(async () => (await e.state()).layers.map((item) => item.name)).toEqual(['Bottom', 'Top', 'Background'])
      await e.kit.getByTestId('photo-undo').click()
      await expect.poll(async () => (await e.state()).layers.map((item) => item.name)).toEqual(['Top', 'Bottom', 'Background'])
      await e.kit.getByTestId('photo-redo').click()
      await e.reopen()
    })
  }
}

test('UI61: an active layer drag cannot reorder an externally replaced document', async ({ editor: e }) => {
  await e.open('Scenario.psd', [layer('Original Bottom'), layer('Original Top')], 'en-US', 'classic')
  await startLayerReorder(e)
  try {
    await replaceExternal(e, [layer('Replacement Bottom'), layer('Replacement Top')], 'Replacement Top')
  } finally { await e.page.mouse.up() }
  await e.kit.getByTestId('kit-canvas-surface').evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  expect((await e.state()).layers.map((item) => item.name)).toEqual(['Replacement Top', 'Replacement Bottom', 'Background'])
  await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
  await startLayerReorder(e)
  await e.page.mouse.up()
  await expect.poll(async () => (await e.state()).layers.map((item) => item.name)).toEqual(['Replacement Bottom', 'Replacement Top', 'Background'])
  await e.kit.getByTestId('photo-undo').click()
  await expect.poll(async () => (await e.state()).layers.map((item) => item.name)).toEqual(['Replacement Top', 'Replacement Bottom', 'Background'])
  await e.kit.getByTestId('photo-redo').click()
  await e.reopen()
})

for (const tool of ['Type', 'VerticalType']) {
  for (const route of ['Enter', 'Save']) {
    test(`UI62: ${tool} ${route} keeps the chosen font while its metadata is pending`, async ({ editor: e }) => {
      await e.open('Scenario.psd', [], 'en-US', 'classic')
      if (tool === 'VerticalType') await e.kit.getByTestId('photo-tool-Type').click({ button: 'right' })
      await e.kit.getByTestId(`photo-tool-${tool}`).click()
      await e.click(24, 48)
      const field = e.kit.getByTestId('photo-text-field')
      await field.fill('Pending font')
      const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
      const surface = e.kit.getByTestId('kit-canvas-surface')
      await surface.evaluate(() => {
        const post = Worker.prototype.postMessage, sends = []
        window.draftFontGate = { held: false, release: () => {
          Worker.prototype.postMessage = post
          for (const send of sends.splice(0)) send()
        } }
        Worker.prototype.postMessage = function (message, ...args) {
          if (message?.request?.method === 'execute' && message.request.command === 'type.fonts' && message.request.params.family === 'JetBrains Mono') {
            window.draftFontGate.held = true
            sends.push(() => post.call(this, message, ...args))
          } else post.call(this, message, ...args)
        }
      })
      try {
        await e.kit.getByTestId('photo-option-font').selectOption('JetBrains Mono')
        await expect.poll(() => surface.evaluate(() => window.draftFontGate.held)).toBe(true)
        if (route === 'Save') await e.kit.getByTestId('photo-save').click()
        else await field.press('Enter')
      } finally { await surface.evaluate(() => window.draftFontGate.release()) }
      const titles = async () => (await e.state()).layers.filter((item) => item.kind === 'Type')
      await expect.poll(async () => (await titles()).length).toBe(1)
      const info = await e.run('type.info', { layer: (await titles())[0].id })
      expect(info).toMatchObject({ text: 'Pending font', orientation: tool === 'Type' ? 'Horizontal' : 'Vertical', runs: [{ style: { font_family: 'JetBrains Mono' } }] })
      if (route === 'Save') {
        await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBeGreaterThan(version)
        await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
      }
      await e.kit.getByTestId('photo-undo').click()
      await expect.poll(async () => (await titles()).length).toBe(0)
      await e.kit.getByTestId('photo-redo').click()
      await expect.poll(async () => (await titles()).length).toBe(1)
      await e.reopen()
      expect((await e.run('type.info', { layer: (await titles())[0].id })).runs[0].style.font_family).toBe('JetBrains Mono')
    })
  }
}

for (const interruption of ['permission restoration', 'document replacement']) {
  test(`UI63: a draft waiting for its font remains recoverable after ${interruption}`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [], 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Type').click()
    await e.click(24, 48)
    const field = e.kit.getByTestId('photo-text-field')
    await field.fill('Retained font draft')
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage, sends = []
      window.cancelFontGate = { held: false, release: (discard) => {
        Worker.prototype.postMessage = post
        for (const send of sends.splice(0)) if (!discard) send()
      } }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'type.fonts' && message.request.params.family === 'JetBrains Mono') {
          window.cancelFontGate.held = true
          sends.push(() => post.call(this, message, ...args))
        } else post.call(this, message, ...args)
      }
    })
    const replaced = interruption === 'document replacement'
    try {
      await e.kit.getByTestId('photo-option-font').selectOption('JetBrains Mono')
      await expect.poll(() => surface.evaluate(() => window.cancelFontGate.held)).toBe(true)
      await field.press('Enter')
      await expect(field).toHaveCount(0)
      expect((await e.state()).layers.filter((item) => item.kind === 'Type')).toHaveLength(0)
      if (replaced) await replaceExternal(e, [layer('Replacement')], 'Replacement')
      else {
        await e.page.evaluate(() => window.devHost.configure({ disable: ['tool:*'], hide: ['action:redo'] }))
        await expect(e.kit.getByTestId('photo-redo')).toHaveCount(0)
        await e.page.evaluate(() => window.devHost.configure({}))
        await expect(e.kit.getByTestId('photo-redo')).toBeVisible()
      }
    } finally { await surface.evaluate((_, discard) => window.cancelFontGate.release(discard), replaced) }
    await expect(field).toHaveValue('Retained font draft')
    await expect(field).toBeEditable()
    expect((await e.state()).layers.filter((item) => item.kind === 'Type')).toHaveLength(0)
    // Explicit retry uses the current document and the latest selected font.
    await e.kit.getByTestId('photo-option-font').selectOption('JetBrains Mono')
    await expect(e.kit.getByTestId('photo-option-font')).toHaveValue('JetBrains Mono')
    await field.press(await shortcut(e.page, 'ControlOrMeta+s'))
    await expect(field).toHaveCount(0)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    const titles = (await e.state()).layers.filter((item) => item.kind === 'Type')
    expect(titles).toHaveLength(1)
    expect(await e.run('type.info', { layer: titles[0].id })).toMatchObject({ text: 'Retained font draft', runs: [{ style: { font_family: 'JetBrains Mono' } }] })
    const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
    await e.reopen('Scenario.psd', bytes)
  })
}

for (const control of ['options', 'properties']) {
  test(`UI64: Save waits for an existing title's pending ${control} font choice`, async ({ editor: e }) => {
    await e.open('Scenario.psd', [{ command: 'type.create', params: { x: 16, y: 48, text: 'Saved font choice', font: 'Inter', size: 24 } }], 'en-US', 'classic')
    await e.kit.getByTestId('photo-tool-Type').click()
    const id = (await e.state()).activeLayer
    const select = e.kit.getByTestId(control === 'options' ? 'photo-option-font' : 'photo-props-type-font')
    await expect(select).toHaveValue('Inter')
    const version = await e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)
    const surface = e.kit.getByTestId('kit-canvas-surface')
    await surface.evaluate(() => {
      const post = Worker.prototype.postMessage, sends = []
      window.savedFontGate = { held: false, release: () => {
        Worker.prototype.postMessage = post
        for (const send of sends.splice(0)) send()
      } }
      Worker.prototype.postMessage = function (message, ...args) {
        if (message?.request?.method === 'execute' && message.request.command === 'type.fonts' && message.request.params.family === 'JetBrains Mono') {
          window.savedFontGate.held = true
          sends.push(() => post.call(this, message, ...args))
        } else post.call(this, message, ...args)
      }
    })
    try {
      await select.selectOption('JetBrains Mono')
      await expect.poll(() => surface.evaluate(() => window.savedFontGate.held)).toBe(true)
      await e.kit.locator('.kui-row[tabindex="-1"]').focus()
      await e.page.keyboard.press(await shortcut(e.page, 'ControlOrMeta+s'))
    } finally { await surface.evaluate(() => window.savedFontGate.release()) }
    await expect.poll(async () => (await e.run('type.info', { layer: id })).runs[0].style.font_family).toBe('JetBrains Mono')
    await expect.poll(() => e.page.evaluate(() => window.devHost.files.get('Scenario.psd').version)).toBeGreaterThan(version)
    await expect(e.kit.getByTestId('photo-save')).toBeDisabled()
    const bytes = Buffer.from(await e.page.evaluate(() => [...new Uint8Array(window.devHost.files.get('Scenario.psd').bytes)]))
    await e.reopen('Scenario.psd', bytes)
    expect((await e.run('type.info', { layer: (await e.state()).activeLayer })).runs[0].style.font_family).toBe('JetBrains Mono')
  })
}
