import { useEffect, useSyncExternalStore } from 'react'
import { parseManifest, type FontCatalog, type ServedFile } from './catalog'

/**
 * The served fonts at run time (plan 2026-11-24, stage 9): the catalog (`fonts/manifest.txt`,
 * fetched once per page), and loading a family: its files fetched once per page, given to each
 * engine document's `addFace` once, and registered with the page too, so the field that shows it
 * and the new-text field draw it in its own face.
 */

/** An engine document as the editor holds it (`session.doc`): `refreshInfo` after type layers were drawn again. */
export type FontEngine = { invoke<T = unknown>(name: string, ...args: unknown[]): Promise<T>; refreshInfo?: () => Promise<unknown> }

type Snapshot = { catalog: FontCatalog; loading: ReadonlySet<string> }

let snapshot: Snapshot = { catalog: new Map(), loading: new Set() }
let catalogLoad: Promise<FontCatalog> | null = null
const listeners = new Set<() => void>()
const files = new Map<string, Promise<Uint8Array | null>>()
const given = new WeakMap<FontEngine, Map<string, Promise<boolean>>>()
const pageFaces = new Set<string>()

const update = (next: Partial<Snapshot>) => {
  snapshot = { ...snapshot, ...next }
  for (const listener of listeners) listener()
}

/** The catalog, fetched once (an editor served without fonts has an empty one). */
export function loadCatalog(): Promise<FontCatalog> {
  catalogLoad ??= fetch(new URL('./fonts/manifest.txt', document.baseURI))
    .then((answer) => (answer.ok ? answer.text() : ''))
    .catch(() => '')
    .then((text) => {
      const catalog = parseManifest(text)
      update({ catalog })
      return catalog
    })
  return catalogLoad
}

/** The served catalog and the families loading now (the catalog is fetched on first use). */
export function useServedFonts(): Snapshot {
  useEffect(() => void loadCatalog(), [])
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => snapshot,
  )
}

const read = (file: string) => {
  let bytes = files.get(file)
  if (!bytes) {
    bytes = fetch(new URL(`./${file}`, document.baseURI))
      .then(async (answer) => (answer.ok ? new Uint8Array(await answer.arrayBuffer()) : null))
      .catch(() => null)
    files.set(file, bytes)
  }
  return bytes
}

/** A face's CSS descriptors from its manifest style (a variable file covers every weight). */
function descriptors(file: ServedFile): FontFaceDescriptors {
  const style = file.style.toLowerCase()
  const weight = /variable/i.test(file.file) ? '100 900' : style.includes('bold') ? '700' : '400'
  return { weight, style: style.includes('italic') ? 'italic' : 'normal' }
}

/** Registers a face with the page (once), for the fields that draw a family in its own face. */
function addToPage(file: ServedFile, bytes: Uint8Array) {
  if (pageFaces.has(file.file) || typeof FontFace === 'undefined') return
  pageFaces.add(file.file)
  try {
    const face = new FontFace(file.family, bytes.slice().buffer, descriptors(file))
    void face.load().then(
      (loaded) => document.fonts.add(loaded),
      () => undefined,
    )
  } catch {
    // The page draws it in a fallback; the engine has it all the same.
  }
}

/**
 * Gives `engine` the served family `family` (every file of it), once per engine: true when the
 * engine has it now. Families the editor does not serve answer false at once.
 */
export async function ensureFamily(engine: FontEngine, family: string): Promise<boolean> {
  const catalog = await loadCatalog()
  const entries = catalog.get(family)
  if (!entries) return false
  let mine = given.get(engine)
  if (!mine) {
    mine = new Map()
    given.set(engine, mine)
  }
  let done = mine.get(family)
  if (!done) {
    update({ loading: new Set([...snapshot.loading, family]) })
    done = (async () => {
      let added = false
      let refreshed = 0
      for (const entry of entries) {
        const bytes = await read(entry.file)
        if (!bytes) continue
        addToPage(entry, bytes)
        // `{families, refreshed}`: the families the file added, and the type layers drawn again in them.
        const answer = JSON.parse(await engine.invoke<string>('addFace', bytes).catch(() => '{}')) as { families?: string[]; refreshed?: number }
        added ||= (answer.families?.length ?? 0) > 0
        refreshed += answer.refreshed ?? 0
      }
      // Type layers drawn again: the page asks the engine for the picture anew.
      if (refreshed > 0) await engine.refreshInfo?.().catch(() => undefined)
      return added
    })().finally(() => {
      const loading = new Set(snapshot.loading)
      loading.delete(family)
      update({ loading })
    })
    mine.set(family, done)
    // A file that failed to load can be tried again next time.
    void done.then((ok) => {
      if (!ok) mine.delete(family)
    })
  }
  return done
}
