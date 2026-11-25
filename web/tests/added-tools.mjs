// The five tools added to the rail (plan 2026-11-24, stage 7) in a real browser: node tests/added-tools.mjs [chromium|webkit]
// Each tool is chosen from its slot's flyout as a person does, its bar named, then used with real
// clicks and drags; checks read the engine's selection, pixels, layers and paths; PASS or FAIL per step.
import { openEditor } from './canvas-harness.mjs'

const which = process.argv[2] ?? 'chromium'
const e = await openEditor(which)
const { page, kit, step, until, details, query, choose, drag, click, workspace, settle, tool } = e
const agent = async (command, params) => (await page.evaluate(([name, values]) => window.devHost.connection().ai.run([{ command: name, params: values }], false), [command, params])).completed?.[0]?.result
const colour = async (x, y, sampleLayer = 'all') => (await query('file.sampleColor', { x, y, size: 'point', sampleLayer })) ?? [0, 0, 0, 0]
const steps = async () => (await details()).history.length
try {
  const doc = await details()

  // ---------------------------------------------------------------- Single Row / Single Column Marquee
  await choose('SingleRowMarquee')
  step('the Single Row Marquee is in the marquees’ flyout, its bar named', (await tool()) === 'SingleRowMarquee' && ((await kit.getByTestId('photo-options-tool').textContent()) ?? '').includes('Single Row'))
  await click(400, 300.5)
  await until(async () => (await details()).hasSelection, 30)
  // The row under the click (WebKit's mouse lands on whole page pixels: within a pixel of 300).
  const row = (await details()).selectionBounds
  step('a click selects the whole row under it, one pixel high', row[0] === 0 && row[2] === doc.width && row[3] === 1 && Math.abs(row[1] - 300) <= 1, JSON.stringify(row))
  await choose('SingleColumnMarquee')
  await page.keyboard.down('Shift')
  await click(500.5, 100)
  await page.keyboard.up('Shift')
  await until(async () => (await details()).selectionBounds?.[3] === doc.height, 30)
  step('Single Column with Shift adds the column under the click', JSON.stringify((await details()).selectionBounds) === JSON.stringify([0, 0, doc.width, doc.height]), JSON.stringify((await details()).selectionBounds))
  // The person's Select › Deselect (an agent's edit.deselectAll keeps to the agent's own selection).
  await kit.getByTestId('photo-menu-Select').click()
  await kit.getByTestId('photo-cmd-edit.deselectAll').click()
  await until(async () => !(await details()).hasSelection, 30)
  await workspace.focus()

  // A red disc on a layer of its own, for Red Eye and Color Replacement (an agent sets the stage).
  const made = await agent('layer.new.layer', { name: 'Eye' })
  const eyeId = made.layer
  await agent('select.rect', { x: 180, y: 480, width: 40, height: 40, ellipse: true })
  await agent('edit.fill', { layer: eyeId, color: '#e01818' })
  await settle()
  const eye = (await details()).layers.find((layer) => layer.name === 'Eye')
  step('(the stage: a red disc on a layer of its own)', (await colour(200, 500))[0] > 0.8, JSON.stringify(await colour(200, 500)))
  await e.selectLayer(eye.id)

  // ---------------------------------------------------------------- Red Eye
  await choose('RedEye')
  const before = await colour(200, 500)
  const count = await steps()
  await click(200, 500)
  await until(async () => (await colour(200, 500))[0] < before[0] - 0.1, 40)
  const after = await colour(200, 500)
  step('Red Eye: a click on the red pupil darkens it, in one step', after[0] < before[0] - 0.1 && (await steps()) === count + 1, JSON.stringify({ before, after }))
  await settle()

  // ---------------------------------------------------------------- Color Replacement
  await agent('select.rect', { x: 180, y: 480, width: 40, height: 40, ellipse: true })
  await agent('edit.fill', { layer: eyeId, color: '#e01818' })
  await settle()
  await e.selectLayer(eye.id)
  await kit.getByTestId('photo-foreground').fill('#1030e0')
  await workspace.focus()
  await choose('ColorReplacement')
  step('Color Replacement is in the brushes’ flyout, with its sampling, limits and tolerance', (await tool()) === 'ColorReplacement' && (await kit.locator('[data-option="bgTolerance"]').count()) === 1)
  await drag([170, 500], [230, 500], { steps: 16 })
  await until(async () => {
    const [r, , b] = await colour(200, 500)
    return b > r
  }, 60)
  const replaced = await colour(200, 500)
  step('a stroke over the red replaces its colour with the foreground’s', replaced[2] > replaced[0] && replaced[0] < 0.5, JSON.stringify(replaced))
  await settle()

  // ---------------------------------------------------------------- Pattern Stamp
  await agent('layer.new.layer', { name: 'Stamped' })
  await settle()
  const stamped = (await details()).layers.find((layer) => layer.name === 'Stamped')
  await e.selectLayer(stamped.id)
  await choose('PatternStamp')
  step('the Pattern Stamp is in the Clone Stamp’s flyout, with Aligned', (await tool()) === 'PatternStamp' && (await kit.locator('[data-option="aligned"]').count()) === 1)
  const empty = await colour(700, 300, 'current')
  await drag([650, 300], [750, 300], { steps: 16 })
  await until(async () => (await colour(700, 300, 'current'))[3] > 0, 60)
  step('a stroke paints the current pattern on an empty layer', empty[3] === 0 && (await colour(700, 300, 'current'))[3] > 0, JSON.stringify(await colour(700, 300, 'current')))
  await settle()

  // ---------------------------------------------------------------- Convert Point
  await choose('Pen')
  await click(300, 650)
  await click(500, 650)
  await click(700, 650)
  await workspace.focus()
  await page.keyboard.press('Enter')
  await until(async () => (await query('path.info', { name: 'work' }))?.path?.subpaths?.[0]?.knots?.length === 3, 40)
  await choose('ConvertPoint')
  step('Convert Point is in the Pen’s flyout', (await tool()) === 'ConvertPoint')
  await drag([500, 650], [560, 610])
  await until(async () => (await query('path.info', { name: 'work' }))?.path?.subpaths?.[0]?.knots?.[1]?.smooth === true, 40)
  const knot = (await query('path.info', { name: 'work' }))?.path?.subpaths?.[0]?.knots?.[1]
  step('dragging from an anchor pulls smooth direction lines out of it', knot?.smooth === true && Math.abs(knot.out[0] - 560) <= 2 && Math.abs(knot.in[0] - 440) <= 2, JSON.stringify(knot))
  await settle()
  await click(500, 650)
  await until(async () => (await query('path.info', { name: 'work' }))?.path?.subpaths?.[0]?.knots?.[1]?.smooth === false, 40)
  step('a click on an anchor makes it a corner again', (await query('path.info', { name: 'work' }))?.path?.subpaths?.[0]?.knots?.[1]?.smooth === false)
  await e.finish()
} catch (error) {
  await e.fail(error)
}
