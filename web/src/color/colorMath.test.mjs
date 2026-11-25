// The Color Picker's conversions (plan 2026-11-24, stage 5): node --test (part of `pnpm test:parity`).
import test from 'node:test'
import assert from 'node:assert/strict'
import { fieldsOf, fieldsOfHsb, hexToRgb, hsbToRgb, labToRgb, normalHex, rgbToHex, rgbToHsb, rgbToLab, squarePoint, squareValue, stripHue, withField } from './colorMath.ts'

const near = (a, b, tolerance, label) => assert.ok(Math.abs(a - b) <= tolerance, `${label}: ${a} is not within ${tolerance} of ${b}`)

test('hex round-trips through RGB and is written one way', () => {
  assert.deepEqual(hexToRgb('#3366cc'), [0x33, 0x66, 0xcc])
  assert.deepEqual(hexToRgb('3366CC'), [0x33, 0x66, 0xcc])
  assert.deepEqual(hexToRgb('#f0a'), [0xff, 0x00, 0xaa])
  for (const bad of ['', '#12', '#12345', 'zzzzzz', '#1234567', 'red']) assert.equal(hexToRgb(bad), null, bad)
  assert.equal(rgbToHex([51, 102, 204]), '#3366cc')
  assert.equal(rgbToHex([300, -5, 12.6]), '#ff000d')
  assert.equal(normalHex(' #ABCDEF '), '#abcdef')
  assert.equal(normalHex('nope'), null)
  for (let value = 0; value < 0x1000000; value += 0x0f3a57) {
    const hex = `#${value.toString(16).padStart(6, '0')}`
    assert.equal(rgbToHex(hexToRgb(hex)), hex)
  }
})

test("HSB is Photoshop's: hue in degrees, saturation and brightness in percent", () => {
  assert.deepEqual(rgbToHsb([255, 0, 0]), { h: 0, s: 100, b: 100 })
  assert.deepEqual(rgbToHsb([0, 255, 0]), { h: 120, s: 100, b: 100 })
  assert.deepEqual(rgbToHsb([0, 0, 255]), { h: 240, s: 100, b: 100 })
  const teal = rgbToHsb([51, 102, 204])
  near(teal.h, 220, 0.01, 'hue')
  near(teal.s, 75, 0.01, 'saturation')
  near(teal.b, 80, 0.01, 'brightness')
  // A grey has no hue of its own: it keeps the one it is given.
  assert.deepEqual(rgbToHsb([128, 128, 128], 200), { h: 200, s: 0, b: (128 / 255) * 100 })
  assert.deepEqual(rgbToHsb([0, 0, 0], 33), { h: 33, s: 0, b: 0 })
  assert.deepEqual(hsbToRgb({ h: 220, s: 75, b: 80 }), [51, 102, 204])
  assert.deepEqual(hsbToRgb({ h: 360, s: 100, b: 100 }), [255, 0, 0])
  assert.deepEqual(hsbToRgb({ h: -60, s: 100, b: 100 }), [255, 0, 255])
  // Every 8-bit colour comes back from its HSB.
  for (let r = 0; r <= 255; r += 17) for (let g = 0; g <= 255; g += 51) for (let b = 0; b <= 255; b += 85) assert.deepEqual(hsbToRgb(rgbToHsb([r, g, b])), [r, g, b], `${r},${g},${b}`)
})

test('Lab is D50 (Photoshop’s): white, black, the primaries, and round trips', () => {
  const white = rgbToLab([255, 255, 255])
  near(white.l, 100, 0.01, 'white L')
  near(white.a, 0, 0.01, 'white a')
  near(white.b, 0, 0.01, 'white b')
  assert.deepEqual(rgbToLab([0, 0, 0]), { l: 0, a: 0, b: 0 })
  // sRGB red under D50 (Bradford): L 54.29, a 80.80, b 69.89.
  const red = rgbToLab([255, 0, 0])
  near(red.l, 54.29, 0.05, 'red L')
  near(red.a, 80.8, 0.1, 'red a')
  near(red.b, 69.89, 0.1, 'red b')
  const mid = rgbToLab([119, 119, 119])
  near(mid.l, 50, 0.5, 'mid grey L')
  for (let r = 0; r <= 255; r += 51) for (let g = 0; g <= 255; g += 51) for (let b = 0; b <= 255; b += 51) assert.deepEqual(labToRgb(rgbToLab([r, g, b])), [r, g, b], `${r},${g},${b}`)
  // Out of the sRGB gamut, clipped rather than wrapped.
  const clipped = labToRgb({ l: 50, a: 127, b: -128 })
  assert.ok(clipped.every((value) => value >= 0 && value <= 255))
})

test('a typed field moves the others; HSB stays as typed so the square does not jump', () => {
  const start = fieldsOf([51, 102, 204])
  assert.equal(start.hex, '#3366cc')
  const hue = withField(start, 'h', 0)
  assert.equal(hue.hsb.h, 0)
  assert.equal(hue.hex, '#cc3333')
  const darker = withField(hue, 'b', 0)
  assert.equal(darker.hex, '#000000')
  assert.equal(darker.hsb.h, 0, 'black keeps the hue')
  near(darker.hsb.s, 75, 1e-9, 'and the saturation it was given')
  const red = withField(start, 'r', 255)
  assert.deepEqual(red.rgb, [255, 102, 204])
  const lab = withField(start, 'l', 100)
  assert.ok(lab.lab.l > 95)
  assert.deepEqual(withField(start, 'g', Number.NaN), start)
  assert.deepEqual(fieldsOfHsb({ h: 400, s: 150, b: -3 }).hsb, { h: 0, s: 100, b: 0 })
})

test('the square and the strip map points to colours and back', () => {
  assert.deepEqual(squarePoint({ h: 10, s: 25, b: 75 }), { x: 0.25, y: 0.25 })
  assert.deepEqual(squareValue(0.25, 0.25), { s: 25, b: 75 })
  assert.deepEqual(squareValue(-1, 2), { s: 0, b: 0 })
  assert.equal(stripHue(0), 360)
  assert.equal(stripHue(1), 0)
  assert.equal(stripHue(0.5), 180)
})
