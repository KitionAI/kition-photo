// The Info, Histogram, Navigator, Character, and Paragraph panels' arithmetic: upstream's CMYK,
// ruler units, histogram statistics, zoom slider, navigator geometry, and the type style's way from
// `text.info` to the panels and back to `text.setStyle`. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { cacheLevel, centredOn, retryDelay, cmykOf, formatLength, histogramStats, navigatorImage, navigatorViewBox, sliderOfZoom, zoomOfSlider } from './readouts.ts'
import { TOOL_FIELDS, alignOf, kerningLabel, parseKerning, shownSize, typeEdit, typeLayerOf, typeValuesOf } from '../options/typeStyle.ts'

const hex = (rgb) => `#${rgb.map((value) => Math.round(value).toString(16).padStart(2, '0')).join('')}`
import { DEFAULT_OPTIONS } from '../tools/options.ts'

test("CMYK is upstream's naive device conversion, in whole percent", () => {
  assert.deepEqual(cmykOf([1, 0, 0]), [0, 100, 100, 0])
  assert.deepEqual(cmykOf([0, 0, 0]), [0, 0, 0, 100])
  assert.deepEqual(cmykOf([1, 1, 1]), [0, 0, 0, 0])
  assert.deepEqual(cmykOf([0.5, 0.25, 0.25]), [0, 50, 50, 50])
})

test('lengths read in the ruler unit with its decimals, as upstream formats them', () => {
  assert.equal(formatLength(150, null, 300, 1000), '150')
  assert.equal(formatLength(150, { rulers: 'inches' }, 300, 1000), '0.50')
  assert.equal(formatLength(300, { rulers: 'cm' }, 300, 1000), '2.54')
  assert.equal(formatLength(300, { rulers: 'mm' }, 300, 1000), '25.4')
  assert.equal(formatLength(300, { rulers: 'points' }, 300, 1000), '72.0')
  assert.equal(formatLength(300, { rulers: 'points', pointSize: 'traditional' }, 300, 1000), '72.3')
  assert.equal(formatLength(300, { rulers: 'picas' }, 300, 1000), '6.00')
  assert.equal(formatLength(250, { rulers: 'percent' }, 300, 1000), '25.0')
})

test("the histogram's statistics: mean, standard deviation, median, and count", () => {
  const bins = new Array(256).fill(0)
  bins[10] = 1
  bins[20] = 3
  const stats = histogramStats(bins)
  assert.equal(stats.count, 4)
  assert.equal(stats.mean, 17.5)
  assert.ok(Math.abs(stats.stdDev - Math.sqrt(18.75)) < 1e-9)
  assert.equal(stats.median, 20)
  assert.equal(histogramStats(new Array(256).fill(0)), null)
})

test("the Cache Level is the histogram extra's own sampling: a whole factor within 1024 pixels", () => {
  assert.equal(cacheLevel(300, 200), 1)
  assert.equal(cacheLevel(1024, 100), 1)
  assert.equal(cacheLevel(1600, 1200), 2, 'halved')
  assert.equal(cacheLevel(3000, 2000), 3, 'a third: two halvings, rounded up')
  assert.equal(cacheLevel(4096, 100), 3)
})

test('the zoom slider runs over log2 of the zoom, from 1 % to 3200 %', () => {
  assert.equal(sliderOfZoom(1), 0)
  assert.equal(sliderOfZoom(4), 2)
  assert.equal(sliderOfZoom(100), 5)
  assert.equal(sliderOfZoom(0.001), -6.64)
  assert.equal(zoomOfSlider(1), 2)
})

test('the Navigator fits the image in its frame and draws the canvas view on it', () => {
  const frame = { width: 216, height: 150 }
  const image = navigatorImage(frame, { width: 400, height: 200 })
  assert.deepEqual(image, { x: 8, y: 75 - 50, width: 200, height: 100 })
  const scale = image.width / 400
  // The canvas, 400 by 200 CSS pixels at 200 %, sees document (100, 50)–(300, 150).
  const box = navigatorViewBox({ zoom: 2, x: 100, y: 50 }, { width: 400, height: 200 }, image, scale, frame)
  assert.deepEqual(box, { x: 58, y: 50, width: 100, height: 50 })
  // Zoomed out past the image, the box stays inside the frame.
  const wide = navigatorViewBox({ zoom: 0.1, x: -1000, y: -1000 }, { width: 400, height: 200 }, image, scale, frame)
  assert.ok(wide.x >= 1 && wide.y >= 1 && wide.x + wide.width <= frame.width - 1 && wide.y + wide.height <= frame.height - 1)
  // A press centres the canvas on the document point under it.
  assert.deepEqual(centredOn({ x: 200, y: 100 }, { zoom: 2 }, { width: 400, height: 200 }, { width: 400, height: 200 }), { x: 100, y: 50 })
  assert.deepEqual(centredOn({ x: 900, y: -5 }, { zoom: 1 }, { width: 100, height: 100 }, { width: 400, height: 200 }), { x: 350, y: -50 })
})

test("kerning reads and parses as upstream's combo does", () => {
  assert.equal(kerningLabel('Metrics', 0), 'Metrics')
  assert.equal(kerningLabel('Optical', 0), 'Optical')
  assert.equal(kerningLabel('Off', 0), '0')
  assert.equal(kerningLabel('Off', -24.6), '-25')
  assert.equal(parseKerning(' optical '), 'Optical')
  assert.equal(parseKerning('METRICS'), 'Metrics')
  assert.equal(parseKerning('-75'), -75)
  assert.equal(parseKerning('12.6'), 13)
  assert.equal(parseKerning('20000'), null)
  assert.equal(parseKerning('wide'), null)
})

const info = {
  antialias: 'Crisp',
  orientation: 'Horizontal',
  transform: [2, 0, 0, 2, 10, 20],
  runs: [
    {
      style: {
        font_family: 'Inter',
        font_style: 'Bold',
        size_pt: 12,
        color: { c: [1, 0, 0, 1] },
        tracking: 50,
        leading_pt: null,
        baseline_shift_pt: 3,
        horizontal_scale: 1.5,
        vertical_scale: 1,
        underline: true,
        strikethrough: false,
        faux_bold: true,
        faux_italic: false,
        kerning: 'Optical',
        kern: 0,
        caps: 'SmallCaps',
      },
    },
  ],
  paragraphs: [{ style: { align: 'JustifyCenter', first_line_indent_pt: 6, start_indent_pt: 2, end_indent_pt: 4, space_before_pt: 8, space_after_pt: 10, auto_leading: 1.2, hyphenate: true } }],
}

test('a type layer shows its first run and paragraph, sizes as its transform scales them', () => {
  const layer = typeLayerOf(7, info, hex)
  assert.equal(layer.layer, 7)
  assert.equal(layer.scale, 2)
  assert.equal(shownSize(layer), 24)
  assert.equal(layer.antialias, 'crisp')
  assert.equal(layer.leading, 28.8, 'Auto leading: 12 pt × 1.2 × 2')
  assert.equal(layer.autoLeading, true)
  assert.equal(layer.kerning, 'Optical')
  assert.equal(layer.horizontalScale, 150)
  assert.equal(layer.color, '#ff0000')
  assert.equal(layer.caps, 'small')
  assert.equal(layer.align, 'justifyCenter')
  assert.deepEqual([layer.startIndent, layer.endIndent, layer.firstLineIndent, layer.spaceBefore, layer.spaceAfter, layer.hyphenate], [2, 4, 6, 8, 10, true])
  assert.equal(typeLayerOf(7, { ...info, runs: [] }, hex), null)
  assert.equal(alignOf('Right'), 'right')
  assert.equal(alignOf(undefined), 'left')
})

test("without a type layer the panels show the Type tool's own options and the foreground, the rest at the engine's stock defaults", () => {
  const values = typeValuesOf({ ...DEFAULT_OPTIONS, fontSize: 20, textAlign: 'center' }, '#123456')
  assert.deepEqual([values.font, values.size, values.align, values.color], ['Inter', 20, 'center', '#123456'])
  assert.equal(values.leading, 24, 'Auto: 20 pt × 1.2')
  assert.deepEqual([values.tracking, values.kerning, values.horizontalScale], [0, 'Metrics', 100])
  assert.deepEqual([...TOOL_FIELDS], ['font', 'fontStyle', 'size', 'align', 'color'])
})

test('an edit becomes text.setStyle keys (sizes divided by the scale), and sets only the tool options the Type tool keeps', () => {
  const { style, options } = typeEdit({ size: 30, leading: 40, tracking: 25, kerning: '-50', align: 'justifyAll', fauxItalic: true, font: 'Inter' }, 2)
  assert.deepEqual(style, { font: 'Inter', tracking: 25, fauxItalic: true, size: 15, leading: 20, kerning: -50, align: 'justifyAll' })
  assert.deepEqual(options, { font: 'Inter', fontSize: 30 }, 'a justified alignment is no tool option')
  assert.deepEqual(typeEdit({ align: 'center' }).options, { textAlign: 'center' })
  assert.deepEqual(typeEdit({ size: 0.5 }).options, { fontSize: 1 }, 'the tool keeps whole points from 1')
  assert.deepEqual(typeEdit({ kerning: 'optical' }).style, { kerning: 'optical' })
  assert.deepEqual(typeEdit({ kerning: 'nonsense' }), { style: {}, options: {} })
})

test('a failed ask backs off (250 ms, 1 s, 4 s), then waits for the next change', () => {
  assert.deepEqual([0, 1, 2, 3, 4, 9].map(retryDelay), [0, 250, 1000, 4000, null, null])
})
