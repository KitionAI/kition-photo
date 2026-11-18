import { useLayoutEffect, useRef, useState, type RefObject } from 'react'
import { useKitEditor, useUi, type KitCanvasView } from '@kition/kit-editor'
import { useLive, type Live } from './panels/live'

/** What the Type bar's Commit and Cancel buttons do to the text being typed. */
export type TextFieldActions = { commit: (focusEditor?: boolean) => Promise<boolean>; cancel: () => void }

/** Where new text goes: a document point, where it shows on the canvas, and which way it runs. */
export type TextDraft = { id: number; x: number; y: number; left: number; top: number; orientation: 'horizontal' | 'vertical' }

/** The Type tool's inline field, at the clicked baseline; Enter places the text, Escape drops it. */
export function TextField({ draft, submitting, focusOnMount, size, view, color, placeholder, label, actions, onTextChange, onCommit, onCancel }: { draft: TextDraft; submitting: boolean; focusOnMount: boolean; size: number; view: Live<KitCanvasView | null>; color: string; placeholder: string; label: string; actions: RefObject<TextFieldActions | null>; onTextChange: (text: string) => void; onCommit: (text: string, focusEditor: boolean) => Promise<boolean>; onCancel: (focusEditor: boolean) => void }) {
  const editor = useKitEditor()
  const ui = useUi()
  const allowed = !editor.host.readOnly && !ui.state(`tool:${draft.orientation === 'vertical' ? 'VerticalType' : 'Type'}`).denied
  const allowedNow = useRef(allowed)
  allowedNow.current = allowed
  const [text, setText] = useState('')
  const finished = useRef(false)
  const completion = useRef<Promise<boolean> | null>(null)
  const current = useLive(view)
  const shown = Math.max(10, size * (current?.zoom ?? 1))
  // Follow the document anchor while the view scrolls or zooms; keep the same input and focus.
  const left = current ? (draft.x - current.x) * current.zoom : draft.left
  const baseline = current ? (draft.y - current.y) * current.zoom : draft.top
  const finish = (focusEditor = true) => {
    // A canvas press can blur this field and explicitly finish it in the same event.
    if (finished.current) return completion.current ?? Promise.resolve(false)
    // Keep the draft for recovery or cancellation. An automatic tool fallback can call an
    // earlier commit closure during layout, so read the permission from the current render.
    if (text.trim() && !allowedNow.current) return Promise.resolve(false)
    finished.current = true
    if (text.trim()) completion.current = onCommit(text, focusEditor).then((committed) => {
      if (!committed) {
        finished.current = false
        completion.current = null
      }
      return committed
    })
    else {
      onCancel(focusEditor)
      completion.current = Promise.resolve(true)
    }
    return completion.current
  }
  const cancel = () => {
    if (finished.current) return
    finished.current = true
    onCancel(true)
  }
  useLayoutEffect(() => {
    actions.current = { commit: (focusEditor = true) => finish(focusEditor), cancel }
    return () => {
      actions.current = null
    }
  })
  // Keep the component's text while its input is absent during submission. A refusal shows
  // the same draft again; only a successful command removes the component from the editor.
  if (submitting) return null
  return (
    <input
      autoFocus={focusOnMount}
      value={text}
      readOnly={!allowed}
      onChange={(event) => {
        setText(event.target.value)
        onTextChange(event.target.value)
      }}
      onKeyDown={(event) => {
        event.stopPropagation()
        // Candidate confirmation/cancellation belongs to the IME. WebKit can report 229
        // with isComposing=false on the key that ends a composition.
        if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return
        if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === 's') {
          event.preventDefault()
          if (!ui.state('action:save').denied && !editor.host.readOnly) {
            // App's shared Save finishes the draft before asking the host to write it.
            void editor.save()
          }
          return
        }
        if (event.key === 'Enter') finish()
        if (event.key === 'Escape') cancel()
      }}
      // The options bar's fields change the text being typed: moving to them keeps it open.
      onBlur={(event) => {
        if (!(event.relatedTarget as Element | null)?.closest('.kui-options-bar')) finish(false)
      }}
      placeholder={placeholder}
      aria-label={label}
      className="photo-text-field"
      style={{ left, top: baseline - shown, fontSize: shown, color }}
      data-testid="photo-text-field"
    />
  )
}
