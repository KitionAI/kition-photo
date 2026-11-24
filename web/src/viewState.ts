/**
 * The person's view of the image, kept by the host apart from the file (Kit UI protocol section
 * 7n, capability `view-state`): the selection, the quick mask, which groups are collapsed, open
 * notes, the timeline's playhead and the last applied layer comp. The `.kphoto` holds none of it,
 * so two people's saves of the same image are the same bytes; without the capability the view is
 * simply not kept between sessions.
 *
 * The engine answers the view (`viewState`: `[json length u32 LE][json][bytes]`) and puts one
 * back (the editor's own `photo.restoreViewState` command, no edit). The view is read again after
 * each change, one read at a time, so the save's `viewState` option has the latest at hand.
 */
import { useCallback, useEffect, useRef, type MutableRefObject } from 'react'
import { readViewState, type HostConnection, type KitEngine, type KitViewState } from '@kition/sdk/editor'

/** The engine command that puts a kept view back. */
export const RESTORE_VIEW_STATE = 'photo.restoreViewState'

/** The view the engine answered, as the host keeps it. */
export function unpackViewState(packed: Uint8Array): KitViewState {
  const view = new DataView(packed.buffer, packed.byteOffset, packed.byteLength)
  const length = view.getUint32(0, true)
  const state = JSON.parse(new TextDecoder().decode(packed.subarray(4, 4 + length))) as Record<string, unknown> | null
  if (state === null) return { state: null }
  const rest = packed.subarray(4 + length)
  return rest.length > 0 ? { state, bytes: new Uint8Array(rest).buffer as ArrayBuffer } : { state }
}

/** Standard base64 of `bytes`, as the restore command takes them. */
export function base64(bytes: ArrayBuffer | undefined): string {
  if (!bytes) return ''
  const all = new Uint8Array(bytes)
  let text = ''
  for (let at = 0; at < all.length; at += 0x8000) text += String.fromCharCode(...all.subarray(at, at + 0x8000))
  return btoa(text)
}

type ViewSession = {
  doc: KitEngine | null
  change: unknown
  path: string
  run: (command: string, params?: Record<string, unknown>) => Promise<unknown>
}

/**
 * Puts back the view the host kept for `session`'s document once each time it opens, then keeps
 * `kept` (what the session's `viewState` option answers) current. Until the kept view is back
 * nothing is answered, so a save in between writes no view over it.
 */
export function usePhotoViewState(host: HostConnection, session: ViewSession, kept: MutableRefObject<KitViewState | undefined>) {
  const { doc, change, path, run } = session
  const opened = useRef<KitEngine | null>(null)
  const ready = useRef(false)
  const reading = useRef(false)
  const again = useRef(false)

  const read = useCallback(() => {
    const open = opened.current
    if (!open || !ready.current) return
    if (reading.current) {
      again.current = true
      return
    }
    reading.current = true
    void open
      .invoke<Uint8Array>('viewState')
      .then((packed) => {
        if (opened.current === open) kept.current = unpackViewState(packed)
      })
      .catch(() => undefined)
      .finally(() => {
        reading.current = false
        if (again.current) {
          again.current = false
          read()
        }
      })
  }, [kept])

  useEffect(() => {
    if (!doc || opened.current === doc) return
    opened.current = doc
    ready.current = false
    kept.current = undefined
    void readViewState(host.files, path)
      .then((view) => (view?.state && opened.current === doc ? run(RESTORE_VIEW_STATE, { state: view.state, bytes: base64(view.bytes) }) : null))
      .catch(() => undefined)
      .finally(() => {
        if (opened.current !== doc) return
        ready.current = true
        read()
      })
  }, [doc, host, path, run, kept, read])

  useEffect(() => {
    read()
  }, [change, read])
}
