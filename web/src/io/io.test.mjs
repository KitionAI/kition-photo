// Plan 2026-11-24, stage 6: Export As's settings, the engine's errors in the person's words, the
// long-command indicator, and the files the editor places. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { defaultSettings, exportName, exportParams, exportPath, fileSize, hasQuality, outputSize, validSize, withScale, withSide } from './exportOptions.ts'
import { ERROR_KINDS, explainError } from './errors.ts'
import { Busy } from './busy.ts'
import { dragHasFiles, dropCenters, fingerprint, fromBase64, isPlaceable, nameFor, toBase64 } from './files.ts'
import { CATALOGS } from '../strings.ts'

const SIZE = { width: 1200, height: 800 }

test('Export As opens as a full-size PNG that keeps transparency, and asks the engine for nothing else', () => {
  const settings = defaultSettings(SIZE)
  assert.deepEqual(exportParams(settings, SIZE), { format: 'png' })
  assert.deepEqual(outputSize(settings, SIZE), SIZE)
})

test('a scale or one side sizes the export, the other side following while linked', () => {
  const half = withScale(defaultSettings(SIZE), 50, SIZE)
  assert.deepEqual([half.width, half.height], [600, 400])
  assert.deepEqual(exportParams(half, SIZE), { format: 'png', width: 600, height: 400 })
  const wide = withSide(defaultSettings(SIZE), 'width', 300, SIZE)
  assert.deepEqual([wide.sizing, wide.width, wide.height], ['pixels', 300, 200])
  const tall = withSide(defaultSettings(SIZE), 'height', 100, SIZE)
  assert.deepEqual([tall.width, tall.height], [150, 100])
  const free = withSide({ ...defaultSettings(SIZE), linked: false }, 'width', 300, SIZE)
  assert.deepEqual([free.width, free.height], [300, 800])
  assert.equal(validSize({ ...defaultSettings(SIZE), sizing: 'pixels', width: 0 }, SIZE), false)
  assert.equal(validSize(withScale(defaultSettings(SIZE), 3000, SIZE), SIZE), false)
  assert.equal(validSize(withScale(defaultSettings(SIZE), 200, SIZE), SIZE), true)
})

test('JPEG takes a quality and the matte; WebP a quality unless lossless; transparency off sends the matte', () => {
  const jpeg = { ...defaultSettings(SIZE), format: 'jpg', quality: 62.4, matte: '#102030' }
  assert.deepEqual(exportParams(jpeg, SIZE), { format: 'jpg', quality: 62, matte: '#102030' })
  assert.equal(hasQuality({ format: 'webp', lossless: false }), true)
  assert.equal(hasQuality({ format: 'webp', lossless: true }), false)
  assert.equal(hasQuality({ format: 'png', lossless: false }), false)
  assert.deepEqual(exportParams({ ...defaultSettings(SIZE), format: 'webp', lossless: true }, SIZE), { format: 'webp', lossless: true })
  assert.deepEqual(exportParams({ ...defaultSettings(SIZE), transparency: false, matte: '#000000' }, SIZE), { format: 'png', transparency: false, matte: '#000000' })
})

test('the exported file is named after the document, beside it', () => {
  assert.equal(exportName('photos/Sample.psd', 'jpg'), 'Sample.jpg')
  assert.equal(exportPath('photos/Sample.psd', 'webp'), 'photos/Sample.webp')
  assert.equal(exportPath('Sample.psd', 'png'), 'Sample.png')
  assert.equal(exportName('', 'png', 'Untitled'), 'Untitled.png')
  assert.equal(exportName('a:b?.psd', 'tiff'), 'a b.tiff')
})

test('file sizes read as B, KB and MB', () => {
  assert.equal(fileSize(950), '950 B')
  assert.equal(fileSize(5 * 1024 + 300), '5.3 KB')
  assert.equal(fileSize(200 * 1024), '200 KB')
  assert.equal(fileSize(3.25 * 1024 * 1024), '3.3 MB')
})

test("the engine's errors become the editor's sentences, without command ids, the engine's text kept as the detail", () => {
  const cases = [
    ['command `edit.paste` is not available right now: the clipboard is empty', 'error.clipboardEmpty'],
    ['no document open', 'error.noDocument'],
    ['Could not complete your request because the layer "Sky" is locked.', 'error.locked'],
    ['invalid parameters for `file.placeEmbedded`: `data` is not valid base64', 'error.unreadableImage'],
    ['decode: not a supported image format', 'error.unreadableImage'],
    ['6000 x 6000 px is 36 megapixels; resampling at this bit depth is limited to 25 megapixels', 'error.tooLarge'],
    ['no active layer', 'error.noLayer'],
    ['the selection is empty', 'error.noSelection'],
    // Plan 2026-11-24, stage 8: Layer via Copy / Cut on empty pixels, Image › Crop without a selection.
    ['step 0 (`layer.new.layerViaCopy`) failed: Could not complete the command: the selected area is empty', 'error.emptyArea'],
    ['nothing to crop: pass x/y/width/height or make a selection', 'error.noSelection'],
    ['the active layer has no pixels', 'error.notPixels'],
    ['not available on the web (no file system)', 'error.noFileSystem'],
    ['cancelled', 'error.cancelled'],
    ['invalid parameters for `filter.blur.gaussianBlur`: `radius` must be a number', 'error.badValue'],
  ]
  for (const [text, key] of cases) {
    const explained = explainError(new Error(text))
    assert.deepEqual(explained, { key, detail: text, known: true }, text)
  }
  const unknown = explainError(new Error('step 2 (`layer.zap`) failed: the zapper overheated'))
  assert.deepEqual(unknown, { key: 'error.generic', detail: 'step 2 (`layer.zap`) failed: the zapper overheated', known: false })
  assert.deepEqual(explainError(''), { key: 'error.generic', detail: null, known: false })
  for (const locale of ['en-US', 'zh-CN']) {
    for (const kind of ERROR_KINDS) assert.ok(CATALOGS[locale][`error.${kind}`], `${locale} error.${kind}`)
    for (const kind of ERROR_KINDS) assert.doesNotMatch(CATALOGS[locale][`error.${kind}`], /`|[a-z]+\.[a-z]+[A-Z]/, `${locale} error.${kind} names no command`)
  }
})

test('a long command shows the indicator after half a second; a quick one never does; Cancel drops its result', () => {
  let time = 0
  const timers = new Map()
  let serial = 0
  const clock = {
    now: () => time,
    schedule: (task, ms) => {
      serial += 1
      timers.set(serial, { at: time + ms, task })
      return serial
    },
    unschedule: (handle) => timers.delete(handle),
  }
  const advance = (ms) => {
    time += ms
    for (const [handle, timer] of [...timers]) {
      if (timer.at <= time) {
        timers.delete(handle)
        timer.task()
      }
    }
  }
  const busy = new Busy(clock, 500)
  let changes = 0
  busy.subscribe(() => (changes += 1))
  const quick = busy.begin('Gaussian Blur')
  advance(200)
  assert.equal(busy.end(quick), false)
  assert.equal(busy.current, null)
  assert.equal(changes, 0, 'a quick command never flashes the indicator')
  const slow = busy.begin('Surface Blur')
  advance(499)
  assert.equal(busy.current, null)
  advance(1)
  assert.equal(busy.current?.label, 'Surface Blur')
  const second = busy.begin('Export', false)
  advance(600)
  assert.equal(busy.current, slow, 'the oldest job shows')
  assert.equal(busy.cancel(), slow)
  assert.equal(busy.current, second, 'the next one shows once the first is cancelled')
  assert.equal(busy.cancel(), null, 'an export being written cannot be cancelled')
  assert.equal(busy.end(slow), true, 'the cancelled job is dropped when it ends')
  assert.equal(busy.end(second), false)
  assert.equal(busy.current, null)
})

test('the editor places image files and knows a drag of files', () => {
  assert.equal(isPlaceable({ name: 'photo.JPG' }), true)
  assert.equal(isPlaceable({ name: 'scan', type: 'image/tiff' }), true)
  assert.equal(isPlaceable({ name: 'notes.txt', type: 'text/plain' }), false)
  assert.equal(dragHasFiles(['text/plain', 'Files']), true)
  assert.equal(dragHasFiles(['text/uri-list']), false)
  assert.equal(nameFor({ name: 'image.png', type: 'image/png' }, 'Pasted Image'), 'Pasted Image.png')
  assert.equal(nameFor({ name: '', type: 'image/jpeg' }, 'Pasted Image'), 'Pasted Image.jpg')
  assert.equal(nameFor({ name: 'Logo.svg', type: 'image/svg+xml' }, 'Pasted Image'), 'Logo.svg')
  assert.deepEqual(dropCenters({ x: 10.4, y: 20.6 }, 3), [[10, 21], [34, 45], [58, 69]])
  const bytes = new Uint8Array(100_000).map((_, index) => (index * 31) % 256)
  assert.deepEqual(fromBase64(toBase64(bytes)), bytes)
  assert.equal(fingerprint(bytes), fingerprint(bytes.slice()))
  assert.notEqual(fingerprint(bytes), fingerprint(bytes.subarray(1)))
})
