// The Type tool's served fonts (plan 2026-11-24, stage 9): the manifest, the font list and what a
// document needs loaded. Run with `pnpm --dir web test:parity`.
import test from 'node:test'
import assert from 'node:assert/strict'
import { familiesToLoad, fontList, parseManifest } from './catalog.ts'

const MANIFEST = `# kition-fonts' format
Lora | Regular | fonts/lora/Lora-Variable.ttf | Latn,Cyrl | OFL-1.1 | fonts/lora/OFL.txt | 822a | https://example.org

Tinos | Regular | fonts/tinos/Tinos-Regular.ttf | Latn | OFL-1.1
Tinos | Bold | fonts/tinos/Tinos-Bold.ttf | Latn
Broken | Regular
Outside | Regular | ../secret.ttf | Latn
Absolute | Regular | /fonts/x.ttf
`

test('reads the served fonts manifest: each family with its files, nothing outside the folder', () => {
  const catalog = parseManifest(MANIFEST)
  assert.deepEqual([...catalog.keys()], ['Lora', 'Tinos'])
  assert.deepEqual(catalog.get('Lora'), [{ family: 'Lora', style: 'Regular', file: 'fonts/lora/Lora-Variable.ttf', scripts: ['Latn', 'Cyrl'] }])
  assert.deepEqual(catalog.get('Tinos').map((file) => file.style), ['Regular', 'Bold'])
})

test('the font list has the engine’s and the served families by name, each with its state, and keeps a missing current family', () => {
  const catalog = parseManifest(MANIFEST)
  const list = fontList(['Inter', 'JetBrains Mono', 'Lora'], catalog, new Set(['Tinos']), 'Helvetica Neue')
  assert.deepEqual(list, [
    { family: 'Helvetica Neue', state: 'missing' },
    { family: 'Inter', state: 'loaded' },
    { family: 'JetBrains Mono', state: 'loaded' },
    { family: 'Lora', state: 'loaded' },
    { family: 'Tinos', state: 'loading' },
  ])
  assert.deepEqual(fontList(['Inter'], catalog, new Set(), 'Tinos').map((item) => [item.family, item.state]), [['Inter', 'loaded'], ['Lora', 'served'], ['Tinos', 'served']])
})

test('a document loads the served families its type names that the engine lacks, once each', () => {
  const catalog = parseManifest(MANIFEST)
  assert.deepEqual(familiesToLoad(['Tinos', 'Inter', 'Lora', 'Tinos', 'Helvetica', ''], ['Inter', 'Lora'], catalog), ['Tinos'])
})
