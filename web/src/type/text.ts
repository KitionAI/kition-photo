/**
 * The pure part of editing type on the canvas (plan 2026-11-24, stage 4): character offsets (the
 * engine's `type.*` commands count Unicode characters, JavaScript strings count UTF-16 units),
 * words and paragraphs, what a key does, where the selection's highlight goes, a paragraph box's
 * handles, and how presses count as double and triple clicks. No browser or engine here, so
 * `text.test.mjs` runs it under `node --test`.
 */

/** A caret position or a selection's end, in characters. */
export type Selection = { anchor: number; focus: number }
export type Range = readonly [number, number]
export type Point = { x: number; y: number }
/** Four corners in document pixels, clockwise from the start of the line's top. */
export type Quad = readonly [Point, Point, Point, Point]
/** An affine transform as `type.info` reports it: x' = a·x + c·y + e, y' = b·x + d·y + f. */
export type Affine = readonly [number, number, number, number, number, number]
/** One laid-out line as `type.info` reports it (line space: along the line, then across it). */
export type Line = { start: number; end: number; baseline: number; x0: number; x1: number; ascent: number; descent: number }

/** The characters of `text` (code points, as the engine counts them). */
export const chars = (text: string): string[] => Array.from(text)
export const charCount = (text: string): number => chars(text).length

/** `text` with characters `a`..`b` replaced by `insert`, line ends normalised as the engine does. */
export function replaceChars(text: string, a: number, b: number, insert: string): string {
  const list = chars(text)
  return [...list.slice(0, a), normalise(insert), ...list.slice(b)].join('')
}

/** Characters `a`..`b` of `text`. */
export const sliceChars = (text: string, a: number, b: number): string => chars(text).slice(a, b).join('')

/** Line ends as the engine keeps them (`\r\n` and `\r` become `\n`). */
export const normalise = (text: string): string => text.replace(/\r\n?/g, '\n')

/** The selection's start and end, in order. */
export const ordered = (selection: Selection): Range => (selection.anchor <= selection.focus ? [selection.anchor, selection.focus] : [selection.focus, selection.anchor])
export const collapsed = (selection: Selection): boolean => selection.anchor === selection.focus
export const caretAt = (index: number): Selection => ({ anchor: index, focus: index })

/** A word's character: a letter or a digit (the engine's `is_alphanumeric`, so Chinese runs are words too). */
const WORD = /[\p{Alphabetic}\p{Number}]/u
const SPACE = /[^\S\n]/u
const kindOf = (c: string | undefined) => (c === undefined || c === '\n' ? 'break' : WORD.test(c) ? 'word' : SPACE.test(c) ? 'space' : 'mark')

/** The word boundary before (`forward` false) or after `index`: the engine's `word_boundary`. */
export function wordBoundary(text: string, index: number, forward: boolean): number {
  const list = chars(text)
  let i = Math.min(Math.max(0, index), list.length)
  const isWord = (c: string) => WORD.test(c)
  if (forward) {
    while (i < list.length && !isWord(list[i])) i += 1
    while (i < list.length && isWord(list[i])) i += 1
  } else {
    while (i > 0 && !isWord(list[i - 1])) i -= 1
    while (i > 0 && isWord(list[i - 1])) i -= 1
  }
  return i
}

/**
 * What a double click at caret `index` selects: the run of word characters, of spaces, or the one
 * mark there. A word on either side of the caret wins (a click on a word's last letter lands its
 * caret after it); else the character after the caret, else the one before; an empty line selects nothing.
 */
export function wordAt(text: string, index: number): Range {
  const list = chars(text)
  const at = Math.min(Math.max(0, index), list.length)
  let probe = kindOf(list[at]) === 'word' ? at : kindOf(list[at - 1]) === 'word' ? at - 1 : at < list.length && list[at] !== '\n' ? at : at - 1
  if (probe < 0 || list[probe] === '\n') return [at, at]
  const kind = kindOf(list[probe])
  if (kind === 'mark') return [probe, probe + 1]
  let start = probe
  while (start > 0 && kindOf(list[start - 1]) === kind) start -= 1
  probe += 1
  while (probe < list.length && kindOf(list[probe]) === kind) probe += 1
  return [start, probe]
}

/** What a triple click selects: the paragraph (the text between line breaks) holding caret `index`. */
export function paragraphAt(text: string, index: number): Range {
  const list = chars(text)
  const at = Math.min(Math.max(0, index), list.length)
  let start = at
  while (start > 0 && list[start - 1] !== '\n') start -= 1
  let end = at
  while (end < list.length && list[end] !== '\n') end += 1
  return [start, end]
}

/** What a run of presses selects at caret `index`: 1 a caret, 2 a word, 3 a paragraph, 4 everything. */
export function clickSelection(text: string, index: number, count: number): Range {
  if (count >= 4) return [0, charCount(text)]
  if (count === 3) return paragraphAt(text, index)
  if (count === 2) return wordAt(text, index)
  return [index, index]
}

/** Presses within this time (ms) and distance (screen pixels) of the last one count as one more click. */
export const CLICK_TIME = 500
export const CLICK_SLOP = 4

/** The click count of a press at `at` (screen pixels, time in ms) after `last` (null for none). */
export function countClick(last: { x: number; y: number; time: number; count: number } | null, at: { x: number; y: number; time: number }): number {
  if (!last || at.time - last.time > CLICK_TIME || Math.hypot(at.x - last.x, at.y - last.y) > CLICK_SLOP) return 1
  return Math.min(4, last.count + 1)
}

/** A drag at least this far both ways (screen pixels) makes a paragraph box; less is a click (point text). */
export const BOX_DRAG = 4
export const isBoxDrag = (from: Point, to: Point): boolean => Math.abs(to.x - from.x) >= BOX_DRAG && Math.abs(to.y - from.y) >= BOX_DRAG

/** What a key does while type is edited; null leaves it to the browser (typing, the clipboard, an input method). */
export type KeyAction =
  | { kind: 'move'; by: 'char' | 'word' | 'line' | 'lineEdge' | 'text'; forward: boolean; extend: boolean }
  | { kind: 'delete'; by: 'char' | 'word' | 'lineEdge'; forward: boolean }
  | { kind: 'insert'; text: string }
  | { kind: 'selectAll' | 'commit' | 'cancel' | 'undo' | 'redo' | 'save' }

export type KeyInput = { key: string; code?: string; shift: boolean; alt: boolean; meta: boolean; ctrl: boolean }

/**
 * What a key does (Photoshop's text editing keys): arrows move the caret (Alt, or Ctrl off the
 * Mac, a word; Cmd on the Mac the line's or the text's edge; Shift extends the selection), Home
 * and End go to the line's edges (Cmd or Ctrl: the text's), Backspace and Delete remove a
 * character (Alt: a word; Cmd: to the line's start), Enter starts a new line, Tab inserts a tab,
 * Cmd+Enter or the keypad's Enter commits, Escape cancels, Cmd+A selects all, Cmd+Z and Cmd+Shift+Z
 * (or Cmd+Y) undo and redo, Cmd+S saves. `mac` says which modifier is the command key.
 */
export function keyAction(input: KeyInput, mac: boolean): KeyAction | null {
  const { key, shift, alt, meta, ctrl } = input
  const command = meta || ctrl
  const lower = key.length === 1 ? key.toLowerCase() : key
  if (key === 'Escape') return { kind: 'cancel' }
  if (key === 'Enter') return command || input.code === 'NumpadEnter' ? { kind: 'commit' } : { kind: 'insert', text: '\n' }
  if (command && !alt) {
    if (lower === 'a') return { kind: 'selectAll' }
    if (lower === 'z') return { kind: shift ? 'redo' : 'undo' }
    if (lower === 'y') return { kind: 'redo' }
    if (lower === 's') return { kind: 'save' }
  }
  const lineEdgeKey = mac ? meta : false
  const wordKey = alt || (!mac && ctrl)
  if (key === 'ArrowLeft' || key === 'ArrowRight') {
    const forward = key === 'ArrowRight'
    return { kind: 'move', by: lineEdgeKey ? 'lineEdge' : wordKey ? 'word' : 'char', forward, extend: shift }
  }
  if (key === 'ArrowUp' || key === 'ArrowDown') {
    const forward = key === 'ArrowDown'
    return { kind: 'move', by: lineEdgeKey || (!mac && ctrl) ? 'text' : 'line', forward, extend: shift }
  }
  if (key === 'Home' || key === 'End') return { kind: 'move', by: command ? 'text' : 'lineEdge', forward: key === 'End', extend: shift }
  if (key === 'Backspace' || key === 'Delete') {
    const forward = key === 'Delete'
    return { kind: 'delete', by: mac && meta ? 'lineEdge' : wordKey ? 'word' : 'char', forward }
  }
  if (key === 'Tab' && !command && !alt) return { kind: 'insert', text: '\t' }
  return null
}

/** The selection after a caret move to `to`: extended from the anchor, or collapsed there. */
export const moved = (selection: Selection, to: number, extend: boolean): Selection => (extend ? { anchor: selection.anchor, focus: to } : caretAt(to))

/**
 * Where a plain Left or Right arrow goes from `selection`: one character, or, with a selection and
 * without Shift, to its start or end (as text fields do).
 */
export function charStep(selection: Selection, forward: boolean, extend: boolean, length: number): number {
  if (!extend && !collapsed(selection)) return ordered(selection)[forward ? 1 : 0]
  return Math.min(length, Math.max(0, selection.focus + (forward ? 1 : -1)))
}

/** The characters a Backspace or Delete removes from `selection`: the selection, else what `by` reaches. */
export function deletion(text: string, selection: Selection, by: 'char' | 'word' | 'lineEdge', forward: boolean, lineStart: number): Range {
  if (!collapsed(selection)) return ordered(selection)
  const at = selection.focus
  const length = charCount(text)
  if (by === 'word') return forward ? [at, wordBoundary(text, at, true)] : [wordBoundary(text, at, false), at]
  if (by === 'lineEdge') return forward ? [at, length] : [Math.min(lineStart, at), at]
  return forward ? [at, Math.min(length, at + 1)] : [Math.max(0, at - 1), at]
}

/** `m` applied to (x, y). */
export const apply = (m: Affine, x: number, y: number): Point => ({ x: m[0] * x + m[2] * y + m[4], y: m[1] * x + m[3] * y + m[5] })

/** The inverse of `m` (the identity when `m` cannot be inverted). */
export function invert(m: Affine): Affine {
  const det = m[0] * m[3] - m[1] * m[2]
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return [1, 0, 0, 1, 0, 0]
  const a = m[3] / det, b = -m[1] / det, c = -m[2] / det, d = m[0] / det
  return [a, b, c, d, -(a * m[4] + c * m[5]), -(b * m[4] + d * m[5])]
}

/** Line space to document pixels: vertical type turns line space a quarter (the engine's `to_text`). */
export function lineToDoc(m: Affine, vertical: boolean, x: number, y: number): Point {
  return vertical ? apply(m, -y, x) : apply(m, x, y)
}

/** Document pixels to line space (the inverse of `lineToDoc`). */
export function docToLine(m: Affine, vertical: boolean, x: number, y: number): Point {
  const t = apply(invert(m), x, y)
  return vertical ? { x: t.y, y: -t.x } : t
}

/** A rectangle in line space, as a document quad. */
export function lineQuad(m: Affine, vertical: boolean, x0: number, y0: number, x1: number, y1: number): Quad {
  return [lineToDoc(m, vertical, x0, y0), lineToDoc(m, vertical, x1, y0), lineToDoc(m, vertical, x1, y1), lineToDoc(m, vertical, x0, y1)]
}

/**
 * The highlight of characters `range` (one rectangle per line it touches, as document quads).
 * `caretX(i)` is caret `i`'s place along its line (line space); a range running past a line's end
 * reaches the line's end, and one that takes the line break gets `breakWidth` more, so a selected
 * empty line still shows.
 */
export function rangeQuads(text: string, lines: readonly Line[], range: Range, caretX: (index: number) => number, m: Affine, vertical: boolean, breakWidth: number): Quad[] {
  const [a, b] = range
  if (a >= b) return []
  const list = chars(text)
  const quads: Quad[] = []
  for (const line of lines) {
    const breaks = list[line.end] === '\n'
    const touches = (a < line.end && b > line.start) || (a <= line.end && b > line.end && breaks)
    if (!touches) continue
    const x0 = a <= line.start ? line.x0 : caretX(a)
    const x1 = b >= line.end ? line.x1 + (b > line.end && breaks ? breakWidth : 0) : caretX(b)
    quads.push(lineQuad(m, vertical, Math.min(x0, x1), line.baseline - line.ascent, Math.max(x0, x1), line.baseline + line.descent))
  }
  return quads
}

/** The eight handles of a paragraph box, clockwise from the top left (Photoshop's order). */
export const BOX_HANDLES = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'] as const
export type BoxHandle = (typeof BOX_HANDLES)[number]
/** A paragraph box in text space: its left, top, width and height. */
export type Box = { x: number; y: number; width: number; height: number }
/** The smallest box a handle drag leaves (text-space pixels). */
export const MIN_BOX = 8

/** Where handle `handle` of `box` is, in text space. */
export function handlePoint(box: Box, handle: BoxHandle): Point {
  const cx = box.x + box.width / 2
  const cy = box.y + box.height / 2
  const x = handle.includes('w') ? box.x : handle.includes('e') ? box.x + box.width : cx
  const y = handle.startsWith('n') ? box.y : handle.startsWith('s') ? box.y + box.height : cy
  return { x, y }
}

/** `box` with handle `handle` dragged to text-space point `to`; the opposite sides stay put. */
export function resizeBox(box: Box, handle: BoxHandle, to: Point): Box {
  let left = box.x, top = box.y, right = box.x + box.width, bottom = box.y + box.height
  if (handle.includes('w')) left = Math.min(to.x, right - MIN_BOX)
  if (handle.includes('e')) right = Math.max(to.x, left + MIN_BOX)
  if (handle.startsWith('n')) top = Math.min(to.y, bottom - MIN_BOX)
  if (handle.startsWith('s')) bottom = Math.max(to.y, top + MIN_BOX)
  return { x: left, y: top, width: right - left, height: bottom - top }
}

/**
 * `type.edit`'s `box` for text-space box `box` of a layer with transform `m`: the document point
 * the box's top left goes to, and its size (the engine keeps the transform's turn and scale).
 */
export function boxParam(m: Affine, box: Box): [number, number, number, number] {
  const origin = apply(m, box.x, box.y)
  return [origin.x, origin.y, box.width, box.height]
}

/** A paragraph box from `type.info`'s `shape` (`{ Box: {…} }`), or null for point text. */
export function boxOf(shape: unknown): Box | null {
  const box = (shape as { Box?: Partial<Box> } | null)?.Box
  if (!box || ![box.x, box.y, box.width, box.height].every((value) => typeof value === 'number' && Number.isFinite(value))) return null
  return { x: box.x!, y: box.y!, width: box.width!, height: box.height! }
}

/** A text-space rectangle from two document points of a drag, for `type.create`'s `box`. */
export function dragBox(from: Point, to: Point): [number, number, number, number] {
  const x = Math.round(Math.min(from.x, to.x))
  const y = Math.round(Math.min(from.y, to.y))
  return [x, y, Math.max(1, Math.round(Math.max(from.x, to.x)) - x), Math.max(1, Math.round(Math.max(from.y, to.y)) - y)]
}
