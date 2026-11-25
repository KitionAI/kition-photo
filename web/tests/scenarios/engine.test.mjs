import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { cases, runScenario, sdk, execute, snapshot, same, font } from './engine.mjs'

for (const scenario of cases) {
  test(`${scenario.id} [${scenario.priority}] ${scenario.title}`, { timeout: 60_000 }, () => runScenario(scenario))
}

test('SC11 [P0] Two documents keep edits and history isolated', () => {
  const a = sdk.PhotoDocument.create('A.psd', 64, 48)
  const b = sdk.PhotoDocument.create('B.psd', 32, 24)
  try {
    const untouchedB = snapshot(b)
    execute(a, 'layer.new.layer', { name: 'Only A' })
    execute(a, 'edit.fill', { color: '#ff0000' })
    const editedA = snapshot(a)
    same(b, untouchedB)
    assert.equal(b.canUndo(), false)
    execute(b, 'layer.new.layer', { name: 'Only B' })
    execute(b, 'edit.fill', { color: '#0000ff' })
    execute(b, 'edit.undo')
    same(a, editedA)
    const reopenedA = sdk.PhotoDocument.open('A.psd', a.save('A.psd'))
    try { same(reopenedA, editedA) } finally { reopenedA.free() }
    assert.equal(a.dirty, true)
    a.markSaved()
    assert.equal(a.dirty, false)
    assert.equal(b.dirty, true)
  } finally { a.free(); b.free() }
})

for (const orientation of ['horizontal', 'vertical']) {
  test(`SC12 [P0] First font registration renders ${orientation} Chinese without prewarming`, async () => {
    // A separate module owns a fresh WASM instance: an earlier test's font registration
    // must not repair the fallback catalog before this scenario gets to exercise it.
    const url = new URL('../../public/engine/kition_photo_kit.js', import.meta.url)
    url.searchParams.set('cold-font', orientation)
    const cold = await import(url.href)
    cold.initSync({ module: readFileSync(new URL('../../public/engine/kition_photo_kit_bg.wasm', import.meta.url)) })
    const text = '你好极光'
    const params = { x: 24, y: 30, text, size: 22, color: '#204060', orientation }
    const withoutFont = cold.PhotoDocument.create('Missing.psd', 128, 160)
    let missing
    try {
      execute(withoutFont, 'text.add', params)
      missing = snapshot(withoutFont)
    } finally { withoutFont.free() }

    assert.ok(cold.addFont(readFileSync(font)) > 0, 'The first CJK font registration must succeed')
    const doc = cold.PhotoDocument.create('Chinese.psd', 128, 160)
    try {
      execute(doc, 'text.add', params)
      const fallback = snapshot(doc)
      const originalStyle = execute(doc, 'text.info')
      assert.equal(originalStyle.text, text)
      assert.equal(originalStyle.runs[0].style.font_family, 'Inter')
      execute(doc, 'text.setStyle', { font: 'Droid Sans Fallback' })
      const artifacts = new URL(`../../test-results/engine/SC12-${orientation}/`, import.meta.url)
      mkdirSync(artifacts, { recursive: true })
      writeFileSync(new URL('missing.png', artifacts), missing.png)
      writeFileSync(new URL('fallback.png', artifacts), fallback.png)
      writeFileSync(new URL('explicit.png', artifacts), doc.save('explicit.png'))
      assert.ok(!Buffer.from(fallback.pixels).equals(Buffer.from(missing.pixels)), 'First registration must replace missing-glyph rendering')
      same(doc, fallback, true)
      execute(doc, 'edit.undo')
      assert.deepEqual(execute(doc, 'text.info'), originalStyle)
      same(doc, fallback)
      const reopened = cold.PhotoDocument.open('Chinese.psd', doc.save('Chinese.psd'))
      try {
        same(reopened, fallback)
        assert.equal(execute(reopened, 'text.info').text, text)
        execute(reopened, 'text.edit', { text: '明天见' })
        assert.equal(execute(reopened, 'text.info').text, '明天见')
      } finally { reopened.free() }
    } finally { doc.free() }
  })
}
