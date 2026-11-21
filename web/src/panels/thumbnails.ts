import { useEffect, useMemo, useState } from 'react'
import type { KitEngine } from '@kition/sdk/editor'

/**
 * Thumbnails the engine draws for the panels: a layer's content (`layerThumbnail`), its mask
 * (`maskThumbnail`), or every channel at once (`channelThumbnails`, one render of the composite
 * for all of them).
 */
export type ThumbnailSource = { kind: 'layer' | 'mask'; layer: number } | { kind: 'channels' }

/** Asks the engine for a source's encoded thumbnail, its longer side `max`. */
export type ThumbnailQuery = (source: ThumbnailSource, max: number) => Promise<Uint8Array>

/** Turns an encoded thumbnail into something to draw; null when it cannot. */
export type ThumbnailDecode<T> = (bytes: Uint8Array) => Promise<T | null>

/** Asks for the next refresh: once per animation frame in the editor. */
export type ThumbnailFrame = (run: () => void) => void

type Closable = { close?: () => void }

export function thumbnailKey(source: ThumbnailSource) {
  return source.kind === 'channels' ? 'channels' : `${source.kind}:${source.layer}`
}

/** An extra's answer, `[width u32 LE][height u32 LE]` then straight RGBA; null when it is not one. */
export function decodeThumbnail(bytes: Uint8Array): { width: number; height: number; rgba: Uint8ClampedArray<ArrayBuffer> } | null {
  if (bytes.byteLength < 8) return null
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const width = view.getUint32(0, true)
  const height = view.getUint32(4, true)
  if (!width || !height || bytes.byteLength < 8 + width * height * 4) return null
  const rgba = new Uint8ClampedArray(width * height * 4)
  rgba.set(bytes.subarray(8, 8 + rgba.length))
  return { width, height, rgba }
}

/**
 * Thumbnails cached per source and document revision. `update` names the revision and the sources
 * shown; the cache refreshes what is stale at most once per frame, one query at a time (the engine
 * answers in order anyway), and tells `changed` once per refresh. A change that lands while a
 * refresh runs is picked up by one more refresh after it, never by a second one alongside. A query
 * that fails leaves that thumbnail empty: a panel never waits on, or fails with, its thumbnails.
 * A thumbnail keeps showing its last image until the new one is decoded, so rows do not flicker.
 */
export class ThumbnailCache<T extends Closable> {
  private readonly entries = new Map<string, { revision: number; image: T | null }>()
  private wanted: readonly ThumbnailSource[] = []
  private revision = 0
  private scheduled = false
  private running = false
  private again = false
  private disposed = false
  private retired: T[] = []

  private readonly query: ThumbnailQuery
  private readonly decode: ThumbnailDecode<T>
  private readonly max: number
  private readonly frame: ThumbnailFrame
  private readonly changed: () => void

  constructor(query: ThumbnailQuery, decode: ThumbnailDecode<T>, max: number, frame: ThumbnailFrame, changed: () => void) {
    this.query = query
    this.decode = decode
    this.max = max
    this.frame = frame
    this.changed = changed
  }

  /** The sources shown at document revision `revision`. */
  update(revision: number, wanted: readonly ThumbnailSource[]) {
    this.revision = revision
    this.wanted = wanted
    this.schedule()
  }

  /** A source's latest image, null while it has none (not drawn yet, failed, or empty). */
  get(source: ThumbnailSource): T | null {
    return this.entries.get(thumbnailKey(source))?.image ?? null
  }

  /** Stops refreshing and releases every image. */
  dispose() {
    this.disposed = true
    for (const entry of this.entries.values()) entry.image?.close?.()
    this.release()
    this.entries.clear()
  }

  private schedule() {
    if (this.scheduled || this.disposed) return
    this.scheduled = true
    this.frame(() => {
      this.scheduled = false
      void this.refresh()
    })
  }

  /** Closes the images a refresh replaced: a frame or more later, once the panel draws the new ones (a closed bitmap cannot be drawn). */
  private release() {
    for (const image of this.retired.splice(0)) image.close?.()
  }

  private async refresh() {
    if (this.running) {
      this.again = true
      return
    }
    this.release()
    this.running = true
    const revision = this.revision
    const wanted = this.wanted
    let changed = false
    try {
      for (const source of wanted) {
        const key = thumbnailKey(source)
        if (this.entries.get(key)?.revision === revision) continue
        let image: T | null = null
        try {
          image = await this.decode(await this.query(source, this.max))
        } catch {
          image = null
        }
        if (this.disposed) {
          image?.close?.()
          return
        }
        const old = this.entries.get(key)?.image
        if (old && old !== image) this.retired.push(old)
        this.entries.set(key, { revision, image })
        changed = true
      }
      const keep = new Set(wanted.map(thumbnailKey))
      for (const [key, entry] of this.entries) {
        if (keep.has(key)) continue
        if (entry.image) this.retired.push(entry.image)
        this.entries.delete(key)
      }
    } finally {
      this.running = false
    }
    if (changed && !this.disposed) this.changed()
    if (this.again || this.revision !== revision || this.wanted !== wanted) {
      this.again = false
      this.schedule()
    }
  }
}

/** Decodes off the main path: `createImageBitmap` turns the pixels into a bitmap the browser draws at once. */
async function toBitmap(bytes: Uint8Array): Promise<ImageBitmap | null> {
  const decoded = decodeThumbnail(bytes)
  return decoded ? createImageBitmap(new ImageData(decoded.rgba, decoded.width, decoded.height)) : null
}

/** `channelThumbnails`' answer, split: a count, then thumbnails that each carry their size. */
export function splitThumbnails(bytes: Uint8Array): Uint8Array[] {
  if (bytes.byteLength < 4) return []
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const out: Uint8Array[] = []
  let at = 4
  for (let index = view.getUint32(0, true); index > 0 && at + 8 <= bytes.byteLength; index -= 1) {
    const end = at + 8 + view.getUint32(at, true) * view.getUint32(at + 4, true) * 4
    if (end > bytes.byteLength) break
    out.push(bytes.subarray(at, end))
    at = end
  }
  return out
}

/** Every channel's bitmap, in the Channels panel's order; released together. */
export type ChannelBitmaps = { images: (ImageBitmap | null)[]; close: () => void }

async function toChannelBitmaps(bytes: Uint8Array): Promise<ChannelBitmaps> {
  const images = await Promise.all(splitThumbnails(bytes).map((part) => toBitmap(part).catch(() => null)))
  return { images, close: () => images.forEach((image) => image?.close()) }
}

/** A cache of `T`s refreshed as the document changes (see `ThumbnailCache`); the returned function answers a source's. */
function useEngineThumbnails<T extends Closable>(doc: KitEngine | null, revision: number, sources: readonly ThumbnailSource[], max: number, decode: ThumbnailDecode<T>) {
  const [, setVersion] = useState(0)
  const cache = useMemo(() => {
    if (!doc) return null
    const query: ThumbnailQuery = (source, side) =>
      source.kind === 'channels' ? doc.invoke<Uint8Array>('channelThumbnails', side) : doc.invoke<Uint8Array>(source.kind === 'layer' ? 'layerThumbnail' : 'maskThumbnail', source.layer, side)
    return new ThumbnailCache<T>(query, decode, max, (run) => requestAnimationFrame(run), () => setVersion((value) => value + 1))
  }, [decode, doc, max])
  useEffect(() => () => cache?.dispose(), [cache])
  const key = sources.map(thumbnailKey).join('|')
  useEffect(() => {
    cache?.update(revision, sources)
    // `key` stands for `sources`, rebuilt every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cache, revision, key])
  return (source: ThumbnailSource) => cache?.get(source) ?? null
}

/**
 * The layer and mask thumbnails of `sources` for the open document at `revision`, `max` device
 * pixels on their longer side; the returned function answers a source's bitmap (null while there
 * is none).
 */
export function useThumbnails(doc: KitEngine | null, revision: number, sources: readonly ThumbnailSource[], max: number) {
  return useEngineThumbnails(doc, revision, sources, max, toBitmap)
}

const CHANNELS: ThumbnailSource[] = [{ kind: 'channels' }]
const NONE: ThumbnailSource[] = []

/** Every channel's thumbnail at `revision` (while `shown`), from one engine call per change. */
export function useChannelThumbnails(doc: KitEngine | null, revision: number, shown: boolean, max: number) {
  const get = useEngineThumbnails(doc, revision, shown ? CHANNELS : NONE, max, toChannelBitmaps)
  return get(CHANNELS[0])?.images ?? []
}
