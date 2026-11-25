/**
 * Export As's settings (plan 2026-11-24, stage 6): the format, its quality, the size (a scale or
 * width × height, the two kept in proportion while the chain is closed), and transparency or the
 * matte under it, as the `file.exportImage` parameters the engine takes and the file name the host
 * writes. Pure, so `node --test` checks it.
 */

export const EXPORT_FORMATS = ['png', 'jpg', 'webp', 'tiff'] as const
export type ExportFormat = (typeof EXPORT_FORMATS)[number]

export const FORMAT_LABELS: Readonly<Record<ExportFormat, string>> = { png: 'PNG', jpg: 'JPEG', webp: 'WebP', tiff: 'TIFF' }
export const FORMAT_MIME: Readonly<Record<ExportFormat, string>> = { png: 'image/png', jpg: 'image/jpeg', webp: 'image/webp', tiff: 'image/tiff' }

/** The largest side the engine exports (`bindings/wasm/src/export.rs`). */
export const MAX_SIDE = 30_000

export type ExportSettings = {
  format: ExportFormat
  /** JPEG and lossy WebP, 1–100. */
  quality: number
  /** WebP only. */
  lossless: boolean
  /** Size as a percentage of the image, or as pixels. */
  sizing: 'scale' | 'pixels'
  scale: number
  width: number
  height: number
  /** Width and height change together. */
  linked: boolean
  /** PNG, WebP and TIFF keep transparent pixels; JPEG never does. */
  transparency: boolean
  /** The colour under transparent pixels when they are not kept, `#rrggbb`. */
  matte: string
}

export type ImageSize = { width: number; height: number }

/** The settings an Export As opens with for an image of `size`: PNG, full size, transparency kept. */
export function defaultSettings(size: ImageSize): ExportSettings {
  return { format: 'png', quality: 80, lossless: false, sizing: 'scale', scale: 100, width: size.width, height: size.height, linked: true, transparency: true, matte: '#ffffff' }
}

/** Whether `format` can keep transparent pixels. */
export const keepsTransparency = (format: ExportFormat) => format !== 'jpg'

/** Whether `format` has a quality to choose (WebP only when lossy). */
export const hasQuality = (settings: Pick<ExportSettings, 'format' | 'lossless'>) => settings.format === 'jpg' || (settings.format === 'webp' && !settings.lossless)

const clampSide = (value: number) => Math.min(MAX_SIDE, Math.max(1, Math.round(value)))

/** The output size in pixels. */
export function outputSize(settings: ExportSettings, size: ImageSize): ImageSize {
  if (settings.sizing === 'pixels') return { width: clampSide(settings.width), height: clampSide(settings.height) }
  const factor = settings.scale / 100
  return { width: clampSide(size.width * factor), height: clampSide(size.height * factor) }
}

/** The settings after typing a width (`side` 'width') or height: the other follows while linked. */
export function withSide(settings: ExportSettings, side: 'width' | 'height', value: number, size: ImageSize): ExportSettings {
  const next = { ...settings, sizing: 'pixels' as const, [side]: value }
  if (!settings.linked || !(value > 0)) return next
  return side === 'width' ? { ...next, height: clampSide((value * size.height) / size.width) } : { ...next, width: clampSide((value * size.width) / size.height) }
}

/** The settings after choosing a scale: the pixel fields follow it. */
export function withScale(settings: ExportSettings, scale: number, size: ImageSize): ExportSettings {
  const next = { ...settings, sizing: 'scale' as const, scale }
  const out = outputSize(next, size)
  return { ...next, width: out.width, height: out.height }
}

/** Whether the size can be exported (a positive size within the engine's limit). */
export function validSize(settings: ExportSettings, size: ImageSize): boolean {
  if (settings.sizing === 'scale') return settings.scale > 0 && settings.scale <= 1000 && outputSize(settings, size).width <= MAX_SIDE && outputSize(settings, size).height <= MAX_SIDE
  return settings.width >= 1 && settings.height >= 1 && settings.width <= MAX_SIDE && settings.height <= MAX_SIDE
}

/** The `file.exportImage` parameters for `settings` on an image of `size`. */
export function exportParams(settings: ExportSettings, size: ImageSize): Record<string, unknown> {
  const params: Record<string, unknown> = { format: settings.format }
  const out = outputSize(settings, size)
  if (out.width !== size.width || out.height !== size.height) Object.assign(params, out)
  if (hasQuality(settings)) params.quality = Math.min(100, Math.max(1, Math.round(settings.quality)))
  if (settings.format === 'webp' && settings.lossless) params.lossless = true
  if (keepsTransparency(settings.format) && !settings.transparency) params.transparency = false
  if (!keepsTransparency(settings.format) || !settings.transparency) params.matte = settings.matte
  return params
}

/** The exported file's name: the document's name with the format's extension (`photos/Sample.psd` → `Sample.jpg`). */
export function exportName(documentPath: string, format: ExportFormat, fallback = 'Untitled'): string {
  const base = documentPath.split(/[\\/]/).pop() ?? ''
  const stem = base.replace(/\.[^.]+$/, '').replace(/[\\/:*?"<>|]+/g, ' ').trim() || fallback
  return `${stem}.${format}`
}

/** Where the exported file goes: beside the document (`photos/Sample.psd` → `photos/Sample.jpg`). */
export function exportPath(documentPath: string, format: ExportFormat, fallback = 'Untitled'): string {
  const at = Math.max(documentPath.lastIndexOf('/'), documentPath.lastIndexOf('\\'))
  return `${at >= 0 ? documentPath.slice(0, at + 1) : ''}${exportName(documentPath, format, fallback)}`
}

/** A file size as people read it: `950 B`, `12.4 KB`, `3.1 MB` (1 KB = 1024 B, as file managers on macOS show it rounded). */
export function fileSize(bytes: number, locale = 'en-US'): string {
  const number = (value: number, digits: number) => value.toLocaleString(locale, { maximumFractionDigits: digits, minimumFractionDigits: digits })
  if (bytes < 1024) return `${Math.round(bytes)} B`
  if (bytes < 1024 * 1024) return `${number(bytes / 1024, bytes < 10 * 1024 ? 1 : 0)} KB`
  return `${number(bytes / (1024 * 1024), 1)} MB`
}
