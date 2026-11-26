// An agent's draft and a restored version in the editor (Kit UI protocol sections 7d and 7m): the
// agent makes a draft, the person edits the image meanwhile, the person accepts the draft and one
// Undo takes the whole draft back (the person's edit stays); a checkpoint restored over a later edit
// is one Undo step too. Through the development host with `?checkpoints=1`:
// node tests/drafts.mjs [chromium|webkit]
import assert from 'node:assert/strict'
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'

const which = process.argv[2] ?? 'chromium'
const server = await createServer({ configFile: new URL('../vite.config.ts', import.meta.url).pathname, server: { port: 0, host: '127.0.0.1' }, logLevel: 'error' })
await server.listen()
const browser = await (which === 'webkit' ? webkit : chromium).launch()
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
const kit = page.frameLocator('#kit')
const errors = []
page.on('pageerror', (error) => errors.push(error.message))
const run = (command, params = {}, options = {}) =>
  page.evaluate(async ({ command, params, options }) => (await window.devHost.connection().ai.run([{ command, params }], false, options)).completed[0].result, { command, params, options })
const until = async (check, message) => {
  for (let n = 0; n < 200; n += 1) {
    if (await check().catch(() => false)) return
    await page.waitForTimeout(100)
  }
  assert.fail(message)
}
/** The layers of the image (every field, children included), or (`draft`) of the agent's draft, as JSON text. */
const names = async (draft = false) => {
  if (draft) return JSON.stringify(await run('file.inspect', {}, { draft: true }))
  const details = (await page.evaluate(async () => window.devHost.connection().ai.inspect())).details
  return JSON.stringify(details.layers)
}
/** How many groups the image holds, at any depth (a group made while a group is active goes into it). */
const groups = async () => (await names()).match(/"name":"Group \d+"/g)?.length ?? 0
/** The person makes a group with the Layers panel's button. */
const groupByHand = async () => {
  const before = await groups()
  await kit.getByTestId('photo-layers-group').click()
  await until(async () => (await groups()) === before + 1, `the person's group shows (${before} before: ${await names()})`)
}
const undo = async () => {
  await until(() => kit.getByTestId('photo-undo').isEnabled(), 'Undo is enabled')
  await kit.getByTestId('photo-undo').click()
}
const path = () => page.evaluate(() => [...window.devHost.files.keys()].find((name) => /\.psd$/i.test(name)))
let failures = 0

try {
  await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/dev.html?locale=en-US&checkpoints=1`)
  await page.waitForFunction(() => document.getElementById('status')?.textContent?.includes(' open in '), null, { timeout: 60000 })
  await page.waitForTimeout(1000)

  // The agent drafts a layer; the image does not show it.
  await run('layer.new.layer', { name: 'From the draft' }, { draft: true })
  assert.ok((await names(true)).includes('From the draft'), 'the draft holds the agent\'s layer')
  assert.ok(!(await names()).includes('From the draft'), 'the image does not, yet')
  // The person makes a group meanwhile.
  await groupByHand()
  const meanwhile = await groups()
  // The person accepts the draft (the host passes the agent's origin): both are kept.
  const merged = await page.evaluate(() => window.devHost.connection().draft.merge({ kind: 'agent', principal: 'agent:test' }))
  assert.equal(merged.merged, true, 'the draft merged')
  await until(async () => (await names()).includes('From the draft'), 'the merged layer shows')
  assert.equal(await groups(), meanwhile, 'the group made meanwhile stays')
  // One Undo of the person takes the whole draft back and leaves the person's group.
  await undo()
  await until(async () => !(await names()).includes('From the draft'), 'Undo takes the draft back')
  assert.equal(await groups(), meanwhile, 'the person\'s group stays')
  console.log(`PASS ${which}: a merged draft is one step of the person's Undo`)

  // A checkpoint of the saved image; an edit after it; the checkpoint restored; Undo.
  const file = await path()
  const version = () => page.evaluate((name) => window.devHost.files.get(name)?.version ?? 0, file)
  const saved = await version()
  await kit.getByTestId('photo-menu-File').click()
  await kit.getByTestId('photo-file-save').click()
  await until(async () => (await version()) > saved, 'saved')
  await until(() => page.evaluate((name) => window.devHost.history.replica(name) !== null, file), 'the replica is stored with the save')
  // Whatever the save left open (the menu, a note) closes, so the next click reaches the panel.
  await kit.locator('body').press('Escape')
  await page.waitForTimeout(300)
  const checkpoint = await page.evaluate(() => window.devHost.recordCheckpoint({ origin: 'save' }))
  assert.ok(checkpoint?.id, 'a checkpoint is recorded')
  await groupByHand()
  assert.equal(await groups(), meanwhile + 1)
  await page.evaluate((id) => window.devHost.restoreCheckpoint(id), checkpoint.id)
  await until(async () => (await groups()) === meanwhile, 'the restore gives back the checkpoint\'s layers')
  await undo()
  await until(async () => (await groups()) === meanwhile + 1, 'Undo takes the restore back')
  console.log(`PASS ${which}: a restored version is one step of the person's Undo`)
  assert.deepEqual(errors, [], 'no page errors')
} catch (error) {
  failures += 1
  console.log(`FAIL ${error instanceof Error ? error.message.split('\n')[0] : String(error)}`)
} finally {
  if (errors.length) console.log(`page errors:\n${errors.slice(0, 8).join('\n')}`)
  await browser.close()
  await server.close()
}
process.exit(failures ? 1 : 0)
