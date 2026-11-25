# Photo writing format

How a model writes Kition Photo documents: the steps it sends, how it names what it changes,
what a new document starts with, the named values commands accept, and worked examples.
The package ships the short version for models as [agent/guide.md](../agent/guide.md); this file is
the complete reference it is drawn from. Everything below comes from the engine itself
(its 852 commands and their parameter schemas, and what `inspect` reports) and from the guide.

## What a model writes

A model never writes the file bytes. It calls the kit's agent tools (prefix `photo`), and the
engine applies each change, keeps it in undo history and saves the file:

- `photo_inspect`: read the document; `photo_commands`: find commands by keyword;
- `photo_run`: run steps `{command, params}` in order;
- `photo_check`: list problems; `photo_query` / `photo_batch`: read or set objects of the object model.

## Syntax

A step is one engine command with its parameters, as JSON:

```json
{"command": "<command id>", "params": {"<name>": <value>}}
```

`photo_run` takes a list of steps and runs them in order, stopping at the first failure. Parameter
names, types and required fields come from each command's schema; `photo_commands` returns them.
Commands by namespace (the part of the id before the first dot):

| Namespace | Commands | Change the document | Open to the agent | Examples |
| --- | ---: | ---: | ---: | --- |
| `action` | 7 | 1 | 7 | `action.move`, `action.list`, `action.get` |
| `arrange` | 5 | 5 | 5 | `arrange.bringForward`, `arrange.sendBackward`, `arrange.bringToFront` |
| `brush` | 14 | 12 | 14 | `brush.presets.list`, `brush.presets.save`, `brush.presets.update` |
| `channel` | 21 | 11 | 21 | `channel.new`, `channel.newSpot`, `channel.duplicate` |
| `cloneSource` | 5 | 4 | 5 | `cloneSource.list`, `cloneSource.select`, `cloneSource.set` |
| `color` | 1 | 1 | 1 | `color.profileMismatch` |
| `command` | 1 | 0 | 1 | `command.list` |
| `count` | 7 | 7 | 7 | `count.add`, `count.remove`, `count.move` |
| `edit` | 78 | 46 | 78 | `edit.undo`, `edit.redo`, `edit.fill` |
| `file` | 59 | 32 | 59 | `file.new`, `file.close`, `file.inspect` |
| `filter` | 124 | 124 | 124 | `filter.blur.gaussianBlur`, `filter.blur.blur`, `filter.blur.blurMore` |
| `gradient` | 12 | 10 | 12 | `gradient.fill.add`, `gradient.fill.set`, `gradient.fill.stop` |
| `history` | 3 | 3 | 3 | `history.goTo`, `history.deleteState`, `history.newDocument` |
| `image` | 64 | 61 | 64 | `image.imageRotation.flipCanvasHorizontal`, `image.imageRotation.flipCanvasVertical`, `image.imageRotation.rotate180` |
| `job` | 2 | 0 | 2 | `job.list`, `job.cancel` |
| `layer` | 198 | 182 | 198 | `layer.new.layer`, `layer.new.group`, `layer.groupLayers` |
| `layerComp` | 13 | 12 | 13 | `layerComp.new`, `layerComp.update`, `layerComp.apply` |
| `measurementLog` | 3 | 2 | 3 | `measurementLog.list`, `measurementLog.delete`, `measurementLog.export` |
| `note` | 4 | 3 | 4 | `note.add`, `note.set`, `note.delete` |
| `paint` | 25 | 25 | 25 | `paint.stroke`, `paint.setSymmetry`, `paint.symmetryFromPath` |
| `path` | 19 | 17 | 19 | `path.list`, `path.info`, `path.set` |
| `pattern` | 10 | 8 | 10 | `pattern.list`, `pattern.rename`, `pattern.delete` |
| `plugin` | 5 | 1 | 5 | `plugin.list`, `plugin.install`, `plugin.remove` |
| `prefs` | 3 | 0 | 3 | `prefs.get`, `prefs.set`, `prefs.reset` |
| `select` | 32 | 5 | 32 | `select.inverse`, `select.rect`, `select.float` |
| `session` | 1 | 0 | 1 | `session.inspect` |
| `shape` | 9 | 7 | 9 | `shape.add`, `shape.edit`, `shape.info` |
| `slice` | 7 | 6 | 7 | `slice.new`, `slice.fromGuides`, `slice.set` |
| `style` | 5 | 4 | 5 | `style.presets.list`, `style.presets.apply`, `style.presets.new` |
| `swatch` | 12 | 11 | 12 | `swatch.list`, `swatch.add`, `swatch.delete` |
| `text` | 59 | 52 | 59 | `text.add`, `text.edit`, `text.setStyle` |
| `timeline` | 7 | 6 | 7 | `timeline.add`, `timeline.delete`, `timeline.setFrame` |
| `tool` | 13 | 4 | 13 | `tool.setColors`, `tool.swapColors`, `tool.defaultColors` |
| `variable` | 1 | 0 | 1 | `variable.list` |
| `view` | 23 | 10 | 23 | `view.newGuide`, `view.moveGuide`, `view.deleteGuide` |

### The image

- `photo_inspect` answers the canvas (`width`, `height` in pixels, `mode`, `depth`, `resolution`),
  the **layers** (top layer first), the channels, the selection state, and a preview.
- Each layer has an `id`, a `name`, a `kind` (`Pixel`, `Type`, `Smart`, `Group`, `Adjustment`, …),
  `visible`, `opacity`, `blend` and `bounds` as `[x, y, width, height]` in pixels. A type layer adds
  `text: { text, font, sizePt }`.
- Many commands act on the **selected layer** and the **selection** (`hasSelection`), as in
  Photoshop.

### Addressing

- **Pixels from the top-left of the canvas**, x right, y down. `select.rect { x, y, width, height }`
  makes a rectangular selection; `text.add { x, y, … }` places text with `y` its baseline.
- **Layer ids are valid only in the session that answered them.** A layer's `id` can change when the
  image is opened again, so the same layer may be `7` in one session and `3` in the next. Read ids
  from `photo_inspect` or `photo_check` in the same session before you use them, and never reuse an
  id remembered from an earlier conversation.
- `photo_check` targets are `{ "path": "layer:<id>" }`.

## Text writing format

The engine reads no text writing format; a model writes through the commands above.

## Defaults

What a new, empty document starts with:

- File name `Untitled photo.psd`, size 1920 × 1080 (`kit.json` `creates`).

What `photo_inspect` reports for a new document made at that size (generated from the engine; lists shortened to 20 entries):

```json
{
  "activeLayer": 2,
  "canRedo": false,
  "canUndo": false,
  "channels": {
    "alpha": [],
    "colors": [
      {
        "name": "Red",
        "visible": true
      },
      {
        "name": "Green",
        "visible": true
      },
      {
        "name": "Blue",
        "visible": true
      }
    ],
    "composite": "RGB",
    "compositeVisible": true,
    "layerMaskView": null,
    "quickMask": null,
    "target": {
      "kind": "composite"
    }
  },
  "depth": 8,
  "effectsCollapsed": [],
  "hasSelection": false,
  "height": 1080,
  "history": [
    "Open"
  ],
  "historyIndex": 0,
  "historyRedo": [],
  "isolatedLayers": [],
  "lastAppliedComp": null,
  "layerComps": [],
  "layerMaskView": null,
  "layers": [
    {
      "blend": "Normal",
      "bounds": [
        0,
        0,
        1920,
        1080
      ],
      "clipped": false,
      "fill": 1,
      "hasMask": false,
      "id": 2,
      "kind": "Pixel",
      "labelColor": "none",
      "linkGroup": null,
      "locks": {
        "all": false,
        "artboard": false,
        "pixels": false,
        "position": true,
        "transparency": true
      },
      "maskEnabled": null,
      "maskLinked": null,
      "name": "Background",
      "opacity": 1,
      "selected": true,
      "visible": true
    }
  ],
  "measurement": {
    "count": 0,
    "countGroups": 0,
    "ruler": null,
    "scale": "1 pixels = 1.0000 pixels"
  },
  "mode": "Rgb",
  "name": "Untitled photo.psd",
  "notes": [],
  "quickMask": false,
  "resolution": 72,
  "revision": 1,
  "selectedLayers": [
    2
  ],
  "selectionBounds": null,
  "symmetry": null,
  "timeline": null,
  "variables": null,
  "width": 1920
}
```

## Themes, layouts and other named values

Parameters that take one of a fixed list of names (themes, layouts, presets, styles, kinds, modes,
units and similar), from the command schemas. A parameter not listed here takes free values; its
schema says which.

| Parameter | Values | Used by |
| --- | --- | --- |
| `anchor` | `topLeft`, `top`, `topRight`, `left`, `center`, `right`, `bottomLeft`, `bottom`, `bottomRight` | `image.canvasSize` |
| `blend` | `normal`, `darken`, `lighten`, `difference` | `cloneSource.overlay` |
| `brushType` | `simple`, `lightRough`, `darkRough`, `wideSharp`, `wideBlurry`, `sparkle` | `filter.gallery.paintDaubs` |
| `caps` | `normal`, `small`, `all` | `text.setStyle` |
| `depthMode` | `random`, `levelBased` | `filter.stylize.extrude` |
| `direction` | `up`, `down` | `layer.layerStyle.bevelEmboss` |
| `direction` | `fromRight`, `fromLeft` | `filter.stylize.wind` |
| `direction` | `auto`, `ltr`, `rtl` | `text.setStyle` |
| `direction` | `cw`, `ccw` | `image.rotation.arbitrary` |
| `fill` | `background`, `foreground`, `inverse`, `unaltered` | `filter.stylize.tiles` |
| `fill` | `black`, `white`, `selection` | `channel.new` |
| `fillRule` | `nonzero`, `evenodd` | `shape.edit` |
| `flameShape` | `parallel`, `toCenter`, `spread`, `oval`, `pointed` | `filter.render.flame` |
| `flameStyle` | `normal`, `violent`, `flat` | `filter.render.flame` |
| `flameType` | `oneFlameAlongPath`, `multipleFlamesAlongPath`, `multipleFlamesPathDirections`, `multipleFlamesVariousLength`, `candleLight`, `multipleFlamesOneDirection` | `filter.render.flame` |
| `format` | `aco`, `ase` | `swatch.import` |
| `format` | `same`, `jpg`, `png`, `tif`, `psd` | `file.automate.lensCorrection` |
| `format` | `gif`, `png8`, `png24`, `jpeg`, `wbmp` | `file.export.saveForWebLegacy` |
| `format` | `kphoto`, `psd`, `psb` | `file.package` |
| `format` | `png`, `jpg`, `tiff`, `webp` | `file.exportImage` |
| `grainType` | `regular`, `soft`, `sprinkles`, `clumped`, `contrasty`, `enlarged`, `stippled`, `horizontal`, `vertical`, `speckle` | `filter.gallery.grain` |
| `inputMode` | `rgb`, `hsb`, `hsl` | `filter.other.hsbHsl` |
| `kind` | `color`, `opacity` | `gradient.fill.stop`, `gradient.presets.stop` |
| `kind` | `dropShadow`, `innerShadow`, `outerGlow`, `innerGlow`, `stroke`, `colorOverlay`, `gradientOverlay`, `patternOverlay`, `satin`, `bevelEmboss` | `layer.layerStyle.replace` |
| `kind` | `stroke`, `dropShadow`, `innerShadow`, `outerGlow`, `innerGlow`, `colorOverlay`, `gradientOverlay`, `patternOverlay`, `satin`, `bevelEmboss` | `layer.layerStyle.makeDefault` |
| `kind` | `rect`, `roundedRect`, `ellipse`, `polygon`, `star`, `line`, `path` | `shape.add` |
| `kind` | `process`, `global`, `spot` | `swatch.add` |
| `kind` | `brushes`, `customShapes`, `patterns` | `edit.presets.presetManager` |
| `kind` | `image`, `noImage`, `table` | `slice.set` |
| `layout` | `auto`, `perspective`, `cylindrical`, `spherical`, `collage`, `reposition` | `file.automate.photomerge` |
| `lightType` | `spot`, `point`, `infinite` | `filter.render.lightingEffects` |
| `mode` | `replace`, `add`, `subtract`, `intersect` | `path.toSelection`, `select.colorRange`, `select.focusArea`, `select.lasso` and 6 more |
| `mode` | `rigid`, `normal`, `distort` | `edit.pinWarp`, `layer.objectLayers.pinWarp` |
| `mode` | `rgb`, `gray`, `cmyk`, `lab` | `file.presets.save`, `file.automate.contactSheetII` |
| `mode` | `rgb`, `gray`, `grayscale`, `cmyk`, `lab` | `file.new` |
| `mode` | `replace`, `toggle`, `range`, `add` | `layer.select` |
| `mode` | `normal`, `horizontalOnly`, `verticalOnly` | `filter.distort.spherize` |
| `mode` | `rectangularToPolar`, `polarToRectangular` | `filter.distort.polarCoordinates` |
| `mode` | `normal`, `darkenOnly`, `lightenOnly`, `anisotropic` | `filter.stylize.diffuse` |
| `mode` | `normal`, `edgeOnly`, `overlayEdge` | `filter.blur.smartBlur` |
| `mode` | `add`, `subtract`, `replace` | `select.quick` |
| `mode` | `vertical`, `horizontal`, `dual`, `diagonal` | `paint.setSymmetry` |
| `mode` | `hue`, `saturation`, `color`, `luminosity` | `paint.colorReplacement` |
| `mode` | `append`, `replace` | `swatch.import` |
| `mode` | `source`, `destination` | `paint.patch` |
| `mode` | `move`, `extend` | `paint.smartMove` |
| `mode` | `desaturate`, `saturate` | `paint.sponge` |
| `mode` | `normal`, `multiply`, `screen`, `overlay`, `softLight`, `hardLight`, `darken`, `lighten`, `difference`, `color`, `luminosity` | `edit.fade` |
| `mode` | `32`, `16`, `8` | `file.automate.mergeToHdrPro` |
| `mode` | `rgb`, `cmyk`, `lab` | `channel.merge` |
| `mode` | `mean`, `median`, `maximum`, `minimum`, `range`, `summation`, `variance`, `standardDeviation`, `skewness`, `kurtosis`, `entropy` | `file.scripts.statistics` |
| `mode` | `replace`, `toggle`, `add` | `layer.pickAt` |
| `mode` | `off`, `gray`, `overlay`, `toggleGray`, `toggleOverlay` | `view.layerMask` |
| `model` | `auto`, `fisheye`, `perspective`, `fullSpherical` | `filter.adaptiveWideAngle` |
| `patternType` | `dot`, `circle`, `line` | `filter.gallery.halftonePattern` |
| `preset` | `default`, `custom` | `image.analysis.setMeasurementScale` |
| `quickExportFormat` | `png`, `jpg`, `gif`, `webp` | `file.export.exportPreferences` |
| `resolutionUnit` | `in`, `cm` | `file.presets.save` |
| `shape` | `hexagon`, `triangle`, `square`, `pentagon`, `heptagon`, `octagon` | `filter.blur.lensBlur` |
| `shape` | `circle`, `ring`, `square`, `diamond`, `triangle`, `hexagon`, `star`, `heart`, `cross` | `filter.blur.shapeBlur` |
| `shape` | `round`, `ellipse`, `line`, `square`, `diamond`, `cross` | `image.mode.bitmap` |
| `strokeDirection` | `rightDiagonal`, `horizontal`, `leftDiagonal`, `vertical` | `filter.gallery.graphicPen`, `filter.gallery.sprayedStrokes` |
| `style` | `linear`, `radial`, `angle`, `reflected`, `diamond` | `gradient.fill.add`, `gradient.fill.set`, `gradient.presets.apply`, `layer.layerStyle.gradientOverlay` and 2 more |
| `style` | `custom`, `none`, `arc`, `arcLower`, `arcUpper`, `arch`, `bulge`, `shellLower`, `shellUpper`, `flag`, `wave`, `fish`, `rise`, `fisheye`, `inflate`, `squeeze`, `twist` | `edit.transform.warp`, `layer.objectLayers.warp` |
| `style` | `inner`, `outer`, `emboss`, `pillow`, `stroke` | `layer.layerStyle.bevelEmboss` |
| `style` | `pondRipples`, `outFromCenter`, `aroundCenter` | `filter.distort.zigZag` |
| `style` | `none`, `arc`, `arcLower`, `arcUpper`, `arch`, `bulge`, `shellLower`, `shellUpper`, `flag`, `wave`, `fish`, `rise`, `fisheye`, `inflate`, `squeeze`, `twist` | `text.warpText` |
| `type` | `sine`, `triangle`, `square` | `filter.distort.wave` |
| `type` | `fineDots`, `mediumDots`, `grainyDots`, `coarseDots`, `shortLines`, `mediumLines`, `longLines`, `shortStrokes`, `mediumStrokes`, `longStrokes` | `filter.pixelate.mezzotint` |
| `type` | `blocks`, `pyramids` | `filter.stylize.extrude` |
| `type` | `smart`, `createTexture`, `proximityMatch` | `paint.spotHealing` |
| `type` | `duotone`, `monotone`, `tritone`, `quadtone` | `image.mode.duotone` |
| `type` | `visibility`, `textReplacement`, `pixelReplacement` | `image.variables.define` |
| `unit` | `px`, `in`, `cm`, `mm`, `pt`, `pica` | `file.presets.save` |
| `units` | `inches`, `cm`, `mm`, `pixels` | `file.automate.contactSheetII` |
| `vignetteStyle` | `highlightPriority`, `colorPriority`, `paintOverlay` | `filter.rawDevelop` |

## Examples

### Example 1: add a red badge and a caption

On a 320 × 200 image with one `Background` layer:

```json
photo_run {
  "path": "poster.psd",
  "steps": [
    { "command": "layer.new.layer", "params": { "name": "Badge" } },
    { "command": "select.rect", "params": { "x": 220, "y": 120, "width": 60, "height": 40 } },
    { "command": "edit.fill", "params": { "color": "#c0392b" } },
    { "command": "edit.deselectAll", "params": {} },
    { "command": "text.add", "params": { "x": 40, "y": 170, "text": "Summer sale", "size": 20, "color": "#000000" } }
  ]
}
```

It answers `"layer": 3` for the new layer and `"layer": 4, "bounds": [41, 155, 162, 171]` for the
text, and the summary becomes `3 layers`. `photo_inspect` then lists `Summer sale` (Type, Inter
20 pt), `Badge` (Pixel, bounds `[220, 120, 60, 40]`) and `Background`. Export it:

```json
photo_export { "path": "poster.psd", "format": "png", "to": "poster.png" }
```

It answers `Exported poster.psd as PNG to poster.png` with `"files": [{ "path": "poster.png", "created": true }]`.

### Example 2: show a hidden layer the check reports

```json
photo_check { "path": "hidden.psd" }
```

It answers `{ "code": "photo.hidden-layer", "message": "Layer \"Badge\" is hidden: …",
"target": { "path": "layer:3" }, "fix": { "command": "layer.showLayers", "params": { "layer": 3 } } }`.
In the same session, run the fix:

```json
photo_run { "path": "hidden.psd", "steps": [{ "command": "layer.showLayers", "params": { "layer": 3 } }] }
```

`photo_check` then answers `the kit found nothing wrong`.

## Workflow

1. `photo_inspect` for the canvas, the layers and a preview.
2. Find commands with specific words: `photo_commands { "query": "new layer" }`,
   `"rectangular selection"`, `"fill"`, `"text"`, `"hue saturation"`. Follow each schema; don't
   invent ids.
3. `photo_run` short batches: select → act → deselect. A batch saves when it succeeds.
4. `photo_check`, then fix what it reports; most diagnostics carry a `fix` with the layer id.
5. To hand over a flat image, `photo_export { "format": "png", "to": "name.png" }`: the image is
   written as a **new** file and the layers are kept.

## Pitfalls

- **Select first.** `edit.fill` fills the selection, or the whole layer when nothing is selected.
  Deselect (`edit.deselectAll`) after the fill so the next command doesn't act on it.
- **New layers start empty.** `photo.empty-layer` (info) reports one with no pixels; fill it, or
  delete it with the diagnostic's fix (`layer.delete`).
- **Hidden layers** are saved but not shown (`photo.hidden-layer`, fix `layer.showLayers`). Show one
  only when the person wants it visible.
- **Off the canvas:** `photo.off-canvas` (warning) is a layer moved entirely outside the canvas.
- **Text:** `text.add` uses Inter unless you give `font`. A font that isn't installed here is
  drawn with a substitute (`photo.font-substituted`, info); the file keeps the font name.
- **Object layers** linked to a missing file show nothing (`photo.missing-object-source`, error);
  embedding or relinking needs the person's file.
- **Exports** need `to`, a new workspace path whose extension matches `format` (`png`, `jpg`,
  `tiff`, `webp`). An existing file is never replaced.
