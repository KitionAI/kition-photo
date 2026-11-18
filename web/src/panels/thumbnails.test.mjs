// The panels' thumbnail cache: per source and revision, at most one refresh per frame, never two
// at once, and an empty thumbnail (not an error) when the engine cannot draw one.
import test from 'node:test'
import assert from 'node:assert/strict'
import { ThumbnailCache, decodeThumbnail, splitThumbnails, thumbnailKey } from './thumbnails.ts'

function encoded(width, height, fill) {
  const bytes = new Uint8Array(8 + width * height * 4).fill(fill)
  const view = new DataView(bytes.buffer)
  view.setUint32(0, width, true)
  view.setUint32(4, height, true)
  return bytes
}

/** A cache whose frames run when the test says, counting queries and change notices. */
function harness({ fail = () => false, delay = null } = {}) {
  const frames = []
  const queries = []
  const closed = []
  let changes = 0
  const query = async (source, max) => {
    queries.push(`${thumbnailKey(source)}@${max}`)
    if (delay) await delay
    if (fail(source)) throw new Error('no layer')
    return encoded(2, 1, queries.length)
  }
  const decode = async (bytes) => {
    const image = decodeThumbnail(bytes)
    return image ? { ...image, close: () => closed.push(image.rgba[0]) } : null
  }
  const cache = new ThumbnailCache(query, decode, 48, (run) => frames.push(run), () => (changes += 1))
  const flush = async () => {
    while (frames.length) {
      frames.shift()()
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
  }
  return { cache, frames, queries, closed, flush, changes: () => changes }
}

const layer = (id) => ({ kind: 'layer', layer: id })

test('decodes the size header and the pixels, and refuses anything shorter', () => {
  const image = decodeThumbnail(encoded(3, 2, 7))
  assert.deepEqual([image.width, image.height, image.rgba.length, image.rgba[0]], [3, 2, 24, 7])
  assert.equal(decodeThumbnail(new Uint8Array(4)), null)
  assert.equal(decodeThumbnail(encoded(3, 2, 7).slice(0, 20)), null)
})

test("splits channelThumbnails' answer into its thumbnails", () => {
  const parts = [encoded(2, 1, 1), encoded(1, 3, 2)]
  const bytes = new Uint8Array(4 + parts[0].length + parts[1].length)
  new DataView(bytes.buffer).setUint32(0, 2, true)
  bytes.set(parts[0], 4)
  bytes.set(parts[1], 4 + parts[0].length)
  const split = splitThumbnails(bytes)
  assert.deepEqual(split.map((part) => [decodeThumbnail(part).width, decodeThumbnail(part).height, part[8]]), [[2, 1, 1], [1, 3, 2]])
  assert.deepEqual(splitThumbnails(bytes.slice(0, 20)).length, 1, 'a cut answer keeps the whole thumbnails')
  assert.deepEqual(splitThumbnails(new Uint8Array(2)), [])
})

test('queries each source once per revision, in one frame however often it is told', async () => {
  const run = harness()
  run.cache.update(1, [layer(1), layer(2)])
  run.cache.update(1, [layer(1), layer(2)])
  assert.equal(run.frames.length, 1, 'one frame asked for')
  await run.flush()
  assert.deepEqual(run.queries, ['layer:1@48', 'layer:2@48'])
  assert.equal(run.changes(), 1, 'one notice for the whole refresh')
  assert.ok(run.cache.get(layer(1)))
  run.cache.update(1, [layer(1), layer(2)])
  await run.flush()
  assert.equal(run.queries.length, 2, 'nothing stale, nothing queried')
  run.cache.update(2, [layer(1), { kind: 'mask', layer: 2 }])
  await run.flush()
  assert.deepEqual(run.queries.slice(2), ['layer:1@48', 'mask:2@48'])
  assert.equal(run.cache.get(layer(2)), null, 'a source no longer shown is dropped')
  assert.equal(run.closed.length, 0, 'replaced images stay drawable until the next refresh')
  run.cache.update(3, [layer(1)])
  await run.flush()
  assert.ok(run.closed.length >= 2, 'replaced and dropped images are released')
})

test('a change during a refresh waits for it, then refreshes once more', async () => {
  let release
  const delay = new Promise((resolve) => (release = resolve))
  const run = harness({ delay })
  run.cache.update(1, [layer(1)])
  run.frames.shift()()
  run.cache.update(2, [layer(1)])
  run.cache.update(3, [layer(1)])
  assert.equal(run.frames.length, 1)
  run.frames.shift()()
  assert.equal(run.queries.length, 1, 'no second query alongside the first')
  release()
  await run.flush()
  await new Promise((resolve) => setTimeout(resolve, 0))
  await run.flush()
  assert.deepEqual(run.queries, ['layer:1@48', 'layer:1@48'], 'one refresh for revisions 2 and 3 together')
})

test('a failed query leaves an empty thumbnail and does not throw', async () => {
  const run = harness({ fail: (source) => source.layer === 2 })
  run.cache.update(1, [layer(1), layer(2)])
  await run.flush()
  assert.ok(run.cache.get(layer(1)))
  assert.equal(run.cache.get(layer(2)), null)
  run.cache.dispose()
  assert.equal(run.cache.get(layer(1)), null, 'disposed')
})
