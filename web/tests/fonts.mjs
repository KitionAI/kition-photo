// The Type tool's fonts in a real browser (plan 2026-11-24, stage 9): node tests/fonts.mjs [chromium|webkit]
// The font list has the served families by name; choosing one fetches it and gives it to the engine
// (a serif changes the drawn glyphs, a Chinese serif draws Chinese without boxes); type that names a
// served family loads it; a family nothing has shows as substituted. Each check reads the engine's
// state or the drawn canvas, and prints PASS or FAIL per step.
import { openEditor } from './canvas-harness.mjs'

const which = process.argv[2] ?? 'chromium'
const e = await openEditor(which)
const { page, kit, step, until, details, query, choose, selectLayer, settle, shot, workspace, view } = e


/** The families the engine has now (`text.fonts`). */
const engineFamilies = async () => (await query('text.fonts'))?.families ?? []
/** Type layer `id`'s first run's family (`text.info`). */
const familyOf = async (id) => (await query('text.info', { layer: id }))?.runs?.[0]?.style?.font_family ?? null
/** An agent's type layer (set-up only); answers its id. */
const addText = async (params) => (await query('text.add', { color: '#000000', ...params }))?.layer
/** The drawn canvas's grey values over document rectangle [x, y, w, h], as rows of `cols` × `rows` cells' mean darkness. */
const darkness = async ([x, y, w, h], cols = 1) => {
  const v = await view()
  return kit.getByTestId('kit-canvas-surface').evaluate(
    (canvas, { v, x, y, w, h, cols }) => {
      const scale = canvas.width / v.box.width
      const px = (dx) => Math.round((dx - v.x) * v.zoom * scale)
      const py = (dy) => Math.round((dy - v.y) * v.zoom * scale)
      const [x0, y0, x1, y1] = [px(x), py(y), px(x + w), py(y + h)]
      const data = canvas.getContext('2d').getImageData(x0, y0, Math.max(1, x1 - x0), Math.max(1, y1 - y0))
      // Each cell: its pixels' darkness (255 − grey), as a small 16 × 16 grid, to compare shapes.
      const cells = []
      for (let c = 0; c < cols; c += 1) {
        const grid = []
        const cx0 = Math.floor((c * data.width) / cols)
        const cx1 = Math.floor(((c + 1) * data.width) / cols)
        for (let gy = 0; gy < 16; gy += 1)
          for (let gx = 0; gx < 16; gx += 1) {
            let sum = 0
            let n = 0
            for (let yy = Math.floor((gy * data.height) / 16); yy < Math.floor(((gy + 1) * data.height) / 16); yy += 1)
              for (let xx = cx0 + Math.floor((gx * (cx1 - cx0)) / 16); xx < cx0 + Math.floor(((gx + 1) * (cx1 - cx0)) / 16); xx += 1) {
                const i = (yy * data.width + xx) * 4
                sum += 255 - (data.data[i] + data.data[i + 1] + data.data[i + 2]) / 3
                n += 1
              }
            grid.push(n ? sum / n : 0)
          }
        cells.push(grid)
      }
      return cells
    },
    { v, x, y, w, h, cols },
  )
}
/** How unlike two cells' shapes are: the mean difference of their grids (0 alike, 255 opposite). */
const unlike = (a, b) => a.reduce((sum, value, i) => sum + Math.abs(value - b[i]), 0) / a.length
const ink = (cell) => cell.reduce((sum, value) => sum + value, 0) / cell.length
const fontField = kit.getByTestId('photo-option-font')
/** Chooses `family` in the options bar's font list, as a person does. */
const pick = async (family) => {
  await fontField.selectOption(family)
  await workspace.focus()
}

try {
  await choose('Type')

  // ---------------------------------------------------------------- the list
  const listed = (await fontField.locator('option').allTextContents()).map((text) => text.trim())
  const wanted = ['Inter', 'JetBrains Mono', 'Droid Sans Fallback', 'Arimo', 'Carlito', 'Bricolage Grotesque', 'Lora', 'Tinos', 'Caladea', 'Cousine', 'Ma Shan Zheng', 'Shippori Mincho', 'BIZ UDMincho', 'BIZ UDPGothic', 'NanumGothic', 'Noto Sans Arabic']
  const missing = wanted.filter((family) => !listed.includes(family))
  step('the Type tool’s font list has the 16 families by their own names (Latin sans, serif, mono, display; Chinese Hei, Kai and a Mincho; Japanese, Korean, Arabic)', missing.length === 0 && listed.length >= 16, missing.length ? `missing ${missing.join(', ')}` : `${listed.length}: ${listed.join(', ')}`)
  const lazily = !(await engineFamilies()).includes('Lora')
  step('a served family is not loaded before it is used', lazily, (await engineFamilies()).join(', '))

  // ---------------------------------------------------------------- a serif changes the glyphs
  const latin = await addText({ x: 80, y: 260, text: 'Hamburgefonstiv', size: 72, font: 'Inter', name: 'Latin' })
  await selectLayer(latin)
  await choose('Type')
  await settle()
  const boundsOf = async (id) => (await details()).layers.find((layer) => layer.id === id).bounds
  const box = await boundsOf(latin)
  await pick('Lora')
  const seenLoading = await until(async () => (await kit.getByTestId('photo-font-loading').count()) === 1 || (await familyOf(latin)) === 'Lora', 50)
  await until(async () => (await familyOf(latin)) === 'Lora', 80)
  await settle()
  await page.waitForTimeout(300)
  // The same area before (undone) and after (redone): both drawings of the word, in one frame.
  const after = await boundsOf(latin)
  const area = [Math.min(box[0], after[0]), Math.min(box[1], after[1]), Math.max(box[0] + box[2], after[0] + after[2]) - Math.min(box[0], after[0]), Math.max(box[1] + box[3], after[1] + after[3]) - Math.min(box[1], after[1])]
  const [serif] = await darkness(area)
  await shot('fonts-lora')
  const ownFace = await kit.locator('body').evaluate(() => document.fonts.check('16px "Lora"'))
  await kit.getByTestId('photo-undo').click()
  await until(async () => (await familyOf(latin)) === 'Inter', 40)
  await settle()
  await page.waitForTimeout(300)
  const [sans] = await darkness(area)
  await kit.getByTestId('photo-redo').click()
  await until(async () => (await familyOf(latin)) === 'Lora', 40)
  await workspace.focus()
  step('choosing Lora loads it (a moment’s loading note) and the layer is drawn in it: the glyphs change', seenLoading && (await familyOf(latin)) === 'Lora' && (await engineFamilies()).includes('Lora') && unlike(sans, serif) > 4, `family ${await familyOf(latin)}, difference ${unlike(sans, serif).toFixed(1)}`)
  step('the field shows the chosen family, in its own face', (await fontField.inputValue()) === 'Lora' && ownFace, `${await fontField.inputValue()}, page has Lora ${ownFace}`)

  // ---------------------------------------------------------------- Chinese in a serif, without boxes
  const chinese = await addText({ x: 80, y: 520, text: '永和山水', size: 96, font: 'Droid Sans Fallback', name: 'Chinese' })
  await selectLayer(chinese)
  await choose('Type')
  await settle()
  const cbox = (await details()).layers.find((layer) => layer.id === chinese).bounds
  const carea = [cbox[0], cbox[1], cbox[2], cbox[3]]
  const hei = await darkness(carea, 4)
  await pick('Shippori Mincho')
  await until(async () => (await familyOf(chinese)) === 'Shippori Mincho', 120)
  await settle()
  await page.waitForTimeout(300)
  const nbox = (await details()).layers.find((layer) => layer.id === chinese).bounds
  const song = await darkness([nbox[0], nbox[1], nbox[2], nbox[3]], 4)
  await shot('fonts-chinese-mincho')
  // Boxes (missing glyphs) would be four alike cells; four different characters are four different shapes.
  let alike = Infinity
  for (let i = 0; i < 4; i += 1) for (let j = i + 1; j < 4; j += 1) alike = Math.min(alike, unlike(song[i], song[j]))
  const inked = song.every((cell) => ink(cell) > 8)
  const changed = hei.reduce((sum, cell, i) => sum + unlike(cell, song[i]), 0) / 4
  step('Chinese in Shippori Mincho (a Mincho, Song-style serif) draws four different characters, not boxes', inked && alike > 6, `least difference between characters ${alike.toFixed(1)}, ink ${song.map((cell) => ink(cell).toFixed(0)).join('/')}`)
  step('… and in another face than the Hei (Droid Sans Fallback) it was in', changed > 4, `difference ${changed.toFixed(1)}`)

  // ---------------------------------------------------------------- a document's type loads its served family
  const before = (await engineFamilies()).includes('Tinos')
  await addText({ x: 80, y: 700, text: 'Times-like Tinos', size: 40, font: 'Tinos', name: 'Tinos type' })
  const loaded = await until(async () => (await engineFamilies()).includes('Tinos'), 100)
  step('type that names a served family (Tinos) loads it without being chosen', !before && loaded, (await engineFamilies()).join(', '))

  // ---------------------------------------------------------------- a family nothing has
  const gone = await addText({ x: 80, y: 760, text: 'Not here', size: 30, font: 'Helvetica Neue', name: 'Missing' })
  await selectLayer(gone)
  await choose('Type')
  await until(async () => (await fontField.inputValue()) === 'Helvetica Neue', 40)
  const shown = (await fontField.locator('option:checked').textContent())?.trim() ?? ''
  await shot('fonts-substituted')
  step('a family the document names and nothing has shows marked as substituted', shown.includes('Helvetica Neue') && /substituted/i.test(shown), shown)
  await e.finish()
} catch (error) {
  await e.fail(error)
}
