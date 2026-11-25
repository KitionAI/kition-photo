// An edit session's traffic with the engine (plan 2026-11-24, stage 4): every command of a session
// carries one coalesce key (one history step); Enter on new text puts the line break in with what is
// typed next; an input method's composition is replaced as it changes; a style goes to the selected
// range only; Escape and an emptied new layer take the step back; someone else's edit ends the
// session. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { TypeSession } from './session.ts'

/** A fake engine with one type layer (id 7): characters 10 px wide, one line per paragraph. */
function fakeEngine(text = 'Hello') {
  const calls = []
  const state = { text, revision: 1, steps: [], coalesce: null, styles: [] }
  const lines = () => {
    const out = []
    let start = 0
    for (const [index, line] of state.text.split('\n').entries()) {
      const chars = Array.from(line).length
      out.push({ start, end: start + chars, baseline: 20 + index * 30, x0: 0, x1: chars * 10, ascent: 15, descent: 5 })
      start += chars + 1
    }
    return out
  }
  const info = () => ({ layer: 7, text: state.text, lines: lines(), transform: [1, 0, 0, 1, 100, 200], orientation: 'Horizontal', shape: 'Point', runs: [{ start: 0, end: Array.from(state.text).length, style: { size_pt: 20 } }], paragraphs: [{ start: 0, end: 0, style: {} }], pxPerPt: 1 })
  const edit = (params) => {
    if (params.coalesce == null || params.coalesce !== state.coalesce) state.steps.push(params.coalesce ?? null)
    state.coalesce = params.coalesce ?? null
    state.revision += 1
  }
  const engine = {
    calls,
    state,
    revision: () => state.revision,
    run: async (command, params) => {
      calls.push([command, params])
      await null
      if (command === 'type.edit') {
        const { start, end, text: insert } = params.replace
        const chars = Array.from(state.text)
        state.text = [...chars.slice(0, start), insert, ...chars.slice(end)].join('')
        edit(params)
        return JSON.stringify(info())
      }
      if (command === 'type.setStyle') {
        state.styles.push(params)
        edit(params)
        return JSON.stringify(info())
      }
      if (command === 'history.deleteState') {
        state.steps.pop()
        state.coalesce = null
        state.revision += 1
        return '{}'
      }
      return null
    },
    query: async (command, params) => {
      calls.push([command, params])
      await null
      if (command === 'type.info') return info()
      if (command === 'type.caret') {
        const before = Array.from(state.text).slice(0, params.index).join('')
        const line = before.split('\n').length - 1
        const x = 100 + Array.from(before.split('\n').pop()).length * 10
        return { index: params.index, line, segment: [[x, 200 + 5 + line * 30], [x, 200 + 25 + line * 30]] }
      }
      if (command === 'type.hitTest') return { layer: 7, index: Math.max(0, Math.round((params.x - 100) / 10)), inside: true }
      if (command === 'type.navigate') return { index: params.move === 'lineStart' ? 0 : Array.from(state.text).length }
      return null
    },
  }
  return engine
}

const edits = (engine) => engine.calls.filter(([command]) => command === 'type.edit' || command === 'type.setStyle')

test('every command of one session carries its key: the session is one history step', async () => {
  const engine = fakeEngine('Hello')
  const session = new TypeSession(engine)
  assert.equal(await session.start({ layer: 7, select: [5, 5] }), true)
  session.insert(' wor')
  session.insert('ld')
  session.key({ key: 'Backspace', shift: false, alt: false, meta: false, ctrl: false }, true)
  session.key({ key: 'Enter', shift: false, alt: false, meta: false, ctrl: false }, true)
  session.insert('!')
  await session.commit()
  assert.equal(engine.state.text, 'Hello worl\n!')
  const keys = new Set(edits(engine).map(([, params]) => params.coalesce))
  assert.equal(keys.size, 1)
  assert.equal(engine.state.steps.length, 1)
  assert.deepEqual(edits(engine)[2][1].replace, { start: 10, end: 11, text: '' })
  assert.equal(session.active, false)
})

test('after Enter on new text the line break goes in with the first thing typed, in the step new text made', async () => {
  const engine = fakeEngine('Draft')
  engine.state.steps.push('draft-key')
  engine.state.coalesce = 'draft-key'
  const session = new TypeSession(engine)
  await session.start({ layer: 7, created: true, key: 'draft-key', pendingBreak: true })
  // Nothing typed yet: the text is as it was.
  assert.equal(engine.state.text, 'Draft')
  const view = session.view.get()
  assert.ok(view.caret[0].y > 220, 'the caret shows on the next line')
  session.insert('More')
  await session.settled()
  assert.equal(engine.state.text, 'Draft\nMore')
  assert.deepEqual(session.selection.range, [10, 10])
  await session.commit()
  assert.equal(engine.state.steps.length, 1)
})

test('Backspace on the line Enter made takes it back without an edit', async () => {
  const engine = fakeEngine('Draft')
  const session = new TypeSession(engine)
  await session.start({ layer: 7, created: true, key: 'k', pendingBreak: true })
  session.key({ key: 'Backspace', shift: false, alt: false, meta: false, ctrl: false }, true)
  session.insert('s')
  await session.commit()
  assert.equal(engine.state.text, 'Drafts')
})

test("an input method's composition replaces itself as it changes, then gives way to the result", async () => {
  const engine = fakeEngine('ab')
  const session = new TypeSession(engine)
  await session.start({ layer: 7, select: [1, 1] })
  session.compose('z')
  session.compose('zh')
  await session.settled()
  assert.equal(engine.state.text, 'azhb')
  assert.equal(session.composing, true)
  assert.equal(session.view.get().composition.length, 1)
  session.composeEnd('中')
  await session.settled()
  assert.equal(engine.state.text, 'a中b')
  assert.equal(session.composing, false)
  assert.deepEqual(session.selection.range, [2, 2])
  assert.deepEqual(edits(engine).map(([, params]) => params.replace), [
    { start: 1, end: 1, text: 'z' },
    { start: 1, end: 2, text: 'zh' },
    { start: 1, end: 3, text: '中' },
  ])
})

test('a style goes to the selected range in the session step; with no range to the whole layer', async () => {
  const engine = fakeEngine('Hello world')
  const session = new TypeSession(engine)
  await session.start({ layer: 7, select: [6, 11] })
  const target = session.target.get()
  assert.deepEqual(target.range, [6, 11])
  await target.run('type.setStyle', { color: '#ff0000', range: target.range })
  assert.deepEqual(engine.state.styles[0], { layer: 7, color: '#ff0000', range: [6, 11], coalesce: session.historyKey })
  session.key({ key: 'ArrowRight', shift: false, alt: false, meta: false, ctrl: false }, true)
  await session.settled()
  assert.equal(session.target.get().range, null)
})

test('Escape takes the session step back; new text left empty is removed on commit', async () => {
  const engine = fakeEngine('Hello')
  const session = new TypeSession(engine)
  await session.start({ layer: 7, select: [0, 5] })
  session.insert('Bye')
  session.key({ key: 'Escape', shift: false, alt: false, meta: false, ctrl: false }, true)
  await session.settled()
  assert.equal(engine.calls.filter(([command]) => command === 'history.deleteState').length, 1)
  assert.equal(engine.state.steps.length, 0)
  assert.equal(session.active, false)

  const fresh = fakeEngine('')
  fresh.state.steps.push('box')
  fresh.state.coalesce = 'box'
  const box = new TypeSession(fresh)
  await box.start({ layer: 7, created: true, key: 'box', select: [0, 0] })
  await box.commit()
  assert.equal(fresh.calls.filter(([command]) => command === 'history.deleteState').length, 1)

  // An existing layer is never removed by a commit, even emptied; an untouched one records nothing.
  const kept = fakeEngine('Hi')
  const edited = new TypeSession(kept)
  await edited.start({ layer: 7, select: [0, 2] })
  await edited.cancel()
  assert.equal(kept.calls.filter(([command]) => command === 'history.deleteState').length, 0)
})

test("someone else's edit ends the session where it is, without touching the history", async () => {
  const engine = fakeEngine('Hello')
  const session = new TypeSession(engine)
  let ended = null
  session.onEnd = (how) => {
    ended = how
  }
  await session.start({ layer: 7 })
  session.insert('!')
  await session.settled()
  engine.state.revision += 1
  session.checkExternal()
  assert.equal(ended, 'external')
  assert.equal(session.active, false)
  assert.equal(engine.calls.filter(([command]) => command === 'history.deleteState').length, 0)
})

test('a press counts clicks: two select a word, three the paragraph; a drag extends from the press', async () => {
  const engine = fakeEngine('one two\nthree')
  const session = new TypeSession(engine)
  await session.start({ layer: 7, select: [0, 0] })
  const at = { x: 155, y: 210 }
  session.press(at, { x: 50, y: 50 }, false)
  session.press(at, { x: 50, y: 50 }, false)
  await session.settled()
  assert.deepEqual(session.selection.range, [4, 7])
  session.press(at, { x: 50, y: 50 }, false)
  await session.settled()
  assert.deepEqual(session.selection.range, [0, 7])
  session.press({ x: 110, y: 210 }, { x: 200, y: 50 }, false)
  session.dragTo({ x: 140, y: 210 })
  await session.settled()
  assert.deepEqual(session.selection.range, [1, 4])
  session.press({ x: 120, y: 210 }, { x: 300, y: 50 }, true)
  await session.settled()
  assert.deepEqual(session.selection.range, [1, 2])
})
