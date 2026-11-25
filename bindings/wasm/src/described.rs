//! What each command does and what each read command answers, for the model choosing a command
//! (`KitCommand.description`, `output_schema`; kition-sdk `testing::check_commands`), written from
//! each command's implementation: what it does, when to use it rather than its neighbours, its
//! preconditions, and what a read command answers.
use kition_kit::KitCommand;
use serde_json::Value;

/// Read commands that answer nothing a model reads.
pub const NO_OUTPUT: &[(&str, &str)] = &[
    ("edit.selectAll", "changes the selection, colors or session state and answers nothing to read"),
    ("tool.setColors", "changes the selection, colors or session state and answers nothing to read"),
    ("tool.swapColors", "changes the selection, colors or session state and answers nothing to read"),
    ("tool.defaultColors", "changes the selection, colors or session state and answers nothing to read"),
    ("select.deselectLayers", "changes the selection, colors or session state and answers nothing to read"),
];

/// Sets each command's description and, for read commands, its output schema.
pub fn fill(commands: &mut [KitCommand]) {
    for command in commands {
        if let Some(Value::Object(properties)) = command.schema.as_mut() {
            for (key, node) in properties.iter_mut() {
                if node.get("type").and_then(Value::as_str) == Some("json") {
                    type_key(key, node);
                }
            }
        }
        command.description = describe(&command.id).to_owned();
        if command.output_schema.is_none() && !command.mutates {
            command.output_schema = output(&command.id);
        }
    }
}

fn describe(id: &str) -> &'static str {
    match id {
        "file.new" => "Creates a new document of `width` × `height` pixels (resolution, color mode, background) and makes it the active one.",
        "file.close" => "Closes the active document, or document `document` (index from 0).",
        "file.render" => {
            "Draws an explicit view of the image as a PNG, without the editor's state: the composite, or only the given layers (even hidden), framed to a region; answers how its pixels map to image coordinates and what it drew. Changes nothing."
        }
        "image.measure" => {
            "Answers what can be told exactly beside the pixels: a layer's (or the composite's) non-transparent bounds, pixel count, mean color and luminosity histogram inside a region, and how much it overlaps another layer (`with`). Changes nothing."
        }
        "edit.undo" => "Takes back the caller's last edit (the person's, or the agent's with `scope: \"agent\"`), leaving the other's edits in place.",
        "edit.redo" => "Applies again the edit the caller's last undo took back, in the same scope as `edit.undo`.",
        "edit.fill" => {
            "Fills the selection (or the whole active layer) with the foreground, background, a color, a pattern or smart fill, with blend mode and opacity."
        }
        "edit.delete" => "Clears the selected pixels of the active layer to transparency (or the background color on a background layer).",
        "edit.selectAll" => "Selects the whole canvas.",
        "edit.deselectAll" => "Removes the selection; needs one.",
        "select.inverse" => "Inverts the selection; needs one.",
        "select.rect" => {
            "Makes a rectangular selection at `x`, `y`, `width`, `height` in pixels, replacing, adding to, subtracting from or intersecting the current one."
        }
        "layer.new.layer" => {
            "Adds a new empty pixel layer above the active layer (at the top inside it when the active layer is a group) and makes it active; answers its id."
        }
        "layer.new.group" => {
            "Adds a new empty layer group above the active layer (inside it when that is a group) and makes it active, so the next new layer goes into it; answers its id."
        }
        "layer.groupLayers" => "Puts `layer` (else every selected layer) into a new group.",
        "layer.duplicate" => "Duplicates `layer` (else the active layer) above itself, named `name` (default \"<name> copy\"), and answers the copy's id.",
        "layer.delete" => "Deletes `layer`, or every selected layer without it.",
        "layer.select" => {
            "Selects layer `layer` (id from file.inspect), replacing the layer selection or adding, toggling or extending a range; commands without `layer` act on it."
        }
        "layer.setProps" => {
            "Sets a layer's name, visibility, opacity (0..1), fill opacity, blend mode, lock and color label, and its layer mask's density (0..1) and feather (px), as Properties › Masks does."
        }
        "arrange.bringForward" => "Bring Forward: moves `layer` (else the active layer) in the layer stack.",
        "arrange.sendBackward" => "Send Backward: moves `layer` (else the active layer) in the layer stack.",
        "arrange.bringToFront" => "Bring to Front: moves `layer` (else the active layer) in the layer stack.",
        "arrange.sendToBack" => "Send to Back: moves `layer` (else the active layer) in the layer stack.",
        "layer.addClippingMask" => {
            "Clips `layer` (else every selected layer but the lowest) to the layer below, so it shows only inside that layer's pixels; undo with layer.releaseClippingMask."
        }
        "layer.releaseClippingMask" => "Unclips `layer` (else every selected layer) from the layer below, so it shows everywhere again.",
        "layer.layerMask.revealAll" => "Layer mask, Reveal All: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.layerMask.hideAll" => "Layer mask, Hide All: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.layerMask.revealSelection" => {
            "Adds a layer mask to `layer` (else the active layer) that shows the selection and hides the rest; the selection stays."
        }
        "layer.layerMask.delete" => "Layer mask, Delete: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.mergeDown" => "Merges `layer` (else the active layer) into the layer below it.",
        "layer.flattenImage" => "Flattens every layer into one background layer.",
        "layer.newFillLayer.solidColor" => "Adds a Solid Color fill layer above the active layer (optionally masked by a path); answers its id.",
        "layer.newFillLayer.gradient" => "Adds a Gradient fill layer above the active layer (optionally masked by a path); answers its id.",
        "layer.setAdjustment" => "Changes the settings of adjustment layer `layer` (else the active one); takes that adjustment kind's parameters.",
        "layer.moveTo" => "Moves `layer` (or `layers`) above, below or into `target` in the layer stack.",
        "layer.translate" => "Moves the pixels of `layer` (else every selected layer) by `dx`, `dy` pixels.",
        "image.imageRotation.flipCanvasHorizontal" => {
            "Mirrors the whole canvas left to right, every layer included. To flip one layer use edit.transform.flipHorizontal."
        }
        "image.imageRotation.flipCanvasVertical" => "Mirrors the whole canvas top to bottom, every layer included.",
        "image.imageRotation.rotate180" => "Rotates the whole canvas and every layer by 180°.",
        "image.imageRotation.rotate90cw" => "Rotates the whole canvas and every layer 90° clockwise, swapping width and height.",
        "image.imageRotation.rotate90ccw" => "Rotates the whole canvas and every layer 90° counter-clockwise, swapping width and height.",
        "image.adjustments.desaturate" => {
            "Removes all color from the active pixel layer (inside the selection), keeping lightness; destructive and without control, unlike black & white."
        }
        "paint.stroke" => {
            "Paints a brush stroke through `points` [[x, y, pressure?…]] on the active layer with the current brush, or a preset, size, color and opacity given here."
        }
        "tool.setColors" => "Sets the foreground and/or background colors (#rrggbb) that fills and brushes use.",
        "tool.swapColors" => "Swaps the foreground and background colors.",
        "tool.defaultColors" => "Resets the foreground to black and the background to white.",
        "session.inspect" => "Answers the open documents (index, name, size, dirty), the active one and the foreground and background colors.",
        "file.inspect" => {
            "Answers the active document (or `document`): size, mode, the layer tree with ids, kinds, names and properties, channels, paths, selection and history. Start here."
        }
        "file.activate" => "Makes document `document` (index from 0) the active one.",
        "file.move" => "Moves document `document` to tab position `to`.",
        "command.list" => "Lists the kit's commands with labels, menu paths, parameters and whether they are enabled.",
        "file.pixel" => "Answers the composite color [r, g, b, a] (0..1) at pixel `x`, `y`.",
        "layer.newAdjustmentLayer.brightnessContrast" => {
            "Adds an editable Brightness/Contrast adjustment layer above the active layer, which shifts all tones lighter or darker (`brightness` −150–150) and spreads or compresses them (`contrast` −50–100); quick but coarse, Levels or Curves give control per tone; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.brightnessContrast" => {
            "Brightness/Contrast on the active pixel layer (inside the selection), destructively: shifts all tones lighter or darker (`brightness` −150–150) and spreads or compresses them (`contrast` −50–100); quick but coarse, Levels or Curves give control per tone. Use layer.newAdjustmentLayer.brightnessContrast to keep it editable."
        }
        "layer.newAdjustmentLayer.levels" => {
            "Adds an editable Levels adjustment layer above the active layer, which remaps tones by input black/white points (`inBlack`, `inWhite`), midtone `gamma` (0.01–9.99) and output range; per channel via `red`/`green`/`blue`; best for setting black and white points; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.levels" => {
            "Levels on the active pixel layer (inside the selection), destructively: remaps tones by input black/white points (`inBlack`, `inWhite`), midtone `gamma` (0.01–9.99) and output range; per channel via `red`/`green`/`blue`; best for setting black and white points. Use layer.newAdjustmentLayer.levels to keep it editable."
        }
        "layer.newAdjustmentLayer.curves" => {
            "Adds an editable Curves adjustment layer above the active layer, which remaps tones along a curve of 2–19 [in,out] points (0–255), per channel via `red`/`green`/`blue`; `eyedropper` sets a black, gray or white point; the most precise tone control; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.curves" => {
            "Curves on the active pixel layer (inside the selection), destructively: remaps tones along a curve of 2–19 [in,out] points (0–255), per channel via `red`/`green`/`blue`; `eyedropper` sets a black, gray or white point; the most precise tone control. Use layer.newAdjustmentLayer.curves to keep it editable."
        }
        "layer.newAdjustmentLayer.exposure" => {
            "Adds an editable Exposure adjustment layer above the active layer, which adjusts in linear light: `exposure` in stops (−20–20), `offset` for shadows, `gamma`; for HDR or correcting under/over-exposure; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.exposure" => {
            "Exposure on the active pixel layer (inside the selection), destructively: adjusts in linear light: `exposure` in stops (−20–20), `offset` for shadows, `gamma`; for HDR or correcting under/over-exposure. Use layer.newAdjustmentLayer.exposure to keep it editable."
        }
        "layer.newAdjustmentLayer.vibrance" => {
            "Adds an editable Vibrance adjustment layer above the active layer, which raises saturation mostly in muted colors and protects skin tones (`vibrance`), plus a uniform `saturation`; gentler than Hue/Saturation; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.vibrance" => {
            "Vibrance on the active pixel layer (inside the selection), destructively: raises saturation mostly in muted colors and protects skin tones (`vibrance`), plus a uniform `saturation`; gentler than Hue/Saturation. Use layer.newAdjustmentLayer.vibrance to keep it editable."
        }
        "layer.newAdjustmentLayer.hueSaturation" => {
            "Adds an editable Hue/Saturation adjustment layer above the active layer, which shifts `hue` (−180–180), `saturation` and `lightness` for all colors or one range (`reds` … `magentas`); `colorize` tints everything one hue; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.hueSaturation" => {
            "Hue/Saturation on the active pixel layer (inside the selection), destructively: shifts `hue` (−180–180), `saturation` and `lightness` for all colors or one range (`reds` … `magentas`); `colorize` tints everything one hue. Use layer.newAdjustmentLayer.hueSaturation to keep it editable."
        }
        "layer.newAdjustmentLayer.colorBalance" => {
            "Adds an editable Color Balance adjustment layer above the active layer, which shifts colors per tone range: `shadows`/`midtones`/`highlights` each [cyan-red, magenta-green, yellow-blue] in −100–100; for correcting a color cast; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.colorBalance" => {
            "Color Balance on the active pixel layer (inside the selection), destructively: shifts colors per tone range: `shadows`/`midtones`/`highlights` each [cyan-red, magenta-green, yellow-blue] in −100–100; for correcting a color cast. Use layer.newAdjustmentLayer.colorBalance to keep it editable."
        }
        "layer.newAdjustmentLayer.blackWhite" => {
            "Adds an editable Black & White adjustment layer above the active layer, which converts to grayscale, setting how light each color range becomes (`reds` … `magentas`, −200–300 %); optional `tint`; more control than desaturate; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.blackWhite" => {
            "Black & White on the active pixel layer (inside the selection), destructively: converts to grayscale, setting how light each color range becomes (`reds` … `magentas`, −200–300 %); optional `tint`; more control than desaturate. Use layer.newAdjustmentLayer.blackWhite to keep it editable."
        }
        "layer.newAdjustmentLayer.photoFilter" => {
            "Adds an editable Photo Filter adjustment layer above the active layer, which tints like a lens filter (`filter` preset or `color`) at `density` %; warming or cooling; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.photoFilter" => {
            "Photo Filter on the active pixel layer (inside the selection), destructively: tints like a lens filter (`filter` preset or `color`) at `density` %; warming or cooling. Use layer.newAdjustmentLayer.photoFilter to keep it editable."
        }
        "layer.newAdjustmentLayer.channelMixer" => {
            "Adds an editable Channel Mixer adjustment layer above the active layer, which rebuilds each output channel from percentages of red, green and blue plus a constant (−200–200); `monochrome` mixes one gray channel; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.channelMixer" => {
            "Channel Mixer on the active pixel layer (inside the selection), destructively: rebuilds each output channel from percentages of red, green and blue plus a constant (−200–200); `monochrome` mixes one gray channel. Use layer.newAdjustmentLayer.channelMixer to keep it editable."
        }
        "layer.newAdjustmentLayer.invert" => {
            "Adds an editable Invert adjustment layer above the active layer, which turns every channel to its negative; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.invert" => {
            "Invert on the active pixel layer (inside the selection), destructively: turns every channel to its negative. Use layer.newAdjustmentLayer.invert to keep it editable."
        }
        "layer.newAdjustmentLayer.posterize" => {
            "Adds an editable Posterize adjustment layer above the active layer, which reduces each channel to `levels` tone steps (2–255), giving flat bands; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.posterize" => {
            "Posterize on the active pixel layer (inside the selection), destructively: reduces each channel to `levels` tone steps (2–255), giving flat bands. Use layer.newAdjustmentLayer.posterize to keep it editable."
        }
        "layer.newAdjustmentLayer.threshold" => {
            "Adds an editable Threshold adjustment layer above the active layer, which turns pixels white above `level` (1–255) and black below; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.threshold" => {
            "Threshold on the active pixel layer (inside the selection), destructively: turns pixels white above `level` (1–255) and black below. Use layer.newAdjustmentLayer.threshold to keep it editable."
        }
        "layer.newAdjustmentLayer.gradientMap" => {
            "Adds an editable Gradient Map adjustment layer above the active layer, which maps luminance onto a gradient of `stops` [[location 0–1, \"#rrggbb\"], …] (shadows get the first color, highlights the last); masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.gradientMap" => {
            "Gradient Map on the active pixel layer (inside the selection), destructively: maps luminance onto a gradient of `stops` [[location 0–1, \"#rrggbb\"], …] (shadows get the first color, highlights the last). Use layer.newAdjustmentLayer.gradientMap to keep it editable."
        }
        "layer.newAdjustmentLayer.selectiveColor" => {
            "Adds an editable Selective Color adjustment layer above the active layer, which adds or removes cyan, magenta, yellow and black (−100–100 %) per color range (`reds`, `yellows`, `greens`, `cyans`, `blues`, `magentas`, `whites`, `neutrals`, `blacks`, each [c, m, y, k]; or one range named by `colors` with `cyan` … `black`), relative or absolute; for print-style color tweaks; masked by `path` or the selection; edit with layer.setAdjustment; answers its id."
        }
        "image.adjustments.selectiveColor" => {
            "Selective Color on the active pixel layer (inside the selection), destructively: adds or removes cyan, magenta, yellow and black (−100–100 %) per color range (`reds`, `yellows`, `greens`, `cyans`, `blues`, `magentas`, `whites`, `neutrals`, `blacks`, each [c, m, y, k]; or one range named by `colors` with `cyan` … `black`), relative or absolute; for print-style color tweaks. Use layer.newAdjustmentLayer.selectiveColor to keep it editable."
        }
        "layer.newAdjustmentLayer.colorLookup" => {
            "Adds a Color Lookup adjustment layer above the active layer (optionally masked by a path), which stays editable with `layer.setAdjustment`; answers its id."
        }
        "image.adjustments.colorLookup" => {
            "Applies Color Lookup to the active layer's pixels (inside the selection if there is one), destructively; use `layer.newAdjustmentLayer.colorLookup` to keep it editable (file: .cube/.3dl/.look path; data: file text + \"fileName\")."
        }
        "layer.layerStyle.dropShadow" => {
            "Sets the Drop Shadow layer effect on the active layer (or `layer`): color, opacity, blend, angle, useGlobalLight, distance."
        }
        "layer.layerStyle.innerShadow" => "Sets the Inner Shadow layer effect on the active layer (or `layer`): color, opacity, blend, angle, distance, choke.",
        "layer.layerStyle.outerGlow" => "Sets the Outer Glow layer effect on the active layer (or `layer`): color, opacity, blend, technique, spread, size.",
        "layer.layerStyle.innerGlow" => "Sets the Inner Glow layer effect on the active layer (or `layer`): color, opacity, blend, technique, source, choke.",
        "layer.layerStyle.stroke" => "Sets the Stroke layer effect on the active layer (or `layer`): size, position, color, from, to, style.",
        "layer.layerStyle.colorOverlay" => "Sets the Color Overlay layer effect on the active layer (or `layer`): color, opacity, blend, add.",
        "layer.layerStyle.gradientOverlay" => {
            "Sets the Gradient Overlay layer effect on the active layer (or `layer`): from, to, style, angle, scale, reverse."
        }
        "layer.layerStyle.patternOverlay" => {
            "Sets the Pattern Overlay layer effect on the active layer (or `layer`): pattern, opacity, blend, scale, angle, link."
        }
        "layer.layerStyle.bevelEmboss" => {
            "Sets the Bevel & Emboss layer effect on the active layer (or `layer`): style, technique, contour, contourRange, texture, textureScale."
        }
        "layer.layerStyle.satin" => "Sets the Satin layer effect on the active layer (or `layer`): color, opacity, blend, angle, distance, size.",
        "layer.layerStyle.replace" => {
            "Replaces the whole layer style of `layer` (the active layer by default) with `effects`, a list of {kind, params, fx?} in order: an entry with `fx` edits that effect with the params present, one without builds the effect from params (the layer.layerStyle.<kind> params). The Layer Style dialog's OK; to set one effect use layer.layerStyle.<kind>."
        }
        "layer.layerStyle.makeDefault" => {
            "Stores `params` as the user default for the layer effect `kind` (the Layer Style dialog's Make Default), which Reset to Default and layer.layerStyle.defaultFor return; changes no layer."
        }
        "layer.layerStyle.get" => {
            "Answers the layer style of `layer` (the active layer by default): whether its effects show, and each effect in order with its kind, label, whether it is on, its settings in the layer.layerStyle.<kind> params' units (`params`) and its snapshot (`fx`, to send back through layer.layerStyle.replace so settings no parameter names survive). Changes nothing."
        }
        "layer.layerStyle.defaultFor" => {
            "Answers the default settings of the layer effect `kind` ({params, user}): the user default saved by layer.layerStyle.makeDefault, else the factory defaults (`user` false); changes nothing."
        }
        "layer.layerStyle.clear" => {
            "Removes every layer effect from `layer` (the active layer by default), as Layer › Layer Style › Clear Layer Style; blending options stay."
        }
        "filter.blur.gaussianBlur" => {
            "Smooths (active pixel layer, inside the selection if any) with a Gaussian of `radius` px (0.1–1000); the general-purpose blur. Use surfaceBlur to keep edges, boxBlur for a flat average, motionBlur for streaks."
        }
        "filter.blur.blur" => {
            "Applies a fixed, very light 3×3 smoothing (active pixel layer, inside the selection if any); no settings. Use gaussianBlur when you need a chosen strength."
        }
        "filter.blur.blurMore" => {
            "Applies a fixed light smoothing about three times stronger than filter.blur.blur (active pixel layer, inside the selection if any); no settings."
        }
        "filter.sharpen.sharpen" => {
            "Applies a fixed, light sharpening (active pixel layer, inside the selection if any); no settings. Use unsharpMask or smartSharpen to control amount and radius."
        }
        "filter.sharpen.sharpenMore" => {
            "Applies a fixed sharpening stronger than filter.sharpen.sharpen (active pixel layer, inside the selection if any); no settings."
        }
        "filter.sharpen.sharpenEdges" => {
            "Sharpens only where contrast is high (edges), leaving smooth areas (active pixel layer, inside the selection if any); no settings."
        }
        "filter.noise.despeckle" => {
            "Softens noise in flat areas while keeping edges (active pixel layer, inside the selection if any); no settings. Use reduceNoise for control, median or dustAndScratches for specks."
        }
        "filter.blur.boxBlur" => {
            "Averages each pixel with its neighbours in a square of `radius` px (1–2000) (active pixel layer, inside the selection if any); flatter and blockier than gaussianBlur."
        }
        "filter.blur.motionBlur" => {
            "Streaks (active pixel layer, inside the selection if any) along `angle` degrees over `distance` px (1–2000), as if the camera moved."
        }
        "filter.blur.radialBlur" => {
            "Blurs (active pixel layer, inside the selection if any) around the point `centerX`,`centerY` (0–1 of the canvas): `spin` turns around it, `zoom` streaks out from it; `amount` 1–100."
        }
        "filter.blur.surfaceBlur" => {
            "Blurs (active pixel layer, inside the selection if any) while keeping edges: pixels differing by more than `threshold` levels (2–255) are not mixed; `radius` 1–100 px. Good for smoothing skin or flat surfaces."
        }
        "filter.sharpen.unsharpMask" => {
            "Sharpens (active pixel layer, inside the selection if any) by boosting edge contrast: `amount` % (1–500), `radius` px of the halo, `threshold` levels below which nothing is sharpened (protects noise and skin)."
        }
        "filter.sharpen.smartSharpen" => {
            "Sharpens (active pixel layer, inside the selection if any) with fewer halos than unsharpMask: `amount` % (1–500), `radius` px (0.1–64), and `reduceNoise` % to keep grain from being sharpened."
        }
        "filter.noise.addNoise" => {
            "Adds random grain (active pixel layer, inside the selection if any): `amount` % (0.1–400), `distribution` uniform or gaussian, `monochromatic` for gray grain; `seed` repeats the same pattern."
        }
        "filter.noise.median" => {
            "Replaces each pixel with the median of its neighbours within `radius` px (active pixel layer, inside the selection if any); removes specks and flattens texture."
        }
        "filter.noise.dustAndScratches" => {
            "Removes small specks and scratches (active pixel layer, inside the selection if any): pixels differing by more than `threshold` levels within `radius` px are replaced by their surroundings."
        }
        "filter.pixelate.mosaic" => {
            "Turns (active pixel layer, inside the selection if any) into square blocks of `cellSize` px (2–200), each its average color; for pixelating or censoring."
        }
        "filter.stylize.emboss" => {
            "Turns (active pixel layer, inside the selection if any) gray with raised edges lit from `angle` degrees; `height` 1–100 px, `amount` % of edge contrast."
        }
        "filter.stylize.findEdges" => "Outlines edges as dark lines on white (active pixel layer, inside the selection if any); no settings.",
        "filter.stylize.solarize" => {
            "Inverts the tones brighter than mid-gray, like an overexposed photo (active pixel layer, inside the selection if any); no settings."
        }
        "filter.distort.twirl" => {
            "Twists (active pixel layer, inside the selection if any) around its center by `angle` degrees (−999–999); more at the center than the edge."
        }
        "filter.distort.pinch" => {
            "Squeezes the center inward (`amount` > 0) or pushes it out (< 0) (active pixel layer, inside the selection if any); −100–100 %."
        }
        "filter.distort.spherize" => {
            "Wraps (active pixel layer, inside the selection if any) around a sphere, bulging (`amount` > 0) or hollowing (< 0); `mode` limits it to one axis."
        }
        "filter.distort.wave" => {
            "Displaces (active pixel layer, inside the selection if any) with summed waves: `generators` count, wavelength and amplitude ranges in px, `type` sine/triangle/square; `undefinedAreas` wrap or repeat edge pixels."
        }
        "filter.distort.ripple" => "Adds water-like ripples (active pixel layer, inside the selection if any): `amount` −999–999, `size` small/medium/large.",
        "filter.distort.polarCoordinates" => {
            "Remaps (active pixel layer, inside the selection if any) between rectangular and polar coordinates (`mode`); e.g. turns a horizon into a planet."
        }
        "filter.other.highPass" => {
            "Keeps only detail finer than `radius` px (0.1–1000) and turns the rest mid-gray (active pixel layer, inside the selection if any); set the layer to Overlay afterwards to sharpen."
        }
        "filter.other.minimum" => {
            "Spreads dark areas: each pixel takes the darkest value within `radius` px (active pixel layer, inside the selection if any); shrinks white shapes and masks. `preserve` squareness or roundness."
        }
        "filter.other.maximum" => {
            "Spreads light areas: each pixel takes the lightest value within `radius` px (active pixel layer, inside the selection if any); grows white shapes and masks. `preserve` squareness or roundness."
        }
        "filter.other.offset" => {
            "Shifts (active pixel layer, inside the selection if any) by `horizontal`,`vertical` px; uncovered areas wrap around, repeat the edge, or become transparent (`undefinedAreas`). Used to make tiles seamless."
        }
        "filter.lastFilter" => "Applies the last filter again with the same settings to the active layer; fails if no filter has run in this session.",
        "filter.pixelate.colorHalftone" => {
            "Simulates print halftone dots per channel (active pixel layer, inside the selection if any): dot size up to `maxRadius` px (4–127), screen angles `channel1`–`channel4` in degrees."
        }
        "filter.pixelate.crystallize" => {
            "Breaks (active pixel layer, inside the selection if any) into flat-colored polygon cells of about `cellSize` px (3–300); `seed` fixes the pattern."
        }
        "filter.pixelate.facet" => {
            "Clumps similar neighbouring colors into flat patches, like a hand-painted look (active pixel layer, inside the selection if any); no settings."
        }
        "filter.pixelate.fragment" => {
            "Overlays four offset copies, giving a shaken, doubled look (active pixel layer, inside the selection if any); no settings."
        }
        "filter.pixelate.mezzotint" => {
            "Converts (active pixel layer, inside the selection if any) into a random pattern of dots, lines or strokes (`type`) at full saturation; `seed` fixes it."
        }
        "filter.pixelate.pointillize" => {
            "Paints (active pixel layer, inside the selection if any) as random colored dots of about `cellSize` px (3–300) on the `background` color."
        }
        "filter.stylize.diffuse" => {
            "Scatters pixels randomly to soften focus (active pixel layer, inside the selection if any); `mode` normal, darkenOnly, lightenOnly or anisotropic."
        }
        "filter.stylize.extrude" => {
            "Pushes (active pixel layer, inside the selection if any) out as 3D blocks or pyramids (`type`) of `size` px and `depth` 1–255, random or brightness-based."
        }
        "filter.stylize.oilPaint" => {
            "Restyles (active pixel layer, inside the selection if any) as brush strokes: `stylization` and `scale` set stroke length and size, `cleanliness` smoothness, `bristleDetail`, and optional lighting at `angle` with `shine`."
        }
        "filter.stylize.tiles" => {
            "Splits (active pixel layer, inside the selection if any) into `count` tiles per side, each shifted up to `maxOffset` %, the gaps filled per `fill`."
        }
        "filter.stylize.traceContour" => {
            "Draws thin lines where brightness crosses `level` (0–255) (active pixel layer, inside the selection if any), per channel, on white; `edge` lower or upper."
        }
        "filter.stylize.wind" => {
            "Adds horizontal streaks from bright edges, as if blown (`method` wind, blast, stagger; `direction` from left or right) (active pixel layer, inside the selection if any)."
        }
        "filter.distort.displace" => {
            "Moves (active pixel layer, inside the selection if any) by a displacement map (`mapPath` file, or `mapDocument`/`mapLayer`): mid-gray stays, lighter and darker shift by up to `horizontal`/`vertical` %."
        }
        "filter.distort.shear" => {
            "Bends (active pixel layer, inside the selection if any) sideways along a curve given by `points` (or `amount`); `undefinedAreas` wrap or repeat."
        }
        "filter.distort.zigZag" => {
            "Distorts (active pixel layer, inside the selection if any) radially like ripples on a pond: `amount` −100–100, `ridges` 0–20, `style`."
        }
        "filter.render.fibers" => {
            "Replaces (active pixel layer, inside the selection if any) with a fiber texture in the foreground and background colors; `variance` and `strength` 1–64."
        }
        "filter.render.lensFlare" => {
            "Adds a bright lens flare at `centerX`,`centerY` (0–1) (active pixel layer, inside the selection if any); `brightness` 10–300 %, `lens` type."
        }
        "filter.render.lightingEffects" => {
            "Lights (active pixel layer, inside the selection if any) with a spot or directional light (position, `angle`, `elevation`, `cone`) plus `ambience`, `gloss` and `exposure`; can emboss from a texture channel."
        }
        "filter.render.relight" => {
            "Adds directional light (active pixel layer, inside the selection if any) from `angle` and `elevation`: `intensity`, `softness`, `ambient` and `warmth` (−100 cool to 100 warm). Simpler than lightingEffects."
        }
        "filter.noise.reduceNoise" => {
            "Reduces noise (active pixel layer, inside the selection if any): `strength` 0–10, `preserveDetails` %, `reduceColorNoise` %, `sharpenDetails` %, optionally `removeJpegArtifact`."
        }
        "filter.blur.smartBlur" => {
            "Blurs (active pixel layer, inside the selection if any) only where pixels differ by less than `threshold`, keeping edges; `mode` edgeOnly or overlayEdge draws the edges instead."
        }
        "filter.blur.lensBlur" => {
            "Imitates a camera's shallow depth of field (active pixel layer, inside the selection if any): blur from a `depthMap` (transparency or layer mask) around `focalDistance`, iris shape, specular highlights and noise."
        }
        "filter.blur.shapeBlur" => "Blurs (active pixel layer, inside the selection if any) with a kernel of the chosen `shape` and `radius` px (5–1000).",
        "filter.blurGallery.tiltShift" => {
            "Fakes a miniature (tilt-shift) look (active pixel layer, inside the selection if any): a sharp band at `centerX`,`centerY` rotated by `angle`, blurred by `blur` px outside it."
        }
        "filter.blurGallery.irisBlur" => {
            "Keeps an ellipse sharp and blurs outside it (active pixel layer, inside the selection if any): center, `radiusX`/`radiusY`, `angle`, `feather` and `blur` px; `pins` adds more."
        }
        "filter.blurGallery.fieldBlur" => {
            "Blurs (active pixel layer, inside the selection if any) by `blur` px with a gradient between `pins`, each with its own amount; a blur that varies across the image."
        }
        "filter.blurGallery.spinBlur" => {
            "Blurs in circles around an ellipse center, like a spinning wheel (active pixel layer, inside the selection if any); `blurAngle` in degrees, `pins` add more."
        }
        "filter.blurGallery.pathBlur" => {
            "Blurs along a motion path from start to end point (0–1) at `speed`, with `taper` (active pixel layer, inside the selection if any); for directional motion."
        }
        "filter.other.custom" => {
            "Convolves (active pixel layer, inside the selection if any) with a 5×5 `kernel` of integers, divided by `scale` and added to `offset`; any custom sharpen, blur or emboss."
        }
        "filter.other.hsbHsl" => {
            "Reinterprets channels between RGB, HSB and HSL (`inputMode`, `rowOrder`) (active pixel layer, inside the selection if any), so a color model's components can be edited as channels."
        }
        "filter.video.deInterlace" => {
            "Removes interlacing lines from a video frame (active pixel layer, inside the selection if any): drops odd or even fields and fills them by interpolation or duplication."
        }
        "filter.video.ntscColors" => {
            "Limits colors to the NTSC broadcast gamut to avoid oversaturation on TV (active pixel layer, inside the selection if any); no settings."
        }
        "filter.other.colorToAlpha" => {
            "Makes `color` transparent (active pixel layer, inside the selection if any), turning near colors partly transparent between `transparencyThreshold` and `opacityThreshold` (0–1); for removing a flat background."
        }
        "filter.filterGallery" => {
            "Applies a stack of Filter Gallery `effects` in order, each `{name, …settings}`; `list: true` answers the available effects and settings instead."
        }
        "filter.gallery.coloredPencil" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it as colored pencil strokes on paper of `paperBrightness`; `pencilWidth`, `strokePressure`."
        }
        "filter.gallery.cutout" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): reduces it to `numberOfLevels` flat color shapes, like cut paper; `edgeSimplicity`, `edgeFidelity`."
        }
        "filter.gallery.dryBrush" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints it with a dry brush, between oil and watercolor; `brushSize`, `brushDetail`, `texture`."
        }
        "filter.gallery.filmGrain" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): adds film grain to shadows and midtones and smooths highlights; `grain`, `highlightArea`, `intensity`."
        }
        "filter.gallery.fresco" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints it in short, rough dabs like fresco; `brushSize`, `brushDetail`, `texture`."
        }
        "filter.gallery.neonGlow" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): adds a glow of `glowColor` to objects; `glowSize`, `glowBrightness`."
        }
        "filter.gallery.paintDaubs" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints it with daubs of `brushType` and `brushSize`; `sharpness`."
        }
        "filter.gallery.paletteKnife" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): reduces detail to thin, knife-applied paint; `strokeSize`, `strokeDetail`, `softness`."
        }
        "filter.gallery.plasticWrap" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): coats it in shiny plastic wrap that follows its surface; `highlightStrength`, `detail`, `smoothness`."
        }
        "filter.gallery.posterEdges" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): posterizes it and draws black edges; `edgeThickness`, `edgeIntensity`, `posterization`."
        }
        "filter.gallery.roughPastels" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it as chalk pastel strokes on a textured ground; stroke length and detail, texture, `scaling`, `relief`, `light`."
        }
        "filter.gallery.smudgeStick" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): smudges darks with short diagonal strokes and brightens highlights; `strokeLength`, `highlightArea`, `intensity`."
        }
        "filter.gallery.sponge" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): textures it as if painted with a sponge; `brushSize`, `definition`, `smoothness`."
        }
        "filter.gallery.underpainting" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints a blurred underpainting on a texture; `brushSize`, texture coverage, `scaling`, `relief`, `light`."
        }
        "filter.gallery.watercolor" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints it as watercolor, saturating and darkening edges; `brushDetail`, `shadowIntensity`, `texture`."
        }
        "filter.gallery.accentedEdges" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): accentuates edges, light or dark by `edgeBrightness`; `edgeWidth`, `smoothness`."
        }
        "filter.gallery.angledStrokes" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): repaints it with diagonal strokes, light and dark in opposite directions; `directionBalance`, `strokeLength`, `sharpness`."
        }
        "filter.gallery.crosshatch" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): adds pencil crosshatching while keeping detail; `strokeLength`, `sharpness`, `strength`."
        }
        "filter.gallery.darkStrokes" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints darks with short tight strokes and lights with long white strokes; `balance`, `blackIntensity`, `whiteIntensity`."
        }
        "filter.gallery.inkOutlines" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it in fine pen-and-ink lines over the original; `strokeLength`, `darkIntensity`, `lightIntensity`."
        }
        "filter.gallery.spatter" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): spatters it like an airbrush; `sprayRadius`, `smoothness`."
        }
        "filter.gallery.sprayedStrokes" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): repaints it with angled, sprayed strokes in its dominant colors; `strokeLength`, `sprayRadius`, `strokeDirection`."
        }
        "filter.gallery.sumiE" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints it in Japanese ink style with a wet brush on rice paper; `strokeWidth`, `strokePressure`, `contrast`."
        }
        "filter.gallery.diffuseGlow" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): renders it as through a soft diffusion filter, adding background-color glow to highlights; `graininess`, `glowAmount`, `clearAmount`."
        }
        "filter.gallery.glass" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): distorts it as if seen through glass of `texture`; `distortion`, `smoothness`, `scaling`."
        }
        "filter.gallery.oceanRipple" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): adds random ripples, as if under water; `rippleSize`, `rippleMagnitude`."
        }
        "filter.gallery.basRelief" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): turns it into a low relief in the foreground and background colors, lit from `light`; `detail`, `smoothness`."
        }
        "filter.gallery.chalkCharcoal" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws highlights in chalk (background color) and shadows in charcoal (foreground color); `chalkArea`, `charcoalArea`, `strokePressure`."
        }
        "filter.gallery.charcoal" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it as smudged charcoal in the foreground color on the background color; `charcoalThickness`, `detail`, `lightDarkBalance`."
        }
        "filter.gallery.chrome" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): renders it as polished chrome; `detail`, `smoothness`."
        }
        "filter.gallery.conteCrayon" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it in Conté crayon textures with the foreground and background colors; levels, texture, `relief`, `light`."
        }
        "filter.gallery.graphicPen" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it with fine ink strokes in the foreground color on the background color; `strokeLength`, `strokeDirection`, `lightDarkBalance`."
        }
        "filter.gallery.halftonePattern" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): simulates a halftone screen of dots, circles or lines (`patternType`) in the foreground and background colors; `size`, `contrast`."
        }
        "filter.gallery.notePaper" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): makes it look embossed in handmade paper, in the foreground and background colors; `imageBalance`, `graininess`, `relief`."
        }
        "filter.gallery.photocopy" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): makes it look photocopied: dark edges, midtones to solid foreground or background color; `detail`, `darkness`."
        }
        "filter.gallery.plaster" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): molds it in 3D plaster, darks raised and lights sunk, in the foreground and background colors; `imageBalance`, `smoothness`, `light`."
        }
        "filter.gallery.reticulation" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): adds the clumped grain of shrunk film emulsion; `density`, `foregroundLevel`, `backgroundLevel`."
        }
        "filter.gallery.stamp" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): simplifies it into a rubber-stamp look in the foreground and background colors; `lightDarkBalance`, `smoothness`."
        }
        "filter.gallery.tornEdges" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): redraws it as ragged torn paper in the foreground and background colors; `imageBalance`, `smoothness`, `contrast`."
        }
        "filter.gallery.waterPaper" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints blotchy daubs that bleed into fibrous damp paper; `fiberLength`, `brightness`, `contrast`."
        }
        "filter.gallery.glowingEdges" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): finds edges and makes them glow like neon on black; `edgeWidth`, `edgeBrightness`, `smoothness`."
        }
        "filter.gallery.craquelure" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): paints a network of fine cracks on a plaster-like relief; `crackSpacing`, `crackDepth`, `crackBrightness`."
        }
        "filter.gallery.grain" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): adds grain of `grainType` (regular, soft, sprinkles, …); `intensity`, `contrast`."
        }
        "filter.gallery.mosaicTiles" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): turns it into small tiles with grout; `tileSize`, `groutWidth`, `lightenGrout`."
        }
        "filter.gallery.patchwork" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): breaks it into squares filled with the dominant color, randomly raised; `squareSize`, `relief`."
        }
        "filter.gallery.stainedGlass" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): repaints it as single-color glass cells outlined in the foreground color; `cellSize`, `borderThickness`, `lightIntensity`."
        }
        "filter.gallery.texturizer" => {
            "Filter Gallery effect (active pixel layer, inside the selection if any): applies a canvas, brick, burlap or sandstone `texture`; `scaling`, `relief`, `light`."
        }
        "gradient.fill.add" => {
            "Adds a gradient fill layer from `from` to `to` px in `style` (linear, radial, angle, reflected, diamond), using a preset `gradient` or `stops`; stays editable."
        }
        "gradient.fill.set" => "Changes gradient fill layer `layer`: endpoints, `style`, `angle`, `scale`, `offset`, `reverse`, `dither`, or its stops.",
        "gradient.fill.stop" => {
            "Adds, moves, deletes or recolors one color or opacity stop (`kind`, `index`, `location` 0–1) of gradient fill layer `layer`, or moves a midpoint."
        }
        "gradient.fill.get" => {
            "Answers the settings of gradient fill layer `layer`: style, angle, scale, offset, endpoints, color stops, transparency stops and midpoints."
        }
        "layer.fill.setSolidColor" => {
            "Changes the color of a solid-color fill layer `layer` (else the active one) to `color` (hex, rgba array or color object)."
        }
        "text.add" => {
            "Adds a type layer with `text` at `x`, `y` (point text), or paragraph text in `box` [x, y, width, height] (any other `box` is refused), with font, size, color, alignment and `hyphenate` (breaks long words at line ends with a hyphen); answers its id and its bounds [x, y, width, height], as file.inspect gives them."
        }
        "text.edit" => "Replaces or edits the text of type layer `layer` (else the active one), keeping its styles.",
        "text.setStyle" => {
            "Sets character and paragraph styles (font, size, color, tracking, leading, alignment, …) on a type layer, for a character range or all of it."
        }
        "text.rasterize" => "Converts type layer `layer` (else the active one) into pixels.",
        "text.info" => {
            "Answers type layer `layer`'s text, its style runs, one entry per paragraph with its paragraph style, its laid-out lines and its frame, and its bounds [x, y, width, height] in document pixels."
        }
        "text.fonts" => "Lists the font families (or one family's faces) type commands can use.",
        "edit.transform" => "Scales, rotates, skews or moves `layer` (else the active layer) or the selection to a target frame or by a matrix.",
        "select.float" => {
            "Lifts the selected pixels into a floating piece above the layer, optionally moved by `dx`,`dy` px or copied; answers {layer, offset}. Drop it with select.drop."
        }
        "select.drop" => "Drops the floating piece back into its layer and moves the selection with it, as one history step; answers {dropped, offset}.",
        "shape.add" => "Adds a shape layer (rectangle, rounded rectangle, ellipse, polygon, star, line or path) with fill and stroke; answers its id.",
        "shape.edit" => "Changes shape layer `layer`'s geometry, fill or stroke.",
        "shape.info" => "Answers shape layer `layer`'s kind, path, fill, stroke and bounds.",
        "shape.rasterize" => "Converts shape layer `layer` (else the active one) into pixels.",
        "path.list" => "Lists the document's saved paths, the work path, the clipping path and the active layer's path.",
        "path.info" => "Answers path `name` (the work path, a saved path, or `layer` for the active shape or vector mask) as subpaths of knots.",
        "path.set" => "Replaces path `name` with `path` (subpaths of knots), or combines it in with `op` (combine, subtract, intersect, exclude).",
        "path.delete" => "Deletes path `name` (default the work path).",
        "path.transform" => "Moves, scales, rotates or skews path `name` (or a shape layer's path) by `matrix` or translate/scale/rotate values.",
        "path.clippingPath.set" => "Marks saved path `name` as the document's clipping path for export (with `flatness`), so other apps show only its inside.",
        "path.clippingPath.clear" => "Removes the document's clipping path setting.",
        "path.style.copyFill" => "Copies the active shape layer's fill settings, to paste with path.style.pasteFill.",
        "path.style.copyStroke" => "Copies the active shape layer's stroke settings, to paste with path.style.pasteStroke.",
        "path.style.pasteFill" => "Applies the copied shape fill to shape layer `layer` (else the active one).",
        "path.style.pasteStroke" => "Applies the copied shape stroke to shape layer `layer` (else the active one).",
        "path.rename" => "Renames path `name` to `to`; renaming the work path saves it as a named path.",
        "path.toSelection" => "Makes a selection from a path (saved, the work path, or the layer's), with feather and anti-aliasing.",
        "select.toWorkPath" => "Converts the selection outline into the work path, smoothed to `tolerance` px (0.5–10); answers {subpaths, knots}.",
        "path.fill" => "Fills the inside of path `name` on pixel layer `layer` with `color` (or a pattern) at `opacity` and blend `mode`; the path stays.",
        "path.stroke" => {
            "Paints along path `name` on pixel layer `layer` with the brush, pencil or eraser (`tool`, `size`, `hardness`, `color`), optionally simulating pressure."
        }
        "layer.vectorMask.add" => "Vector mask, Add Vector Mask: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.fromPath" => "Vector mask, Vector Mask from Current Path: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.edit" => "Vector mask, Edit Vector Mask: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.delete" => "Vector mask, Delete Vector Mask: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.rasterize.vectorMask" => "Converts the vector mask of `layer` into pixels, multiplied into its layer mask; the path is no longer editable.",
        "layer.vectorMask.revealAll" => "Vector mask, Vector Mask: Reveal All: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.hideAll" => "Vector mask, Vector Mask: Hide All: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.currentPath" => "Vector mask, Vector Mask: Current Path: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.enabled" => "Vector mask, Enable Vector Mask: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.vectorMask.linked" => "Vector mask, Link Vector Mask: acts on `layer`'s (else the active layer's) vector mask.",
        "layer.rasterize.shape" => "Converts shape layer `layer` into a pixel layer with the shape painted in; the path and fill are no longer editable.",
        "layer.combineShapes.unite" => "Unite Shapes on shape layer `layer` (else the active one): combines its subpaths.",
        "layer.combineShapes.subtractFrontShape" => "Subtract Front Shape on shape layer `layer` (else the active one): combines its subpaths.",
        "layer.combineShapes.intersectShapeAreas" => "Intersect Shape Areas on shape layer `layer` (else the active one): combines its subpaths.",
        "layer.combineShapes.excludeOverlappingShapes" => "Exclude Overlapping Shapes on shape layer `layer` (else the active one): combines its subpaths.",
        "layer.combineShapes.mergeShapeComponents" => {
            "Merge Shape Components on shape layer `layer` (else the active one): combines its subpaths (bakes the path operations into plain combined outlines, traced from the exact coverage)."
        }
        "select.convertToShape" => "Converts the selection outline into a new shape layer filled with `fill`, traced at `tolerance` px.",
        "layer.vectorMask.info" => {
            "Vector mask, Vector Mask Info: acts on `layer`'s (else the active layer's) vector mask; answers {layer,path,enabled,linked,density,feather}."
        }
        "path.moveAnchors" => "Moves anchor points `anchors` ([subpath, knot] pairs) of a path by `move` [dx,dy] px, with their handles.",
        "path.moveHandle" => "Moves the in or out direction handle of knot `knot` in `subpath` to `to`; `independent` breaks the smooth link.",
        "path.bendSegment" => "Bends the segment leaving knot `knot` so its point at `t` (0–1) passes through a new position, as dragging a segment does.",
        "path.convertPoint" => "Converts knot `knot` between a corner and a smooth point, setting or retracting its handles.",
        "select.quick" => {
            "Selects areas like the Quick Selection brush: grows from brush `points` of `size` px to similar color and texture; `mode` replaces, adds to, subtracts from or intersects the current selection."
        }
        "select.object" => {
            "Selects the main object inside the rectangle `rect` [x,y,w,h] by segmentation; `mode` replaces, adds to, subtracts from or intersects the current selection."
        }
        "select.subject" => "Selects the main subject of the image automatically.",
        "select.edgeRefine" => {
            "Refines the selection edge (`radius` or `smartRadius`, `smooth`, `feather`, `contrast`, `shiftEdge`) and outputs it as a selection, layer mask or new layer; `decontaminate` removes color fringes."
        }
        "select.focusArea" => {
            "Selects the in-focus parts of the image (`range` 0–1, `noise` 0–1); `mode` replaces, adds to, subtracts from or intersects the current selection."
        }
        "layer.removeBackground" => "Masks out the background of `layer` (else the active one) with a layer mask.",
        "paint.setSymmetry" => "Painting: Set Painting Symmetry on the active layer along `points` with the current brush.",
        "paint.symmetryFromPath" => {
            "Painting: Make Symmetry Path on the active layer along `points` with the current brush (mirror Brush, Pencil and Eraser strokes across the path)."
        }
        "paint.symmetryDisable" => "Painting: Disable Painting Symmetry on the active layer along `points` with the current brush.",
        "edit.cut" => "Cuts the selected pixels of the active layer to the clipboard.",
        "edit.copy" => "Copies the selected pixels (or the whole layer) of the active layer to the clipboard; answers the bounds.",
        "edit.copyMerged" => "Copies the selected pixels of every visible layer merged; answers the bounds.",
        "edit.paste" => "Pastes the clipboard as a new layer, centered or at `center`.",
        "file.newFromClipboard" => "Creates a new document the size of the clipboard image, holding it as one layer.",
        "edit.pasteSpecial.pasteInPlace" => {
            "Pastes the clipboard as a new layer at the position it was copied from, or into `target` (layer mask, quick mask or a channel)."
        }
        "layer.new.layerViaCopy" => "Copies the selected pixels of the active layer (else the whole layer) to a new layer above it; the original stays.",
        "layer.new.layerViaCut" => "Cuts the selected pixels of the active layer to a new layer above it, leaving transparency (or background color) behind.",
        "layer.mergeVisible" => "Merges every visible layer into one layer; hidden layers stay separate. Use layer.stampVisible to keep the originals.",
        "image.autoTone" => "Adjusts the active layer's tonal range automatically (per-channel levels).",
        "image.autoContrast" => "Adjusts the active layer's contrast automatically, keeping its colors.",
        "image.autoColor" => "Corrects the active layer's color cast and contrast automatically.",
        "edit.toggleLastState" => "Undoes the last step, or redoes it if the last thing done was this toggle; for flipping before/after.",
        "edit.transform.again" => "Repeats the last free transform (scale, rotate, skew, move) on the active layer.",
        "edit.transform.againCopy" => "Duplicates the active layer and repeats the last transform on the copy, for step-and-repeat patterns.",
        "view.newGuide" => "Adds a horizontal or vertical guide at `position` px.",
        "view.moveGuide" => "Moves guide `index` (counted per orientation) to `position` px.",
        "view.deleteGuide" => "Deletes guide `index` of the given orientation.",
        "view.clearGuides" => "Deletes every guide in the document.",
        "edit.assignProfile" => {
            "Tags the document with color `profile` without converting pixel values, so colors look different; use edit.convertToProfile to keep appearance."
        }
        "edit.convertToProfile" => {
            "Converts pixel values to color `profile` with rendering `intent` so colors look the same; changes the numbers, unlike edit.assignProfile."
        }
        "edit.colorSettings" => {
            "The Edit › Color Settings command (reads, or changes only the view or selection); takes `workingRgb`, `workingCmyk`, `workingGray`, `policyRgb`, `policyCmyk`, `policyGray` (working spaces and the monitor profile also accept .icc paths; monitor `auto` = the main display's profile when the platform provides it, else sRGB; the reply's `monitorStatus` says which profile is in use and why: source auto|manual|fallback, reason)."
        }
        "color.profileMismatch" => "Applies the Embedded Profile Mismatch filter to the active layer, inside the selection if there is one; takes `action`.",
        "edit.profileInfo" => "Answers information about a color profile: the document's (default), a built-in id, or an .icc file.",
        "view.proofSetup" => "Editor view, Proof Setup: changes how the document is shown; does not change pixels.",
        "view.proofColors" => "Turns soft-proofing on or off (`on`, else toggle): the view simulates the proof profile; pixels don't change.",
        "view.gamutWarning" => {
            "Turns the gamut warning on or off: colors farther than `threshold` ΔE from the proof gamut are shown in the warning color; view only."
        }
        "paint.pencil" => "Painting: Pencil on the active layer along `points` with the current brush.",
        "paint.mixerBrush" => "Painting: Mixer Brush on the active layer along `points` with the current brush.",
        "paint.colorReplacement" => "Painting: Color Replacement on the active layer along `points` with the current brush.",
        "brush.presets.list" => "Lists the brush presets (name, tip, size, hardness).",
        "brush.presets.save" => "Saves the current brush (or `brush` settings) as a new brush preset named `name`.",
        "brush.presets.update" => "Overwrites brush preset `name` (default the one the current brush came from) with the current brush settings.",
        "brush.presets.delete" => "Deletes brush preset `name` from the library.",
        "brush.presets.setCurrent" => "Makes brush preset `name` the current brush for painting tools.",
        "brush.defineFromSelection" => "Creates a brush tip preset named `name` from the selected pixels (grayscale; dark is opaque).",
        "brush.get" => "Answers the current brush settings.",
        "tool.setBrush" => "Sets the current brush from a preset and/or brush settings fields, and answers the brush.",
        "brush.presets.rename" => "Renames brush preset `name` to `newName`; answers {name}.",
        "brush.presets.move" => "Moves brush preset `name` to another `group` or nested `folder`, before a given preset or at `index`.",
        "brush.presets.moveGroup" => "Reorders brush preset group `group` (or a nested `folder`) among its siblings.",
        "brush.presets.renameGroup" => "Renames brush preset group `group` (or a nested `folder`) to `newName`.",
        "brush.presets.deleteGroup" => "Deletes brush preset group `group` (or a nested `folder`) and the presets in it; answers {deleted, count}.",
        "paint.magicEraser" => "Painting: Magic Eraser on the active layer along `points` with the current brush.",
        "paint.backgroundEraser" => "Painting: Background Eraser on the active layer along `points` with the current brush.",
        "brush.presets.importAbr" => "The Import Brushes command (changes the document); takes `path`, `data`, `group`, `replace`, `select`.",
        "gradient.presets.importGrd" => "The Import Gradients command (changes the document); takes `path`, `data`, `group`, `replace`.",
        "swatch.list" => "Lists the swatch groups and their colors.",
        "swatch.add" => "Adds a swatch of `color` (hex or a color model with values; default the foreground) named `name` to a group.",
        "swatch.delete" => "Deletes swatch `swatch` (name, list of names, or `index`); answers {deleted}.",
        "swatch.rename" => "Renames swatch `swatch` to `name`.",
        "swatch.move" => "Moves swatch `swatch` to group `to` at `position`.",
        "swatch.newGroup" => "Creates a swatch group named `name`, optionally moving `swatches` into it.",
        "swatch.renameGroup" => "Renames swatch group `group` to `name`.",
        "swatch.deleteGroup" => "Deletes swatch group `group`, with its swatches unless `keepSwatches` moves them up.",
        "swatch.use" => "Sets the foreground (or background, `target`) color to swatch `swatch`; answers {swatch, target, hex}.",
        "swatch.reset" => "Replaces the swatch library with the defaults, or adds them with `append`.",
        "swatch.import" => "The Import Swatches command (changes the document); takes `path`, `data`, `format`, `mode`, `group`.",
        "swatch.export" => "The Export Swatches command (changes the document); takes `format`, `path`, `group`, `swatches`.",
        "paint.cloneStamp" => "Painting: Clone Stamp on the active layer along `points` with the current brush.",
        "paint.patternStamp" => "Painting: Pattern Stamp on the active layer along `points` with the current brush.",
        "paint.healingBrush" => "Painting: Healing Brush on the active layer along `points` with the current brush.",
        "paint.spotHealing" => "Painting: Spot Healing Brush on the active layer along `points` with the current brush.",
        "paint.patch" => "Painting: Patch on the active layer along `points` with the current brush.",
        "paint.smartMove" => {
            "Painting: Smart Move on the active layer along `points` with the current brush (a background job; the selection moves with the content)."
        }
        "paint.dodge" => "Painting: Dodge on the active layer along `points` with the current brush.",
        "paint.burn" => "Painting: Burn on the active layer along `points` with the current brush.",
        "paint.sponge" => "Painting: Sponge on the active layer along `points` with the current brush.",
        "paint.blur" => "Painting: Blur on the active layer along `points` with the current brush.",
        "paint.sharpen" => "Painting: Sharpen on the active layer along `points` with the current brush.",
        "paint.smudge" => "Painting: Smudge on the active layer along `points` with the current brush.",
        "paint.historyBrush" => "Painting: History Brush on the active layer along `points` with the current brush.",
        "paint.redEye" => "Painting: Red Eye on the active layer along `points` with the current brush.",
        "image.imageSize" => {
            "Resizes the whole image to `width`×`height` px and/or `resolution` ppi, resampling pixels with `resample` (none changes only the ppi); every layer scales."
        }
        "image.canvasSize" => {
            "Changes the canvas to `width`×`height` px (or by them with `relative`) around `anchor`, without scaling layers; new area takes `extensionColor`, shrinking crops."
        }
        "image.crop" => {
            "Crops the canvas to the rectangle `x`,`y`,`width`,`height` px, optionally rotated by `angle`; `deleteCroppedPixels: false` keeps hidden pixels outside."
        }
        "image.trim" => "Crops away borders that are transparent or match the top-left/bottom-right color (`basedOn`), on the chosen sides.",
        "image.mode.rgb" => "Converts the document to RGB color with ICC `profile` and rendering `intent`.",
        "image.mode.grayscale" => "Converts the document to grayscale, discarding color, with ICC `profile` and rendering `intent`.",
        "image.mode.cmyk" => "Converts the document to CMYK for print with ICC `profile` and rendering `intent`; colors outside the gamut shift.",
        "image.mode.lab" => "Converts the document to Lab color (lightness separate from color) with rendering `intent`.",
        "image.mode.bits8" => "Converts the document to 8 bits per channel; required by some filters, loses tonal precision.",
        "image.mode.bits16" => "Converts the document to 16 bits per channel for more tonal precision in heavy edits.",
        "image.mode.bits32" => "Converts the document to 32-bit float (HDR); many filters and adjustments are unavailable in this mode.",
        "image.duplicate" => "Opens a copy of the active document as a new document named `name`; `mergedOnly` flattens the copy. The original is unchanged.",
        "select.magicWand" => "Selects contiguous (or all) pixels similar in color to the one at `x`, `y` within `tolerance`.",
        "select.colorRange" => "Selects pixels of a color range (a sampled color, highlights, midtones, shadows, skin tones) with fuzziness.",
        "select.modify.border" => "Changes the selection: Border by the given pixels; needs a selection.",
        "select.modify.smooth" => "Changes the selection: Smooth by the given pixels; needs a selection.",
        "select.modify.expand" => "Changes the selection: Expand by the given pixels; needs a selection.",
        "select.modify.contract" => "Changes the selection: Contract by the given pixels; needs a selection.",
        "select.modify.feather" => "Changes the selection: Feather by the given pixels; needs a selection.",
        "select.grow" => "Expands the selection to adjacent pixels within `tolerance` (0–255) of the selected colors; needs a selection.",
        "select.similar" => "Adds every pixel in the image within `tolerance` (0–255) of the selected colors, adjacent or not; needs a selection.",
        "select.lasso" => {
            "Selects a freehand polygon through `points` [[x,y],…] px, with `feather` and `antiAlias`; `mode` replaces, adds to, subtracts from or intersects the current selection."
        }
        "select.magneticLasso" => {
            "Selects along edges between `points`, snapping to edges of `contrast` within `width` px (`trace`), like the magnetic lasso; `mode` replaces, adds to, subtracts from or intersects the current selection."
        }
        "select.sky" => "Selects the sky automatically.",
        "select.isolateLayers" => {
            "Shows only the selected layers in the Layers panel (`on`, else toggle); a view filter, no document change. Answers {isolated, layers}."
        }
        "select.transformSelection" => {
            "Moves, scales, rotates or distorts the selection outline itself (`dx`/`dy`, `scaleX`/`scaleY`, `rotate`, skew, `matrix` or `corners`); the pixels stay."
        }
        "paint.bucket" => "Painting: Paint Bucket on the active layer along `points` with the current brush.",
        "paint.gradient" => "Painting: Gradient on the active layer along `points` with the current brush.",
        "edit.stroke" => {
            "Draws a line of `width` px in `color` along the selection edge (inside, center or outside), with blend `mode` and `opacity`; needs a selection."
        }
        "edit.transform.rotate180" => "Rotates the active layer (or the selected pixels) 180° within the canvas.",
        "edit.transform.rotate90Cw" => {
            "Rotates the active layer (or the selected pixels) 90° clockwise within the canvas; to rotate the whole canvas use image.imageRotation.rotate90cw."
        }
        "edit.transform.rotate90Ccw" => "Rotates the active layer (or the selected pixels) 90° counter-clockwise within the canvas.",
        "edit.transform.flipHorizontal" => {
            "Mirrors the active layer (or the selected pixels) left to right; to flip the whole canvas use image.imageRotation.flipCanvasHorizontal."
        }
        "edit.transform.flipVertical" => "Mirrors the active layer (or the selected pixels) top to bottom.",
        "edit.pasteSpecial.pasteInto" => "Pastes the clipboard as a new layer masked by the current selection, so it shows only inside it; needs a selection.",
        "edit.pasteSpecial.pasteOutside" => "Pastes the clipboard as a new layer masked to show only outside the current selection; needs a selection.",
        "select.reselect" => "Restores the last selection that was deselected.",
        "image.adjustments.equalize" => {
            "Spreads the brightness values of the active layer (inside the selection) evenly across the range, maximising contrast; destructive."
        }
        "image.revealAll" => "Enlarges the canvas so every layer's pixels outside it become visible.",
        "layer.new.layerFromBackground" => "Turns the Background layer into a normal layer, so it can hold transparency and be moved in the stack.",
        "layer.layerStyle.copyLayerStyle" => {
            "Copies the layer style of `layer` (the active layer by default): its effects, blend mode, fill opacity and advanced blending, for layer.layerStyle.pasteLayerStyle; changes nothing."
        }
        "layer.layerStyle.pasteLayerStyle" => {
            "Pastes the layer style copied with layer.layerStyle.copyLayerStyle onto `layer` (the active layer by default), replacing its effects, blend mode, fill opacity and advanced blending; fails when nothing was copied."
        }
        "layer.layerStyle.hideAllEffects" => {
            "Hides the layer effects of every layer in the document (they stay on the layers; layer.layerStyle.showAllEffects shows them again)."
        }
        "layer.layerStyle.showAllEffects" => "Shows the layer effects of every layer in the document again after layer.layerStyle.hideAllEffects.",
        "layer.layerMask.enabled" => "Layer mask, Disable Layer Mask: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.layerMask.linked" => "Layer mask, Unlink Layer Mask: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.rasterize.layer" => "Converts type, shape, fill or object layer `layer` into a plain pixel layer as it looks now; its editable content is lost.",
        "layer.rasterize.allLayers" => "Converts every type, shape, fill and object layer in the document into plain pixel layers.",
        "layer.rasterize.type" => "Converts type layer `layer` into pixels; the text is no longer editable. Needed before painting or filtering text.",
        "layer.rasterize.fillContent" => "Converts the content of fill layer `layer` (solid, gradient, pattern) into pixels, keeping its mask.",
        "layer.rasterize.objectLayer" => "Converts object layer `layer` into a pixel layer, dropping its embedded contents and editable filters.",
        "layer.delete.hiddenLayers" => "Deletes every hidden layer in the document.",
        "file.scripts.deleteAllEmptyLayers" => "Deletes every layer that has no visible content.",
        "layer.ungroupLayers" => "Removes group `layer` (else the active group), moving its layers up into the parent; the layers keep their order.",
        "layer.hideLayers" => "Hides `layer` (else every selected layer).",
        "layer.showLayers" => "Shows `layer` (else every selected layer).",
        "layer.showOnly" => {
            "Shows only `layer` (else the active layer) and hides all others; calling it again restores their visibility. Answers {shownAlone}."
        }
        "filter.blur.average" => "Fills (active pixel layer, inside the selection if any) with its single average color; useful to find a color cast.",
        "filter.render.clouds" => {
            "Replaces (active pixel layer, inside the selection if any) with soft cloud noise in the foreground and background colors; `seed` fixes the pattern."
        }
        "filter.render.differenceClouds" => {
            "Generates cloud noise and blends it with Difference over (active pixel layer, inside the selection if any); repeating it builds marble-like veins."
        }
        "file.closeAll" => "Closes every open document; unsaved changes are lost unless saved first.",
        "file.closeOthers" => "Closes every document except `document` (index from 0; default the active one).",
        "file.revealInFinder" => "The Reveal in Finder command (reads, or changes only the view or selection); takes `document`, `dryRun`.",
        "file.revert" => "The File › Revert command (changes the document) (reloads the saved file as one undoable step).",
        "file.saveACopy" => "The File › Save a Copy command (reads, or changes only the view or selection); takes `path`, `quality`, `layers`, `tiffLayers`.",
        "file.openAs" => "The File › Open As command (changes the document); takes `path`, `as`.",
        "file.placeEmbedded" => "The File › Place Embedded command (changes the document); takes `path`, `scale`, `fit`, `center`.",
        "file.placeLinked" => "The File › Place Linked command (changes the document); takes `path`, `scale`, `fit`, `center`.",
        "file.fileInfo" => "Sets document metadata (title, author, description, keywords, copyright and more), saved with the file.",
        "file.automate.fitImage" => "Resizes the image proportionally to fit within `width`×`height` px; `dontEnlarge` keeps smaller images as they are.",
        "file.automate.conditionalModeChange" => "Converts the document to color mode `to` only if its current mode is one of `from`; for batch actions.",
        "file.automate.batch" => {
            "The File › Automate › Batch command (changes the document); takes `steps`, `command`, `params`, `input`, `output`, `format` (an input whose output name was already written in the run goes to errors)."
        }
        "file.scripts.imageProcessor" => {
            "The File › Scripts › Image Processor command (changes the document); takes `input`, `output`, `format`, `quality`, `tiffLayers`, `width` (an input whose output name was already written in the run goes to errors)."
        }
        "file.scripts.loadFilesIntoStack" => "The File › Scripts › Load Files into Stack command (changes the document); takes `paths`, `createObjectLayer`.",
        "file.scripts.flattenAllLayerEffects" => "Bakes every layer's effects (layer styles) into its pixels.",
        "file.scripts.flattenAllMasks" => "Applies the masks of all pixel layers into their pixels and deletes the masks; masks on other layer kinds stay.",
        "file.export.layersToFiles" => {
            "The File › Export › Layers to Files command (reads, or changes only the view or selection); takes `dir`, `format`, `prefix`, `visibleOnly`, `quality`, `tiffLayers`; answers {files}."
        }
        "file.export.colorLookupTables" => {
            "The File › Export › Color Lookup Tables command (reads, or changes only the view or selection); takes `path`, `size`, `title`, `scope`, `layers` (selected layer IDs use original document stacking; masks are spatial and omitted)."
        }
        "view.newGuideLayout" => {
            "Adds guides for `columns` and `rows` with `gutter`, `rowGutter` and `margin` px, optionally clearing existing guides; for page layouts."
        }
        "view.newGuidesFromShape" => "Adds guides along the bounds of shape `layer` (else the active layer).",
        "view.clearCanvasGuides" => "Deletes the guides on the canvas outside artboards.",
        "text.antiAlias.none" => "Renders type layer `layer` with no anti-aliasing (jagged edges); affects how glyph edges look, not the text.",
        "text.antiAlias.sharp" => "Renders type layer `layer` with sharp anti-aliasing; affects how glyph edges look, not the text.",
        "text.antiAlias.crisp" => "Renders type layer `layer` with crisp anti-aliasing; affects how glyph edges look, not the text.",
        "text.antiAlias.strong" => "Renders type layer `layer` with strong (heavier) anti-aliasing; affects how glyph edges look, not the text.",
        "text.antiAlias.smooth" => "Renders type layer `layer` with smooth anti-aliasing; affects how glyph edges look, not the text.",
        "text.antiAlias.windowsLcd" => "Renders type layer `layer` with Windows LCD-style anti-aliasing; affects how glyph edges look, not the text.",
        "text.antiAlias.windows" => "Renders type layer `layer` with Windows-style anti-aliasing; affects how glyph edges look, not the text.",
        "text.orientation.horizontal" => "Sets type layer `layer` to horizontal text.",
        "text.orientation.vertical" => "Sets type layer `layer` to vertical text (top to bottom, columns right to left).",
        "text.openType.standardLigatures" => {
            "Turns OpenType standard ligatures (fi, fl) on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.contextualAlternates" => {
            "Turns OpenType contextual alternates on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.discretionaryLigatures" => {
            "Turns OpenType discretionary ligatures on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.swash" => {
            "Turns OpenType swash glyphs on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.oldstyle" => {
            "Turns OpenType oldstyle figures on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.stylisticAlternates" => {
            "Turns OpenType stylistic alternates on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.titlingAlternates" => {
            "Turns OpenType titling alternates on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.ornaments" => {
            "Turns OpenType ornaments on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.ordinals" => {
            "Turns OpenType ordinal superscripts (1st, 2nd) on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.openType.fractions" => {
            "Turns OpenType fractions (1/2 as ½) on or off (`on`, else toggle) for type layer `layer`, all text or the characters in `range`; needs a font that has them."
        }
        "text.convertToWorkPath" => "Creates a work path from the outlines of type layer `layer`; the text layer stays editable.",
        "text.convertToShape" => "Converts type layer `layer` into a shape layer with the glyph outlines; the text is no longer editable.",
        "text.rasterizeTypeLayer" => "Converts type layer `layer` into pixels; the text is no longer editable.",
        "text.convertToParagraphText" => "Converts point text `layer` into paragraph (area) text that wraps inside a box.",
        "text.convertToPointText" => "Converts paragraph text `layer` into point text; line breaks are inserted where lines wrapped.",
        "text.warpText" => "Warps type layer `layer` with a `style` (arc, flag, wave, …), `bend` and distortions (−100–100 %); the text stays editable.",
        "text.updateAllTextLayers" => "Re-lays out every type layer, e.g. after fonts changed or were installed.",
        "text.replaceAllMissingFonts" => "Replaces every missing font in the document with the default family.",
        "text.resolveMissingFonts" => "Maps missing font families to installed ones (`map`); with no map, answers the missing families.",
        "text.pasteLoremIpsum" => "Inserts placeholder Lorem Ipsum text into type layer `layer` at character `at`, or as a `new` paragraph layer.",
        "text.saveDefaultTypeStyles" => "Saves the active type layer's styles as the defaults new type layers start from.",
        "text.loadDefaultTypeStyles" => "Applies the saved default type styles to type layer `layer`.",
        "text.characterStyle.new" => {
            "Creates a character style (font, size, color and other character attributes) from `attrs` or the selected text (`fromSelection`), optionally applying it; answers its id."
        }
        "text.characterStyle.duplicate" => "Duplicates character style `id`; answers the copy's id.",
        "text.characterStyle.delete" => "Deletes character style `id`; text that used it keeps its formatting as overrides.",
        "text.characterStyle.rename" => "Renames character style `id` to `name`.",
        "text.characterStyle.set" => "Changes the attributes of character style `id` (`attrs`), or removes some (`clear`); text using it updates.",
        "text.characterStyle.apply" => {
            "Applies character style `id` (0 or null for none) to the targeted text of `layer`/`layers`; `clearOverrides` drops local formatting."
        }
        "text.characterStyle.redefine" => "Redefines character style `id` from the targeted text's current formatting, so all text using it changes.",
        "text.characterStyle.clearOverride" => "Removes local formatting that differs from the applied character style in the targeted text.",
        "text.characterStyle.list" => "Answers the document's character styles and which one the targeted text uses.",
        "text.paragraphStyle.new" => {
            "Creates a paragraph style (alignment, indents, spacing plus character attributes) from `attrs` or the selected text (`fromSelection`), optionally applying it; answers its id."
        }
        "text.paragraphStyle.duplicate" => "Duplicates paragraph style `id`; answers the copy's id.",
        "text.paragraphStyle.delete" => "Deletes paragraph style `id`; text that used it keeps its formatting as overrides.",
        "text.paragraphStyle.rename" => "Renames paragraph style `id` to `name`.",
        "text.paragraphStyle.set" => "Changes the attributes of paragraph style `id` (`attrs`), or removes some (`clear`); text using it updates.",
        "text.paragraphStyle.apply" => {
            "Applies paragraph style `id` (0 or null for none) to the targeted text of `layer`/`layers`; `clearOverrides` drops local formatting."
        }
        "text.paragraphStyle.redefine" => "Redefines paragraph style `id` from the targeted text's current formatting, so all text using it changes.",
        "text.paragraphStyle.clearOverride" => "Removes local formatting that differs from the applied paragraph style in the targeted text.",
        "text.paragraphStyle.list" => "Answers the document's paragraph styles and which one the targeted text uses.",
        "edit.checkSpelling" => {
            "Checks spelling in the active type layer or `allLayers`: `action` list, suggest, change, changeAll, or add/remove a dictionary word."
        }
        "text.insertText" => {
            "Inserts `text` into type layer `layer` at character `at` (default the end), or replaces the characters in `range`; answers {layer, caret}."
        }
        "text.hitTest" => "Answers which character of type layer `layer` is at `x`,`y` px: {layer, index, line, inside}.",
        "text.caret" => {
            "Answers the caret line and segment (document px) for character `index` in type layer `layer`: {index, line, segment}; works for rotated and vertical type."
        }
        "text.navigate" => {
            "Answers the character index reached by moving from `index` by word, line, line start/end or text start/end (`move`), or nearest `x`; for placing the caret."
        }
        "layer.objectLayers.convertToObjectLayer" => {
            "Wraps `layer` (else the selected layers, combined) in an object layer, so transforms and filters on it stay non-destructive and editable."
        }
        "filter.convertForEditableFilters" => {
            "Converts layer `layer` (else the selected layers, combined) into an object layer, so filters applied to it afterwards stay editable as editable filters."
        }
        "layer.objectLayers.newObjectLayerViaCopy" => "The Layer › Object Layers › New Object Layer via Copy command (changes the document); takes `layer`.",
        "layer.objectLayers.rasterize" => "The Layer › Object Layers › Rasterize command (changes the document); takes `layer`.",
        "layer.objectLayers.editContents" => {
            "Opens the contents of object layer `layer` as a new document; save it with layer.objectLayers.saveContents to update the object layer."
        }
        "layer.objectLayers.saveContents" => "In a document opened by editContents, writes its current state back into the parent object layer.",
        "layer.objectLayers.replaceContents" => "The Layer › Object Layers › Replace Contents command (changes the document); takes `layer`, `path`.",
        "layer.objectLayers.exportContents" => "The Layer › Object Layers › Export Contents command (changes the document); takes `layer`, `path`.",
        "layer.objectLayers.relinkToFile" => "The Layer › Object Layers › Relink to File command (changes the document); takes `layer`, `path`.",
        "layer.objectLayers.updateModifiedContent" => "The Layer › Object Layers › Update Modified Content command (changes the document); takes `layer`.",
        "layer.objectLayers.updateAllModifiedContent" => "The Layer › Object Layers › Update All Modified Content command (changes the document).",
        "layer.objectLayers.convertToLayers" => {
            "Unpacks object layer `layer` into its layers (one layer, or a group named after it) at its placement; editable filters are discarded."
        }
        "layer.objectLayers.convertToEmbedded" => "The Layer › Object Layers › Convert to Embedded command (changes the document); takes `layer`.",
        "layer.objectLayers.convertToLinked" => {
            "The Layer › Object Layers › Convert to Linked command (changes the document); takes `layer`, `path` (writes the contents there)."
        }
        "layer.editableFilter.disableEditableFilters" => "Turns all editable filters of `layer` on or off (`enabled`, else toggle) without deleting them.",
        "layer.editableFilter.deleteFilterMask" => "Deletes the editable filter mask of `layer`, so the filters apply everywhere.",
        "layer.editableFilter.disableFilterMask" => "Turns the editable filter mask of `layer` on or off (`enabled`, else toggle).",
        "layer.editableFilter.blendingOptions" => "Sets `blend` mode and `opacity` (0–1) of editable filter `index` (0 = bottom; default top) on `layer`.",
        "layer.editableFilter.clearEditableFilters" => "Removes every editable filter from `layer`; the object layer stays.",
        "layer.editableFilter.setVisible" => "Shows or hides editable filter `index` of `layer` (`visible`, else toggle).",
        "layer.editableFilter.setParams" => {
            "Changes settings of editable filter `index` of `layer`; `params` are merged into its current settings and it re-renders."
        }
        "layer.editableFilter.delete" => "Deletes editable filter `index` from object layer `layer`; the other editable filters stay.",
        "layer.editableFilter.move" => "Moves editable filter `index` of `layer` to position `to` (0 = bottom, applied first).",
        "select.allLayers" => "Selects every layer (not the Background).",
        "select.deselectLayers" => "Clears the layer selection; commands that need a layer then fail until one is selected.",
        "select.findLayers" => "Selects the layers whose names contain `name` (case-insensitive).",
        "layer.selectLinkedLayers" => "Selects every layer linked to the active layer.",
        "layer.align.topEdges" => {
            "Aligns the top edges of the selected layers to each other (`to: layers`), the selection, or the canvas; `auto` uses the selection with one layer, else the layers."
        }
        "layer.align.verticalCenters" => {
            "Aligns the vertical centers of the selected layers to each other (`to: layers`), the selection, or the canvas; `auto` uses the selection with one layer, else the layers."
        }
        "layer.align.bottomEdges" => {
            "Aligns the bottom edges of the selected layers to each other (`to: layers`), the selection, or the canvas; `auto` uses the selection with one layer, else the layers."
        }
        "layer.align.leftEdges" => {
            "Aligns the left edges of the selected layers to each other (`to: layers`), the selection, or the canvas; `auto` uses the selection with one layer, else the layers."
        }
        "layer.align.horizontalCenters" => {
            "Aligns the horizontal centers of the selected layers to each other (`to: layers`), the selection, or the canvas; `auto` uses the selection with one layer, else the layers."
        }
        "layer.align.rightEdges" => {
            "Aligns the right edges of the selected layers to each other (`to: layers`), the selection, or the canvas; `auto` uses the selection with one layer, else the layers."
        }
        "layer.distribute.topEdges" => "Spaces the selected layers (3 or more) so their top edges are evenly spaced.",
        "layer.distribute.verticalCenters" => "Spaces the selected layers (3 or more) so their vertical centers are evenly spaced.",
        "layer.distribute.bottomEdges" => "Spaces the selected layers (3 or more) so their bottom edges are evenly spaced.",
        "layer.distribute.leftEdges" => "Spaces the selected layers (3 or more) so their left edges are evenly spaced.",
        "layer.distribute.horizontalCenters" => "Spaces the selected layers (3 or more) so their horizontal centers are evenly spaced.",
        "layer.distribute.rightEdges" => "Spaces the selected layers (3 or more) so their right edges are evenly spaced.",
        "layer.distribute.horizontally" => "Spaces the selected layers so the horizontal gaps between them are equal (not their edges).",
        "layer.distribute.vertically" => "Spaces the selected layers so the vertical gaps between them are equal (not their edges).",
        "layer.linkLayers" => "Links the selected layers so they move and transform together; unlinks them if they already form one link group.",
        "layer.mergeLayers" => "Merges the selected layers into one pixel layer (with one selected, merges it into the layer below).",
        "layer.new.groupFromLayers" => "Puts the selected layers into a new group named `name`.",
        "arrange.reverse" => "Reverse: moves `layer` (else the active layer) in the layer stack.",
        "layer.lockLayers" => "Locks the selected layers' `transparency`, `pixels`, `position`, `artboard` nesting, or `all`, so those can't be edited.",
        "layer.renameLayer" => "Renames `layer` (else the active layer) to `name`.",
        "layer.copyToDocument" => {
            "Copies the selected layers into another open document (`document`, index from 0), optionally centered or placed `at` a point."
        }
        "prefs.get" => "Answers every preference section and its values (color settings, interface, tools, performance, …).",
        "prefs.set" => {
            "Changes application preferences: one `path` (section.key) to `value`, or several in `values`; validated, all or nothing. Read them with prefs.get or edit.preferences.*."
        }
        "prefs.reset" => "Restores preferences to defaults: one `path` (section or section.key), or everything.",
        "edit.preferences.general" => {
            "Answers the general preference section (color picker, image interpolation, placing as object layers, zoom behaviour, legacy free transform) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.interface" => {
            "Answers the interface preference section (color theme, canvas color and border, tooltips, menu colors) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.workspace" => {
            "Answers the workspace preference section (panel collapsing, documents as tabs, docking) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.tools" => {
            "Answers the tools preference section (tooltips, Shift tool switching, snapping vector tools to the pixel grid, overscroll) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.fileHandling" => {
            "Answers the fileHandling preference section (image previews, background saving, EXIF profile, maximize PSD compatibility) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.export" => {
            "Answers the export preference section (quick export format and location) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.performance" => {
            "Answers the performance preference section (memory use, history states, cache levels and tile size, GPU) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.scratchDisks" => {
            "Answers the scratchDisks preference section (scratch disk locations) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.cursors" => {
            "Answers the cursors preference section (painting and other cursors) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.transparencyAndGamut" => {
            "Answers the transparencyAndGamut preference section (the transparency checkerboard and gamut warning color) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.unitsAndRulers" => {
            "Answers the unitsAndRulers preference section (ruler and type units, column size and print resolution) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.guidesGridAndSlices" => {
            "Answers the guidesGridAndSlices preference section (guide, grid and slice colors and spacing) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.plugIns" => {
            "Answers the plugIns preference section (the plug-ins folder and extensions) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.type" => {
            "Answers the type preference section (smart quotes, missing glyph protection, English font names, font preview) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.enhancedControls" => {
            "Answers the enhancedControls preference section (scrubby slider acceleration, touch and trackpad gestures) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.rawDefaults" => {
            "Answers the rawDefaults preference section (raw develop default settings) as {section, title, values}; change values with prefs.set."
        }
        "edit.preferences.integrations" => {
            "Answers the integrations preference section (agent control and its port) as {section, title, values}; change values with prefs.set."
        }
        "edit.keyboardShortcuts" => {
            "Lists, sets or resets application keyboard shortcuts (`set` maps command ids to keys; `removeConflicts`); app-wide, not part of the document."
        }
        "edit.menus" => "Hides, shows or color-codes menu items by command id, or resets them; app-wide UI customisation.",
        "edit.toolbar" => "Sets which tools the toolbar shows and their `order`, or resets it; app-wide UI customisation.",
        "edit.fade" => "Fades the last filter, adjustment or paint stroke to `opacity` % with blend `mode`; run it right after that step.",
        "edit.purge.undo" => "Frees memory by dropping the last undo step of the active document; it can't be undone after. Answers {purged, bytes, message}.",
        "edit.purge.clipboard" => "Frees memory by emptying the clipboard. Answers {purged, bytes, message}.",
        "edit.purge.histories" => "Frees memory by dropping the undo history of every open document. Answers {purged, bytes, message}.",
        "edit.purge.videoCache" => "Frees memory held by cached video frames. Answers {purged, bytes, message}.",
        "edit.purge.all" => "Frees memory by purging undo, clipboard, histories and caches at once; history is lost. Answers {purged, bytes, message}.",
        "edit.smartFill" => {
            "Fills the selection with content synthesized from the surrounding image (`sampling` area, color and rotation adaptation, `scale`, `mirror`), to the current or a new layer; needs a selection."
        }
        "edit.deleteAndFillSelection" => "Deletes the selected pixels and fills the hole with smart fill in one step; needs a selection.",
        "edit.smartScale" => {
            "Resizes the active layer to `width`×`height` (or `scaleX`/`scaleY` %) while keeping important content undistorted; `protect` a channel or skin tones."
        }
        "edit.defineBrushPreset" => "Saves the selected pixels (else the layer) as a new brush tip preset named `name`.",
        "edit.defineCustomShape" => "Saves the work path or a saved path as a custom shape preset named `name`.",
        "edit.findAndReplaceText" => {
            "Finds `find` in the active type layer or all layers and replaces it with `replace` (`action` find, change, changeAll); case and whole-word options."
        }
        "edit.presets.presetManager" => "Lists, renames, deletes or reorders brush, custom shape or pattern presets (`kind`, `action`, `index`).",
        "edit.presets.exportImportPresets" => "Exports the chosen preset `kinds` as data, or imports presets from `data`.",
        "edit.fillForeground" => "Fills the selection (else the whole layer) of `layer` with the foreground color; `target` can be a channel or quick mask.",
        "edit.fillBackground" => "Fills the selection (else the whole layer) of `layer` with the background color; `target` can be a channel or quick mask.",
        "edit.fillForegroundPreserve" => "Fills with the foreground color only where the layer already has pixels, keeping transparency.",
        "edit.fillBackgroundPreserve" => "Fills with the background color only where the layer already has pixels, keeping transparency.",
        "file.sampleColor" => {
            "Answers the color at `x`,`y` px, averaged over `size` and sampled from the current layer, layers below, or all (`sampleLayer`); like the eyedropper."
        }
        "tool.decreaseBrushSize" => "Makes the current brush smaller by one step, as the [ key does.",
        "tool.increaseBrushSize" => "Makes the current brush larger by one step, as the ] key does.",
        "tool.decreaseBrushHardness" => "Makes the current brush softer by one step.",
        "tool.increaseBrushHardness" => "Makes the current brush harder by one step.",
        "layer.stampVisible" => "Merges every visible layer into a new pixel layer above the active one, keeping the originals; answers {layer}.",
        "layer.stampDown" => {
            "Merges a copy of the active layer into the pixel layer below it (several selected: into a new layer above them); answers {layer}."
        }
        "edit.autoAlignLayers" => {
            "Aligns the selected layers by their shared content (`projection` auto, perspective, cylindrical, …) to a reference; for panoramas or stacks."
        }
        "edit.autoBlendLayers" => {
            "Blends the aligned selected layers with masks into a seamless result: `method` panorama or focus `stack`; `seamlessTones` matches color."
        }
        "file.automate.photomerge" => {
            "The File › Automate › Photomerge command (changes the document); takes `paths`, `useOpenDocuments`, `layout`, `blend`, `vignetteRemoval`, `geometricCorrection`."
        }
        "file.automate.mergeToHdrPro" => {
            "The File › Automate › Merge to HDR Pro command (changes the document); takes `paths`, `useOpenDocuments`, `exposures`, `align`, `removeGhosts`, `ghostBase`."
        }
        "file.automate.cropAndStraightenPhotos" => "The File › Automate › Crop and Straighten Photos command (changes the document).",
        "filter.lensCorrection" => {
            "Corrects lens defects (active pixel layer, inside the selection if any): barrel/pincushion `distortion`, chromatic aberration, vignetting, rotation `angle` and perspective; optionally automatic from the profile."
        }
        "filter.adaptiveWideAngle" => {
            "Straightens lines curved by wide-angle or fisheye lenses (active pixel layer, inside the selection if any), from `model`, `focalLength` and line `constraints`."
        }
        "filter.rawDevelop" => {
            "Applies Raw Develop-style tone and color controls (active pixel layer, inside the selection if any): exposure, contrast, highlights, shadows, whites, blacks, clarity, vibrance, saturation, tone curve, temperature and tint."
        }
        "file.automate.lensCorrection" => "The File › Automate › Lens Correction command (changes the document); takes `input`, `output`, `format`, `quality`.",
        "filter.vanishingPoint" => {
            "Clones or pastes in perspective along defined `planes` (corners in px) (active pixel layer, inside the selection if any); `newLayer` keeps the result on its own layer."
        }
        "select.saveSelection" => "Saves the selection as a new alpha channel or into an existing one (`channel`), combining per `operation`.",
        "select.loadSelection" => {
            "Loads a channel, layer transparency or mask (`channel`) as the selection, combining it with the current one per `operation`, optionally `invert`ed."
        }
        "select.editInQuickMaskMode" => "Turns quick mask mode on or off (`on`, else toggle): the selection becomes a red overlay you can paint, and back.",
        "image.applyImage" => {
            "Blends a source layer and channel into the active layer or channel with `blending`, `opacity`, optional `mask`, `scale` and `offset`; channel-level compositing."
        }
        "image.calculations" => "Blends two source channels with `blending` and writes the result as a new channel, a new document or a selection (`result`).",
        "channel.new" => "Adds an alpha channel named `name`, filled black, white or from the selection, with its overlay `color` and `opacity`.",
        "channel.newSpot" => "Adds a spot color channel for a printing ink of `color` and `solidity` %, optionally from the selection.",
        "channel.duplicate" => "Copies `channel` (a color channel, alpha channel or quick mask) to a new channel or another document, optionally inverted.",
        "channel.delete" => "Deletes alpha or spot `channel` (default the targeted one).",
        "channel.rename" => "Renames alpha or spot `channel` to `name`.",
        "channel.move" => "Moves alpha or spot `channel` to position `to` in the Channels panel.",
        "channel.target" => "Makes `channel` the one edits apply to (composite, one color channel, or an alpha channel by index or name).",
        "channel.setVisible" => "Shows or hides `channel` in the view (`visible`, else toggle); pixels don't change.",
        "channel.options" => "Sets an alpha or spot channel's name, what it indicates (masked or selected areas, spot), overlay `color` and `opacity`.",
        "channel.mergeSpot" => "Merges spot `channel` into the color channels, simulating its ink, and deletes it; flattens the document.",
        "channel.split" => "Splits the document into one grayscale document per channel; flattens; `closeOriginal` closes the source.",
        "channel.merge" => "Merges open grayscale documents (`documents`) into one RGB, CMYK or Lab document, one per channel.",
        "channel.list" => "Answers the document's channels: color channels, alpha and spot channels, and which is targeted and visible.",
        "channel.target.composite" => "Targets the composite channel, so edits apply to all color channels.",
        "channel.target.slot3" => "Targets the channel in slot 3 of the Channels panel (the third row, as ⌘3 does), e.g. a color or alpha channel.",
        "channel.target.slot4" => "Targets the channel in slot 4 of the Channels panel (the fourth row, as ⌘4 does), e.g. a color or alpha channel.",
        "channel.target.slot5" => "Targets the channel in slot 5 of the Channels panel (the fifth row, as ⌘5 does), e.g. a color or alpha channel.",
        "channel.target.slot6" => "Targets the channel in slot 6 of the Channels panel (the sixth row, as ⌘6 does), e.g. a color or alpha channel.",
        "channel.target.slot7" => "Targets the channel in slot 7 of the Channels panel (the seventh row, as ⌘7 does), e.g. a color or alpha channel.",
        "channel.target.slot8" => "Targets the channel in slot 8 of the Channels panel (the eighth row, as ⌘8 does), e.g. a color or alpha channel.",
        "channel.target.slot9" => "Targets the channel in slot 9 of the Channels panel (the ninth row, as ⌘9 does), e.g. a color or alpha channel.",
        "image.adjustments.shadowsHighlights" => {
            "Lightens shadows and darkens highlights locally (amount, tone range and radius for each), with color and midtone controls; for backlit photos. Destructive."
        }
        "image.adjustments.replaceColor" => {
            "Selects pixels near `color` within `fuzziness` (0–200) and shifts their `hue`, `saturation` and `lightness`; destructive."
        }
        "image.adjustments.matchColor" => {
            "Matches the active layer's colors to a `source` document or layer (`luminance`, `intensity`, `fade`), or `neutralize` a cast; destructive."
        }
        "image.adjustments.hdrToning" => {
            "Tone-maps a 32-bit or flattened image: local `radius`/`strength` edge glow, `exposure`, `gamma`, `detail`, saturation and a tone `curve`; flattens the document."
        }
        "image.adjustments.listColorLookups" => {
            "Lists the color lookup tables (LUTs) available to layer.newAdjustmentLayer.colorLookup and image.adjustments.colorLookup."
        }
        "layer.layerMask.apply" => "Layer mask, Apply: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.layerMask.fromTransparency" => "Layer mask, From Transparency: acts on `layer`'s (else the active layer's) layer mask.",
        "layer.layerMask.hideSelection" => {
            "Adds a layer mask to `layer` (else the active layer) that hides the selection and shows the rest; the selection stays."
        }
        "layer.maskAllObjects" => {
            "Detects the main objects in `layer` (as select subject does) and masks the layer to them, hiding the background; all objects share one mask."
        }
        "layer.matting.defringe" => "Replaces the colors of edge pixels within `width` px with colors from inside, removing halos after a cut-out.",
        "layer.matting.removeBlackMatte" => "Removes the dark fringe left on a cut-out that was anti-aliased against black.",
        "layer.matting.removeWhiteMatte" => "Removes the light fringe left on a cut-out that was anti-aliased against white.",
        "layer.matting.colorDecontaminate" => {
            "Replaces background color bleeding into semi-transparent edge pixels with nearby foreground color; `amount` %, `radius` px."
        }
        "layer.stackMode.entropy" => "The Layer › Object Layers › Stack Mode › Entropy command (changes the document); takes `layer`.",
        "layer.stackMode.kurtosis" => "The Layer › Object Layers › Stack Mode › Kurtosis command (changes the document); takes `layer`.",
        "layer.stackMode.maximum" => "The Layer › Object Layers › Stack Mode › Maximum command (changes the document); takes `layer`.",
        "layer.stackMode.mean" => "The Layer › Object Layers › Stack Mode › Mean command (changes the document); takes `layer`.",
        "layer.stackMode.median" => "The Layer › Object Layers › Stack Mode › Median command (changes the document); takes `layer`.",
        "layer.stackMode.minimum" => "The Layer › Object Layers › Stack Mode › Minimum command (changes the document); takes `layer`.",
        "layer.stackMode.range" => "The Layer › Object Layers › Stack Mode › Range command (changes the document); takes `layer`.",
        "layer.stackMode.skewness" => "The Layer › Object Layers › Stack Mode › Skewness command (changes the document); takes `layer`.",
        "layer.stackMode.standardDeviation" => "The Layer › Object Layers › Stack Mode › Standard Deviation command (changes the document); takes `layer`.",
        "layer.stackMode.summation" => "The Layer › Object Layers › Stack Mode › Summation command (changes the document); takes `layer`.",
        "layer.stackMode.variance" => "The Layer › Object Layers › Stack Mode › Variance command (changes the document); takes `layer`.",
        "layer.stackMode.none" => "The Layer › Object Layers › Stack Mode › None command (changes the document); takes `layer`.",
        "layer.objectLayers.revealInFinder" => "The Layer › Object Layers › Reveal in Finder command (changes the document); takes `dryRun`.",
        "layer.layerStyle.blendingOptions" => {
            "Sets the blending options of `layer` (the active layer by default): blend mode, opacity and fillOpacity (0–100), Blend If ranges (`blendIf`, values 0–255; split points fade; null resets) and Advanced Blending (`channels` = which colour channels blend, R G B / C M Y K / L a b; knockout; interior effects, clipped layers as group, transparency shapes, masks hide effects). Kition Photo's defaults are knockout none, interior effects off, clipped layers as group on, transparency shapes on, masks hide effects off."
        }
        "layer.layerStyle.globalLight" => {
            "Sets the document's global light (`angle` −180–180°, `altitude` 0–90°), which every effect with Use Global Light (drop shadow, inner shadow, bevel & emboss) follows; answers {angle, altitude}."
        }
        "layer.layerStyle.convertToLayer" => {
            "Turns the layer effects of `layer` (the active layer by default) into pixel layers of their own, one per effect (below the layer, or clipped above it), and removes them from the layer; answers {layers} (how many were made)."
        }
        "layer.layerStyle.scaleEffects" => {
            "Scales the sizes and distances of every layer effect on `layer` (the active layer by default) by `scale` percent (1–1000), as Layer › Layer Style › Scale Effects; the effects' colours and angles stay."
        }
        "layer.layerStyle.transferEffects" => {
            "Moves the layer effects of layer `from` to layer `to` (dragging fx in the Layers panel); all of them by default, replacing `to`'s style, or only the one at index `effect`, replacing `to`'s effect of that kind; with `copy` they stay on `from` too."
        }
        "layer.layerContentOptions" => {
            "Answers which editor the active layer's content opens in ({layer, editor}: adjustment:<kind>, fill, text, objectLayer or shape); fails on pixel layers."
        }
        "layer.quickExportAsPng" => "The Layer › Quick Export as PNG command (changes the document); takes `layer`, `path`.",
        "layer.exportAs" => "The Layer › Export As command (changes the document); takes `layer`, `path`, `scale` (format from the path's extension).",
        "layer.setLabelColor" => "Sets the color label of `layer` in the Layers panel (`none` removes it); for organizing, no effect on pixels.",
        "image.rotation.arbitrary" => "Rotates the whole canvas and every layer by `angle` degrees in `direction`; the canvas grows to fit.",
        "image.mode.indexedColor" => {
            "Converts to indexed color with at most `colors` (2–256) from a `palette`, with `dither` and `forced` colors; for GIF-style images. Flattens."
        }
        "image.mode.colorTable" => "Edits the color table of an indexed-color document: a preset `table` or custom `entries`, and the `transparent` index.",
        "image.mode.bitmap" => {
            "Converts a grayscale document to 1-bit black and white by threshold, pattern, diffusion dither or halftone screen (`method`, `frequency`, `angle`, `shape`)."
        }
        "image.mode.duotone" => "Converts a grayscale document to mono/duo/tri/quadtone printed with `inks`, each a color and a curve (0–100).",
        "image.mode.multichannel" => "Converts to multichannel mode: flattens, and each color channel becomes a separate spot-style channel.",
        "edit.definePattern" => "Saves the selected rectangle `rect` (else the canvas) as a pattern preset named `name`; answers {pattern}.",
        "pattern.list" => "Answers the patterns available: id, name, size, mode and depth, and whether each is in the document or the library.",
        "pattern.rename" => "Renames pattern `pattern` (id or name) to `name`.",
        "pattern.delete" => "Deletes pattern `pattern` from the library; documents keep their copies.",
        "pattern.import" => "The Import Patterns command (changes the document); takes `path`.",
        "pattern.export" => "The Export Patterns command (changes the document); takes `path`, `patterns`.",
        "layer.newFillLayer.pattern" => "Adds a Pattern fill layer above the active layer (optionally masked by a path); answers its id.",
        "brush.texturePattern" => "Gives the current brush a Texture of pattern `pattern` at `scale` %, or turns it off (`enabled`).",
        "edit.transform.warp" => "Warps the active layer with a preset style (`bend`, distortion) or a custom `mesh`/`grid` of control points.",
        "layer.objectLayers.warp" => {
            "The Layer › Object Layers › Warp command (changes the document); takes `layer`, `rect`, `style`, `bend`, `hDistort`, `vDistort`."
        }
        "edit.transform.splitWarpCrosswise" => "Splits a custom warp grid both ways at `at`, adding control points; answers the updated warp.",
        "edit.transform.splitWarpHorizontally" => "Splits a custom warp grid with a horizontal line at `at`; answers the updated warp.",
        "edit.transform.splitWarpVertically" => "Splits a custom warp grid with a vertical line at `at`; answers the updated warp.",
        "edit.transform.removeWarpSplit" => "Removes the warp grid split nearest `at`; answers the updated warp.",
        "edit.transform.warpGrid" => "Sets a custom warp to a square grid of `size` patches (1–64); answers the updated warp.",
        "layerComp.new" => "Saves the current layer visibility, position and appearance as a layer comp named `name`; answers {comp}.",
        "layerComp.update" => "Re-records layer comp `comp` (default last applied, `*` for all) from the current layers, for all or some properties (`what`).",
        "layerComp.apply" => "Applies layer comp `comp`, restoring the layer states it recorded; answers {comp, missingLayers}.",
        "layerComp.previous" => "Applies the previous layer comp in the list.",
        "layerComp.next" => "Applies the next layer comp in the list.",
        "layerComp.delete" => "Deletes layer comp `comp` (default the last applied); the layers themselves are unchanged.",
        "layerComp.duplicate" => "Duplicates layer comp `comp`; answers {comp}.",
        "layerComp.rename" => "Renames layer comp `comp` to `name`.",
        "layerComp.setComment" => "Sets the comment of layer comp `comp`.",
        "layerComp.setOptions" => "Changes which properties layer comp `comp` records (visibility, position, appearance), its name and comment.",
        "layerComp.restoreLastDocumentState" => "Restores the layers to how they were before a layer comp was last applied.",
        "layerComp.updateWarnings" => "Answers layer comps that refer to deleted layers; `clear` drops those states. Answers {warnings, count}.",
        "layerComp.list" => "Answers the layer comps with their recorded properties, layer count, missing layers and which was applied last.",
        "file.export.layerCompsToFiles" => {
            "The File › Export › Layer Comps to Files command (reads, or changes only the view or selection); takes `dir`, `format`, `prefix`, `selectedOnly`, `comps`, `quality`; answers {files}."
        }
        "layer.new.artboard" => "Adds an artboard named `name` at `rect` or of a `preset` size, with a `background`; answers its id.",
        "layer.new.artboardFromGroup" => "Turns top-level group `layer` into an artboard fitted to its contents; answers {layer, rect}.",
        "layer.new.artboardFromLayers" => "Puts the selected layers on a new artboard fitted to them; answers {layer, rect}.",
        "layer.artboard.set" => "Changes artboard `layer`'s name, `rect` or `preset` size and background; `moveContents` moves its layers with it.",
        "view.clearSelectedArtboardGuides" => "Deletes the guides inside the active artboard.",
        "file.export.artboardsToFiles" => {
            "The File › Export › Artboards to Files command (reads, or changes only the view or selection); takes `dir`, `format`, `prefix`, `artboards`, `quality`; answers {files}."
        }
        "file.export.artboardsToPdf" => {
            "The File › Export › Artboards to PDF command (reads, or changes only the view or selection); takes `path`, `artboards`, `quality` (one raster page per board); answers {path, pages}."
        }
        "filter.liquify" => {
            "Pushes, twirls, puckers or bloats pixels of `layer` along brush `strokes`, as the Liquify dialog does; `meshSize` sets the field resolution."
        }
        "edit.pinWarp" => {
            "Deforms the active layer by moving `pins` on a triangle mesh (`mode` rigid, normal, distort; mesh `density`); pins not moved hold their area in place."
        }
        "layer.objectLayers.pinWarp" => {
            "The Layer › Object Layers › Pin Warp command (changes the document); takes `pins`, `src`, `dst`, `rotate`, `depth`, `mode` (mesh over the opaque region, as-rigid-as-possible; on an object layer it becomes an editable filter)."
        }
        "edit.perspectiveWarp" => "Changes the perspective of the active layer by moving the corners of defined `planes`; optionally straightens lines.",
        "layer.objectLayers.perspectiveWarp" => {
            "The Layer › Object Layers › Perspective Warp command (changes the document); takes `planes`, `src`, `dst`, `straighten`, `interpolation`, `layer`."
        }
        "image.analysis.setMeasurementScale" => {
            "Sets how many pixels equal one logical unit (`pixelLength` px = `logicalLength` `units`), used by measurements and the ruler."
        }
        "image.analysis.selectDataPoints" => {
            "Chooses which values (area, perimeter, length, angle, count, …) recordMeasurements writes for selections, the ruler and counts."
        }
        "image.analysis.recordMeasurements" => {
            "Measures the selection, ruler line or counts (`source`) at the measurement scale and appends rows to the Measurement Log; answers the rows."
        }
        "image.analysis.rulerTool" => {
            "Sets the ruler line from `start` to `end` (and an optional `protractor` arm) or clears it; with no params reads it. Answers position, size, angle and lengths."
        }
        "image.analysis.countTool" => "Answers the count groups and their markers; edit them with count.* commands.",
        "image.analysis.placeScaleMarker" => "Adds a scale bar of `length` logical units with optional text to the document, as a layer group.",
        "image.analysis.straightenLayer" => {
            "Rotates the active layer (or, with `crop`, the canvas) so the ruler line becomes level; set the ruler first with image.analysis.rulerTool."
        }
        "image.analysis.info" => "Answers the measurement scale, ruler readout, count groups, and note and log counts.",
        "count.add" => "Adds a count marker at `x`,`y` px to the active (or `group`) count group, for counting objects.",
        "count.remove" => "Removes count marker `index`, or the marker nearest `x`,`y` within `radius`.",
        "count.move" => "Moves a count marker (by `index` or position) to `to`.",
        "count.clear" => "Removes all markers from a count group, or from every group (`all`).",
        "count.newGroup" => "Adds a count group with `name` and marker `color`.",
        "count.deleteGroup" => "Deletes count group `group` and its markers.",
        "count.setGroup" => "Changes count group `group`: name, color, marker and label size, visibility, or makes it active.",
        "measurementLog.list" => "Answers the Measurement Log rows and columns recorded by image.analysis.recordMeasurements.",
        "measurementLog.delete" => "Deletes Measurement Log `rows` by id, or all rows.",
        "measurementLog.export" => "The Export Measurements command (changes the document); takes `path`, `rows`.",
        "note.add" => "Adds an annotation note at `x`,`y` px with `text`, `author` and `color`; notes are saved with the document but not rendered.",
        "note.set" => "Changes note `index`: text, author, color, position or whether it is open.",
        "note.delete" => "Deletes note `index`, or every note (`all`).",
        "note.list" => "Answers the document's notes: index, author, text, color, position, open state and modified time.",
        "file.import.notes" => "The File › Import › Notes command (changes the document); takes `path`.",
        "history.goTo" => "Undoes or redoes to History state `state` (0 = oldest) in one command; answers {current}.",
        "history.deleteState" => "Deletes History state `state` (default current) and every later one; answers {states, current}.",
        "history.newDocument" => "Creates a new document from History state `state` (default current); answers {document, name}.",
        "view.proofSetup.workingCyanPlate" => {
            "Soft-proofs the view as the working CMYK cyan plate alone and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.workingMagentaPlate" => {
            "Soft-proofs the view as the working CMYK magenta plate alone and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.workingYellowPlate" => {
            "Soft-proofs the view as the working CMYK yellow plate alone and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.workingBlackPlate" => {
            "Soft-proofs the view as the working CMYK black plate alone and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.workingCmyPlate" => {
            "Soft-proofs the view as the working CMYK cyan, magenta and yellow plates without black and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.legacyMacintoshRgb" => {
            "Soft-proofs the view as legacy Macintosh RGB (gamma 1.8) display and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.colorBlindnessProtanopia" => {
            "Soft-proofs the view as protanopia (red-blind) color vision and turns proof colors on; view only, pixels don't change."
        }
        "view.proofSetup.colorBlindnessDeuteranopia" => {
            "Soft-proofs the view as deuteranopia (green-blind) color vision and turns proof colors on; view only, pixels don't change."
        }
        "view.thirtyTwoBitPreviewOptions" => "Sets how a 32-bit document is shown on screen: `method` exposure-and-gamma or highlight compression; view only.",
        "gradient.presets.list" => "Answers the gradient preset library by group (stops and transparency) and the current gradient.",
        "gradient.presets.select" => {
            "Makes gradient preset `preset` (or custom `stops`) current for the Gradient tool and new fills; can also change a selected gradient fill layer."
        }
        "gradient.presets.stop" => "Edits one color or opacity stop of the current gradient (`action`, `kind`, `index`, `location`, `color`).",
        "gradient.presets.apply" => "Adds a gradient fill layer with preset `preset` (or `stops`) at `angle` in `style`; answers the layer.",
        "gradient.presets.new" => "Saves `stops` (default the current gradient) as gradient preset `name` in `group`.",
        "gradient.presets.edit" => {
            "Edits the preset library (`action`: rename, delete, move, newGroup, renameGroup, deleteGroup on `preset`/`group`); app-wide, not the document."
        }
        "gradient.presets.reset" => "Restores the default gradient presets, or adds them with `append`.",
        "pattern.presets.list" => "Answers the pattern preset library by group and the selected pattern.",
        "pattern.presets.select" => {
            "Selects pattern `pattern` as the default for fills and new pattern fill layers; can also change a selected pattern fill layer."
        }
        "pattern.presets.apply" => "Adds a pattern fill layer with `pattern` at `scale` % and `angle`; answers {layer}.",
        "pattern.presets.new" => "Defines a pattern preset from the selection or canvas (`rect`) named `name` in `group`.",
        "pattern.presets.edit" => {
            "Edits the preset library (`action`: rename, delete, move, newGroup, renameGroup, deleteGroup on `preset`/`group`); app-wide, not the document."
        }
        "style.presets.list" => "Answers the layer style preset library by group: each preset's effects, blend mode and fill opacity.",
        "style.presets.apply" => "Applies layer style preset `preset` to `layers` (default the selected ones), replacing their effects unless `add`.",
        "style.presets.new" => "Saves the effects (and blending) of `layer`, or an explicit `effects` list, as a style preset named `name`.",
        "style.presets.edit" => {
            "Edits the preset library (`action`: rename, delete, move, newGroup, renameGroup, deleteGroup on `preset`/`group`); app-wide, not the document."
        }
        "style.presets.reset" => "Restores the default style presets, or adds them with `append`.",
        "shape.presets.list" => "Answers the custom shape preset library by group.",
        "shape.presets.place" => {
            "Adds custom shape `preset` as a new shape layer in `rect` (default centered, half the canvas) with `fill` and `stroke`, or adds it to an existing shape."
        }
        "shape.presets.new" => "Saves a path (work path, saved path, shape layer or path string) as custom shape preset `name`.",
        "shape.presets.edit" => {
            "Edits the preset library (`action`: rename, delete, move, newGroup, renameGroup, deleteGroup on `preset`/`group`); app-wide, not the document."
        }
        "shape.presets.reset" => "Restores the default custom shapes, or adds them with `append`.",
        "tool.presets.list" => "Answers the tool presets (name and tool), optionally only for `tool`.",
        "tool.presets.new" => "Saves tool `tool`'s `options` (and the current brush, `includeColor`) as tool preset `name`.",
        "tool.presets.select" => "Applies tool preset `preset`: its brush and color become current; answers {name, tool, options}.",
        "tool.presets.edit" => "Renames or deletes tool presets (`action`, `preset`).",
        "tool.presets.reset" => "Restores the default tool presets, removing presets saved with tool.presets.new.",
        "cloneSource.list" => "Answers the five clone sources (source point, offset, scale, rotation, flip, layer) and the overlay settings.",
        "cloneSource.select" => "Makes clone source `index` (0–4) the active one for the Clone Stamp and Healing Brush.",
        "cloneSource.set" => "Sets clone source `index`: its `source` point, `offset`, scale, `rotation` and flips.",
        "cloneSource.resetTransform" => "Resets the scale, rotation and flips of clone source `index`.",
        "cloneSource.overlay" => "Sets the clone source overlay (shown while cloning): visibility, opacity, clipping, auto-hide, invert and blend.",
        "file.presets.list" => "Answers the saved new-document presets (size, resolution, mode); use one with file.presets.get and file.new.",
        "file.presets.get" => "Answers new-document preset `name` and the file.new params it stands for; creates nothing.",
        "file.presets.save" => "Saves new-document `settings` (size, resolution, mode, background) as preset `name`.",
        "file.presets.delete" => "Deletes the saved new-document preset `name`; existing documents are unchanged.",
        "filter.render.flame" => {
            "Draws flames of `flameType` (along a path, candle light, …) onto the active layer, with length, width, color, lines and shape settings; `path` is a saved path's name, \"work\", SVG path data (\"M 20 110 L 20 20\") or a path object (default: the work path, else an arc across the lower canvas). Answers how many flames it drew; past its primitive limit it says so (`truncated`, `note`) instead of stopping silently."
        }
        "filter.render.pictureFrame" => {
            "Draws a decorative frame (vines, flowers, waves, …) around the canvas onto the active layer, with color, size and arrangement settings."
        }
        "filter.render.tree" => "Draws a tree of `baseTreeType` (1–34) onto the active layer, with leaf, branch and size settings.",
        "slice.new" => "Adds a user web slice at `rect` with slice options (name, kind, URL, target, alt text); answers the slice.",
        "slice.fromGuides" => "Replaces every slice with the grid made by the canvas guides; answers {slices}.",
        "slice.set" => "Changes slice options of `slice` (or auto slice `number`, which becomes a user slice): name, kind, URL, target, alt, cell text.",
        "slice.promote" => "Turns an auto or layer-based slice into a user slice that can be edited; answers {slice}.",
        "slice.delete" => "Deletes `slice`, slice `number`, or several `slices`; answers {deleted}.",
        "slice.divide" => "Divides `slice` into `horizontal` rows and `vertical` columns of equal size; answers {slices}.",
        "slice.list" => {
            "Answers every slice (auto, layer-based and user): number, id, origin, name, rect, kind, layer, URL, alt, and whether slices are locked."
        }
        "layer.newLayerBasedSlice" => {
            "Adds a web slice that follows `layer`'s bounds (effects included), with optional `name`, `url` and `alt`; answers the slice."
        }
        "view.lockSlices" => "Locks or unlocks all slices so they can't be moved or resized (`on`, else toggle); answers {locked}.",
        "view.clearSlices" => "Deletes every user and layer-based slice; answers {cleared}.",
        "file.export.saveForWebLegacy" => {
            "The File › Export › Save for Web (Legacy) command (reads, or changes only the view or selection); takes `preset`, `format`, `palette`, `colors`, `dither`, `ditherAmount`."
        }
        "file.export.exportPreferences" => "Sets the quick-export format, location, JPEG and WebP quality and metadata; app-wide.",
        "file.export.quickExport" => "The Quick Export command (reads, or changes only the view or selection); takes `path`; answers {path, format, bytes}.",
        "file.generate.imageAssets" => "The File › Generate › Image Assets command (changes the document); takes `on`, `dir`.",
        "file.automate.contactSheetII" => {
            "The File › Automate › Contact Sheet II command (changes the document); takes `input`, `units`, `width`, `height`, `resolution`, `mode`."
        }
        "file.automate.addDroplet" => {
            "The File › Automate › Create Droplet command (changes the document); takes `path`, `steps`, `name`, `output`, `format`, `quality`."
        }
        "file.automate.runDroplet" => {
            "The Run Droplet command (changes the document); takes `droplet`, `input`, `output` (an input whose output name was already written in the run goes to errors)."
        }
        "file.scripts.statistics" => "The File › Scripts › Statistics command (changes the document); takes `mode`, `input`, `align`.",
        "file.scripts.browse" => "The File › Scripts › Browse command (changes the document); takes `path`, `script`, `steps`.",
        "file.scripts.scriptEventsManager" => {
            "The File › Scripts › Script Events Manager command (changes the document); takes `enabled`, `add`, `event`, `script`, `steps`, `name`."
        }
        "file.print" => {
            "The File › Print command (reads, or changes only the view or selection); takes `printer`, `copies`, `paper`, `orientation`, `center`, `top`; answers {pdf, imageRect, command, sent}."
        }
        "file.printOneCopy" => {
            "The File › Print One Copy command (reads, or changes only the view or selection) (the last Print settings, one copy; any Print key overrides)."
        }
        "file.package" => {
            "The File › Package command (changes the document); takes `dir`, `format` (copies the document and its linked files into <dir>/<name>/, relinked to Links/)."
        }
        "file.export.pathsToIllustrator" => {
            "The File › Export › Paths to Illustrator command (reads, or changes only the view or selection); takes `path`, `paths`; answers {path, paths}."
        }
        "layer.pickAt" => {
            "Finds the topmost layer (or group, `target`) with pixels at `x`,`y` px and selects it with `select`; `list: true` answers every layer there, topmost first."
        }
        "layer.selectAbove" => "Makes the layer one row above the active layer in the Layers panel active (collapsed groups count as one row).",
        "layer.selectBelow" => "Makes the layer one row below the active layer in the Layers panel active (collapsed groups count as one row).",
        "layer.selectTop" => "Makes the top row of the Layers panel the active layer.",
        "layer.selectBottom" => "Makes the bottom row of the Layers panel the active layer.",
        "layer.addAboveToSelection" => "Adds the layer one row above the active layer to the layer selection.",
        "layer.addBelowToSelection" => "Adds the layer one row below the active layer to the layer selection.",
        "layer.new.frameFromLayers" => "Groups the selected layers and clips them to a frame rectangle (placeholder frame); answers {layer, frame}.",
        "edit.presets.migratePresets" => {
            "The Edit › Presets › Migrate Presets command (reads, or changes only the view or selection); answers {migrated, added:{gradients,styles,shapes,patternGroups,toolPresets}."
        }
        "image.trap" => "Spreads inks at color edges by `width` px to hide registration gaps in print; CMYK documents only. Answers {trapped, width}.",
        "timeline.add" => "Creates a video timeline of `duration` frames at `fps` for the document; answers {timeline}.",
        "timeline.delete" => "Removes the document's timeline; answers {timeline: null}.",
        "timeline.setFrame" => "Moves the timeline playhead to `frame`; video layers show that frame. Answers {timeline}.",
        "timeline.nextFrame" => "Moves the playhead one frame forward; answers {timeline}.",
        "timeline.previousFrame" => "Moves the playhead one frame back; answers {timeline}.",
        "timeline.setProps" => "Changes the timeline's `fps`, `duration` and work area (`workStart`, `workEnd`); answers {timeline}.",
        "timeline.info" => "Answers the document's timeline: frame rate, duration, playhead and work area.",
        "layer.videoLayers.newBlankVideoLayer" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Adds a new empty video layer with one blank frame above the active layer."
        }
        "layer.videoLayers.insertBlankFrame" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Inserts a blank frame at the playhead into the active video layer; fails on other layers."
        }
        "layer.videoLayers.duplicateFrame" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Duplicates the active video layer's frame at the playhead into the next frame."
        }
        "layer.videoLayers.deleteFrame" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Deletes the active video layer's frame at the playhead."
        }
        "layer.videoLayers.restoreFrame" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Discards edits to the active video layer's frame at the playhead, restoring the original footage frame."
        }
        "layer.videoLayers.restoreAllFrames" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Discards edits to every frame of the active video layer, restoring the original footage."
        }
        "layer.videoLayers.showAlteredVideo" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Toggles whether the active video layer shows its edited frames or the original footage."
        }
        "layer.videoLayers.rasterize" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Drops the active video layer's frame stack, keeping the current frame as a normal pixel layer."
        }
        "layer.rasterize.video" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Same as layer.videoLayers.rasterize: keeps the current frame of the active video layer as a pixel layer."
        }
        "layer.videoLayers.newVideoLayerFromFile" => "The Layer › Video Layers › New Video Layer from File command (changes the document).",
        "layer.videoLayers.replaceFootage" => "The Layer › Video Layers › Replace Footage command (changes the document).",
        "layer.videoLayers.interpretFootage" => {
            "Video layers keep a stack of frames; the layer shows the frame at the timeline playhead. Sets the frame rate `fps` at which the active video layer's footage plays."
        }
        "layer.videoLayers.reloadFrame" => "The Layer › Video Layers › Reload Frame command (changes the document).",
        "file.import.videoFramesToLayers" => "The File › Import › Video Frames to Layers command (changes the document).",
        "file.export.renderVideo" => "The File › Export › Render Video command (reads, or changes only the view or selection); answers {}.",
        "job.list" => "Answers {jobs}: the background jobs (long filters and exports), running and recently finished, with their ids and progress.",
        "job.cancel" => "Cancels background job `job`, or every running job.",
        "file.import.wiaSupport" => {
            "The File › Import › WIA Support command (reads, or changes only the view or selection); answers {available, devices, note}."
        }
        "image.variables.define" => {
            "Defines data-driven variables bound to layers (visibility, text replacement or pixel replacement); answers the definitions."
        }
        "image.variables.dataSets" => "Sets or appends data sets: named values for each variable; answers definitions and data sets.",
        "image.applyDataSet" => "Applies a data set by `name` or `index`, setting layer visibility, text and pixels as one history step.",
        "file.import.variableDataSets" => "The File › Import › Variable Data Sets command (changes the document).",
        "file.export.dataSetsAsFiles" => {
            "The File › Export › Data Sets as Files command (reads, or changes only the view or selection); answers {files,count}."
        }
        "variable.list" => "Answers the document's variable definitions, data sets and the active data set.",
        "plugin.list" => "Answers the installed WebAssembly plug-ins with their ids and parameters.",
        "plugin.install" => {
            "The Filter › Plug-ins › Install Plug-in command (reads, or changes only the view or selection); takes `path`, `data`, `replace` (path: a .wasm file, native only; data: the module as base64)."
        }
        "plugin.remove" => "Uninstalls WebAssembly plug-in `id` (see plugin.list); its filters disappear from the menus.",
        "plugin.reload" => {
            "The Reload Plug-ins command (reads, or changes only the view or selection); takes `path` (a folder of .wasm plug-ins; default: the Plug-ins preference folder)."
        }
        "plugin.run" => "Runs plug-in `id` on the active document with `params` (see plugin.list).",
        "layer.setExpanded" => {
            "Expands or collapses group `layer` in the Layers panel (`expanded`, else toggle; `all` for every group); view state only, not an undo step."
        }
        "layer.setEffectsExpanded" => "Shows or hides the list of effects under `layer` in the Layers panel (`all` for every layer); view state only.",
        "layer.setEffectsVisible" => {
            "Shows or hides all effects of `layer`, or only the effect at `index` in its list (`visible`, else toggle); changes the render."
        }
        "view.layerMask" => {
            "Shows the mask of `layer` alone in gray, as a red overlay, or turns that off (`mode`); view only, painting while shown paints the mask."
        }
        "layer.toggleMaskOverlay" => {
            "Shows or hides the active layer's mask as a 50% red overlay on the composite; view only, and painting while it shows paints the mask."
        }
        "action.move" => "Moves an action, or step `step` within it, to zero-based position `to`.",
        "action.list" => "Answers the recorded actions with their step counts, and which one is recording.",
        "action.get" => "Answers action `action` as {name, steps: [[command id, params], …]}, the form file.automate.batch takes.",
        "action.record" => "Starts recording commands into a new action `name`, or appends to `action`; stop with action.stop. Answers {action, index}.",
        "action.stop" => "Stops recording; answers {action, steps}. Queries and action-editing commands are not recorded.",
        "action.play" => "Runs action `action` from step `from` (0-based), stopping at the first error; answers {action, ran, failed?}.",
        "action.delete" => "Deletes action `action`, or only step `step` of it.",
        "file.exportImage" => "Answers the flattened document encoded as PNG, JPEG, TIFF or WebP (`format`) as base64 {data, mime, bytes}; writes no file.",
        _ => "",
    }
}

fn output(id: &str) -> Option<Value> {
    let schema = match id {
        "session.inspect" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"active":{"type":"integer"},"background":{"type":"array","items":{"type":"number"}},"documents":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"dirty":{"type":"boolean"},"height":{"type":"integer"},"index":{"type":"integer"},"name":{"type":"string"},"path":{"type":"string"},"revision":{"type":"integer"},"width":{"type":"integer"}}}},"foreground":{"type":"array","items":{"type":"number"}}}}"#
        }
        "file.inspect" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"activeLayer":{"type":"integer"},"canRedo":{"type":"boolean"},"canUndo":{"type":"boolean"},"channels":{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"json"}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"kind":{"type":"string"}}}}},"depth":{"type":"integer"},"effectsCollapsed":{"type":"array"},"hasSelection":{"type":"boolean"},"height":{"type":"integer"},"history":{"type":"array","items":{"type":"string"}},"historyIndex":{"type":"integer"},"historyRedo":{"type":"array"},"isolatedLayers":{"type":"array"},"lastAppliedComp":{"type":"json"},"layerComps":{"type":"array"},"layerMaskView":{"type":"json"},"layers":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"blend":{"type":"string"},"bounds":{"type":"json"},"clipped":{"type":"boolean"},"fill":{"type":"number"},"hasMask":{"type":"boolean"},"id":{"type":"integer"},"kind":{"type":"string"},"labelColor":{"type":"string"},"linkGroup":{"type":"json"},"locks":{"type":"json"},"maskEnabled":{"type":"json"},"maskLinked":{"type":"json"},"name":{"type":"string"},"opacity":{"type":"number"},"selected":{"type":"boolean"},"visible":{"type":"boolean"}}}},"measurement":{"type":"object","additionalProperties":true,"properties":{"count":{"type":"integer"},"countGroups":{"type":"integer"},"ruler":{"type":"json"},"scale":{"type":"string"}}},"mode":{"type":"string"},"name":{"type":"string"},"notes":{"type":"array"},"quickMask":{"type":"boolean"},"resolution":{"type":"number"},"revision":{"type":"integer"},"selectedLayers":{"type":"array","items":{"type":"integer"}},"selectionBounds":{"type":"json"},"symmetry":{"type":"json"},"timeline":{"type":"json"},"variables":{"type":"json"},"width":{"type":"integer"}}}"#
        }
        "command.list" => {
            r#"{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"id":{"type":"string"},"label":{"type":"string"},"menu":{"type":"array","items":{"type":"string"}},"params":{"type":"string"},"shortcut":{"type":"json"}}}}"#
        }
        "file.pixel" => r#"{"type":"array","items":{"type":"number"}}"#,
        "file.render" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"png":{"type":"string"},"width":{"type":"integer"},"height":{"type":"integer"},"scale":{"type":"array","items":{"type":"number"}},"region":{"type":"array","items":{"type":"integer"}},"mapping":{"type":"string"},"drawn":{"type":"json"},"background":{"type":"string"}}}"#
        }
        "image.measure" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"json"},"region":{"type":"array","items":{"type":"integer"}},"bounds":{"type":"json"},"pixels":{"type":"integer"},"mean":{"type":"json"},"histogram":{"type":"array","items":{"type":"integer"}},"overlap":{"type":"json"}}}"#
        }
        "text.fonts" => r#"{"type":"object","additionalProperties":true,"properties":{"families":{"type":"array","items":{"type":"string"}}}}"#,
        "path.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"clippingPath":{"type":"json"},"layerPath":{"type":"json"},"paths":{"type":"array"},"workPath":{"type":"json"}}}"#
        }
        "layer.vectorMask.info" => r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"vectorMask":{"type":"json"}}}"#,
        "select.subject" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bounds":{"type":"array","items":{"type":"integer"}},"changed":{"type":"boolean"},"selected":{"type":"boolean"}}}"#
        }
        "select.focusArea" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bounds":{"type":"array","items":{"type":"integer"}},"changed":{"type":"boolean"},"selected":{"type":"boolean"}}}"#
        }
        "edit.copy" => r#"{"type":"object","additionalProperties":true,"properties":{"bounds":{"type":"array","items":{"type":"integer"}}}}"#,
        "edit.copyMerged" => r#"{"type":"object","additionalProperties":true,"properties":{"bounds":{"type":"array","items":{"type":"integer"}}}}"#,
        "edit.colorSettings" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"displays":{"type":"array"},"monitor":{"type":"string"},"monitorDetected":{"type":"boolean"},"monitorStatus":{"type":"object","additionalProperties":true,"properties":{"detection":{"type":"object","additionalProperties":true,"properties":{"state":{"type":"string"}}},"display":{"type":"json"},"fingerprint":{"type":"string"},"profile":{"type":"string"},"profileName":{"type":"json"},"reason":{"type":"string"},"requested":{"type":"string"},"source":{"type":"string"}}},"settings":{"type":"object","additionalProperties":true,"properties":{"askOnMismatch":{"type":"boolean"},"askOnMissing":{"type":"boolean"},"askOnPaste":{"type":"boolean"},"blendTextGamma":{"type":"number"},"bpc":{"type":"boolean"},"dither":{"type":"boolean"},"intent":{"type":"string"},"monitorProfile":{"type":"string"},"policyCmyk":{"type":"string"},"policyGray":{"type":"string"},"policyRgb":{"type":"string"},"workingCmyk":{"type":"string"},"workingGray":{"type":"string"},"workingRgb":{"type":"string"}}},"working":{"type":"object","additionalProperties":true,"properties":{"cmyk":{"type":"string"},"gray":{"type":"string"},"rgb":{"type":"string"}}}}}"#
        }
        "edit.profileInfo" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"builtins":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"colorSpace":{"type":"string"},"description":{"type":"string"},"id":{"type":"string"}}}},"profile":{"type":"object","additionalProperties":true,"properties":{"bytes":{"type":"integer"},"class":{"type":"string"},"colorSpace":{"type":"string"},"description":{"type":"string"},"embedded":{"type":"boolean"},"intents":{"type":"array","items":{"type":"string"}},"lut":{"type":"boolean"},"version":{"type":"string"}}}}}"#
        }
        "view.proofSetup" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bpc":{"type":"boolean"},"intent":{"type":"string"},"profile":{"type":"string"},"proofColors":{"type":"boolean"},"simulatePaper":{"type":"boolean"}}}"#
        }
        "view.proofColors" => r#"{"type":"object","additionalProperties":true,"properties":{"on":{"type":"boolean"},"profile":{"type":"string"}}}"#,
        "view.gamutWarning" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"fraction":{"type":"number"},"on":{"type":"boolean"},"outOfGamut":{"type":"integer"},"profile":{"type":"string"},"threshold":{"type":"number"}}}"#
        }
        "brush.presets.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"presets":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"builtin":{"type":"boolean"},"hardness":{"type":"number"},"name":{"type":"string"},"size":{"type":"number"},"tip":{"type":"string"}}}}}}"#
        }
        "brush.get" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"aliased":{"type":"boolean"},"angle":{"type":"number"},"background":{"type":"array","items":{"type":"number"}},"buildUp":{"type":"boolean"},"buildUpRate":{"type":"number"},"color":{"type":"array","items":{"type":"number"}},"colorDynamics":{"type":"object","additionalProperties":true,"properties":{"brightnessJitter":{"type":"number"},"enabled":{"type":"boolean"},"fgBg":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"hueJitter":{"type":"number"},"perTip":{"type":"boolean"},"purity":{"type":"number"},"saturationJitter":{"type":"number"}}},"dualBrush":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"number"},"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"enabled":{"type":"boolean"},"flip":{"type":"boolean"},"hardness":{"type":"number"},"mode":{"type":"string"},"roundness":{"type":"number"},"scatter":{"type":"number"},"size":{"type":"number"},"spacing":{"type":"number"},"tip":{"type":"string"}}},"erase":{"type":"boolean"},"flipX":{"type":"boolean"},"flipY":{"type":"boolean"},"flow":{"type":"number"},"hardness":{"type":"number"},"locks":{"type":"object","additionalProperties":true,"properties":{"buildUp":{"type":"boolean"},"colorDynamics":{"type":"boolean"},"dualBrush":{"type":"boolean"},"noise":{"type":"boolean"},"pose":{"type":"boolean"},"protectTexture":{"type":"boolean"},"scattering":{"type":"boolean"},"shapeDynamics":{"type":"boolean"},"smoothing":{"type":"boolean"},"texture":{"type":"boolean"},"transfer":{"type":"boolean"},"wetEdges":{"type":"boolean"}}},"mixer":{"type":"object","additionalProperties":true,"properties":{"flow":{"type":"number"},"load":{"type":"number"},"mix":{"type":"number"},"sampleAllLayers":{"type":"boolean"},"wet":{"type":"number"}}},"mode":{"type":"string"},"noise":{"type":"boolean"},"opacity":{"type":"number"},"pose":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"overridePressure":{"type":"boolean"},"overrideRotation":{"type":"boolean"},"overrideTilt":{"type":"boolean"},"pressure":{"type":"number"},"rotation":{"type":"number"},"tiltX":{"type":"number"},"tiltY":{"type":"number"}}},"pressureOpacity":{"type":"boolean"},"pressureSize":{"type":"boolean"},"protectTexture":{"type":"boolean"},"roundness":{"type":"number"},"scattering":{"type":"object","additionalProperties":true,"properties":{"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"countJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"enabled":{"type":"boolean"},"scatter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"seed":{"type":"integer"},"shapeDynamics":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"brushProjection":{"type":"boolean"},"enabled":{"type":"boolean"},"flipXJitter":{"type":"boolean"},"flipYJitter":{"type":"boolean"},"roundness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"size":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"tiltScale":{"type":"number"}}},"size":{"type":"number"},"smoothing":{"type":"object","additionalProperties":true,"properties":{"adjustForZoom":{"type":"boolean"},"amount":{"type":"number"},"catchUp":{"type":"boolean"},"catchUpOnEnd":{"type":"boolean"},"pulledString":{"type":"boolean"}}},"spacing":{"type":"number"},"spacingEnabled":{"type":"boolean"},"texture":{"type":"object","additionalProperties":true,"properties":{"brightness":{"type":"number"},"contrast":{"type":"number"},"depth":{"type":"number"},"depthJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"eachTip":{"type":"boolean"},"enabled":{"type":"boolean"},"invert":{"type":"boolean"},"mode":{"type":"string"},"pattern":{"type":"object","additionalProperties":true,"properties":{"procedural":{"type":"json"}}},"scale":{"type":"number"}}},"tip":{"type":"string"},"transfer":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"flow":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"mix":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"opacity":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"wetness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"wetEdges":{"type":"boolean"}}}"#
        }
        "tool.setBrush" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"aliased":{"type":"boolean"},"angle":{"type":"number"},"background":{"type":"array","items":{"type":"number"}},"buildUp":{"type":"boolean"},"buildUpRate":{"type":"number"},"color":{"type":"array","items":{"type":"number"}},"colorDynamics":{"type":"object","additionalProperties":true,"properties":{"brightnessJitter":{"type":"number"},"enabled":{"type":"boolean"},"fgBg":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"hueJitter":{"type":"number"},"perTip":{"type":"boolean"},"purity":{"type":"number"},"saturationJitter":{"type":"number"}}},"dualBrush":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"number"},"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"enabled":{"type":"boolean"},"flip":{"type":"boolean"},"hardness":{"type":"number"},"mode":{"type":"string"},"roundness":{"type":"number"},"scatter":{"type":"number"},"size":{"type":"number"},"spacing":{"type":"number"},"tip":{"type":"string"}}},"erase":{"type":"boolean"},"flipX":{"type":"boolean"},"flipY":{"type":"boolean"},"flow":{"type":"number"},"hardness":{"type":"number"},"locks":{"type":"object","additionalProperties":true,"properties":{"buildUp":{"type":"boolean"},"colorDynamics":{"type":"boolean"},"dualBrush":{"type":"boolean"},"noise":{"type":"boolean"},"pose":{"type":"boolean"},"protectTexture":{"type":"boolean"},"scattering":{"type":"boolean"},"shapeDynamics":{"type":"boolean"},"smoothing":{"type":"boolean"},"texture":{"type":"boolean"},"transfer":{"type":"boolean"},"wetEdges":{"type":"boolean"}}},"mixer":{"type":"object","additionalProperties":true,"properties":{"flow":{"type":"number"},"load":{"type":"number"},"mix":{"type":"number"},"sampleAllLayers":{"type":"boolean"},"wet":{"type":"number"}}},"mode":{"type":"string"},"noise":{"type":"boolean"},"opacity":{"type":"number"},"pose":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"overridePressure":{"type":"boolean"},"overrideRotation":{"type":"boolean"},"overrideTilt":{"type":"boolean"},"pressure":{"type":"number"},"rotation":{"type":"number"},"tiltX":{"type":"number"},"tiltY":{"type":"number"}}},"pressureOpacity":{"type":"boolean"},"pressureSize":{"type":"boolean"},"protectTexture":{"type":"boolean"},"roundness":{"type":"number"},"scattering":{"type":"object","additionalProperties":true,"properties":{"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"countJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"enabled":{"type":"boolean"},"scatter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"seed":{"type":"integer"},"shapeDynamics":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"brushProjection":{"type":"boolean"},"enabled":{"type":"boolean"},"flipXJitter":{"type":"boolean"},"flipYJitter":{"type":"boolean"},"roundness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"size":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"tiltScale":{"type":"number"}}},"size":{"type":"number"},"smoothing":{"type":"object","additionalProperties":true,"properties":{"adjustForZoom":{"type":"boolean"},"amount":{"type":"number"},"catchUp":{"type":"boolean"},"catchUpOnEnd":{"type":"boolean"},"pulledString":{"type":"boolean"}}},"spacing":{"type":"number"},"spacingEnabled":{"type":"boolean"},"texture":{"type":"object","additionalProperties":true,"properties":{"brightness":{"type":"number"},"contrast":{"type":"number"},"depth":{"type":"number"},"depthJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"eachTip":{"type":"boolean"},"enabled":{"type":"boolean"},"invert":{"type":"boolean"},"mode":{"type":"string"},"pattern":{"type":"object","additionalProperties":true,"properties":{"procedural":{"type":"json"}}},"scale":{"type":"number"}}},"tip":{"type":"string"},"transfer":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"flow":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"mix":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"opacity":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"wetness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"wetEdges":{"type":"boolean"}}}"#
        }
        "swatch.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"count":{"type":"integer"},"groups":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"swatches":{"type":"json"}}}}}}"#
        }
        "select.magicWand" => r#"{"type":"object","additionalProperties":true,"properties":{"selected":{"type":"boolean"}}}"#,
        "select.colorRange" => r#"{"type":"object","additionalProperties":true,"properties":{"selected":{"type":"boolean"}}}"#,
        "select.sky" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bounds":{"type":"array","items":{"type":"integer"}},"changed":{"type":"boolean"},"coverage":{"type":"number"},"selected":{"type":"boolean"}}}"#
        }
        "select.isolateLayers" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"isolated":{"type":"boolean"},"layers":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "file.close" => r#"{"type":"object","additionalProperties":false,"properties":{"closed":{"type":"boolean","required":true}}}"#,
        "file.closeAll" => r#"{"type":"object","additionalProperties":false,"properties":{"closed":{"type":"boolean","required":true}}}"#,
        "file.closeOthers" => r#"{"type":"object","additionalProperties":true,"properties":{"closed":{"type":"integer"}}}"#,
        "file.revealInFinder" => r#"{"type":"object","additionalProperties":true,"properties":{"path":{"type":"string"}}}"#,
        "text.characterStyle.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"current":{"type":"json"},"styles":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"attrs":{"type":"json"},"id":{"type":"integer"},"name":{"type":"string"}}}}}}"#
        }
        "text.paragraphStyle.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"current":{"type":"json"},"styles":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"characterAttrs":{"type":"json"},"id":{"type":"integer"},"name":{"type":"string"},"paragraphAttrs":{"type":"json"},"resolved":{"type":"json"}}}}}}"#
        }
        "prefs.get" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"colorSettings":{"type":"object","additionalProperties":true,"properties":{"askOnMismatch":{"type":"boolean"},"askOnMissing":{"type":"boolean"},"askOnPaste":{"type":"boolean"},"blendTextGamma":{"type":"number"},"bpc":{"type":"boolean"},"dither":{"type":"boolean"},"intent":{"type":"string"},"monitorProfile":{"type":"string"},"policyCmyk":{"type":"string"},"policyGray":{"type":"string"},"policyRgb":{"type":"string"},"workingCmyk":{"type":"string"},"workingGray":{"type":"string"},"workingRgb":{"type":"string"}}},"cursors":{"type":"object","additionalProperties":true,"properties":{"brushPreviewColor":{"type":"string"},"other":{"type":"string"},"painting":{"type":"string"},"showCrosshairInBrushTip":{"type":"boolean"},"showOnlyCrosshairWhilePainting":{"type":"boolean"}}},"dialogs":{"type":"object","additionalProperties":true,"properties":{}},"enhancedControls":{"type":"object","additionalProperties":true,"properties":{"rotateViewWithTrackpad":{"type":"boolean"},"scrubbySliderAcceleration":{"type":"boolean"},"touchGestures":{"type":"boolean"},"zoomWithTrackpadPinch":{"type":"boolean"}}},"export":{"type":"object","additionalProperties":true,"properties":{"convertToSrgb":{"type":"boolean"},"jpegQuality":{"type":"integer"},"metadata":{"type":"string"},"quickExportFormat":{"type":"string"},"quickExportLocation":{"type":"string"},"webpLossless":{"type":"boolean"},"webpQuality":{"type":"integer"}}},"fileHandling":{"type":"object","additionalProperties":true,"properties":{"ignoreExifProfileTag":{"type":"boolean"},"imagePreviews":{"type":"string"},"maximizePsdCompatibility":{"type":"string"},"saveInBackground":{"type":"boolean"}}},"general":{"type":"object","additionalProperties":true,"properties":{"alwaysCreateObjectLayersWhenPlacing":{"type":"boolean"},"animatedZoom":{"type":"boolean"},"beepWhenDone":{"type":"boolean"},"colorPicker":{"type":"string"},"imageInterpolation":{"type":"string"},"resizeImageDuringPlace":{"type":"boolean"},"useLegacyFreeTransform":{"type":"boolean"},"zoomResizesWindows":{"type":"boolean"},"zoomWithScrollWheel":{"type":"boolean"}}},"guidesGridAndSlices":{"type":"object","additionalProperties":true,"properties":{"gridColor":{"type":"string"},"gridStyle":{"type":"string"},"gridUnit":{"type":"string"},"gridlineEvery":{"type":"number"},"guideColor":{"type":"string"},"guideStyle":{"type":"string"},"showSliceNumbers":{"type":"boolean"},"sliceColor":{"type":"string"},"smartGuideColor":{"type":"string"},"subdivisions":{"type":"integer"}}},"integrations":{"type":"object","additionalProperties":true,"properties":{"allowAgentControl":{"type":"boolean"},"controlPort":{"type":"integer"}}},"interface":{"type":"object","additionalProperties":true,"properties":{"canvasBorder":{"type":"string"},"canvasColor":{"type":"string"},"canvasCustomColor":{"type":"string"},"dynamicColorSliders":{"type":"boolean"},"showBoundingBoxWhenDraggingLayer":{"type":"boolean"},"showChannelsInColor":{"type":"boolean"},"showMenuColors":{"type":"boolean"},"showTooltips":{"type":"boolean"},"theme":{"type":"string"}}},"menus":{"type":"object","additionalProperties":true,"properties":{"colors":{"type":"object","additionalProperties":true,"properties":{}},"hidden":{"type":"array"}}},"panelLayout":{"type":"json"},"performance":{"type":"object","additionalProperties":true,"properties":{"cacheLevels":{"type":"integer"},"cacheTileSize":{"type":"integer"},"effectCacheMb":{"type":"integer"},"gpuBackend":{"type":"string"},"historyStates":{"type":"integer"},"legacyCompositing":{"type":"boolean"},"lowResolutionPreviews":{"type":"boolean"},"memoryUsageMb":{"type":"integer"},"renderingMode":{"type":"json"},"useGpu":{"type":"boolean"}}},"plugIns":{"type":"object","additionalProperties":true,"properties":{"additionalPluginsFolder":{"type":"string"},"allowScriptsToConnect":{"type":"boolean"},"generatorEnabled":{"type":"boolean"},"showExtensionPanels":{"type":"boolean"},"useAdditionalPluginsFolder":{"type":"boolean"}}},"rawDefaults":{"type":"object","additionalProperties":true,"properties":{"applyAutoTone":{"type":"boolean"},"bitDepth":{"type":"string"},"colorSpace":{"type":"string"},"openAsObjectLayer":{"type":"boolean"},"openInRawDevelop":{"type":"boolean"},"resolution":{"type":"number"},"sharpenFor":{"type":"string"}}},"scratchDisks":{"type":"object","additionalProperties":true,"properties":{"disks":{"type":"array","items":{"type":"json"}}}},"scriptEvents":{"type":"object","additionalProperties":true,"properties":{"bindings":{"type":"array"},"enabled":{"type":"boolean"}}},"shortcuts":{"type":"object","additionalProperties":true,"properties":{}},"toolbar":{"type":"object","additionalProperties":true,"properties":{"hidden":{"type":"array"},"order":{"type":"array"}}},"tools":{"type":"object","additionalProperties":true,"properties":{"doubleClickLayerMaskLaunchesRefineSelection":{"type":"boolean"},"enableFlickPanning":{"type":"boolean"},"overscroll":{"type":"boolean"},"rightClickWithPaintingTools":{"type":"string"},"showTooltips":{"type":"boolean"},"showTransformationValues":{"type":"boolean"},"snapVectorToolsAndTransformsToPixelGrid":{"type":"boolean"},"useShiftKeyForToolSwitch":{"type":"boolean"},"useTabletPressure":{"type":"boolean"},"varyRoundBrushHardnessOnHud":{"type":"boolean"},"zoomClickedPointToCenter":{"type":"boolean"}}},"transparencyAndGamut":{"type":"object","additionalProperties":true,"properties":{"customDark":{"type":"string"},"customLight":{"type":"string"},"gamutWarningColor":{"type":"string"},"gamutWarningOpacity":{"type":"integer"},"gridColors":{"type":"string"},"gridSize":{"type":"string"}}},"type":{"type":"object","additionalProperties":true,"properties":{"fillNewTypeLayersWithPlaceholder":{"type":"boolean"},"fontPreview":{"type":"string"},"missingGlyphProtection":{"type":"boolean"},"recentFonts":{"type":"integer"},"showFontNamesInEnglish":{"type":"boolean"},"smartQuotes":{"type":"boolean"},"textEngine":{"type":"string"},"useEscToCommit":{"type":"boolean"}}},"unitsAndRulers":{"type":"object","additionalProperties":true,"properties":{"columnWidth":{"type":"number"},"gutter":{"type":"number"},"pointSize":{"type":"string"},"printResolution":{"type":"number"},"rulers":{"type":"string"},"screenResolution":{"type":"number"},"typeUnits":{"type":"string"}}},"userDictionary":{"type":"array"},"workspace":{"type":"object","additionalProperties":true,"properties":{"autoCollapseIconPanels":{"type":"boolean"},"autoShowHiddenPanels":{"type":"boolean"},"enableFloatingDocumentWindowDocking":{"type":"boolean"},"enableNarrowOptionsBar":{"type":"boolean"},"largeTabs":{"type":"boolean"},"openDocumentsAsTabs":{"type":"boolean"},"rememberWorkspaceChanges":{"type":"boolean"}}},"workspaceLocked":{"type":"boolean"},"workspaces":{"type":"object","additionalProperties":true,"properties":{}}}}"#
        }
        "prefs.reset" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"colorSettings":{"type":"object","additionalProperties":true,"properties":{"askOnMismatch":{"type":"boolean"},"askOnMissing":{"type":"boolean"},"askOnPaste":{"type":"boolean"},"blendTextGamma":{"type":"number"},"bpc":{"type":"boolean"},"dither":{"type":"boolean"},"intent":{"type":"string"},"monitorProfile":{"type":"string"},"policyCmyk":{"type":"string"},"policyGray":{"type":"string"},"policyRgb":{"type":"string"},"workingCmyk":{"type":"string"},"workingGray":{"type":"string"},"workingRgb":{"type":"string"}}},"cursors":{"type":"object","additionalProperties":true,"properties":{"brushPreviewColor":{"type":"string"},"other":{"type":"string"},"painting":{"type":"string"},"showCrosshairInBrushTip":{"type":"boolean"},"showOnlyCrosshairWhilePainting":{"type":"boolean"}}},"dialogs":{"type":"object","additionalProperties":true,"properties":{}},"enhancedControls":{"type":"object","additionalProperties":true,"properties":{"rotateViewWithTrackpad":{"type":"boolean"},"scrubbySliderAcceleration":{"type":"boolean"},"touchGestures":{"type":"boolean"},"zoomWithTrackpadPinch":{"type":"boolean"}}},"export":{"type":"object","additionalProperties":true,"properties":{"convertToSrgb":{"type":"boolean"},"jpegQuality":{"type":"integer"},"metadata":{"type":"string"},"quickExportFormat":{"type":"string"},"quickExportLocation":{"type":"string"},"webpLossless":{"type":"boolean"},"webpQuality":{"type":"integer"}}},"fileHandling":{"type":"object","additionalProperties":true,"properties":{"ignoreExifProfileTag":{"type":"boolean"},"imagePreviews":{"type":"string"},"maximizePsdCompatibility":{"type":"string"},"saveInBackground":{"type":"boolean"}}},"general":{"type":"object","additionalProperties":true,"properties":{"alwaysCreateObjectLayersWhenPlacing":{"type":"boolean"},"animatedZoom":{"type":"boolean"},"beepWhenDone":{"type":"boolean"},"colorPicker":{"type":"string"},"imageInterpolation":{"type":"string"},"resizeImageDuringPlace":{"type":"boolean"},"useLegacyFreeTransform":{"type":"boolean"},"zoomResizesWindows":{"type":"boolean"},"zoomWithScrollWheel":{"type":"boolean"}}},"guidesGridAndSlices":{"type":"object","additionalProperties":true,"properties":{"gridColor":{"type":"string"},"gridStyle":{"type":"string"},"gridUnit":{"type":"string"},"gridlineEvery":{"type":"number"},"guideColor":{"type":"string"},"guideStyle":{"type":"string"},"showSliceNumbers":{"type":"boolean"},"sliceColor":{"type":"string"},"smartGuideColor":{"type":"string"},"subdivisions":{"type":"integer"}}},"integrations":{"type":"object","additionalProperties":true,"properties":{"allowAgentControl":{"type":"boolean"},"controlPort":{"type":"integer"}}},"interface":{"type":"object","additionalProperties":true,"properties":{"canvasBorder":{"type":"string"},"canvasColor":{"type":"string"},"canvasCustomColor":{"type":"string"},"dynamicColorSliders":{"type":"boolean"},"showBoundingBoxWhenDraggingLayer":{"type":"boolean"},"showChannelsInColor":{"type":"boolean"},"showMenuColors":{"type":"boolean"},"showTooltips":{"type":"boolean"},"theme":{"type":"string"}}},"menus":{"type":"object","additionalProperties":true,"properties":{"colors":{"type":"object","additionalProperties":true,"properties":{}},"hidden":{"type":"array"}}},"panelLayout":{"type":"json"},"performance":{"type":"object","additionalProperties":true,"properties":{"cacheLevels":{"type":"integer"},"cacheTileSize":{"type":"integer"},"effectCacheMb":{"type":"integer"},"gpuBackend":{"type":"string"},"historyStates":{"type":"integer"},"legacyCompositing":{"type":"boolean"},"lowResolutionPreviews":{"type":"boolean"},"memoryUsageMb":{"type":"integer"},"renderingMode":{"type":"json"},"useGpu":{"type":"boolean"}}},"plugIns":{"type":"object","additionalProperties":true,"properties":{"additionalPluginsFolder":{"type":"string"},"allowScriptsToConnect":{"type":"boolean"},"generatorEnabled":{"type":"boolean"},"showExtensionPanels":{"type":"boolean"},"useAdditionalPluginsFolder":{"type":"boolean"}}},"rawDefaults":{"type":"object","additionalProperties":true,"properties":{"applyAutoTone":{"type":"boolean"},"bitDepth":{"type":"string"},"colorSpace":{"type":"string"},"openAsObjectLayer":{"type":"boolean"},"openInRawDevelop":{"type":"boolean"},"resolution":{"type":"number"},"sharpenFor":{"type":"string"}}},"scratchDisks":{"type":"object","additionalProperties":true,"properties":{"disks":{"type":"array","items":{"type":"json"}}}},"scriptEvents":{"type":"object","additionalProperties":true,"properties":{"bindings":{"type":"array"},"enabled":{"type":"boolean"}}},"shortcuts":{"type":"object","additionalProperties":true,"properties":{}},"toolbar":{"type":"object","additionalProperties":true,"properties":{"hidden":{"type":"array"},"order":{"type":"array"}}},"tools":{"type":"object","additionalProperties":true,"properties":{"doubleClickLayerMaskLaunchesRefineSelection":{"type":"boolean"},"enableFlickPanning":{"type":"boolean"},"overscroll":{"type":"boolean"},"rightClickWithPaintingTools":{"type":"string"},"showTooltips":{"type":"boolean"},"showTransformationValues":{"type":"boolean"},"snapVectorToolsAndTransformsToPixelGrid":{"type":"boolean"},"useShiftKeyForToolSwitch":{"type":"boolean"},"useTabletPressure":{"type":"boolean"},"varyRoundBrushHardnessOnHud":{"type":"boolean"},"zoomClickedPointToCenter":{"type":"boolean"}}},"transparencyAndGamut":{"type":"object","additionalProperties":true,"properties":{"customDark":{"type":"string"},"customLight":{"type":"string"},"gamutWarningColor":{"type":"string"},"gamutWarningOpacity":{"type":"integer"},"gridColors":{"type":"string"},"gridSize":{"type":"string"}}},"type":{"type":"object","additionalProperties":true,"properties":{"fillNewTypeLayersWithPlaceholder":{"type":"boolean"},"fontPreview":{"type":"string"},"missingGlyphProtection":{"type":"boolean"},"recentFonts":{"type":"integer"},"showFontNamesInEnglish":{"type":"boolean"},"smartQuotes":{"type":"boolean"},"textEngine":{"type":"string"},"useEscToCommit":{"type":"boolean"}}},"unitsAndRulers":{"type":"object","additionalProperties":true,"properties":{"columnWidth":{"type":"number"},"gutter":{"type":"number"},"pointSize":{"type":"string"},"printResolution":{"type":"number"},"rulers":{"type":"string"},"screenResolution":{"type":"number"},"typeUnits":{"type":"string"}}},"userDictionary":{"type":"array"},"workspace":{"type":"object","additionalProperties":true,"properties":{"autoCollapseIconPanels":{"type":"boolean"},"autoShowHiddenPanels":{"type":"boolean"},"enableFloatingDocumentWindowDocking":{"type":"boolean"},"enableNarrowOptionsBar":{"type":"boolean"},"largeTabs":{"type":"boolean"},"openDocumentsAsTabs":{"type":"boolean"},"rememberWorkspaceChanges":{"type":"boolean"}}},"workspaceLocked":{"type":"boolean"},"workspaces":{"type":"object","additionalProperties":true,"properties":{}}}}"#
        }
        "edit.preferences.general" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"alwaysCreateObjectLayersWhenPlacing":{"type":"boolean"},"animatedZoom":{"type":"boolean"},"beepWhenDone":{"type":"boolean"},"colorPicker":{"type":"string"},"imageInterpolation":{"type":"string"},"resizeImageDuringPlace":{"type":"boolean"},"useLegacyFreeTransform":{"type":"boolean"},"zoomResizesWindows":{"type":"boolean"},"zoomWithScrollWheel":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.interface" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"canvasBorder":{"type":"string"},"canvasColor":{"type":"string"},"canvasCustomColor":{"type":"string"},"dynamicColorSliders":{"type":"boolean"},"showBoundingBoxWhenDraggingLayer":{"type":"boolean"},"showChannelsInColor":{"type":"boolean"},"showMenuColors":{"type":"boolean"},"showTooltips":{"type":"boolean"},"theme":{"type":"string"}}}}}"#
        }
        "edit.preferences.workspace" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"autoCollapseIconPanels":{"type":"boolean"},"autoShowHiddenPanels":{"type":"boolean"},"enableFloatingDocumentWindowDocking":{"type":"boolean"},"enableNarrowOptionsBar":{"type":"boolean"},"largeTabs":{"type":"boolean"},"openDocumentsAsTabs":{"type":"boolean"},"rememberWorkspaceChanges":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.tools" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"doubleClickLayerMaskLaunchesRefineSelection":{"type":"boolean"},"enableFlickPanning":{"type":"boolean"},"overscroll":{"type":"boolean"},"rightClickWithPaintingTools":{"type":"string"},"showTooltips":{"type":"boolean"},"showTransformationValues":{"type":"boolean"},"snapVectorToolsAndTransformsToPixelGrid":{"type":"boolean"},"useShiftKeyForToolSwitch":{"type":"boolean"},"useTabletPressure":{"type":"boolean"},"varyRoundBrushHardnessOnHud":{"type":"boolean"},"zoomClickedPointToCenter":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.fileHandling" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"ignoreExifProfileTag":{"type":"boolean"},"imagePreviews":{"type":"string"},"maximizePsdCompatibility":{"type":"string"},"saveInBackground":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.export" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"convertToSrgb":{"type":"boolean"},"jpegQuality":{"type":"integer"},"metadata":{"type":"string"},"quickExportFormat":{"type":"string"},"quickExportLocation":{"type":"string"},"webpLossless":{"type":"boolean"},"webpQuality":{"type":"integer"}}}}}"#
        }
        "edit.preferences.performance" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"cacheLevels":{"type":"integer"},"cacheTileSize":{"type":"integer"},"effectCacheMb":{"type":"integer"},"gpuBackend":{"type":"string"},"historyStates":{"type":"integer"},"legacyCompositing":{"type":"boolean"},"lowResolutionPreviews":{"type":"boolean"},"memoryUsageMb":{"type":"integer"},"renderingMode":{"type":"json"},"useGpu":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.scratchDisks" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"disks":{"type":"array","items":{"type":"json"}}}}}}"#
        }
        "edit.preferences.cursors" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"brushPreviewColor":{"type":"string"},"other":{"type":"string"},"painting":{"type":"string"},"showCrosshairInBrushTip":{"type":"boolean"},"showOnlyCrosshairWhilePainting":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.transparencyAndGamut" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"customDark":{"type":"string"},"customLight":{"type":"string"},"gamutWarningColor":{"type":"string"},"gamutWarningOpacity":{"type":"integer"},"gridColors":{"type":"string"},"gridSize":{"type":"string"}}}}}"#
        }
        "edit.preferences.unitsAndRulers" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"columnWidth":{"type":"number"},"gutter":{"type":"number"},"pointSize":{"type":"string"},"printResolution":{"type":"number"},"rulers":{"type":"string"},"screenResolution":{"type":"number"},"typeUnits":{"type":"string"}}}}}"#
        }
        "edit.preferences.guidesGridAndSlices" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"gridColor":{"type":"string"},"gridStyle":{"type":"string"},"gridUnit":{"type":"string"},"gridlineEvery":{"type":"number"},"guideColor":{"type":"string"},"guideStyle":{"type":"string"},"showSliceNumbers":{"type":"boolean"},"sliceColor":{"type":"string"},"smartGuideColor":{"type":"string"},"subdivisions":{"type":"integer"}}}}}"#
        }
        "edit.preferences.plugIns" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"additionalPluginsFolder":{"type":"string"},"allowScriptsToConnect":{"type":"boolean"},"generatorEnabled":{"type":"boolean"},"showExtensionPanels":{"type":"boolean"},"useAdditionalPluginsFolder":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.type" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"fillNewTypeLayersWithPlaceholder":{"type":"boolean"},"fontPreview":{"type":"string"},"missingGlyphProtection":{"type":"boolean"},"recentFonts":{"type":"integer"},"showFontNamesInEnglish":{"type":"boolean"},"smartQuotes":{"type":"boolean"},"textEngine":{"type":"string"},"useEscToCommit":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.enhancedControls" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"rotateViewWithTrackpad":{"type":"boolean"},"scrubbySliderAcceleration":{"type":"boolean"},"touchGestures":{"type":"boolean"},"zoomWithTrackpadPinch":{"type":"boolean"}}}}}"#
        }
        "edit.preferences.rawDefaults" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"applyAutoTone":{"type":"boolean"},"bitDepth":{"type":"string"},"colorSpace":{"type":"string"},"openAsObjectLayer":{"type":"boolean"},"openInRawDevelop":{"type":"boolean"},"resolution":{"type":"number"},"sharpenFor":{"type":"string"}}}}}"#
        }
        "edit.preferences.integrations" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"title":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"allowAgentControl":{"type":"boolean"},"controlPort":{"type":"integer"}}}}}"#
        }
        "edit.keyboardShortcuts" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"commands":{"type":"array"},"conflicts":{"type":"array"},"overrides":{"type":"object","additionalProperties":true,"properties":{}}}}"#
        }
        "edit.menus" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"colors":{"type":"object","additionalProperties":true,"properties":{}},"hidden":{"type":"array"}}}"#
        }
        "edit.toolbar" => r#"{"type":"object","additionalProperties":true,"properties":{"hidden":{"type":"array"},"order":{"type":"array"}}}"#,
        "edit.purge.videoCache" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bytes":{"type":"integer"},"message":{"type":"string"},"purged":{"type":"array"}}}"#
        }
        "tool.decreaseBrushSize" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"aliased":{"type":"boolean"},"angle":{"type":"number"},"background":{"type":"array","items":{"type":"number"}},"buildUp":{"type":"boolean"},"buildUpRate":{"type":"number"},"color":{"type":"array","items":{"type":"number"}},"colorDynamics":{"type":"object","additionalProperties":true,"properties":{"brightnessJitter":{"type":"number"},"enabled":{"type":"boolean"},"fgBg":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"hueJitter":{"type":"number"},"perTip":{"type":"boolean"},"purity":{"type":"number"},"saturationJitter":{"type":"number"}}},"dualBrush":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"number"},"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"enabled":{"type":"boolean"},"flip":{"type":"boolean"},"hardness":{"type":"number"},"mode":{"type":"string"},"roundness":{"type":"number"},"scatter":{"type":"number"},"size":{"type":"number"},"spacing":{"type":"number"},"tip":{"type":"string"}}},"erase":{"type":"boolean"},"flipX":{"type":"boolean"},"flipY":{"type":"boolean"},"flow":{"type":"number"},"hardness":{"type":"number"},"locks":{"type":"object","additionalProperties":true,"properties":{"buildUp":{"type":"boolean"},"colorDynamics":{"type":"boolean"},"dualBrush":{"type":"boolean"},"noise":{"type":"boolean"},"pose":{"type":"boolean"},"protectTexture":{"type":"boolean"},"scattering":{"type":"boolean"},"shapeDynamics":{"type":"boolean"},"smoothing":{"type":"boolean"},"texture":{"type":"boolean"},"transfer":{"type":"boolean"},"wetEdges":{"type":"boolean"}}},"mixer":{"type":"object","additionalProperties":true,"properties":{"flow":{"type":"number"},"load":{"type":"number"},"mix":{"type":"number"},"sampleAllLayers":{"type":"boolean"},"wet":{"type":"number"}}},"mode":{"type":"string"},"noise":{"type":"boolean"},"opacity":{"type":"number"},"pose":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"overridePressure":{"type":"boolean"},"overrideRotation":{"type":"boolean"},"overrideTilt":{"type":"boolean"},"pressure":{"type":"number"},"rotation":{"type":"number"},"tiltX":{"type":"number"},"tiltY":{"type":"number"}}},"pressureOpacity":{"type":"boolean"},"pressureSize":{"type":"boolean"},"protectTexture":{"type":"boolean"},"roundness":{"type":"number"},"scattering":{"type":"object","additionalProperties":true,"properties":{"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"countJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"enabled":{"type":"boolean"},"scatter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"seed":{"type":"integer"},"shapeDynamics":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"brushProjection":{"type":"boolean"},"enabled":{"type":"boolean"},"flipXJitter":{"type":"boolean"},"flipYJitter":{"type":"boolean"},"roundness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"size":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"tiltScale":{"type":"number"}}},"size":{"type":"number"},"smoothing":{"type":"object","additionalProperties":true,"properties":{"adjustForZoom":{"type":"boolean"},"amount":{"type":"number"},"catchUp":{"type":"boolean"},"catchUpOnEnd":{"type":"boolean"},"pulledString":{"type":"boolean"}}},"spacing":{"type":"number"},"spacingEnabled":{"type":"boolean"},"texture":{"type":"object","additionalProperties":true,"properties":{"brightness":{"type":"number"},"contrast":{"type":"number"},"depth":{"type":"number"},"depthJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"eachTip":{"type":"boolean"},"enabled":{"type":"boolean"},"invert":{"type":"boolean"},"mode":{"type":"string"},"pattern":{"type":"object","additionalProperties":true,"properties":{"procedural":{"type":"json"}}},"scale":{"type":"number"}}},"tip":{"type":"string"},"transfer":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"flow":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"mix":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"opacity":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"wetness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"wetEdges":{"type":"boolean"}}}"#
        }
        "tool.increaseBrushSize" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"aliased":{"type":"boolean"},"angle":{"type":"number"},"background":{"type":"array","items":{"type":"number"}},"buildUp":{"type":"boolean"},"buildUpRate":{"type":"number"},"color":{"type":"array","items":{"type":"number"}},"colorDynamics":{"type":"object","additionalProperties":true,"properties":{"brightnessJitter":{"type":"number"},"enabled":{"type":"boolean"},"fgBg":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"hueJitter":{"type":"number"},"perTip":{"type":"boolean"},"purity":{"type":"number"},"saturationJitter":{"type":"number"}}},"dualBrush":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"number"},"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"enabled":{"type":"boolean"},"flip":{"type":"boolean"},"hardness":{"type":"number"},"mode":{"type":"string"},"roundness":{"type":"number"},"scatter":{"type":"number"},"size":{"type":"number"},"spacing":{"type":"number"},"tip":{"type":"string"}}},"erase":{"type":"boolean"},"flipX":{"type":"boolean"},"flipY":{"type":"boolean"},"flow":{"type":"number"},"hardness":{"type":"number"},"locks":{"type":"object","additionalProperties":true,"properties":{"buildUp":{"type":"boolean"},"colorDynamics":{"type":"boolean"},"dualBrush":{"type":"boolean"},"noise":{"type":"boolean"},"pose":{"type":"boolean"},"protectTexture":{"type":"boolean"},"scattering":{"type":"boolean"},"shapeDynamics":{"type":"boolean"},"smoothing":{"type":"boolean"},"texture":{"type":"boolean"},"transfer":{"type":"boolean"},"wetEdges":{"type":"boolean"}}},"mixer":{"type":"object","additionalProperties":true,"properties":{"flow":{"type":"number"},"load":{"type":"number"},"mix":{"type":"number"},"sampleAllLayers":{"type":"boolean"},"wet":{"type":"number"}}},"mode":{"type":"string"},"noise":{"type":"boolean"},"opacity":{"type":"number"},"pose":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"overridePressure":{"type":"boolean"},"overrideRotation":{"type":"boolean"},"overrideTilt":{"type":"boolean"},"pressure":{"type":"number"},"rotation":{"type":"number"},"tiltX":{"type":"number"},"tiltY":{"type":"number"}}},"pressureOpacity":{"type":"boolean"},"pressureSize":{"type":"boolean"},"protectTexture":{"type":"boolean"},"roundness":{"type":"number"},"scattering":{"type":"object","additionalProperties":true,"properties":{"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"countJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"enabled":{"type":"boolean"},"scatter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"seed":{"type":"integer"},"shapeDynamics":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"brushProjection":{"type":"boolean"},"enabled":{"type":"boolean"},"flipXJitter":{"type":"boolean"},"flipYJitter":{"type":"boolean"},"roundness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"size":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"tiltScale":{"type":"number"}}},"size":{"type":"number"},"smoothing":{"type":"object","additionalProperties":true,"properties":{"adjustForZoom":{"type":"boolean"},"amount":{"type":"number"},"catchUp":{"type":"boolean"},"catchUpOnEnd":{"type":"boolean"},"pulledString":{"type":"boolean"}}},"spacing":{"type":"number"},"spacingEnabled":{"type":"boolean"},"texture":{"type":"object","additionalProperties":true,"properties":{"brightness":{"type":"number"},"contrast":{"type":"number"},"depth":{"type":"number"},"depthJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"eachTip":{"type":"boolean"},"enabled":{"type":"boolean"},"invert":{"type":"boolean"},"mode":{"type":"string"},"pattern":{"type":"object","additionalProperties":true,"properties":{"procedural":{"type":"json"}}},"scale":{"type":"number"}}},"tip":{"type":"string"},"transfer":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"flow":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"mix":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"opacity":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"wetness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"wetEdges":{"type":"boolean"}}}"#
        }
        "tool.decreaseBrushHardness" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"aliased":{"type":"boolean"},"angle":{"type":"number"},"background":{"type":"array","items":{"type":"number"}},"buildUp":{"type":"boolean"},"buildUpRate":{"type":"number"},"color":{"type":"array","items":{"type":"number"}},"colorDynamics":{"type":"object","additionalProperties":true,"properties":{"brightnessJitter":{"type":"number"},"enabled":{"type":"boolean"},"fgBg":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"hueJitter":{"type":"number"},"perTip":{"type":"boolean"},"purity":{"type":"number"},"saturationJitter":{"type":"number"}}},"dualBrush":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"number"},"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"enabled":{"type":"boolean"},"flip":{"type":"boolean"},"hardness":{"type":"number"},"mode":{"type":"string"},"roundness":{"type":"number"},"scatter":{"type":"number"},"size":{"type":"number"},"spacing":{"type":"number"},"tip":{"type":"string"}}},"erase":{"type":"boolean"},"flipX":{"type":"boolean"},"flipY":{"type":"boolean"},"flow":{"type":"number"},"hardness":{"type":"number"},"locks":{"type":"object","additionalProperties":true,"properties":{"buildUp":{"type":"boolean"},"colorDynamics":{"type":"boolean"},"dualBrush":{"type":"boolean"},"noise":{"type":"boolean"},"pose":{"type":"boolean"},"protectTexture":{"type":"boolean"},"scattering":{"type":"boolean"},"shapeDynamics":{"type":"boolean"},"smoothing":{"type":"boolean"},"texture":{"type":"boolean"},"transfer":{"type":"boolean"},"wetEdges":{"type":"boolean"}}},"mixer":{"type":"object","additionalProperties":true,"properties":{"flow":{"type":"number"},"load":{"type":"number"},"mix":{"type":"number"},"sampleAllLayers":{"type":"boolean"},"wet":{"type":"number"}}},"mode":{"type":"string"},"noise":{"type":"boolean"},"opacity":{"type":"number"},"pose":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"overridePressure":{"type":"boolean"},"overrideRotation":{"type":"boolean"},"overrideTilt":{"type":"boolean"},"pressure":{"type":"number"},"rotation":{"type":"number"},"tiltX":{"type":"number"},"tiltY":{"type":"number"}}},"pressureOpacity":{"type":"boolean"},"pressureSize":{"type":"boolean"},"protectTexture":{"type":"boolean"},"roundness":{"type":"number"},"scattering":{"type":"object","additionalProperties":true,"properties":{"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"countJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"enabled":{"type":"boolean"},"scatter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"seed":{"type":"integer"},"shapeDynamics":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"brushProjection":{"type":"boolean"},"enabled":{"type":"boolean"},"flipXJitter":{"type":"boolean"},"flipYJitter":{"type":"boolean"},"roundness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"size":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"tiltScale":{"type":"number"}}},"size":{"type":"number"},"smoothing":{"type":"object","additionalProperties":true,"properties":{"adjustForZoom":{"type":"boolean"},"amount":{"type":"number"},"catchUp":{"type":"boolean"},"catchUpOnEnd":{"type":"boolean"},"pulledString":{"type":"boolean"}}},"spacing":{"type":"number"},"spacingEnabled":{"type":"boolean"},"texture":{"type":"object","additionalProperties":true,"properties":{"brightness":{"type":"number"},"contrast":{"type":"number"},"depth":{"type":"number"},"depthJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"eachTip":{"type":"boolean"},"enabled":{"type":"boolean"},"invert":{"type":"boolean"},"mode":{"type":"string"},"pattern":{"type":"object","additionalProperties":true,"properties":{"procedural":{"type":"json"}}},"scale":{"type":"number"}}},"tip":{"type":"string"},"transfer":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"flow":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"mix":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"opacity":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"wetness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"wetEdges":{"type":"boolean"}}}"#
        }
        "tool.increaseBrushHardness" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"aliased":{"type":"boolean"},"angle":{"type":"number"},"background":{"type":"array","items":{"type":"number"}},"buildUp":{"type":"boolean"},"buildUpRate":{"type":"number"},"color":{"type":"array","items":{"type":"number"}},"colorDynamics":{"type":"object","additionalProperties":true,"properties":{"brightnessJitter":{"type":"number"},"enabled":{"type":"boolean"},"fgBg":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"hueJitter":{"type":"number"},"perTip":{"type":"boolean"},"purity":{"type":"number"},"saturationJitter":{"type":"number"}}},"dualBrush":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"number"},"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"enabled":{"type":"boolean"},"flip":{"type":"boolean"},"hardness":{"type":"number"},"mode":{"type":"string"},"roundness":{"type":"number"},"scatter":{"type":"number"},"size":{"type":"number"},"spacing":{"type":"number"},"tip":{"type":"string"}}},"erase":{"type":"boolean"},"flipX":{"type":"boolean"},"flipY":{"type":"boolean"},"flow":{"type":"number"},"hardness":{"type":"number"},"locks":{"type":"object","additionalProperties":true,"properties":{"buildUp":{"type":"boolean"},"colorDynamics":{"type":"boolean"},"dualBrush":{"type":"boolean"},"noise":{"type":"boolean"},"pose":{"type":"boolean"},"protectTexture":{"type":"boolean"},"scattering":{"type":"boolean"},"shapeDynamics":{"type":"boolean"},"smoothing":{"type":"boolean"},"texture":{"type":"boolean"},"transfer":{"type":"boolean"},"wetEdges":{"type":"boolean"}}},"mixer":{"type":"object","additionalProperties":true,"properties":{"flow":{"type":"number"},"load":{"type":"number"},"mix":{"type":"number"},"sampleAllLayers":{"type":"boolean"},"wet":{"type":"number"}}},"mode":{"type":"string"},"noise":{"type":"boolean"},"opacity":{"type":"number"},"pose":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"overridePressure":{"type":"boolean"},"overrideRotation":{"type":"boolean"},"overrideTilt":{"type":"boolean"},"pressure":{"type":"number"},"rotation":{"type":"number"},"tiltX":{"type":"number"},"tiltY":{"type":"number"}}},"pressureOpacity":{"type":"boolean"},"pressureSize":{"type":"boolean"},"protectTexture":{"type":"boolean"},"roundness":{"type":"number"},"scattering":{"type":"object","additionalProperties":true,"properties":{"bothAxes":{"type":"boolean"},"count":{"type":"integer"},"countJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"enabled":{"type":"boolean"},"scatter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"seed":{"type":"integer"},"shapeDynamics":{"type":"object","additionalProperties":true,"properties":{"angle":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"brushProjection":{"type":"boolean"},"enabled":{"type":"boolean"},"flipXJitter":{"type":"boolean"},"flipYJitter":{"type":"boolean"},"roundness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"size":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"tiltScale":{"type":"number"}}},"size":{"type":"number"},"smoothing":{"type":"object","additionalProperties":true,"properties":{"adjustForZoom":{"type":"boolean"},"amount":{"type":"number"},"catchUp":{"type":"boolean"},"catchUpOnEnd":{"type":"boolean"},"pulledString":{"type":"boolean"}}},"spacing":{"type":"number"},"spacingEnabled":{"type":"boolean"},"texture":{"type":"object","additionalProperties":true,"properties":{"brightness":{"type":"number"},"contrast":{"type":"number"},"depth":{"type":"number"},"depthJitter":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"eachTip":{"type":"boolean"},"enabled":{"type":"boolean"},"invert":{"type":"boolean"},"mode":{"type":"string"},"pattern":{"type":"object","additionalProperties":true,"properties":{"procedural":{"type":"json"}}},"scale":{"type":"number"}}},"tip":{"type":"string"},"transfer":{"type":"object","additionalProperties":true,"properties":{"enabled":{"type":"boolean"},"flow":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"mix":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"opacity":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}},"wetness":{"type":"object","additionalProperties":true,"properties":{"control":{"type":"string"},"fadeSteps":{"type":"integer"},"jitter":{"type":"number"},"minimum":{"type":"number"}}}}},"wetEdges":{"type":"boolean"}}}"#
        }
        "select.editInQuickMaskMode" => r#"{"type":"object","additionalProperties":true,"properties":{"quickMask":{"type":"boolean"}}}"#,
        "channel.target" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"visible":{"type":"boolean"}}}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"kind":{"type":"string"}}}}}"#
        }
        "channel.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"visible":{"type":"boolean"}}}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"kind":{"type":"string"}}}}}"#
        }
        "channel.target.composite" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"visible":{"type":"boolean"}}}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"kind":{"type":"string"}}}}}"#
        }
        "channel.target.slot3" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"visible":{"type":"boolean"}}}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"index":{"type":"integer"},"kind":{"type":"string"}}}}}"#
        }
        "channel.target.slot4" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"visible":{"type":"boolean"}}}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"index":{"type":"integer"},"kind":{"type":"string"}}}}}"#
        }
        "channel.target.slot5" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"array"},"colors":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"visible":{"type":"boolean"}}}},"composite":{"type":"string"},"compositeVisible":{"type":"boolean"},"layerMaskView":{"type":"json"},"quickMask":{"type":"json"},"target":{"type":"object","additionalProperties":true,"properties":{"index":{"type":"integer"},"kind":{"type":"string"}}}}}"#
        }
        "image.adjustments.listColorLookups" => {
            r#"{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"id":{"type":"string"},"label":{"type":"string"}}}}"#
        }
        "pattern.list" => {
            r#"{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"alpha":{"type":"boolean"},"depth":{"type":"string"},"height":{"type":"integer"},"id":{"type":"string"},"inDocument":{"type":"boolean"},"inLibrary":{"type":"boolean"},"mode":{"type":"string"},"name":{"type":"string"},"storedName":{"type":"string"},"width":{"type":"integer"}}}}"#
        }
        "layerComp.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"comps":{"type":"array"},"hasLastDocumentState":{"type":"boolean"},"lastApplied":{"type":"json"}}}"#
        }
        "image.analysis.countTool" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"activeGroup":{"type":"integer"},"groups":{"type":"array"},"total":{"type":"integer"}}}"#
        }
        "image.analysis.info" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"count":{"type":"object","additionalProperties":true,"properties":{"activeGroup":{"type":"integer"},"groups":{"type":"array"},"total":{"type":"integer"}}},"logRows":{"type":"integer"},"notes":{"type":"integer"},"ruler":{"type":"json"},"scale":{"type":"object","additionalProperties":true,"properties":{"default":{"type":"boolean"},"factor":{"type":"number"},"logicalLength":{"type":"number"},"pixelLength":{"type":"number"},"text":{"type":"string"},"units":{"type":"string"}}}}}"#
        }
        "measurementLog.list" => r#"{"type":"object","additionalProperties":true,"properties":{"columns":{"type":"array"},"rows":{"type":"array"}}}"#,
        "note.list" => r#"{"type":"object","additionalProperties":true,"properties":{"notes":{"type":"array"}}}"#,
        "view.proofSetup.workingCyanPlate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.workingMagentaPlate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.workingYellowPlate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.workingBlackPlate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.workingCmyPlate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.legacyMacintoshRgb" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.colorBlindnessProtanopia" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "view.proofSetup.colorBlindnessDeuteranopia" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"profile":{"type":"string"},"proof":{"type":"string"},"proofColors":{"type":"boolean"}}}"#
        }
        "gradient.presets.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"current":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"stops":{"type":"array","items":{"type":"json"}},"transparency":{"type":"array"}}},"groups":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"presets":{"type":"json"}}}}}}"#
        }
        "pattern.presets.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"current":{"type":"json"},"groups":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"patterns":{"type":"json"}}}}}}"#
        }
        "style.presets.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"groups":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"presets":{"type":"json"}}}}}}"#
        }
        "shape.presets.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"groups":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"shapes":{"type":"json"}}}}}}"#
        }
        "tool.presets.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"presets":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"name":{"type":"string"},"tool":{"type":"string"}}}}}}"#
        }
        "cloneSource.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"active":{"type":"integer"},"overlay":{"type":"object","additionalProperties":true,"properties":{"auto_hide":{"type":"boolean"},"blend":{"type":"string"},"clipped":{"type":"boolean"},"invert":{"type":"boolean"},"opacity":{"type":"number"},"show":{"type":"boolean"}}},"sources":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"anchor":{"type":"json"},"flipH":{"type":"boolean"},"flipV":{"type":"boolean"},"height":{"type":"number"},"index":{"type":"integer"},"layer":{"type":"json"},"offset":{"type":"json"},"rotation":{"type":"number"},"source":{"type":"json"},"width":{"type":"number"}}}}}}"#
        }
        "file.presets.list" => r#"{"type":"object","additionalProperties":true,"properties":{"presets":{"type":"array"}}}"#,
        "slice.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"locked":{"type":"boolean"},"slices":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"alt":{"type":"string"},"background":{"type":"json"},"cellText":{"type":"string"},"id":{"type":"json"},"kind":{"type":"string"},"layer":{"type":"json"},"message":{"type":"string"},"name":{"type":"string"},"number":{"type":"integer"},"origin":{"type":"string"},"rect":{"type":"json"},"target":{"type":"string"},"url":{"type":"string"}}}}}}"#
        }
        "file.export.saveForWebLegacy" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bytes":{"type":"integer"},"colors":{"type":"json"},"format":{"type":"string"},"height":{"type":"integer"},"width":{"type":"integer"}}}"#
        }
        "file.export.exportPreferences" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"section":{"type":"string"},"values":{"type":"object","additionalProperties":true,"properties":{"convertToSrgb":{"type":"boolean"},"jpegQuality":{"type":"integer"},"metadata":{"type":"string"},"quickExportFormat":{"type":"string"},"quickExportLocation":{"type":"string"},"webpLossless":{"type":"boolean"},"webpQuality":{"type":"integer"}}}}}"#
        }
        "file.print" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bytes":{"type":"integer"},"color":{"type":"object","additionalProperties":true,"properties":{"colorHandling":{"type":"string"}}},"command":{"type":"array","items":{"type":"string"}},"copies":{"type":"integer"},"imageRect":{"type":"array","items":{"type":"number"}},"paper":{"type":"array","items":{"type":"number"}},"pdf":{"type":"string"},"scale":{"type":"number"},"sent":{"type":"boolean"},"spooler":{"type":"string"}}}"#
        }
        "file.printOneCopy" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bytes":{"type":"integer"},"color":{"type":"object","additionalProperties":true,"properties":{"colorHandling":{"type":"string"}}},"command":{"type":"array","items":{"type":"string"}},"copies":{"type":"integer"},"imageRect":{"type":"array","items":{"type":"number"}},"paper":{"type":"array","items":{"type":"number"}},"pdf":{"type":"string"},"scale":{"type":"number"},"sent":{"type":"boolean"},"spooler":{"type":"string"}}}"#
        }
        "layer.selectAbove" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"selected":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "layer.selectBelow" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"selected":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "layer.selectTop" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"selected":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "layer.selectBottom" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"selected":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "layer.addAboveToSelection" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"selected":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "layer.addBelowToSelection" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"selected":{"type":"array","items":{"type":"integer"}}}}"#
        }
        "timeline.info" => r#"{"type":"object","additionalProperties":true,"properties":{"timeline":{"type":"json"}}}"#,
        "job.list" => r#"{"type":"object","additionalProperties":true,"properties":{"jobs":{"type":"array"}}}"#,
        "job.cancel" => r#"{"type":"object","additionalProperties":true,"properties":{"cancelled":{"type":"integer"}}}"#,
        "file.import.wiaSupport" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"available":{"type":"boolean"},"devices":{"type":"array"},"note":{"type":"string"}}}"#
        }
        "variable.list" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"active":{"type":"json"},"dataSets":{"type":"array"},"defs":{"type":"array"}}}"#
        }
        "plugin.list" => r#"{"type":"object","additionalProperties":true,"properties":{"folder":{"type":"json"},"plugins":{"type":"array"}}}"#,
        "action.list" => r#"{"type":"object","additionalProperties":true,"properties":{"actions":{"type":"array"},"recording":{"type":"json"}}}"#,
        "action.record" => r#"{"type":"object","additionalProperties":true,"properties":{"action":{"type":"string"},"index":{"type":"integer"}}}"#,
        "file.exportImage" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"bytes":{"type":"integer"},"data":{"type":"string"},"mime":{"type":"string"}}}"#
        }
        "edit.deselectAll" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.inverse" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.rect" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.select" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.activate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.move" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.layerStyle.makeDefault" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.layerStyle.get" => {
            r#"{"type":"object","additionalProperties":true,"properties":{"layer":{"type":"integer"},"enabled":{"type":"boolean"},"items":{"type":"array","items":{"type":"object","additionalProperties":true,"properties":{"kind":{"type":"string"},"label":{"type":"string"},"enabled":{"type":"boolean"},"params":{"type":"json"},"fx":{"type":"json"}}}}}}"#
        }
        "layer.layerStyle.defaultFor" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "gradient.fill.get" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "text.info" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "shape.info" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "path.info" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.quick" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.object" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.edgeRefine" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.modify.border" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.modify.smooth" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.modify.expand" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.modify.contract" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.modify.feather" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.grow" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.similar" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.lasso" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.magneticLasso" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.transformSelection" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.reselect" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.saveACopy" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.layersToFiles" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.colorLookupTables" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "text.hitTest" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "text.caret" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "text.navigate" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.allLayers" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.findLayers" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.selectLinkedLayers" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "prefs.set" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "edit.purge.undo" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "edit.purge.clipboard" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "edit.purge.histories" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "edit.purge.all" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.sampleColor" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "select.loadSelection" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "channel.target.slot6" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "channel.target.slot7" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "channel.target.slot8" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "channel.target.slot9" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.layerContentOptions" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.layerCompsToFiles" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.artboardsToFiles" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.artboardsToPdf" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "view.thirtyTwoBitPreviewOptions" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.presets.get" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.quickExport" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.pathsToIllustrator" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.pickAt" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "edit.presets.migratePresets" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.renderVideo" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "file.export.dataSetsAsFiles" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "plugin.install" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "plugin.remove" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "plugin.reload" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.setExpanded" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.setEffectsExpanded" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "view.layerMask" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "layer.toggleMaskOverlay" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "action.move" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "action.get" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "action.stop" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        "action.delete" => {
            r#"{"type":"object","additionalProperties":true,"properties":{},"description":"an object; the command's description says what it holds"}"#
        }
        _ => return None,
    };
    serde_json::from_str(schema).ok()
}

/// Types for keys the notation leaves as any value (`json`), by what each key name holds across the
/// engine's commands (each type admits every form the commands' notes list).
fn type_key(key: &str, node: &mut Value) {
    use serde_json::json;
    let any_of = |kinds: &[Value]| json!({ "oneOf": kinds });
    let number_list = json!({ "type": "array", "items": { "type": "number" } });
    let typed = match key {
        "color" | "foreground" | "background" => {
            any_of(&[json!({ "type": "string" }), number_list.clone(), json!({ "type": "object", "additionalProperties": true })])
        }
        "target" | "path" => any_of(&[json!({ "type": "string" }), json!({ "type": "object", "additionalProperties": true })]),
        "layer" | "channel" | "pattern" | "comp" => any_of(&[json!({ "type": "integer" }), json!({ "type": "string" })]),
        "preset" | "input" => any_of(&[json!({ "type": "string" }), json!({ "type": "array", "items": { "type": "string" } })]),
        "red" | "green" | "blue" | "gray" | "reds" | "yellows" | "greens" | "cyans" | "blues" | "magentas" | "whites" | "neutrals" | "blacks" | "shadows"
        | "midtones" | "highlights" => any_of(&[json!({ "type": "array" }), json!({ "type": "object", "additionalProperties": true })]),
        "points" | "stops" | "pins" => json!({ "type": "array" }),
        "rect" | "radii" => number_list,
        "index" => json!({ "type": "integer" }),
        "group" | "name" | "to" | "output" => json!({ "type": "string" }),
        _ => return,
    };
    let (description, required, default) = (node.get("description").cloned(), node.get("required").cloned(), node.get("default").cloned());
    *node = typed;
    for (field, value) in [("description", description), ("required", required), ("default", default)] {
        if let Some(value) = value {
            node[field] = value;
        }
    }
}
