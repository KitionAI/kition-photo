// Takes the README's pictures of the Photo editor (assets/readme/*.png at the repository root) on
// kition-sdk's shared runner (tools/readme-shots/shots.mjs): the development host opens the
// showcase sample autumn-sale-poster, and each shot shows one ability. Not a test; re-run it by hand
// when the editor changes:
//
//   cd web && pnpm install && pnpm engine          # the engine (and fonts) in public/
//   node scripts/readme-shots.mjs [name ...]       # every shot, or only the named ones
//
// The sample comes from a kition-showcase checkout next to this repository (or the one
// KITION_SHOWCASE names). Run it with no HTTP proxy set. Shrink the pictures afterwards with
// pngquant --quality 70-90 (a README picture is at most 400 KB).
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const sdk = path.resolve(process.env.KITION_SDK ?? path.join(web, '..', '..', 'kition-sdk'))
const showcase = path.resolve(process.env.KITION_SHOWCASE ?? path.join(web, '..', '..', 'kition-showcase'))
const { takeShots } = await import(pathToFileURL(path.join(sdk, 'tools/readme-shots/shots.mjs')).href)

/** The editor's state as the engine reports it (layers with ids, names and kinds). */
const details = (page) => page.evaluate(async () => (await window.devHost.connection().ai.inspect()).details)
/** The id of the layer named `name`, searched through groups. */
async function layerId(page, name) {
  const find = (layers) => {
    for (const layer of layers ?? []) {
      if (layer.name === name) return layer.id
      const inner = find(layer.children)
      if (inner !== undefined) return inner
    }
    return undefined
  }
  const id = find((await details(page)).layers)
  if (id === undefined) throw new Error(`no layer named ${name}`)
  return id
}
/** Clicks a layer's row in the Layers panel, as a person selects it. */
async function selectLayer(page, kit, name) {
  await kit.getByTestId(`photo-layer-${await layerId(page, name)}`).click()
  await page.waitForTimeout(300)
}

/**
 * Where image pixel (x, y) is on the page: the image is drawn centred in the canvas region at the
 * zoom the status bar shows.
 */
async function imagePoint(page, kit, x, y) {
  const region = await kit.getByTestId('photo-canvas-region').boundingBox()
  const zoom = Number(await kit.getByTestId('photo-status-zoom').locator('input').inputValue()) / 100
  const { width, height } = (await page.evaluate(async () => (await window.devHost.connection().ai.inspect()).details))
  return { x: region.x + region.width / 2 + (x - width / 2) * zoom, y: region.y + region.height / 2 + (y - height / 2) * zoom }
}
/** Drags from image pixel `from` to image pixel `to` with the current tool. */
async function drag(page, kit, from, to) {
  const a = await imagePoint(page, kit, ...from)
  const b = await imagePoint(page, kit, ...to)
  await page.mouse.move(a.x, a.y)
  await page.mouse.down()
  await page.mouse.move(b.x, b.y, { steps: 12 })
  await page.mouse.up()
  await page.waitForTimeout(400)
}
/** Window › Color (the ribbon's Window tab, then back to Home): hides the Color group, so the other groups have the dock's height. */
async function hideColor(kit, { character = false } = {}) {
  await kit.getByRole('tab', { name: 'Window' }).click()
  await kit.getByTestId('photo-control-window.panel.color').click()
  await kit.getByTestId('photo-dock-area-tab-color').waitFor({ state: 'detached' })
  // Window › Character: the Character group, for the active type layer.
  if (character) {
    await kit.getByTestId('photo-control-window.panel.character').click()
    await kit.getByTestId('photo-character').waitFor()
  }
  await kit.getByRole('tab', { name: 'Home' }).click()
}
/** Chooses a tool from its slot in the rail, or from the slot's flyout (a right click on the slot). */
async function chooseTool(kit, id, slotMate) {
  const slot = kit.locator(`.kui-tool-slot > [data-testid="photo-tool-${id}"]`)
  if (await slot.count()) return slot.click()
  await kit.locator(`.kui-tool-slot > [data-testid="photo-tool-${slotMate}"]`).click({ button: 'right' })
  await kit.locator(`.kui-tool-flyout [data-testid="photo-tool-${id}"]`).click()
}

await takeShots({
  web,
  sample: path.join(showcase, 'photo', 'autumn-sale-poster', 'autumn-sale-poster.psd'),
  ready: '[data-testid="photo-canvas-region"]',
  query: { modes: 'classic,line,menus,focus' },
  shots: [
    // The poster as it opens: nine layers (a gradient fill, shapes with layer styles, type).
    { name: 'hero' },
    {
      // The Layers panel: the badge selected, its Stroke style listed, the blend modes open.
      name: 'see-1',
      steps: async (page, { kit }) => {
        await selectLayer(page, kit, 'Badge')
        await kit.getByTestId(`photo-layer-${await layerId(page, 'Badge')}`).click({ button: 'right' })
      },
    },
    {
      // A Hue/Saturation adjustment layer on top of the stack, its settings in Properties.
      name: 'see-2',
      steps: async (page, { kit }) => {
        await hideColor(kit)
        await selectLayer(page, kit, 'Dates')
        await kit.getByTestId('photo-dock-area-tab-adjustments').click()
        await kit.getByTestId('photo-adjustments-hueSaturation').click()
        const hue = kit.getByTestId('photo-adjustment-master-hue').locator('input')
        await hue.waitFor()
        await hue.fill('150')
        await hue.press('Enter')
      },
    },
    {
      // A layer mask from a selection: an elliptical marquee on the badge, then Add a mask.
      name: 'see-3',
      steps: async (page, { kit }) => {
        await selectLayer(page, kit, 'Badge')
        await chooseTool(kit, 'EllipseMarquee', 'RectMarquee')
        await drag(page, kit, [835, 185], [1045, 395])
        await kit.getByTestId('photo-layers-mask').click()
      },
    },
    {
      // A type layer given a Drop Shadow layer style from the Layers panel's fx menu; with the Color
      // group hidden (Window › Color), Properties shows the layer's Character settings.
      name: 'see-4',
      steps: async (page, { kit }) => {
        await hideColor(kit, { character: true })
        await selectLayer(page, kit, 'Headline')
        await kit.getByTestId('photo-layers-style').click()
        await kit.getByTestId('photo-layers-style-dropShadow').click()
        const dialog = kit.getByRole('dialog')
        await dialog.getByText('More options').click()
        for (const [field, value] of [['distance', '6'], ['size', '10'], ['opacity', '45']]) {
          await dialog.getByLabel(field, { exact: true }).fill(value)
        }
        await dialog.getByRole('button', { name: 'Run' }).click()
        await dialog.waitFor({ state: 'detached' })
        // The Layers list scrolled to the type layer and its effect.
        await kit.getByTestId(`photo-layer-${await layerId(page, 'Subtitle')}`).evaluate((element) => element.scrollIntoView({ block: 'start' }))
      },
    },
  ],
})
