import { useLayoutEffect, useRef, type ClipboardEvent, type CompositionEvent, type FormEvent, type KeyboardEvent, type RefObject } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'
import { useLive, type Live } from '../panels/live'
import type { TypeSession, TypeSessionView } from './session'
import type { Point, Quad } from './text'

/** Whether the command key is Cmd (the Mac) rather than Ctrl, for the word and line keys. */
const MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform)

/**
 * Type being edited on the canvas (plan 2026-11-24, stage 4): the caret, the selection's highlight,
 * an input method's composition (underlined; its text is the engine's, at the caret) and a
 * paragraph box's frame and handles, drawn over the engine's rendering of the text; and the hidden
 * text area that takes the keys, the input method and the clipboard for the session
 * (`TypeSession`). Undo, Redo and Save end the session first (`onAction`). Nothing is drawn, and
 * the canvas's view is not followed, while no text is edited.
 */
export function TypeEditor({ session, view, label, onAction }: { session: TypeSession; view: Live<KitCanvasView | null>; label: string; onAction: (kind: 'undo' | 'redo' | 'save') => void }) {
  const shown = useLive(session.view)
  const input = useRef<HTMLTextAreaElement>(null)
  // The keyboard goes to the text area once it is there (a session's start asks before it is drawn).
  const wantsFocus = useRef(false)
  useLayoutEffect(() => {
    session.focusInput = () => {
      wantsFocus.current = true
      if (!input.current) return
      wantsFocus.current = false
      input.current.focus({ preventScroll: true })
    }
    return () => {
      session.focusInput = () => undefined
    }
  }, [session])
  return shown ? <SessionMarks session={session} shown={shown} view={view} label={label} onAction={onAction} input={input} wantsFocus={wantsFocus} /> : null
}

function SessionMarks({ session, shown, view, label, onAction, input, wantsFocus }: { session: TypeSession; shown: TypeSessionView; view: Live<KitCanvasView | null>; label: string; onAction: (kind: 'undo' | 'redo' | 'save') => void; input: RefObject<HTMLTextAreaElement | null>; wantsFocus: RefObject<boolean> }) {
  const current = useLive(view)
  const composing = useRef(false)
  // What the text area holds: the selected text, selected, so the browser's own Copy finds it.
  const mirror = useRef('')
  const open = current != null
  useLayoutEffect(() => {
    if (!open || !wantsFocus.current || !input.current) return
    wantsFocus.current = false
    input.current.focus({ preventScroll: true })
    input.current.select()
  }, [open, input, wantsFocus])
  const rangeKey = `${shown.range[0]} ${shown.range[1]} ${shown.text.length}`
  useLayoutEffect(() => {
    const area = input.current
    if (!area || composing.current) return
    const text = session.selectedText()
    mirror.current = text
    if (area.value !== text) area.value = text
    if (document.activeElement === area) area.select()
  }, [rangeKey, session, input])

  const toScreen = (p: Point) => (current ? { x: (p.x - current.x) * current.zoom, y: (p.y - current.y) * current.zoom } : p)
  const points = (quad: Quad) => quad.map((p) => toScreen(p)).map((p) => `${p.x},${p.y}`).join(' ')
  const caret = shown.caret ? [toScreen(shown.caret[0]), toScreen(shown.caret[1])] : null
  const anchor = caret ? caret[1] : shown.selection.length ? toScreen(shown.selection[shown.selection.length - 1][2]) : { x: 0, y: 0 }

  const keyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    // The session's keys are not the editor's shortcuts (a letter types, it picks no tool).
    event.stopPropagation()
    // Keys that confirm or drop an input method's candidate are the input method's.
    if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229 || composing.current) return
    const action = session.key({ key: event.key, code: event.code, shift: event.shiftKey, alt: event.altKey, meta: event.metaKey, ctrl: event.ctrlKey }, MAC)
    if (!action) return
    event.preventDefault()
    if (action.kind === 'undo' || action.kind === 'redo' || action.kind === 'save') onAction(action.kind)
  }
  const typed = (event: FormEvent<HTMLTextAreaElement>) => {
    const area = event.currentTarget
    if (composing.current || (event.nativeEvent as InputEvent).isComposing) return
    const value = area.value
    if (value === mirror.current) return
    if (value) session.insert(value)
    else session.cut()
    mirror.current = ''
    area.value = ''
  }
  const compositionStart = () => {
    composing.current = true
  }
  const compositionUpdate = (event: CompositionEvent<HTMLTextAreaElement>) => {
    composing.current = true
    session.compose(event.data)
  }
  const compositionEnd = (event: CompositionEvent<HTMLTextAreaElement>) => {
    composing.current = false
    session.composeEnd(event.data)
    mirror.current = ''
    event.currentTarget.value = ''
  }
  const copy = (event: ClipboardEvent<HTMLTextAreaElement>, cut: boolean) => {
    const text = session.selectedText()
    event.preventDefault()
    if (!text) return
    event.clipboardData.setData('text/plain', text)
    if (cut) session.cut()
  }
  const paste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    event.preventDefault()
    const text = event.clipboardData.getData('text/plain')
    if (text) session.insert(text)
  }

  if (!current) return null
  return (
    <div className="photo-type-editor" data-testid="photo-type-editor" data-active="true" data-layer={shown.layer} data-selection={`${shown.range[0]} ${shown.range[1]}`} data-text={shown.text} data-lines={shown.lines}>
      <svg className="photo-type-marks" aria-hidden="true">
          {shown.box ? (
            <g data-testid="photo-type-box">
              <polygon points={points(shown.box.quad)} className="photo-type-box-frame" />
              {shown.box.handles.map((handle) => {
                const at = toScreen(handle.at)
                return <rect key={handle.id} x={at.x - 3.5} y={at.y - 3.5} width={7} height={7} className="photo-type-box-handle" data-handle={handle.id} data-testid={`photo-type-handle-${handle.id}`} />
              })}
            </g>
          ) : null}
          {shown.selection.map((quad, index) => (
            <polygon key={`s${index}`} points={points(quad)} className="photo-type-selection" data-testid="photo-type-selection" />
          ))}
          {shown.composition.map((quad, index) => {
            const [from, to] = [toScreen(quad[3]), toScreen(quad[2])]
            return <line key={`c${index}`} x1={from.x} y1={from.y} x2={to.x} y2={to.y} className="photo-type-composition" data-testid="photo-type-composition" />
          })}
          {caret ? (
            <g className="photo-type-caret" data-testid="photo-type-caret" data-x={Math.round(shown.caret![0].x)} data-y={Math.round(shown.caret![1].y)}>
              <line x1={caret[0].x} y1={caret[0].y} x2={caret[1].x} y2={caret[1].y} className="photo-type-caret-halo" />
              <line x1={caret[0].x} y1={caret[0].y} x2={caret[1].x} y2={caret[1].y} className="photo-type-caret-line" />
            </g>
          ) : null}
      </svg>
      <textarea
        ref={input}
        className="photo-type-input"
        style={{ left: anchor.x, top: anchor.y }}
        aria-label={label}
        autoCapitalize="off"
        autoComplete="off"
        autoCorrect="off"
        spellCheck={false}
        onKeyDown={keyDown}
        onInput={typed}
        onCompositionStart={compositionStart}
        onCompositionUpdate={compositionUpdate}
        onCompositionEnd={compositionEnd}
        onCopy={(event) => copy(event, false)}
        onCut={(event) => copy(event, true)}
        onPaste={paste}
        data-testid="photo-type-input"
      />
    </div>
  )
}
