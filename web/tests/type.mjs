// Type on the canvas (plan 2026-11-24, stage 4):
//   node tests/type.mjs [chromium|webkit]
// New point text with two lines (Enter starts the second line, Cmd+Enter commits, one history step);
// Chinese text through an input method, its composition shown in the text while it is composed;
// clicking existing type edits it at the character clicked; arrows, Home/End and Shift select;
// a double click selects a word, whose colour then changes alone; Escape puts the text back as it
// was; copy and paste; a dragged paragraph box wraps its text and its handle resizes it; the Move
// tool's double click and a Layers thumbnail's start editing. Real pointer and keyboard input on
// dev.html; the text and styles are read back from the engine (`text.info`). Prints PASS or FAIL per step.
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
  if (!shots) return
  // Let the canvas finish painting the engine's last change first.
  await page.waitForTimeout(500)
  await page.screenshot({ path: `${shots}/${name}.png` })
}

try {
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60_000 })
  await page.waitForTimeout(800)
  const kit = page.frameLocator('#kit')
  const surface = kit.getByTestId('kit-canvas-surface')
  const workspace = kit.getByTestId('photo-workspace')
  const editor = kit.getByTestId('photo-type-editor')
  const input = kit.getByTestId('photo-type-input')
  const field = kit.getByTestId('photo-text-field')
  const inspect = () => page.evaluate(async () => window.devHost.connection().ai.inspect())
  const details = async () => (await inspect()).details
  const history = async () => (await details()).historyIndex
  const engine = (command, params = {}) => page.evaluate(async ([command, params]) => (await window.devHost.connection().ai.run([{ command, params }], false)).completed[0].result, [command, params])
  const info = (layer) => engine('text.info', { layer })
  const view = async () => {
    const [zoom, x, y] = (await workspace.getAttribute('data-view')).split(' ').map(Number)
    return { zoom, x, y, box: await surface.boundingBox() }
  }
  /** Where document point (x, y) is on the page. */
  const at = async (x, y) => {
    const v = await view()
    return { x: v.box.x + (x - v.x) * v.zoom, y: v.box.y + (y - v.y) * v.zoom }
  }
  const click = async (x, y, options) => {
    const p = await at(x, y)
    await page.mouse.click(p.x, p.y, options)
  }
  // The editor's marks are there only while text is edited.
  const editing = async () => (await editor.count()) === 1
  const selection = async () => ((await editing()) ? (await editor.getAttribute('data-selection')).split(' ').map(Number) : null)
  /** The text being edited and its line count, as the engine last answered the editor (no query of ours comes between its edits). */
  const edited = async () => ((await editing()) ? { text: await editor.getAttribute('data-text'), lines: Number(await editor.getAttribute('data-lines')) } : { text: null, lines: 0 })
  const typeLayers = async () => (await details()).layers.filter((layer) => layer.kind === 'Type')
  const caretOf = async (layer, index) => (await engine('text.caret', { layer, index })).segment
  /** The middle of character `index` of `layer` (document pixels). */
  const middleOf = async (layer, index) => {
    const [a] = await caretOf(layer, index)
    const [b, bottom] = await caretOf(layer, index + 1)
    return { x: (a[0] + b[0]) / 2, y: (a[1] + bottom[1]) / 2 }
  }
  const pick = async (key, tool) => {
    await workspace.focus()
    await page.keyboard.press(key)
    return until(async () => (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === tool, 20)
  }
  const press = (key) => page.keyboard.press(key)
  const command = (key) => press(`ControlOrMeta+${key}`)
  /** Waits until the document's revision stops moving. */
  const settle = async () => {
    let before = (await inspect()).revision
    for (let i = 0; i < 20; i += 1) {
      await page.waitForTimeout(150)
      const now = (await inspect()).revision
      if (now === before) return
      before = now
    }
  }
  const frame = await (await page.$('#kit')).contentFrame()
  /** A clipboard event on the session's text area: `copy` and `cut` answer what they put there, `paste` gives `text`. */
  const clipboard = (type, text = '') =>
    frame.evaluate(
      ([type, text]) => {
        const area = document.querySelector('[data-testid="photo-type-input"]')
        const data = new DataTransfer()
        if (text) data.setData('text/plain', text)
        area.dispatchEvent(new ClipboardEvent(type, { clipboardData: data, bubbles: true, cancelable: true }))
        return data.getData('text/plain')
      },
      [type, text],
    )
  /** An input method's events on the session's text area (WebKit's driver has no input method). */
  const composition = (type, data) =>
    frame.evaluate(
      ([type, data]) => {
        const area = document.querySelector('[data-testid="photo-type-input"]')
        area.dispatchEvent(new CompositionEvent(type, { data, bubbles: true }))
      },
      [type, data],
    )

  // New text at 40 pt: with no type layer active the bar's size is the tool's.
  const pixelLayer = (await details()).layers.find((layer) => layer.kind === 'Pixel' && layer.name !== 'Background')
  await kit.getByTestId(`photo-layer-${pixelLayer.id}`).click()
  await pick('t', 'Type')
  await kit.getByTestId('photo-option-fontSize').locator('input').fill('40')
  await kit.getByTestId('photo-option-fontSize').locator('input').press('Enter')
  await settle()

  // New point text with two lines: Enter starts the second, Cmd+Enter commits; one history step.
  let twoLines
  {
    const before = await history()
    const count = (await typeLayers()).length
    await click(200, 200)
    await field.waitFor()
    await page.keyboard.type('Hello')
    await shot('type-new-draft')
    await press('Enter')
    const going = await until(editing, 50)
    await page.keyboard.type('World')
    await until(async () => (await typeLayers()).length === count + 1 && (await info((await details()).activeLayer)).text === 'Hello\nWorld', 50)
    await shot('type-new-second-line')
    await command('Enter')
    await until(async () => !(await editing()), 30)
    twoLines = (await details()).activeLayer
    const text = await info(twoLines)
    step('new point text: Enter starts a second line, Cmd+Enter commits, in one history step', going && text.text === 'Hello\nWorld' && text.lines.length === 2 && (await history()) === before + 1 && !(await editing()), JSON.stringify({ going, text: text.text, lines: text.lines.length, history: (await history()) - before }))
  }

  // Plan 2026-11-24, stage 8: fast typing keeps up: 70 keys 15 ms apart all arrive, in order, within
  // a second and a half of the last one (keys typed while an edit is under way go in together).
  {
    const before = await history()
    const fast = 'The quick brown fox jumps over the lazy dog and keeps running far away'
    await click(60, 700)
    await field.waitFor()
    await page.keyboard.type('Q')
    await press('Enter')
    await until(editing, 50)
    await page.keyboard.type(fast, { delay: 15 })
    const typedAt = Date.now()
    const kept = await until(async () => (await info((await details()).activeLayer)).text === `Q\n${fast}`, 15)
    const waited = Date.now() - typedAt
    await command('Enter')
    await until(async () => !(await editing()), 30)
    const text = (await info((await details()).activeLayer)).text
    step('70 keys typed 15 ms apart all arrive, in order, in one history step', kept && text === `Q\n${fast}` && (await history()) === before + 1, JSON.stringify({ kept, waited, got: text.length - 2 }))
    await command('z')
    await until(async () => (await history()) === before, 30)
  }

  // Clicking existing type edits it at the character clicked; typing goes in there; one step.
  {
    const before = await history()
    const [top, bottom] = await caretOf(twoLines, 2)
    await click(top[0] + 1, (top[1] + bottom[1]) / 2)
    const started = await until(async () => (await editing()) && (await selection())?.join() === '2,2', 50)
    const caretShown = (await kit.getByTestId('photo-type-caret').count()) === 1
    await page.keyboard.type('X')
    await until(async () => (await edited()).text === 'HeXllo\nWorld', 50)
    await shot('type-re-edit')
    await command('Enter')
    await until(async () => !(await editing()), 30)
    await settle()
    step('a click on type edits it at the character clicked, with a caret, in one step', started && caretShown && (await info(twoLines)).text === 'HeXllo\nWorld' && (await history()) === before + 1, JSON.stringify({ started, caretShown, text: (await info(twoLines)).text, history: (await history()) - before }))
  }

  // Keys: Shift+End and Shift+Home select to the line's edges, an arrow collapses, Down goes to the
  // next line, Shift+Up selects back, Cmd+A selects everything; Escape leaves the text as it was.
  {
    const before = await history()
    const [top, bottom] = await caretOf(twoLines, 3)
    await click(top[0] + 1, (top[1] + bottom[1]) / 2)
    await until(async () => (await selection())?.join() === '3,3', 50)
    await press('Shift+End')
    const toEnd = await until(async () => (await selection())?.join() === '3,6', 30)
    await press('Shift+Home')
    const toStart = await until(async () => (await selection())?.join() === '0,3', 30)
    await shot('type-shift-home')
    await press('ArrowRight')
    const collapsedRight = await until(async () => (await selection())?.join() === '3,3', 30)
    await press('ArrowDown')
    const down = await until(async () => (await selection())?.[0] >= 7, 30)
    const below = (await selection())[0]
    await press('Shift+ArrowUp')
    const up = await until(async () => (await selection())?.join() === `3,${below}`, 30)
    await command('a')
    const all = await until(async () => (await selection())?.join() === '0,12', 30)
    await shot('type-select-all')
    await press('Escape')
    await until(async () => !(await editing()), 30)
    step('Shift+End, Shift+Home, an arrow, Down, Shift+Up and Cmd+A move and select', toEnd && toStart && collapsedRight && down && up && all, JSON.stringify({ toEnd, toStart, collapsedRight, down, below, up, all }))
    step('moving the caret and selecting record nothing', (await history()) === before && (await info(twoLines)).text === 'HeXllo\nWorld')
  }

  // Chinese through an input method: the composition shows in the text (underlined) while it is composed.
  {
    const before = await history()
    const length = 12
    const [top, bottom] = await caretOf(twoLines, length)
    await click(top[0] + 2, (top[1] + bottom[1]) / 2)
    await until(async () => (await selection())?.join() === `${length},${length}`, 50)
    if (which === 'chromium') {
      const cdp = await page.context().newCDPSession(page)
      await cdp.send('Input.imeSetComposition', { text: 'zhong', selectionStart: 5, selectionEnd: 5 })
      var composed = await until(async () => (await edited()).text === 'HeXllo\nWorldzhong' && (await kit.getByTestId('photo-type-composition').count()) > 0, 50)
      await shot('type-composing')
      await cdp.send('Input.insertText', { text: '中文' })
    } else {
      await composition('compositionstart', '')
      await composition('compositionupdate', 'zhong')
      var composed = await until(async () => (await edited()).text === 'HeXllo\nWorldzhong' && (await kit.getByTestId('photo-type-composition').count()) > 0, 50)
      await shot('type-composing')
      await composition('compositionend', '中文')
    }
    const chinese = await until(async () => (await edited()).text === 'HeXllo\nWorld中文' && (await kit.getByTestId('photo-type-composition').count()) === 0, 50)
    await shot('type-chinese')
    await command('Enter')
    await until(async () => !(await editing()), 30)
    await settle()
    step('Chinese through an input method: the composition shows at the caret, then the text, one step', composed && chinese && (await history()) === before + 1, JSON.stringify({ composed, chinese, text: (await info(twoLines)).text, history: (await history()) - before }))
  }

  // A double click selects a word (letters, digits and Chinese characters run together, the
  // engine's words); the bar's colour then colours that word only.
  {
    const before = await history()
    const middle = await middleOf(twoLines, 9)
    const p = await at(middle.x, middle.y)
    await page.mouse.dblclick(p.x, p.y)
    const word = await until(async () => (await selection())?.join() === '7,14', 50)
    await shot('type-word-selected')
    await kit.locator(`[data-kit-ui-id="control:options.textColor"] input`).evaluate((element) => {
      const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      setValue.call(element, '#ff0000')
      element.dispatchEvent(new Event('input', { bubbles: true }))
      element.dispatchEvent(new Event('change', { bubbles: true }))
    })
    const colourAt = async (index) => {
      const runs = (await info(twoLines)).runs
      const run = runs.find((item) => index >= item.start && index < item.end)
      return run?.style.color.c.slice(0, 3).map((value) => Math.round(value * 255)).join()
    }
    const coloured = await until(async () => (await colourAt(9)) === '255,0,0', 50)
    const others = [await colourAt(0), await colourAt(5)]
    const inside = [await colourAt(7), await colourAt(13)]
    await shot('type-word-coloured')
    await kit.getByTestId('photo-option-commit').click()
    await until(async () => !(await editing()), 30)
    await settle()
    step('a double click selects a word and the colour changes that word only, in one step', word && coloured && inside.every((value) => value === '255,0,0') && others.every((value) => value !== '255,0,0') && (await history()) === before + 1, JSON.stringify({ word, coloured, inside, others, history: (await history()) - before }))
  }

  // Escape puts the text back as it was, with no history step.
  {
    const before = await history()
    const text = (await info(twoLines)).text
    const [top, bottom] = await caretOf(twoLines, 1)
    await click(top[0] + 1, (top[1] + bottom[1]) / 2)
    await until(editing, 50)
    await page.keyboard.type('ZZZ')
    await press('Backspace')
    await press('Enter')
    const changed = await until(async () => (await edited()).text === 'HZZ\neXllo\nWorld中文', 50)
    await press('Escape')
    const restored = await until(async () => !(await editing()) && (await info(twoLines)).text === text, 50)
    await settle()
    step('Escape restores the text and leaves no history step', changed && restored && (await history()) === before, JSON.stringify({ changed, restored, history: (await history()) - before }))
  }

  // Copy a word, paste it at the end.
  {
    const middle = await middleOf(twoLines, 1)
    const p = await at(middle.x, middle.y)
    await page.mouse.dblclick(p.x, p.y)
    await until(async () => (await selection())?.join() === '0,6', 50)
    const copied = await clipboard('copy')
    await command('ArrowDown')
    await until(async () => (await selection())?.join() === '14,14', 30)
    await clipboard('paste', ' HeXllo')
    const pasted = await until(async () => (await edited()).text === 'HeXllo\nWorld中文 HeXllo', 50)
    await command('a')
    await until(async () => (await selection())?.join() === '0,21', 30)
    const cut = await clipboard('cut')
    const emptied = await until(async () => (await edited()).text === '', 50)
    await press('Escape')
    const back = await until(async () => !(await editing()) && (await info(twoLines)).text === 'HeXllo\nWorld中文', 50)
    step('copy, paste and cut through the clipboard; Escape takes them back', copied === 'HeXllo' && pasted && cut === 'HeXllo\nWorld中文 HeXllo' && emptied && back, JSON.stringify({ copied, pasted, cut, emptied, back }))
  }

  // A dragged box makes paragraph text that wraps; its handle resizes the box while editing.
  {
    const before = await history()
    const count = (await typeLayers()).length
    const from = await at(300, 300)
    const to = await at(520, 560)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 6 })
    await page.mouse.up()
    const opened = await until(async () => (await editing()) && (await kit.getByTestId('photo-type-box').count()) === 1, 50)
    await page.keyboard.type('The quick brown fox jumps over the lazy dog')
    const layer = (await details()).activeLayer
    const wrapped = await until(async () => (await edited()).text === 'The quick brown fox jumps over the lazy dog' && (await edited()).lines >= 3, 80)
    const narrowLines = (await edited()).lines
    await shot('type-paragraph')
    const handle = await kit.getByTestId('photo-type-handle-e').boundingBox()
    const wide = await at(1000, 400)
    await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2)
    await page.mouse.down()
    await page.mouse.move(wide.x, handle.y + handle.height / 2, { steps: 6 })
    await page.mouse.up()
    let widened = await until(async () => (await edited()).lines < narrowLines, 50)
    await shot('type-paragraph-wider')
    await command('Enter')
    await until(async () => !(await editing()), 30)
    await settle()
    const box = (await info(layer)).shape.Box
    widened = widened && box?.width > 600
    step('a dragged box wraps its text and its handle resizes it while editing, in one step', opened && wrapped && widened && (await typeLayers()).length === count + 1 && (await history()) === before + 1, JSON.stringify({ opened, wrapped, narrowLines, widened, box, history: (await history()) - before }))
  }

  // New paragraph text dropped with Escape leaves nothing.
  {
    const before = await history()
    const count = (await typeLayers()).length
    const from = await at(600, 60)
    const to = await at(800, 140)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 6 })
    await page.mouse.up()
    await until(editing, 50)
    await page.keyboard.type('Gone')
    await until(async () => (await edited()).text === 'Gone', 50)
    await press('Escape')
    const gone = await until(async () => !(await editing()) && (await typeLayers()).length === count, 50)
    await settle()
    step('Escape on new text removes it, leaving no history step', gone && (await history()) === before, JSON.stringify({ gone, history: (await history()) - before }))
  }

  // The Move tool's double click on type edits it with the Type tool.
  {
    const title = (await typeLayers()).find((layer) => layer.name === 'Northern Lights')
    await pick('v', 'Move')
    const middle = await middleOf(title.id, 3)
    const p = await at(middle.x, middle.y)
    await page.mouse.dblclick(p.x, p.y)
    const started = await until(async () => (await editing()) && Number(await editor.getAttribute('data-layer')) === title.id && (await kit.getByTestId('photo-options-tool').getAttribute('data-tool')) === 'Type', 50)
    const at3 = (await selection())?.[0]
    await shot('type-move-double-click')
    await press('Escape')
    await until(async () => !(await editing()), 30)
    step("the Move tool's double click on type edits it at the character there", started && (at3 === 3 || at3 === 4), JSON.stringify({ started, at3 }))
  }

  // A Layers thumbnail's double click edits the layer's text, all of it selected.
  {
    await kit.getByTestId(`photo-layer-thumb-${twoLines}`).dblclick()
    const started = await until(async () => (await editing()) && Number(await editor.getAttribute('data-layer')) === twoLines && (await selection())?.join() === '0,14', 50)
    await press('Escape')
    await until(async () => !(await editing()), 30)
    step("a type thumbnail's double click edits its text, all selected", started, JSON.stringify(await selection()))
  }

  // A click away from the text commits it; the click then starts new text there (as for new text).
  {
    const [top, bottom] = await caretOf(twoLines, 2)
    await click(top[0] + 1, (top[1] + bottom[1]) / 2)
    await until(editing, 50)
    await page.keyboard.type('Q')
    await until(async () => (await edited()).text === 'HeQXllo\nWorld中文', 50)
    await click(700, 460)
    const committed = await until(async () => !(await editing()) && (await field.count()) === 1, 50)
    await field.press('Escape')
    step('a click away from the text commits it', committed && (await info(twoLines)).text === 'HeQXllo\nWorld中文', JSON.stringify({ committed }))
  }

  step('no page errors', errors.length === 0, errors.slice(0, 3).join(' | '))
} finally {
  await browser.close()
  await server.close()
}
console.log(failures ? `${failures} FAILED (${which})` : `ALL PASSED (${which})`)
process.exit(failures ? 1 : 0)
