/**
 * The arithmetic behind the Info, Histogram, and Navigator panels (upstream's `info_panel`,
 * `tone::histogram_panel`, and `navigator`), kept pure so it is tested without a browser.
 */

/** Upstream's naive device CMYK (no ICC profile), in whole percent like Photoshop's readout; `rgb` 0 to 1. */
export function cmykOf(rgb: readonly number[]): [number, number, number, number] {
  const [r, g, b] = rgb.map((value) => Math.min(1, Math.max(0, value)))
  const k = 1 - Math.max(r, g, b)
  const ink = (value: number) => (k >= 1 ? 0 : (1 - value - k) / (1 - k))
  return [ink(r), ink(g), ink(b), k].map((value) => Math.round(value * 100)) as [number, number, number, number]
}

/** A channel (0 to 1) as the readout's byte (upstream's `q`). */
export const byteOf = (value: number) => Math.round(Math.min(1, Math.max(0, value)) * 255)

/** Preferences › Units & Rulers, as the Info panel reads it (`prefs.get` `unitsAndRulers`). */
export type RulerUnits = { rulers?: string; pointSize?: string }

/**
 * A length in document pixels in the ruler unit, as upstream's `UnitsAndRulers::format` writes it:
 * the number only, with the unit's decimals (pixels none; points, percent, and millimetres one;
 * the rest two). `dpi` is the document's resolution, `extent` the 100 % length.
 */
export function formatLength(px: number, units: RulerUnits | null, dpi: number, extent: number): string {
  const unit = units?.rulers ?? 'pixels'
  const perInch = units?.pointSize === 'traditional' ? 72.27 : 72
  const inches = px / Math.max(dpi, 1e-6)
  const [value, decimals] =
    unit === 'inches'
      ? [inches, 2]
      : unit === 'cm'
        ? [inches * 2.54, 2]
        : unit === 'mm'
          ? [inches * 25.4, 1]
          : unit === 'points'
            ? [inches * perInch, 1]
            : unit === 'picas'
              ? [(inches * perInch) / 12, 2]
              : unit === 'percent'
                ? [(px / Math.max(extent, 1e-6)) * 100, 1]
                : [px, 0]
  return value.toFixed(decimals)
}

/** The Histogram panel's statistics of one channel's bins (upstream's `histogram_panel`): mean, standard deviation, median, and the pixels counted; null with none. */
export function histogramStats(bins: ArrayLike<number>): { mean: number; stdDev: number; median: number; count: number } | null {
  let count = 0
  let sum = 0
  for (let i = 0; i < bins.length; i += 1) {
    count += bins[i]
    sum += i * bins[i]
  }
  if (count === 0) return null
  const mean = sum / count
  let variance = 0
  for (let i = 0; i < bins.length; i += 1) variance += (i - mean) ** 2 * bins[i]
  let seen = 0
  let median = 0
  for (let i = 0; i < bins.length; i += 1) {
    seen += bins[i]
    if (seen * 2 >= count) {
      median = i
      break
    }
  }
  return { mean, stdDev: Math.sqrt(variance / count), median, count }
}

/** The longest side of the reduced render the engine's `histogram` extra samples (`HISTOGRAM_SIDE`). */
export const HISTOGRAM_SIDE = 1024

/**
 * The Cache Level the Histogram panel's numbers come from, as Photoshop counts them (level n is
 * the image reduced 2^(n-1) times): the `histogram` extra samples the image reduced by the
 * smallest whole factor that brings it within 1024 pixels, so its level is that factor's
 * halvings, rounded up, plus one. (Upstream counts toward a 384-pixel thumbnail it does not use:
 * its histogram is exact.)
 */
export function cacheLevel(width: number, height: number) {
  const factor = Math.max(1, Math.ceil(Math.max(width, height) / HISTOGRAM_SIDE))
  return Math.ceil(Math.log2(factor)) + 1
}

/** Upstream's zoom slider runs over log2 of the zoom, from 1 % (-6.64) to 3200 % (5). */
export const ZOOM_SLIDER = { min: -6.64, max: 5 } as const

/** The slider's position for a zoom. */
export const sliderOfZoom = (zoom: number) => Math.min(ZOOM_SLIDER.max, Math.max(ZOOM_SLIDER.min, Math.log2(Math.max(zoom, 0.01))))

/** The zoom for a slider position. */
export const zoomOfSlider = (value: number) => 2 ** value

export type Box = { x: number; y: number; width: number; height: number }

/** Where the Navigator draws the image in its frame: fitted, centred, `padding` from the frame's edges (upstream's 8 on each side). */
export function navigatorImage(frame: { width: number; height: number }, image: { width: number; height: number }, padding = 8): Box {
  const room = { width: Math.max(1, frame.width - padding * 2), height: Math.max(1, frame.height - padding * 2) }
  const aspect = image.width / Math.max(1, image.height)
  const [width, height] = aspect > room.width / room.height ? [room.width, room.width / aspect] : [room.height * aspect, room.height]
  return { x: (frame.width - width) / 2, y: (frame.height - height) / 2, width, height }
}

/**
 * The canvas's visible area on the Navigator's image (upstream's red rectangle), clipped to the
 * frame: `view` is the canvas's zoom and the document point at its top-left, `canvas` its size in
 * CSS pixels, `image` where the image is drawn and `scale` its frame pixels per document pixel.
 */
export function navigatorViewBox(view: { zoom: number; x: number; y: number }, canvas: { width: number; height: number }, image: Box, scale: number, frame: { width: number; height: number }): Box {
  const x0 = Math.max(1, image.x + view.x * scale)
  const y0 = Math.max(1, image.y + view.y * scale)
  const x1 = Math.min(frame.width - 1, image.x + (view.x + canvas.width / view.zoom) * scale)
  const y1 = Math.min(frame.height - 1, image.y + (view.y + canvas.height / view.zoom) * scale)
  return { x: x0, y: y0, width: Math.max(0, x1 - x0), height: Math.max(0, y1 - y0) }
}

/** The view's top-left that centres the canvas on document point (`x`, `y`), clamped to the document (upstream sets the view's centre where the Navigator is pressed). */
export function centredOn(point: { x: number; y: number }, view: { zoom: number }, canvas: { width: number; height: number }, size: { width: number; height: number }) {
  const cx = Math.min(size.width, Math.max(0, point.x))
  const cy = Math.min(size.height, Math.max(0, point.y))
  return { x: cx - canvas.width / 2 / view.zoom, y: cy - canvas.height / 2 / view.zoom }
}

/** How long a failed ask (the Navigator's image, the histogram) waits before it is asked again: 250 ms, then 1 s, then 4 s. */
export const RETRY_DELAYS_MS = [250, 1000, 4000] as const

/** The wait before asking again after `failures` failures of the same ask, or null once it has failed every retry (it waits for the next change). */
export const retryDelay = (failures: number): number | null => (failures <= 0 ? 0 : (RETRY_DELAYS_MS[failures - 1] ?? null))
