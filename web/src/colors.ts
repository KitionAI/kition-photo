import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { rgbToHex, useKitStore } from '@kition/sdk/editor'

/** A query as the editor's `query` runs it: a command answered as JSON, null when refused or failed. */
type Query = <R>(id: string, params?: Record<string, unknown>) => Promise<R | null>

export type ColorTarget = 'foreground' | 'background'
export type PhotoColors = { foreground: string; background: string }
export type Swatch = { name: string; hex: string }
export type SwatchList = { groups: { name: string; swatches: Swatch[] }[] }

/** Upstream's defaults (`tool.defaultColors`): black on white. */
const DEFAULTS: PhotoColors = { foreground: '#000000', background: '#ffffff' }

/** The kit whose store keeps the colours (`kit:kit.photo:colors`) and the swatch library (`kit:kit.photo:swatches`). */
const KIT = 'kit.photo'

/**
 * The largest swatch library every Photo tab shares (its Adobe Swatch Exchange bytes, base64): a
 * larger one (thousands of swatches) stays with the tab that made it, which says so once.
 */
export const SWATCHES_MAX_BYTES = 256 * 1024

const isHex = (value: unknown): value is string => typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value)
const validColors = (raw: unknown): PhotoColors | undefined => {
  const value = raw as Partial<PhotoColors> | null
  return value && isHex(value.foreground) && isHex(value.background) ? { foreground: value.foreground.toLowerCase(), background: value.background.toLowerCase() } : undefined
}
const validLibrary = (raw: unknown) => (typeof raw === 'string' && raw.length > 0 ? raw : undefined)
const same = (a: PhotoColors, b: PhotoColors) => a.foreground === b.foreground && a.background === b.background

/** `#rrggbb` from the engine's colour (`[r, g, b, a]`, 0 to 1), or null for anything else. */
function hexOfEngine(value: unknown): string | null {
  if (!Array.isArray(value) || value.length < 3 || !value.slice(0, 3).every((channel) => typeof channel === 'number' && Number.isFinite(channel))) return null
  return rgbToHex(value.slice(0, 3).map((channel: number) => channel * 255) as [number, number, number])
}

/** Whether this page has said its swatches stay with it (said once). */
let saidLocal = false

/**
 * The foreground and background colours and the swatch library. The engine's session holds them
 * (`session.tools`, `presets.swatches`), as upstream's does: colours are read with
 * `session.inspect` and written with `tool.setColors`, swapped and reset with `tool.swapColors`
 * and `tool.defaultColors`, swatches used, added, and deleted with `swatches.*`. None of these is
 * a history step.
 *
 * Every Photo tab shares them through Kit UI's kit store, as upstream's one session does: a
 * session takes them when its document opens, and a change another tab stores reaches it. This tab
 * stores only what changes here: what it sets, and what its own engine changed (an answer that
 * differs from the colours this tab last showed). An engine answer equal to what the tab showed
 * is never stored, so a tab that has not yet heard another's newer colour cannot write its older
 * one back over it. A write shows at once; an answer to a read that started before a write is
 * dropped. `say` tells the person, once, that a library too large to share stays with this tab.
 */
export function usePhotoColors(doc: unknown, revision: number, query: Query, say: () => void) {
  /** The colours this tab last showed, and gave its engine. */
  const shown = useRef<PhotoColors>(DEFAULTS)
  const writes = useRef(0)
  const [colors, store] = useKitStore<PhotoColors>(KIT, 'colors', {
    initial: DEFAULTS,
    validate: validColors,
    onExternal: (next) => {
      writes.current += 1
      shown.current = next
      void query('tool.setColors', { ...next })
    },
  })
  shown.current = colors

  const [swatches, setSwatches] = useState<SwatchList | null>(null)
  /** The library's bytes as this session last had them, to tell another tab's change from ours. */
  const library = useRef<string | null>(null)
  const readSwatches = useCallback(async () => setSwatches(await query<SwatchList>('swatch.list')), [query])
  /** A stored library into this session. */
  const takeSwatches = useCallback(
    async (data: string) => {
      if (data === library.current) return
      library.current = data
      await query('swatch.import', { data, format: 'ase', mode: 'replace' })
      await readSwatches()
    },
    [query, readSwatches],
  )
  const [storedLibrary, storeLibrary] = useKitStore<string | null>(KIT, 'swatches', {
    initial: null,
    validate: validLibrary,
    maxBytes: SWATCHES_MAX_BYTES,
    onExternal: (data) => void takeSwatches(data as string),
  })
  const storedLibraryNow = useRef(storedLibrary)
  storedLibraryNow.current = storedLibrary

  /** Shows the engine's colours, storing them only when this session's engine changed them. */
  const read = useCallback(async () => {
    const started = writes.current
    const answer = await query<{ foreground?: unknown; background?: unknown }>('session.inspect')
    if (!answer || writes.current !== started) return
    const foreground = hexOfEngine(answer.foreground)
    const background = hexOfEngine(answer.background)
    if (!foreground || !background || same(shown.current, { foreground, background })) return
    shown.current = { foreground, background }
    store(shown.current)
  }, [query, store])

  // A newly opened document's session takes what the editors share, then shows its own.
  useEffect(() => {
    if (!doc) return
    library.current = null
    void (async () => {
      writes.current += 1
      await query('tool.setColors', { ...shown.current })
      const data = storedLibraryNow.current
      if (data) await takeSwatches(data)
      else await readSwatches()
    })()
  }, [doc, query, readSwatches, takeSwatches])

  // After every later change (a swatch, the agent), the colours as the session has them.
  const opened = useRef<unknown>(null)
  useEffect(() => {
    if (!doc) return
    if (opened.current !== doc) {
      opened.current = doc
      return
    }
    void read()
  }, [doc, read, revision])

  /** Sets one or both colours (`#rrggbb`). */
  const set = useCallback(
    (patch: Partial<PhotoColors>) => {
      writes.current += 1
      shown.current = { ...shown.current, ...patch }
      store(shown.current)
      void query('tool.setColors', patch)
    },
    [query, store],
  )

  /** Runs one of the engine's colour commands, then shows (and shares) what it left. */
  const command = useCallback(
    async (id: string, params: Record<string, unknown> = {}) => {
      writes.current += 1
      const answer = await query(id, params)
      await read()
      return answer
    },
    [query, read],
  )

  /** Runs a swatch library command, then shows and shares the library (a too-large one stays with this tab). */
  const editLibrary = useCallback(
    async (id: string, params: Record<string, unknown>) => {
      const answer = await query(id, params)
      await readSwatches()
      const exported = await query<{ data?: string }>('swatch.export', { format: 'ase' })
      if (typeof exported?.data === 'string') {
        library.current = exported.data
        if (storeLibrary(exported.data) === 'tooLarge' && !saidLocal) {
          saidLocal = true
          say()
        }
      }
      return answer
    },
    [query, readSwatches, say, storeLibrary],
  )

  return useMemo(
    () => ({
      ...colors,
      set,
      /** X: the foreground and background trade places. */
      swap: () => void command('tool.swapColors'),
      /** D: black on white. */
      reset: () => void command('tool.defaultColors'),
      /** The swatch library, by group (null until the session answers). */
      swatches,
      /** A swatch (its index across every group) becomes the foreground or the background. */
      pickSwatch: (index: number, target: ColorTarget) => command('swatch.use', { index, target }),
      /** Adds `color` as a new swatch (Photoshop names it). */
      addSwatch: (color: string) => editLibrary('swatch.add', { color }),
      /** Deletes the swatch at `index` across every group. */
      deleteSwatch: (index: number) => editLibrary('swatch.delete', { index }),
    }),
    [colors, command, editLibrary, set, swatches],
  )
}

export type PhotoColorStore = ReturnType<typeof usePhotoColors>
