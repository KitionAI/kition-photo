// The pure part of editing type on the canvas (plan 2026-11-24, stage 4): characters as the engine
// counts them, words and paragraphs, click counts, keys, the selection's highlight and a paragraph
// box's handles. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import {
  BOX_DRAG,
  boxOf,
  boxParam,
  charCount,
  charStep,
  clickSelection,
  countClick,
  deletion,
  docToLine,
  dragBox,
  handlePoint,
  invert,
  isBoxDrag,
  keyAction,
  lineToDoc,
  paragraphAt,
  rangeQuads,
  replaceChars,
  resizeBox,
  sliceChars,
  wordAt,
  wordBoundary,
} from './text.ts'

test('characters are counted as the engine counts them (code points, not UTF-16 units)', () => {
  assert.equal(charCount('极光😀'), 3)
  assert.equal(replaceChars('a😀b', 1, 2, 'x'), 'axb')
  assert.equal(replaceChars('ab', 1, 1, 'one\r\ntwo\r'), 'aone\ntwo\nb')
  assert.equal(sliceChars('极光 Lights', 0, 2), '极光')
})

test('words follow the engine (letters, digits and Chinese run together)', () => {
  const text = 'Hello, wonderful 世界2!'
  assert.equal(wordBoundary(text, 3, false), 0)
  assert.equal(wordBoundary(text, 3, true), 5)
  assert.equal(wordBoundary(text, 5, true), 16)
  assert.equal(wordBoundary(text, 7, false), 0)
  // A double click: the word under the click, the caret after a word's last letter counting as on it.
  assert.deepEqual(wordAt(text, 2), [0, 5])
  assert.deepEqual(wordAt(text, 5), [0, 5])
  assert.deepEqual(wordAt(text, 18), [17, 20])
  assert.deepEqual(wordAt('a  b', 2), [1, 3])
  assert.deepEqual(wordAt('a!b', 1), [0, 1])
  assert.deepEqual(wordAt('!?', 1), [1, 2])
  assert.deepEqual(wordAt('a\n\nb', 2), [2, 2])
})

test('a triple click selects the paragraph, a fourth everything', () => {
  const text = 'one two\nthree four\nfive'
  assert.deepEqual(paragraphAt(text, 10), [8, 18])
  assert.deepEqual(paragraphAt(text, 0), [0, 7])
  assert.deepEqual(paragraphAt(text, 23), [19, 23])
  assert.deepEqual(clickSelection(text, 10, 1), [10, 10])
  assert.deepEqual(clickSelection(text, 10, 2), [8, 13])
  assert.deepEqual(clickSelection(text, 10, 3), [8, 18])
  assert.deepEqual(clickSelection(text, 10, 4), [0, 23])
})

test('presses close in time and place count as double and triple clicks', () => {
  const first = { x: 10, y: 10, time: 1000 }
  assert.equal(countClick(null, first), 1)
  assert.equal(countClick({ ...first, count: 1 }, { x: 12, y: 11, time: 1300 }), 2)
  assert.equal(countClick({ ...first, count: 2 }, { x: 12, y: 11, time: 1300 }), 3)
  assert.equal(countClick({ ...first, count: 4 }, { x: 10, y: 10, time: 1100 }), 4)
  assert.equal(countClick({ ...first, count: 1 }, { x: 30, y: 10, time: 1100 }), 1)
  assert.equal(countClick({ ...first, count: 1 }, { x: 10, y: 10, time: 1700 }), 1)
})

test('a drag of at least four screen pixels both ways is a paragraph box', () => {
  assert.equal(isBoxDrag({ x: 0, y: 0 }, { x: BOX_DRAG, y: BOX_DRAG }), true)
  assert.equal(isBoxDrag({ x: 0, y: 0 }, { x: 200, y: 3 }), false)
  assert.deepEqual(dragBox({ x: 120.4, y: 80.6 }, { x: 20.2, y: 30.1 }), [20, 30, 100, 51])
})

test('keys: arrows, words, line edges, deleting, new lines, commit and cancel', () => {
  const key = (k, mods = {}, mac = true) => keyAction({ key: k, shift: false, alt: false, meta: false, ctrl: false, ...mods }, mac)
  assert.deepEqual(key('ArrowLeft'), { kind: 'move', by: 'char', forward: false, extend: false })
  assert.deepEqual(key('ArrowRight', { shift: true }), { kind: 'move', by: 'char', forward: true, extend: true })
  assert.deepEqual(key('ArrowLeft', { alt: true }), { kind: 'move', by: 'word', forward: false, extend: false })
  assert.deepEqual(key('ArrowLeft', { ctrl: true }, false), { kind: 'move', by: 'word', forward: false, extend: false })
  assert.deepEqual(key('ArrowRight', { meta: true }), { kind: 'move', by: 'lineEdge', forward: true, extend: false })
  assert.deepEqual(key('ArrowDown', { meta: true, shift: true }), { kind: 'move', by: 'text', forward: true, extend: true })
  assert.deepEqual(key('ArrowUp'), { kind: 'move', by: 'line', forward: false, extend: false })
  assert.deepEqual(key('Home', { shift: true }), { kind: 'move', by: 'lineEdge', forward: false, extend: true })
  assert.deepEqual(key('End', { ctrl: true }, false), { kind: 'move', by: 'text', forward: true, extend: false })
  assert.deepEqual(key('Backspace'), { kind: 'delete', by: 'char', forward: false })
  assert.deepEqual(key('Backspace', { alt: true }), { kind: 'delete', by: 'word', forward: false })
  assert.deepEqual(key('Backspace', { meta: true }), { kind: 'delete', by: 'lineEdge', forward: false })
  assert.deepEqual(key('Delete'), { kind: 'delete', by: 'char', forward: true })
  assert.deepEqual(key('Enter'), { kind: 'insert', text: '\n' })
  assert.deepEqual(key('Enter', { meta: true }), { kind: 'commit' })
  assert.deepEqual(keyAction({ key: 'Enter', code: 'NumpadEnter', shift: false, alt: false, meta: false, ctrl: false }, true), { kind: 'commit' })
  assert.deepEqual(key('Escape'), { kind: 'cancel' })
  assert.deepEqual(key('Tab'), { kind: 'insert', text: '\t' })
  assert.deepEqual(key('a', { meta: true }), { kind: 'selectAll' })
  assert.deepEqual(key('z', { ctrl: true }, false), { kind: 'undo' })
  assert.deepEqual(key('Z', { meta: true, shift: true }), { kind: 'redo' })
  assert.deepEqual(key('s', { meta: true }), { kind: 'save' })
  // Typing, the clipboard and the input method are the browser's.
  assert.equal(key('a'), null)
  assert.equal(key('c', { meta: true }), null)
  assert.equal(key('v', { meta: true }), null)
})

test('a plain arrow collapses a selection to its edge; Backspace and Delete take what they reach', () => {
  assert.equal(charStep({ anchor: 2, focus: 5 }, false, false, 9), 2)
  assert.equal(charStep({ anchor: 5, focus: 2 }, true, false, 9), 5)
  assert.equal(charStep({ anchor: 5, focus: 5 }, true, false, 9), 6)
  assert.equal(charStep({ anchor: 9, focus: 9 }, true, false, 9), 9)
  assert.equal(charStep({ anchor: 2, focus: 5 }, true, true, 9), 6)
  const text = 'one two three'
  assert.deepEqual(deletion(text, { anchor: 7, focus: 4 }, 'char', false, 0), [4, 7])
  assert.deepEqual(deletion(text, { anchor: 7, focus: 7 }, 'char', false, 0), [6, 7])
  assert.deepEqual(deletion(text, { anchor: 0, focus: 0 }, 'char', false, 0), [0, 0])
  assert.deepEqual(deletion(text, { anchor: 7, focus: 7 }, 'word', false, 0), [4, 7])
  assert.deepEqual(deletion(text, { anchor: 4, focus: 4 }, 'word', true, 0), [4, 7])
  assert.deepEqual(deletion(text, { anchor: 9, focus: 9 }, 'lineEdge', false, 4), [4, 9])
  assert.deepEqual(deletion(text, { anchor: 13, focus: 13 }, 'char', true, 0), [13, 13])
})

test('line space maps to the document through the layer transform, vertical type turned a quarter', () => {
  const m = [2, 0, 0, 2, 100, 50]
  assert.deepEqual(lineToDoc(m, false, 10, -5), { x: 120, y: 40 })
  assert.deepEqual(docToLine(m, false, 120, 40), { x: 10, y: -5 })
  // Vertical: text space = (-line y, line x).
  assert.deepEqual(lineToDoc([1, 0, 0, 1, 300, 20], true, 16, -12), { x: 312, y: 36 })
  assert.deepEqual(docToLine([1, 0, 0, 1, 300, 20], true, 312, 36), { x: 16, y: -12 })
  const turned = [0, 1, -1, 0, 10, 10]
  const back = invert(turned)
  const p = lineToDoc(turned, false, 3, 4)
  assert.deepEqual(lineToDoc(back, false, p.x, p.y), { x: 3, y: 4 })
  assert.deepEqual(invert([0, 0, 0, 0, 1, 1]), [1, 0, 0, 1, 0, 0])
})

test("the selection's highlight is one rectangle per line, reaching the line's end where it runs on", () => {
  // 'AB\nC' on two lines; carets at x = 10 per character; line 0 is [0, 2], line 1 is [3, 4].
  const lines = [
    { start: 0, end: 2, baseline: 20, x0: 0, x1: 20, ascent: 15, descent: 5 },
    { start: 3, end: 4, baseline: 50, x0: 0, x1: 10, ascent: 15, descent: 5 },
  ]
  const caretX = (index) => (index <= 2 ? index * 10 : (index - 3) * 10)
  const identity = [1, 0, 0, 1, 0, 0]
  const quads = rangeQuads('AB\nC', lines, [1, 4], caretX, identity, false, 6)
  assert.equal(quads.length, 2)
  assert.deepEqual(quads[0].map((p) => [p.x, p.y]), [[10, 5], [26, 5], [26, 25], [10, 25]])
  assert.deepEqual(quads[1].map((p) => [p.x, p.y]), [[0, 35], [10, 35], [10, 55], [0, 55]])
  // Within one line; an empty range has none; the line break alone shows its width.
  assert.deepEqual(rangeQuads('AB\nC', lines, [0, 1], caretX, identity, false, 6)[0].map((p) => p.x), [0, 10, 10, 0])
  assert.deepEqual(rangeQuads('AB\nC', lines, [2, 2], caretX, identity, false, 6), [])
  assert.deepEqual(rangeQuads('AB\nC', lines, [2, 3], caretX, identity, false, 6)[0].map((p) => p.x), [20, 26, 26, 20])
  // A soft-wrapped line (no line break) ends at its x1, with no break width.
  const wrapped = [
    { start: 0, end: 6, baseline: 20, x0: 0, x1: 60, ascent: 15, descent: 5 },
    { start: 6, end: 9, baseline: 50, x0: 0, x1: 30, ascent: 15, descent: 5 },
  ]
  const at = (index) => (index < 6 ? index * 10 : (index - 6) * 10)
  const soft = rangeQuads('Hello world', wrapped, [3, 8], at, identity, false, 6)
  assert.deepEqual(soft.map((quad) => [quad[0].x, quad[1].x]), [[30, 60], [0, 20]])
})

test("a paragraph box's handles resize it, the opposite side staying put", () => {
  const box = { x: 0, y: 0, width: 100, height: 50 }
  assert.deepEqual(handlePoint(box, 'nw'), { x: 0, y: 0 })
  assert.deepEqual(handlePoint(box, 'e'), { x: 100, y: 25 })
  assert.deepEqual(handlePoint(box, 's'), { x: 50, y: 50 })
  assert.deepEqual(resizeBox(box, 'e', { x: 180, y: 999 }), { x: 0, y: 0, width: 180, height: 50 })
  assert.deepEqual(resizeBox(box, 'nw', { x: 20, y: 10 }), { x: 20, y: 10, width: 80, height: 40 })
  assert.deepEqual(resizeBox(box, 'w', { x: 300, y: 0 }), { x: 92, y: 0, width: 8, height: 50 })
  assert.deepEqual(boxParam([1, 0, 0, 1, 20, 30], { x: 5, y: 6, width: 70, height: 80 }), [25, 36, 70, 80])
  assert.deepEqual(boxOf({ Box: { x: 0, y: 0, width: 120, height: 200 } }), { x: 0, y: 0, width: 120, height: 200 })
  assert.equal(boxOf('Point'), null)
  assert.equal(boxOf({ Box: { x: 0, y: 0, width: 'wide', height: 1 } }), null)
})
