import { spawnSync } from 'node:child_process'
import { mkdirSync, existsSync, writeFileSync, readFileSync, rmSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'

const web = fileURLToPath(new URL('..', import.meta.url))
process.chdir(web)
rmSync('test-results', { recursive: true, force: true })
mkdirSync('test-results', { recursive: true })
for (const path of ['public/engine/kition_photo_kit.js', 'public/engine/kition_photo_kit_bg.wasm', 'public/fonts/DroidSansFallbackFull.ttf']) {
  if (!existsSync(path)) throw new Error(`Missing ${path}; check out kition-fonts and run pnpm engine first. Required tests cannot be skipped.`)
}
const hash = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')
writeFileSync('test-results/build.json', JSON.stringify({
  node: process.version,
  platform: process.platform,
  wasmSha256: hash('public/engine/kition_photo_kit_bg.wasm'),
  fontSha256: hash('public/fonts/DroidSansFallbackFull.ttf'),
}, null, 2))
const full = process.argv.includes('--full')
const steps = [
  [process.execPath, ['tests/scenarios/coverage.mjs']],
  ['pnpm', ['build']],
  ['pnpm', ['test:parity']],
  [process.execPath, ['--test', '--test-reporter=spec', '--test-reporter=junit', '--test-reporter-destination=stdout', '--test-reporter-destination=test-results/engine.xml', 'tests/scenarios/engine.test.mjs']],
  ['pnpm', ['exec', 'playwright', 'test', ...(full ? [] : ['--project=chromium'])]],
  ...(full ? [['pnpm', ['test:flow']]] : []),
]
for (const [command, args] of steps) {
  console.log(`\nRunning ${command} ${args.join(' ')}`)
  const result = spawnSync(command, args, { stdio: 'inherit', cwd: web, env: process.env })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}
