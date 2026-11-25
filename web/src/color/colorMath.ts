/**
 * The Color Picker's arithmetic (plan 2026-11-24, stage 5): one colour shown as HSB, RGB, Lab and
 * hex, each field set from the others. Pure, so `node --test` checks it without a browser.
 *
 * RGB is 8-bit sRGB. HSB is Photoshop's: hue in degrees (0 to 360), saturation and brightness in
 * percent. Lab is CIE L*a*b* under D50 (Photoshop's Lab), from sRGB through linear light, XYZ (D65)
 * and the Bradford adaptation to D50; L from 0 to 100, a and b from -128 to 127.
 */

export type Rgb = readonly [number, number, number]
export type Hsb = { h: number; s: number; b: number }
export type Lab = { l: number; a: number; b: number }

const clamp = (value: number, low: number, high: number) => Math.min(high, Math.max(low, Number.isFinite(value) ? value : low))
const byte = (value: number) => Math.round(clamp(value, 0, 255))

/** `#rrggbb` (lower case) from 8-bit RGB. */
export function rgbToHex([r, g, b]: Rgb): string {
  return `#${[r, g, b].map((value) => byte(value).toString(16).padStart(2, '0')).join('')}`
}

/** 8-bit RGB from `#rgb`, `#rrggbb`, with or without `#`, any case; null for anything else. */
export function hexToRgb(text: string): Rgb | null {
  const hex = text.trim().replace(/^#/, '')
  if (/^[0-9a-f]{3}$/i.test(hex)) return [0, 1, 2].map((index) => parseInt(hex[index] + hex[index], 16)) as unknown as Rgb
  if (/^[0-9a-f]{6}$/i.test(hex)) return [0, 2, 4].map((index) => parseInt(hex.slice(index, index + 2), 16)) as unknown as Rgb
  return null
}

/** `#rrggbb` written the one way the editor stores colours, or null when `text` is not a colour. */
export function normalHex(text: string): string | null {
  const rgb = hexToRgb(text)
  return rgb ? rgbToHex(rgb) : null
}

/** HSB (hue in degrees, saturation and brightness in percent) from 8-bit RGB; a grey keeps `hue`. */
export function rgbToHsb([r, g, b]: Rgb, hue = 0): Hsb {
  const [rr, gg, bb] = [r, g, b].map((value) => clamp(value, 0, 255) / 255)
  const max = Math.max(rr, gg, bb)
  const min = Math.min(rr, gg, bb)
  const delta = max - min
  let h = hue
  if (delta > 1e-9) {
    if (max === rr) h = 60 * (((gg - bb) / delta) % 6)
    else if (max === gg) h = 60 * ((bb - rr) / delta + 2)
    else h = 60 * ((rr - gg) / delta + 4)
    if (h < 0) h += 360
  }
  return { h, s: max <= 1e-9 ? 0 : (delta / max) * 100, b: max * 100 }
}

/** 8-bit RGB (rounded) from HSB. */
export function hsbToRgb({ h, s, b }: Hsb): Rgb {
  const hue = (((Number.isFinite(h) ? h : 0) % 360) + 360) % 360
  const sat = clamp(s, 0, 100) / 100
  const val = clamp(b, 0, 100) / 100
  const chroma = val * sat
  const x = chroma * (1 - Math.abs(((hue / 60) % 2) - 1))
  const m = val - chroma
  const [r, g, bl] = hue < 60 ? [chroma, x, 0] : hue < 120 ? [x, chroma, 0] : hue < 180 ? [0, chroma, x] : hue < 240 ? [0, x, chroma] : hue < 300 ? [x, 0, chroma] : [chroma, 0, x]
  return [byte((r + m) * 255), byte((g + m) * 255), byte((bl + m) * 255)]
}

const toLinear = (value: number) => {
  const c = clamp(value, 0, 255) / 255
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
}
const fromLinear = (value: number) => (value <= 0.0031308 ? value * 12.92 : 1.055 * value ** (1 / 2.4) - 0.055) * 255

/** sRGB (D65) linear light to XYZ under D50, through the Bradford adaptation (Lindbloom's matrix). */
const RGB_TO_XYZ_D50 = [
  [0.4360747, 0.3850649, 0.1430804],
  [0.2225045, 0.7168786, 0.0606169],
  [0.0139322, 0.0971045, 0.7141733],
] as const
const XYZ_D50_TO_RGB = [
  [3.1338561, -1.6168667, -0.4906146],
  [-0.9787684, 1.9161415, 0.033454],
  [0.0719453, -0.2289914, 1.4052427],
] as const
/** D50's white point. */
const WHITE = [0.96422, 1, 0.82521] as const
const EPSILON = 216 / 24389
const KAPPA = 24389 / 27

/** CIE L*a*b* (D50) from 8-bit sRGB. */
export function rgbToLab(rgb: Rgb): Lab {
  const linear = rgb.map(toLinear)
  const [x, y, z] = RGB_TO_XYZ_D50.map((row) => row[0] * linear[0] + row[1] * linear[1] + row[2] * linear[2]).map((value, index) => value / WHITE[index])
  const f = (t: number) => (t > EPSILON ? Math.cbrt(t) : (KAPPA * t + 16) / 116)
  const [fx, fy, fz] = [f(x), f(y), f(z)]
  return { l: 116 * fy - 16, a: 500 * (fx - fy), b: 200 * (fy - fz) }
}

/** 8-bit sRGB (rounded, clipped to the gamut) from CIE L*a*b* (D50). */
export function labToRgb({ l, a, b }: Lab): Rgb {
  const fy = (clamp(l, 0, 100) + 16) / 116
  const fx = fy + clamp(a, -128, 127) / 500
  const fz = fy - clamp(b, -128, 127) / 200
  const inverse = (t: number) => (t ** 3 > EPSILON ? t ** 3 : (116 * t - 16) / KAPPA)
  const yy = l > KAPPA * EPSILON ? fy ** 3 : clamp(l, 0, 100) / KAPPA
  const xyz = [inverse(fx) * WHITE[0], yy * WHITE[1], inverse(fz) * WHITE[2]]
  return XYZ_D50_TO_RGB.map((row) => byte(fromLinear(clamp(row[0] * xyz[0] + row[1] * xyz[1] + row[2] * xyz[2], 0, 1)))) as unknown as Rgb
}

/** Every view of one colour, as the dialog's fields show it (whole numbers; hue kept for greys). */
export type ColorFields = { hex: string; rgb: Rgb; hsb: Hsb; lab: Lab }

/** The fields of a colour given as RGB, keeping `hue` where RGB has none (a grey, black). */
export function fieldsOf(rgb: Rgb, hue = 0): ColorFields {
  const clean: Rgb = [byte(rgb[0]), byte(rgb[1]), byte(rgb[2])]
  return { hex: rgbToHex(clean), rgb: clean, hsb: rgbToHsb(clean, hue), lab: rgbToLab(clean) }
}

/** The fields of a colour given as HSB: the HSB itself stays exactly as given (the square and strip do not jump). */
export function fieldsOfHsb(hsb: Hsb): ColorFields {
  const h = clamp(hsb.h, 0, 360)
  const exact = { h: h === 360 ? 0 : h, s: clamp(hsb.s, 0, 100), b: clamp(hsb.b, 0, 100) }
  const rgb = hsbToRgb(exact)
  return { hex: rgbToHex(rgb), rgb, hsb: exact, lab: rgbToLab(rgb) }
}

/** Which field a person typed in, and its new number. */
export type ColorField = 'h' | 's' | 'b' | 'r' | 'g' | 'bl' | 'l' | 'la' | 'lb'

/** The colour after a person typed `value` into `field`, from `current`. */
export function withField(current: ColorFields, field: ColorField, value: number): ColorFields {
  if (!Number.isFinite(value)) return current
  switch (field) {
    case 'h':
      return fieldsOfHsb({ ...current.hsb, h: value })
    case 's':
      return fieldsOfHsb({ ...current.hsb, s: value })
    case 'b':
      return fieldsOfHsb({ ...current.hsb, b: value })
    case 'r':
      return fieldsOf([value, current.rgb[1], current.rgb[2]], current.hsb.h)
    case 'g':
      return fieldsOf([current.rgb[0], value, current.rgb[2]], current.hsb.h)
    case 'bl':
      return fieldsOf([current.rgb[0], current.rgb[1], value], current.hsb.h)
    case 'l':
      return fieldsOf(labToRgb({ ...current.lab, l: value }), current.hsb.h)
    case 'la':
      return fieldsOf(labToRgb({ ...current.lab, a: value }), current.hsb.h)
    case 'lb':
      return fieldsOf(labToRgb({ ...current.lab, b: value }), current.hsb.h)
  }
}

/** Where a colour sits in the saturation and brightness square (0 to 1 across, 0 to 1 down). */
export function squarePoint(hsb: Hsb) {
  return { x: clamp(hsb.s, 0, 100) / 100, y: 1 - clamp(hsb.b, 0, 100) / 100 }
}

/** The saturation and brightness at a point of the square (fractions across and down, clamped). */
export function squareValue(x: number, y: number) {
  return { s: clamp(x, 0, 1) * 100, b: (1 - clamp(y, 0, 1)) * 100 }
}

/** The hue at a point of the strip (a fraction down: red at the top, then magenta, … back to red). */
export function stripHue(y: number) {
  return (1 - clamp(y, 0, 1)) * 360
}
