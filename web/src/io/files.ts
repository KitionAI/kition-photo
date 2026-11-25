/**
 * Files from the computer and the system clipboard (plan 2026-11-24, stage 6): which the editor
 * places as a layer (an image the engine reads: PNG, JPEG, WebP, GIF, TIFF, BMP, PSD, …), what a
 * drag carries, and the bytes as the base64 `file.placeEmbedded` takes. Pure, so `node --test`
 * checks it; the editor's handlers are in `useFileIo.ts`.
 */

/** Extensions of files the engine opens as an image (`kition-photo-io`'s import). */
export const PLACEABLE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'tif', 'tiff', 'bmp', 'psd', 'psb', 'kphoto', 'svg', 'tga', 'ico', 'heic', 'avif'] as const

/** What the host's picker offers for Place (its `accept`). */
export const PLACE_ACCEPT: readonly string[] = PLACEABLE_EXTENSIONS.map((extension) => `.${extension}`)

/** What the host's picker offers for Open: the documents Photo edits, and images it opens as one. */
export const OPEN_ACCEPT: readonly string[] = PLACE_ACCEPT

const extensionOf = (name: string) => (/\.([^./\\]+)$/.exec(name)?.[1] ?? '').toLowerCase()

/** Whether a file (its name and MIME type) is one to place: an image type, or a known extension. */
export function isPlaceable(file: { name: string; type?: string }): boolean {
  if (file.type && /^image\//i.test(file.type)) return true
  return (PLACEABLE_EXTENSIONS as readonly string[]).includes(extensionOf(file.name))
}

/** Whether a drag carries files (not text or a link): only then is the canvas a drop target. */
export function dragHasFiles(types: readonly string[] | DOMStringList | null | undefined): boolean {
  if (!types) return false
  return Array.from(types as ArrayLike<string>).includes('Files')
}

/** A name for an image that came without one (a screenshot on the clipboard): `Pasted Image.png`. */
export function nameFor(file: { name?: string; type?: string }, fallback: string): string {
  if (file.name && file.name.trim() && file.name !== 'image.png') return file.name.trim()
  const extension = /^image\/(png|jpeg|webp|gif|tiff|bmp|svg\+xml|avif|heic)$/i.exec(file.type ?? '')?.[1]?.toLowerCase().replace('jpeg', 'jpg').replace('svg+xml', 'svg') ?? 'png'
  return `${fallback}.${extension}`
}

/** Standard base64 of `bytes`, in chunks (a large image overflows one `String.fromCharCode` call). */
export function toBase64(bytes: Uint8Array): string {
  let binary = ''
  const chunk = 0x8000
  for (let at = 0; at < bytes.length; at += chunk) binary += String.fromCharCode(...bytes.subarray(at, at + chunk))
  return btoa(binary)
}

/** The bytes of standard base64 `text`. */
export function fromBase64(text: string): Uint8Array {
  const binary = atob(text)
  const bytes = new Uint8Array(binary.length)
  for (let at = 0; at < binary.length; at += 1) bytes[at] = binary.charCodeAt(at)
  return bytes
}

/** A cheap fingerprint of bytes (length and an FNV-1a hash of a sample), to know our own clipboard copy again. */
export function fingerprint(bytes: Uint8Array): string {
  let hash = 0x811c9dc5
  const step = Math.max(1, Math.floor(bytes.length / 4096))
  for (let at = 0; at < bytes.length; at += step) {
    hash ^= bytes[at]
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return `${bytes.length}:${hash.toString(16)}`
}

/**
 * Where dropped files go, each a layer: the first at the drop point, the next ones 24 px further
 * down and right, as a stack the person can tell apart.
 */
export function dropCenters(point: { x: number; y: number }, count: number, step = 24): Array<[number, number]> {
  return Array.from({ length: count }, (_, index) => [Math.round(point.x + index * step), Math.round(point.y + index * step)])
}
