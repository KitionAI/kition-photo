import type { Live } from '../panels/live'
import {
  BOX_HANDLES,
  boxOf,
  boxParam,
  caretAt,
  charCount,
  charStep,
  clickSelection,
  collapsed,
  countClick,
  deletion,
  docToLine,
  handlePoint,
  keyAction,
  lineQuad,
  lineToDoc,
  moved,
  normalise,
  ordered,
  rangeQuads,
  resizeBox,
  apply,
  sliceChars,
  wordBoundary,
  type Affine,
  type Box,
  type BoxHandle,
  type KeyAction,
  type KeyInput,
  type Line,
  type Point,
  type Quad,
  type Range,
  type Selection,
} from './text.ts'

/** A value the canvas and the panels follow (`Live`, without React, so `node --test` loads this file). */
function live<T>(initial: T): Live<T> {
  let value = initial
  const listeners = new Set<() => void>()
  return {
    get: () => value,
    set(next) {
      if (Object.is(next, value)) return
      value = next
      for (const listener of listeners) listener()
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}

/**
 * Editing a type layer on the canvas (plan 2026-11-24, stage 4): the text shown is the engine's
 * own rendering, changed by `text.edit` as keys are typed, so what is seen while editing is what
 * stays. Every command of one edit session carries the session's `coalesce` key, so the whole
 * session is one history step (Photoshop's "Edit Type Layer"). The caret, the selection's
 * highlight, an input method's composition and a paragraph box's frame are drawn over the canvas
 * from the engine's `text.caret` and `text.info` (`TypeEditor.tsx`); a hidden text area takes the
 * keyboard, the input method and the clipboard.
 *
 * Steps run one at a time, in the order they came (`enqueue`): a key typed while the engine is
 * still answering the previous one acts on the text that answer left.
 */

/** What `text.info` answers that editing reads. */
export type TypeInfo = {
  layer: number
  text: string
  lines: Line[]
  transform: number[]
  orientation: string
  shape: unknown
  runs: { start: number; end: number; style: { size_pt: number; leading_pt?: number | null } }[]
  paragraphs: { start: number; end: number; style: { auto_leading?: number } }[]
  pxPerPt?: number
}

/** The engine as editing reaches it: commands (their result as JSON text, null when refused), queries, and the document's revision. */
export type TypeEngine = {
  run: (command: string, params: Record<string, unknown>) => Promise<string | null>
  query: <R>(command: string, params?: Record<string, unknown>) => Promise<R | null>
  revision: () => number
}

/** What the canvas draws for the session, in document pixels. */
export type TypeSessionView = {
  layer: number
  vertical: boolean
  /** The caret's two ends; null while a range is selected. */
  caret: readonly [Point, Point] | null
  selection: Quad[]
  composition: Quad[]
  box: { quad: Quad; handles: { id: BoxHandle; at: Point }[] } | null
  range: Range
  /** The text and its line count as the engine last laid them out. */
  text: string
  lines: number
}

/** What the type style controls act on while text is edited: the layer, the selected range (null: the whole layer), and how to run a command in the session's step. */
export type TypeTarget = {
  layer: number
  range: Range | null
  run: (command: string, params: Record<string, unknown>) => Promise<boolean>
}

export type StartOptions = {
  layer: number
  /** The caret's character, or a range to select. */
  select?: Range
  /** The session made the layer (new text): Escape, or committing it empty, removes it. */
  created?: boolean
  /** The step to join (new text's `text.add` used it); a fresh key otherwise. */
  key?: string
  /** The caret sits on a new line after the text that is not typed yet (Enter on new text). */
  pendingBreak?: boolean
}

type State = {
  layer: number
  key: string
  created: boolean
  edited: boolean
  text: string
  info: TypeInfo
  selection: Selection
  pendingBreak: boolean
  composing: { start: number; length: number } | null
  /** The document x the caret keeps across Up and Down. */
  column: number | null
  /** What a drag from the last press extends from (a word or paragraph after a double or triple click). */
  dragFrom: Range
}

let made = 0
/** A fresh history key for one edit session. */
export const sessionKey = (): string => `type-edit-${Date.now().toString(36)}-${++made}`

/** Photoshop's auto leading, a multiple of the size, when a paragraph does not say. */
const AUTO_LEADING = 1.2
/** How near a box handle a press must land (screen pixels). */
export const HANDLE_RADIUS = 6

const parse = (json: string | null): Record<string, unknown> | null => {
  if (json == null) return null
  try {
    const value = JSON.parse(json) as unknown
    return value && typeof value === 'object' ? (value as Record<string, unknown>) : null
  } catch {
    return null
  }
}

export class TypeSession {
  /** What the canvas draws (null: no session). */
  readonly view: Live<TypeSessionView | null> = live<TypeSessionView | null>(null)
  /** What the type style controls act on (null: no session). */
  readonly target: Live<TypeTarget | null> = live<TypeTarget | null>(null)
  /** Gives the keyboard to the session's text area (set by `TypeEditor`). */
  focusInput: () => void = () => undefined
  /** Told when a session ends: committed, cancelled, or ended by someone else's change. */
  onEnd: (how: 'commit' | 'cancel' | 'external') => void = () => undefined

  private state: State | null = null
  private chain: Promise<void> = Promise.resolve()
  private queued = 0
  /**
   * Text typed while steps ahead of it are still running, not yet started: the next keys join it,
   * so fast typing becomes one engine edit per round trip instead of one per key (plan 2026-11-24,
   * stage 8: 70 keys 15 ms apart kept up with, in order).
   */
  private waitingInsert: { text: string } | null = null
  /** The revision the session's own last command left; anything else means another edit came between. */
  private ours = 0
  private editing = 0
  private lastClick: { x: number; y: number; time: number; count: number } | null = null
  private pendingDrag: Point | null = null
  private pendingResize: { handle: BoxHandle; to: Point } | null = null

  private readonly engine: TypeEngine

  constructor(engine: TypeEngine) {
    this.engine = engine
  }

  get active(): boolean {
    return this.state != null
  }

  get layer(): number | null {
    return this.state?.layer ?? null
  }

  /** The session's history key (null: no session). */
  get historyKey(): string | null {
    return this.state?.key ?? null
  }

  /** The text being edited and the selection in it, for tests and the clipboard. */
  get selection(): { text: string; range: Range } | null {
    const s = this.state
    return s ? { text: s.text, range: ordered(s.selection) } : null
  }

  /** Runs `step` after every step before it; the canvas's marks follow once the queue is empty. */
  private enqueue(step: () => Promise<void> | void): Promise<void> {
    // A new step closes the waiting insert: later keys go after it, in order.
    this.waitingInsert = null
    this.queued += 1
    const go = async () => {
      try {
        await step()
      } catch (error) {
        console.error('type edit', error)
      } finally {
        this.queued -= 1
      }
      if (this.queued === 0 && this.state) await this.refresh().catch(() => undefined)
    }
    this.chain = this.chain.then(go)
    return this.chain
  }

  /** Waits for every step asked for so far. */
  settled(): Promise<void> {
    return this.chain
  }

  /**
   * Starts editing `layer`; false when it is no type layer any more (an undo or another person
   * removed it) or the document changed meanwhile.
   */
  async start(options: StartOptions): Promise<boolean> {
    if (this.state) await this.commit()
    const revision = this.engine.revision()
    const info = await this.engine.query<TypeInfo>('text.info', { layer: options.layer })
    if (!info || typeof info.text !== 'string' || this.engine.revision() !== revision || this.state) return false
    const length = charCount(info.text)
    const [a, b] = options.select ?? [length, length]
    const range: Range = [Math.min(length, Math.max(0, a)), Math.min(length, Math.max(0, b))]
    this.state = {
      layer: options.layer,
      key: options.key ?? sessionKey(),
      created: options.created ?? false,
      edited: false,
      text: info.text,
      info,
      selection: { anchor: range[0], focus: range[1] },
      pendingBreak: options.pendingBreak ?? false,
      composing: null,
      column: null,
      dragFrom: range,
    }
    this.ours = revision
    this.publishTarget()
    await this.refresh()
    this.focusInput()
    return true
  }

  /** Ends the session, keeping what was typed; new text left empty is removed. */
  commit(): Promise<void> {
    return this.enqueue(async () => {
      const s = this.state
      if (!s) return
      s.pendingBreak = false
      if (s.created && !s.text.trim() && this.engine.revision() === this.ours) await this.dropStep()
      this.end('commit')
    })
  }

  /** Ends the session and takes its step back (new text goes away); the text is as it was. */
  cancel(): Promise<void> {
    return this.enqueue(async () => {
      const s = this.state
      if (!s) return
      if ((s.edited || s.created) && this.engine.revision() === this.ours) await this.dropStep()
      this.end('cancel')
    })
  }

  /** Someone else changed the document (an undo, a panel, an agent): the session ends where it is. */
  checkExternal(): void {
    if (this.state && this.editing === 0 && this.queued === 0 && this.engine.revision() !== this.ours) this.end('external')
  }

  /** Ends at once without touching the document (the document was replaced). */
  drop(): void {
    if (this.state) this.end('external')
  }

  private end(how: 'commit' | 'cancel' | 'external') {
    this.state = null
    this.pendingDrag = null
    this.pendingResize = null
    this.view.set(null)
    this.target.set(null)
    this.onEnd(how)
  }

  /** Deletes the session's history step (else undoes it): the document is as before the session. */
  private async dropStep() {
    if ((await this.engine.run('history.deleteState', {})) === null) await this.engine.run('edit.undo', {})
    this.ours = this.engine.revision()
  }

  private publishTarget() {
    const s = this.state
    if (!s) return this.target.set(null)
    const range = ordered(s.selection)
    const current = this.target.get()
    const next: TypeTarget = { layer: s.layer, range: range[0] === range[1] ? null : range, run: (command, params) => this.runInSession(command, params) }
    if (current && current.layer === next.layer && current.range?.[0] === next.range?.[0] && current.range?.[1] === next.range?.[1]) return
    this.target.set(next)
  }

  /** A command on the edited layer in the session's step (a style change from the options bar or a panel). */
  runInSession(command: string, params: Record<string, unknown>): Promise<boolean> {
    let done = false
    return this.enqueue(async () => {
      if (!this.state) return
      done = await this.edit(command, params)
    }).then(() => done)
  }

  /** Runs `command` on the session's layer in its step, taking the text and lines it leaves. */
  private async edit(command: string, params: Record<string, unknown>): Promise<boolean> {
    const s = this.state
    if (!s) return false
    if (this.engine.revision() !== this.ours) {
      // Another edit came between: this session's step is not the last one any more.
      this.end('external')
      return false
    }
    this.editing += 1
    try {
      const result = parse(await this.engine.run(command, { layer: s.layer, ...params, coalesce: s.key }))
      this.ours = this.engine.revision()
      if (!result || this.state !== s) return false
      s.edited = true
      const info = typeof result.text === 'string' && Array.isArray(result.lines) ? (result as unknown as TypeInfo) : await this.engine.query<TypeInfo>('text.info', { layer: s.layer })
      if (info && typeof info.text === 'string') {
        s.info = info
        s.text = info.text
      }
      return true
    } finally {
      this.editing -= 1
    }
  }

  /** Replaces characters `a`..`b` with `insert`; answers where the inserted text starts (null when refused). */
  private async replace(a: number, b: number, insert: string): Promise<number | null> {
    const s = this.state
    if (!s) return null
    let text = normalise(insert)
    let start = a
    let end = b
    let offset = 0
    const pendingBreak = s.pendingBreak
    if (pendingBreak) {
      // The caret is on the new line Enter made after new text: the line break goes in with what is typed there.
      s.pendingBreak = false
      start = end = charCount(s.text)
      text = `\n${text}`
      offset = 1
    }
    if (start === end && !text) return start
    const ok = await this.edit('text.edit', { replace: { start, end, text } })
    if (!ok) {
      if (this.state === s) s.pendingBreak = pendingBreak
      return null
    }
    if (this.state === s) {
      s.selection = caretAt(Math.min(charCount(s.text), start + charCount(text)))
      s.column = null
    }
    return start + offset
  }

  /** Types `text` over the selection (a key, a paste). */
  insert(text: string): void {
    const waiting = this.waitingInsert
    if (waiting) {
      waiting.text += text
      return
    }
    const job = { text }
    const behind = this.queued > 0
    this.enqueue(async () => {
      if (this.waitingInsert === job) this.waitingInsert = null
      const s = this.state
      if (!s || !job.text) return
      const [a, b] = ordered(s.selection)
      await this.replace(a, b, job.text)
      this.publishTarget()
    })
    // Only behind other steps: a key typed with nothing under way goes to the engine at once.
    if (behind) this.waitingInsert = job
  }

  /** The selected text, for Copy and Cut. */
  selectedText(): string {
    const s = this.state
    if (!s) return ''
    const [a, b] = ordered(s.selection)
    return sliceChars(s.text, a, b)
  }

  /** Cut: the selection goes (Copy has already put it on the clipboard). */
  cut(): void {
    this.enqueue(async () => {
      const s = this.state
      if (!s || collapsed(s.selection)) return
      const [a, b] = ordered(s.selection)
      await this.replace(a, b, '')
      this.publishTarget()
    })
  }

  /** An input method's text while it is being composed: shown at the caret, replaced by each update. */
  compose(data: string): void {
    this.enqueue(async () => {
      const s = this.state
      if (!s) return
      const composing = s.composing
      const [start, end] = composing ? [composing.start, composing.start + composing.length] : ordered(s.selection)
      const at = await this.replace(start, end, data)
      if (at === null || this.state !== s) return
      s.composing = data ? { start: at, length: charCount(normalise(data)) } : null
    })
  }

  /** The input method finished: `data` replaces the composition (empty: it was dropped). */
  composeEnd(data: string): void {
    this.enqueue(async () => {
      const s = this.state
      if (!s) return
      const composing = s.composing
      s.composing = null
      if (composing) await this.replace(composing.start, composing.start + composing.length, data)
      else if (data) {
        const [a, b] = ordered(s.selection)
        await this.replace(a, b, data)
      }
      this.publishTarget()
    })
  }

  /** Whether the session is composing input-method text. */
  get composing(): boolean {
    return this.state?.composing != null
  }

  /**
   * What `input` does as a key (`keyAction`); the session does it, except Undo, Redo and Save,
   * which it answers for the editor to finish the session and run. Null: not the session's key.
   */
  key(input: KeyInput, mac: boolean): KeyAction | null {
    const action = keyAction(input, mac)
    if (!action || !this.state) return action
    if (action.kind === 'commit') void this.commit()
    else if (action.kind === 'cancel') void this.cancel()
    else if (action.kind === 'insert') this.insert(action.text)
    else if (action.kind === 'selectAll') this.selectAll()
    else if (action.kind === 'move' || action.kind === 'delete') this.enqueue(() => this.perform(action))
    return action
  }

  /** Selects all the text (Cmd+A). */
  selectAll(): void {
    this.enqueue(() => {
      const s = this.state
      if (!s) return
      s.pendingBreak = false
      s.selection = { anchor: 0, focus: charCount(s.text) }
      s.column = null
      this.publishTarget()
    })
  }

  private async navigate(index: number, move: string, x?: number): Promise<number | null> {
    const s = this.state
    if (!s) return null
    const answer = await this.engine.query<{ index: number }>('text.navigate', { layer: s.layer, index, move, ...(x != null ? { x } : {}) })
    return typeof answer?.index === 'number' ? answer.index : null
  }

  private async perform(action: Extract<KeyAction, { kind: 'move' | 'delete' }>) {
    const s = this.state
    if (!s) return
    const length = charCount(s.text)
    if (s.pendingBreak) {
      // The line Enter made is not typed yet: Backspace or a move takes it away, the caret back at the text's end.
      s.pendingBreak = false
      if (action.kind === 'delete' || !action.forward) {
        s.selection = caretAt(length)
        return
      }
    }
    if (action.kind === 'delete') {
      const lineStart = action.by === 'lineEdge' && !action.forward ? ((await this.navigate(s.selection.focus, 'lineStart')) ?? 0) : 0
      const [a, b] = deletion(s.text, s.selection, action.by, action.forward, lineStart)
      if (a !== b) await this.replace(a, b, '')
      this.publishTarget()
      return
    }
    const focus = s.selection.focus
    let to: number | null = focus
    let column: number | null = null
    if (action.by === 'char') to = charStep(s.selection, action.forward, action.extend, length)
    else if (action.by === 'word') to = wordBoundary(s.text, focus, action.forward)
    else if (action.by === 'text') to = action.forward ? length : 0
    else if (action.by === 'lineEdge') to = await this.navigate(focus, action.forward ? 'lineEnd' : 'lineStart')
    else {
      column = s.column ?? (await this.caretSegment(focus))?.[0].x ?? null
      to = await this.navigate(focus, action.forward ? 'lineNext' : 'linePrev', column ?? undefined)
    }
    if (to === null || this.state !== s) return
    s.selection = moved(s.selection, to, action.extend)
    s.column = column
    this.publishTarget()
  }

  private async caretSegment(index: number): Promise<[Point, Point] | null> {
    const s = this.state
    if (!s) return null
    const answer = await this.engine.query<{ segment: number[][] }>('text.caret', { layer: s.layer, index: Math.min(index, charCount(s.text)) })
    const seg = answer?.segment
    if (!Array.isArray(seg) || seg.length !== 2) return null
    return [
      { x: seg[0][0], y: seg[0][1] },
      { x: seg[1][0], y: seg[1][1] },
    ]
  }

  /** The edited layer's transform and orientation. */
  private frame(s: State): { m: Affine; vertical: boolean } {
    const t = s.info.transform
    const m = (Array.isArray(t) && t.length === 6 ? t : [1, 0, 0, 1, 0, 0]) as unknown as Affine
    return { m, vertical: s.info.orientation === 'Vertical' }
  }

  /** The line height of the last line (for the caret on the new line Enter makes). */
  private lineStep(s: State): number {
    const lines = s.info.lines
    if (lines.length >= 2) return lines[lines.length - 1].baseline - lines[lines.length - 2].baseline
    const run = s.info.runs[s.info.runs.length - 1]?.style
    const auto = s.info.paragraphs[s.info.paragraphs.length - 1]?.style.auto_leading ?? AUTO_LEADING
    const size = run?.size_pt ?? 12
    return (run?.leading_pt ?? size * auto) * (s.info.pxPerPt ?? 1)
  }

  /** Recomputes what the canvas draws from the engine's caret and lines. */
  private async refresh() {
    const s = this.state
    if (!s) return
    const { m, vertical } = this.frame(s)
    const [a, b] = ordered(s.selection)
    const wanted = new Set<number>([a, b])
    if (s.composing) {
      wanted.add(s.composing.start)
      wanted.add(s.composing.start + s.composing.length)
    }
    const segments = new Map<number, [Point, Point] | null>()
    for (const index of wanted) segments.set(index, await this.caretSegment(index))
    if (this.state !== s || this.queued > 0) return
    const caretX = (index: number) => {
      const seg = segments.get(index)
      return seg ? docToLine(m, vertical, seg[0].x, seg[0].y).x : 0
    }
    const size = (s.info.runs[0]?.style.size_pt ?? 12) * (s.info.pxPerPt ?? 1)
    const selection = rangeQuads(s.text, s.info.lines, [a, b], caretX, m, vertical, size * 0.3)
    const composition = s.composing ? rangeQuads(s.text, s.info.lines, [s.composing.start, s.composing.start + s.composing.length], caretX, m, vertical, 0) : []
    let caret: readonly [Point, Point] | null = a === b ? (segments.get(a) ?? null) : null
    if (s.pendingBreak) {
      const last = s.info.lines[s.info.lines.length - 1]
      if (last) {
        const y = last.baseline + this.lineStep(s)
        caret = [lineToDoc(m, vertical, 0, y - last.ascent), lineToDoc(m, vertical, 0, y + last.descent)]
      }
    }
    const box = boxOf(s.info.shape)
    this.view.set({
      layer: s.layer,
      vertical,
      caret,
      selection,
      composition,
      box: box ? { quad: boxQuad(m, box), handles: BOX_HANDLES.map((id) => ({ id, at: apply(m, handlePoint(box, id).x, handlePoint(box, id).y) })) } : null,
      range: [a, b],
      text: s.text,
      lines: s.info.lines.length,
    })
  }

  /** Whether document point `p` is on the edited text: its paragraph box, or its lines (`slop` document pixels round them). */
  contains(p: Point, slop: number): boolean {
    const s = this.state
    if (!s) return false
    const { m, vertical } = this.frame(s)
    const box = boxOf(s.info.shape)
    if (box) {
      const t = docToLine(m, false, p.x, p.y)
      return t.x >= box.x - slop && t.x <= box.x + box.width + slop && t.y >= box.y - slop && t.y <= box.y + box.height + slop
    }
    const at = docToLine(m, vertical, p.x, p.y)
    const lines = s.info.lines
    if (!lines.length) return false
    const x0 = Math.min(0, ...lines.map((line) => line.x0))
    const x1 = Math.max(0, ...lines.map((line) => line.x1))
    const y0 = Math.min(...lines.map((line) => line.baseline - line.ascent))
    let y1 = Math.max(...lines.map((line) => line.baseline + line.descent))
    if (s.pendingBreak) y1 += this.lineStep(s)
    return at.x >= x0 - slop && at.x <= x1 + slop && at.y >= y0 - slop && at.y <= y1 + slop
  }

  /** The paragraph box's handle under screen point `p` (`toScreen` maps document points), or null. */
  handleAt(p: Point, toScreen: (x: number, y: number) => Point): BoxHandle | null {
    const box = this.view.get()?.box
    if (!box) return null
    for (const handle of box.handles) {
      const at = toScreen(handle.at.x, handle.at.y)
      if (Math.hypot(at.x - p.x, at.y - p.y) <= HANDLE_RADIUS) return handle.id
    }
    return null
  }

  /** Counts a press at screen point `screen` with the presses before it (a double click is 2), whether or not text is edited yet. */
  countPress(screen: Point): number {
    const count = countClick(this.lastClick, { ...screen, time: Date.now() })
    this.lastClick = { ...screen, time: Date.now(), count }
    return count
  }

  /**
   * A press on the text at document point `at` (`screen`: where on the screen, for counting clicks;
   * `counted`: the count when the press was counted already): the caret goes to the character
   * there; a double click selects the word, a triple click the paragraph, a fourth everything;
   * Shift extends the selection.
   */
  press(at: Point, screen: Point, shift: boolean, counted?: number): void {
    const count = shift ? 1 : (counted ?? this.countPress(screen))
    this.enqueue(async () => {
      const s = this.state
      if (!s) return
      const hit = await this.engine.query<{ index: number }>('text.hitTest', { layer: s.layer, x: at.x, y: at.y })
      if (typeof hit?.index !== 'number' || this.state !== s) return
      s.pendingBreak = false
      s.column = null
      if (shift) {
        s.selection = moved(s.selection, hit.index, true)
        s.dragFrom = [s.selection.anchor, s.selection.anchor]
      } else {
        const range = clickSelection(s.text, hit.index, count)
        s.selection = { anchor: range[0], focus: range[1] }
        s.dragFrom = range
      }
      this.publishTarget()
    })
  }

  /** The pointer dragged to `at` after a press on the text: the selection runs from the press to there. */
  dragTo(at: Point): void {
    const first = this.pendingDrag == null
    this.pendingDrag = at
    if (!first) return
    this.enqueue(async () => {
      const s = this.state
      const point = this.pendingDrag
      this.pendingDrag = null
      if (!s || !point) return
      const hit = await this.engine.query<{ index: number }>('text.hitTest', { layer: s.layer, x: point.x, y: point.y })
      if (typeof hit?.index !== 'number' || this.state !== s) return
      const [from, to] = s.dragFrom
      s.selection = hit.index < from ? { anchor: to, focus: hit.index } : { anchor: from, focus: Math.max(to, hit.index) }
      this.publishTarget()
    })
  }

  /** A box handle dragged to document point `to`: the paragraph box follows, its text wrapping again. */
  resize(handle: BoxHandle, to: Point): void {
    const first = this.pendingResize == null
    this.pendingResize = { handle, to }
    if (!first) return
    this.enqueue(async () => {
      const s = this.state
      const next = this.pendingResize
      this.pendingResize = null
      const box = s ? boxOf(s.info.shape) : null
      if (!s || !next || !box) return
      const { m } = this.frame(s)
      const local = docToLine(m, false, next.to.x, next.to.y)
      await this.edit('text.edit', { box: boxParam(m, resizeBox(box, next.handle, local)) })
    })
  }
}

/** A text-space box as a document quad. */
function boxQuad(m: Affine, box: Box): Quad {
  return lineQuad(m, false, box.x, box.y, box.x + box.width, box.y + box.height)
}
