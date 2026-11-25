// The Pen, Direct Selection and Path Selection editing paths in a real browser (plan 2026-11-24, stage 7):
// node tests/paths.mjs [chromium|webkit]. Real clicks, drags and keys on dev.html; every check reads
// the engine's paths (`path.info`, `shape.info`) and history, or the canvas's pixels; PASS or FAIL per step.
import { openEditor } from './canvas-harness.mjs'

const which = process.argv[2] ?? 'chromium'
const e = await openEditor(which)
const { page, kit, step, until, details, query, choose, drag, click, at, shot, workspace, surface, primary, settle } = e
const work = async () => (await query('path.info', { name: 'work' }))?.path ?? null
const knots = async (subpath = 0) => (await work())?.subpaths?.[subpath]?.knots ?? []
const near = (a, b, tolerance = 2) => Math.abs(a[0] - b[0]) <= tolerance && Math.abs(a[1] - b[1]) <= tolerance
const steps = async () => (await details()).history.length
/** The canvas's colour at page point (x, y). */
const pixel = async (x, y, locator = surface) => {
  const box = await locator.boundingBox()
  return locator.evaluate((canvas, [lx, ly, width]) => {
    const ratio = canvas.width / width
    return [...canvas.getContext('2d').getImageData(Math.round(lx * ratio), Math.round(ly * ratio), 1, 1).data]
  }, [x - box.x, y - box.y, box.width])
}
try {
  await choose('Pen')
  const before = await steps()

  // ---------------------------------------------------------------- the Pen draws
  await click(100, 100)
  {
    // A drag pulls the anchor's direction lines; mid-drag they are drawn.
    const a = await at(300, 100)
    const b = await at(360, 160)
    await page.mouse.move(a.x, a.y)
    await page.mouse.down()
    await page.mouse.move(b.x, b.y, { steps: 10 })
    await page.waitForTimeout(150)
    const handleEnd = await pixel(b.x, b.y, kit.locator('.kui-canvas-overlay'))
    const mirror = await at(240, 40)
    const handleStart = await pixel(mirror.x, mirror.y, kit.locator('.kui-canvas-overlay'))
    await shot('pen-direction-lines')
    step('dragging after a Pen click pulls direction lines, drawn on both sides as it goes', handleEnd[3] > 0 && handleStart[3] > 0, JSON.stringify([handleEnd, handleStart]))
    await page.mouse.up()
  }
  await click(300, 300)
  await click(100, 100)
  await until(async () => (await knots()).length === 3, 40)
  const drawn = await work()
  const smooth = drawn?.subpaths[0].knots[1]
  step('a click on the first anchor closes the path: three anchors, one history step', drawn?.subpaths[0].closed === true && (await knots()).length === 3 && (await steps()) === before + 1, JSON.stringify(drawn?.subpaths[0]))
  step('the dragged anchor is smooth, its direction points mirrored', smooth?.smooth === true && near(smooth.out, [360, 160]) && near(smooth.in, [240, 40]), JSON.stringify(smooth))
  await settle()

  // ---------------------------------------------------------------- Auto Add/Delete and Alt
  {
    const p = await at(200, 200)
    await page.mouse.move(p.x, p.y)
    await page.waitForTimeout(150)
    const cursor = await surface.evaluate((canvas) => canvas.style.cursor)
    step('over a segment the Pen’s cursor says it adds an anchor', cursor.includes('svg'), cursor.slice(0, 40))
  }
  let count = await steps()
  await click(200, 200)
  await until(async () => (await knots()).length === 4, 40)
  const added = await knots()
  step('a click on a segment adds an anchor there, in one step', added.length === 4 && added.some((knot) => near(knot.anchor, [200, 200])) && (await steps()) === count + 1, JSON.stringify(added.map((knot) => knot.anchor)))
  await click(200, 200)
  await until(async () => (await knots()).length === 3, 40)
  step('a click on an anchor deletes it', (await knots()).length === 3 && !(await knots()).some((knot) => near(knot.anchor, [200, 200])), JSON.stringify((await knots()).map((knot) => knot.anchor.map(Math.round))))
  await settle()
  await page.keyboard.down('Alt')
  await click(300, 100)
  await page.keyboard.up('Alt')
  await until(async () => (await knots())[1]?.smooth === false, 40)
  const cornered = (await knots())[1]
  step('Alt-click on a smooth anchor makes it a corner, its direction points retracted', cornered?.smooth === false && near(cornered.in, cornered.anchor, 0.01) && near(cornered.out, cornered.anchor, 0.01), JSON.stringify(cornered))
  await drag([300, 300], [340, 260], { keys: ['Alt'] })
  await until(async () => (await knots())[2]?.smooth === true, 40)
  const pulled = (await knots())[2]
  step('Alt-drag from an anchor pulls smooth direction lines out of it', pulled?.smooth === true && near(pulled.out, [340, 260], 3), JSON.stringify(pulled))
  await settle()
  await drag([100, 100], [120, 90], { keys: [primary] })
  await until(async () => near((await knots())[0]?.anchor ?? [0, 0], [120, 90], 2), 40)
  step('Cmd lends Direct Selection: a Cmd-drag on an anchor moves it', near((await knots())[0].anchor, [120, 90], 2), JSON.stringify((await knots())[0]))
  await settle()

  // ---------------------------------------------------------------- Direct Selection
  await choose('DirectSelection')
  count = await steps()
  {
    const from = await at(300, 300)
    const to = await at(330, 320)
    await page.mouse.move(from.x, from.y)
    await page.mouse.down()
    await page.mouse.move(to.x, to.y, { steps: 10 })
    await page.waitForTimeout(150)
    const following = await pixel(to.x, to.y, kit.locator('.kui-canvas-overlay'))
    await shot('direct-selection-drag')
    step('mid-drag the anchor and its path follow the pointer', following[3] > 0, JSON.stringify(following))
    await page.mouse.up()
  }
  await until(async () => near((await knots())[2]?.anchor ?? [0, 0], [330, 320], 2), 40)
  step('Direct Selection drags an anchor (with its direction points) in one step', near((await knots())[2].anchor, [330, 320], 2) && near((await knots())[2].out, [370, 280], 3) && (await steps()) === count + 1, JSON.stringify((await knots())[2]))
  await settle()
  // Its direction point: the selected anchor's out handle at (370, 280); a smooth point keeps the other in line.
  await drag([370, 280], [380, 330])
  await until(async () => near((await knots())[2]?.out ?? [0, 0], [380, 330], 2), 40)
  const handled = (await knots())[2]
  const turnedIn = [2 * handled.anchor[0] - handled.out[0], 2 * handled.anchor[1] - handled.out[1]]
  const collinear = Math.abs((handled.out[0] - handled.anchor[0]) * (handled.in[1] - handled.anchor[1]) - (handled.out[1] - handled.anchor[1]) * (handled.in[0] - handled.anchor[0])) < 5
  step('dragging a direction point moves it; a smooth point turns its other one to stay in line', near(handled.out, [380, 330], 2) && collinear, JSON.stringify({ handled, turnedIn }))
  await settle()
  // A marquee selects the anchors inside it; Shift+arrows nudge them 10 px, two presses one step; Delete takes them out.
  await drag([80, 60], [340, 140])
  await page.waitForTimeout(200)
  const nudgeFrom = (await knots()).map((knot) => knot.anchor)
  count = await steps()
  await workspace.focus()
  await page.keyboard.press('Shift+ArrowDown')
  await page.keyboard.press('Shift+ArrowDown')
  await page.waitForTimeout(1300)
  const nudged = (await knots()).map((knot) => knot.anchor)
  step('a marquee selects the anchors in it, and arrows nudge only them, presses in a row one step', near(nudged[0], [nudgeFrom[0][0], nudgeFrom[0][1] + 20]) && near(nudged[1], [nudgeFrom[1][0], nudgeFrom[1][1] + 20]) && near(nudged[2], nudgeFrom[2]) && (await steps()) === count + 1, JSON.stringify({ nudgeFrom, nudged, steps: (await steps()) - count }))
  await page.keyboard.press('Delete')
  await until(async () => (await knots()).length === 1, 40)
  step('Delete takes the selected anchors out', (await knots()).length === 1, JSON.stringify((await knots()).map((knot) => knot.anchor)))
  await settle()

  // ---------------------------------------------------------------- Enter and Escape finish the Pen; a new path joins the work path
  await choose('Pen')
  await click(500, 500)
  await click(600, 520)
  await workspace.focus()
  await page.keyboard.press('Escape')
  await until(async () => (await work())?.subpaths?.length === 2, 40)
  step('Escape finishes the path being drawn, as another component of the work path', (await work())?.subpaths?.length === 2 && (await knots(1)).length === 2, JSON.stringify((await work())?.subpaths?.map((subpath) => subpath.knots.length)))
  await settle()

  // ---------------------------------------------------------------- Path Selection moves a whole path
  await choose('PathSelection')
  const whole = (await work()).subpaths.flatMap((subpath) => subpath.knots.map((knot) => knot.anchor))
  await drag([550, 510], [570, 540])
  await until(async () => !near((await knots(1))[0]?.anchor ?? [0, 0], whole[1]), 40)
  const moved = (await work()).subpaths.flatMap((subpath) => subpath.knots.map((knot) => knot.anchor))
  step('Path Selection drags the component under the press (the other stays)', near(moved[1], [whole[1][0] + 20, whole[1][1] + 30]) && near(moved[0], whole[0]), JSON.stringify({ whole, moved }))
  await settle()

  // ---------------------------------------------------------------- a shape layer's path
  await choose('Rectangle')
  await drag([700, 200], [900, 350])
  await until(async () => (await details()).layers.some((layer) => layer.kind === 'Shape' && layer.name.startsWith('Rectangle')), 40)
  const rectangle = (await details()).layers.find((layer) => layer.kind === 'Shape' && layer.name.startsWith('Rectangle'))
  const shapePath = async () => (await query('shape.info', { layer: rectangle.id }))?.path?.subpaths?.[0]?.knots ?? []
  const corners = (await shapePath()).map((knot) => knot.anchor)
  await choose('DirectSelection')
  const corner = corners.reduce((best, p) => (p[0] + p[1] > best[0] + best[1] ? p : best))
  await drag(corner, [corner[0] + 30, corner[1] + 20])
  await until(async () => (await shapePath()).some((knot) => near(knot.anchor, [corner[0] + 30, corner[1] + 20])), 40)
  await shot('shape-anchor')
  step("Direct Selection moves a shape layer's anchor too; it stays a shape layer", (await shapePath()).some((knot) => near(knot.anchor, [corner[0] + 30, corner[1] + 20])) && (await details()).layers.find((layer) => layer.id === rectangle.id)?.kind === 'Shape', JSON.stringify((await shapePath()).map((knot) => knot.anchor)))
  await settle()
  await choose('Pen')
  const [p0, p1] = (await shapePath()).map((knot) => knot.anchor)
  await click((p0[0] + p1[0]) / 2, (p0[1] + p1[1]) / 2)
  await until(async () => (await shapePath()).length === 5, 40)
  step("the Pen adds an anchor on a shape layer's segment", (await shapePath()).length === 5, String((await shapePath()).length))
  await e.finish()
} catch (error) {
  await e.fail(error)
}
