//! What each write rests on beyond the pixel families of [`crate::place`] (task card
//! C8-photo-read): read from every write command's handler and the engine operation behind it
//! (`tests/declarations_independent.rs` holds the readings, command by command).
//!
//! - **The layer a command acts on**, named or not: a command that reads the active layer holds
//!   that layer even when the call does not name it (the active one), with the fields its
//!   handler reads or replaces ([`layer_fields`]); a command that inserts a new layer at the
//!   active one holds where that layer is and what kind it is (a group takes the new layer in, a
//!   clipping base passes its clip on).
//! - **The selected layers** a command acts on without `layer` ([`selected_fields`]).
//! - **The image's own fields and stored members** a command reads ([`image_fields`]): the canvas
//!   a geometry's coordinates are in, the mode a conversion converts from, the notes, counts,
//!   slices, timeline, variables, guides, channels, paths and metadata a command addresses by
//!   index or name, and the composite (every layer and what they draw with) for one that
//!   samples or flattens it.
//! - **Other layers** a command reads: the layer below for a merge or stamp down.

use kition_kit::Premise;
use kition_photo_engine::doc::{Document, Effect, Fill, LayerContent, LayerId, effects::FxPaint};
use serde_json::Value;

/// A premise's field list, or `None` for every field.
pub type Fields = Option<&'static [&'static str]>;

const NONE: [&str; 0] = [];

/// Commands that make a new layer above the active one (or inside it, when it is a group): they
/// rest on where the active layer is and its kind and clip.
fn inserts_at_active(command: &str) -> bool {
    command.starts_with("layer.newAdjustmentLayer.")
        || command.starts_with("layer.newFillLayer.")
        || matches!(
            command,
            "layer.new.layer"
                | "layer.new.group"
                | "layer.stampVisible"
                | "type.create"
                | "shape.create"
                | "shape.presets.place"
                | "gradient.fill.create"
                | "gradient.presets.apply"
                | "pattern.presets.apply"
                | "file.placeEmbedded"
                | "file.placeLinked"
                | "file.import.videoFramesToLayers"
                | "layer.videoLayers.newBlankVideoLayer"
                | "layer.videoLayers.newVideoLayerFromFile"
                | "edit.paste"
                | "edit.pasteSpecial.pasteInPlace"
                | "edit.pasteSpecial.pasteInto"
                | "edit.pasteSpecial.pasteOutside"
                | "image.analysis.placeScaleMarker"
        )
}

/// What a command reads or replaces of the one layer it acts on (by `layer`, `at`, or the active
/// layer), for commands outside the pixel families; `None` for every field. Positions: whether
/// where the layer is matters.
pub fn layer_fields(command: &str, params: &Value) -> (Fields, bool) {
    const EFFECTS: &[&str] = &["styleSettings", "styles"];
    const STYLE: &[&str] = &["styleSettings", "styles", "blend", "fillOpacity", "extra"];
    const MASK: &[&str] = &["kind", "locks", "mask", "name"];
    const MASK_PIXELS: &[&str] = &["kind", "locks", "mask", "data", "text", "tiles"];
    const VECTOR_MASK: &[&str] = &["kind", "locks", "extra.vector_mask"];
    const CONTENT: &[&str] = &["kind", "name", "data", "text"];
    const SHAPE_PATH: &[&str] = &["kind", "data", "text", "extra"];
    const INSERT: &[&str] = &["kind", "clipped"];
    if inserts_at_active(command) {
        // `shape.create` with `addTo` adds to that shape layer instead.
        return (Some(INSERT), true);
    }
    match command {
        "layer.layerStyle.dropShadow"
        | "layer.layerStyle.innerShadow"
        | "layer.layerStyle.outerGlow"
        | "layer.layerStyle.innerGlow"
        | "layer.layerStyle.stroke"
        | "layer.layerStyle.colorOverlay"
        | "layer.layerStyle.gradientOverlay"
        | "layer.layerStyle.patternOverlay"
        | "layer.layerStyle.bevelEmboss"
        | "layer.layerStyle.satin"
        | "layer.layerStyle.replace"
        | "layer.layerStyle.clear"
        | "layer.layerStyle.scaleEffects"
        | "layer.setEffectsVisible" => (Some(EFFECTS), false),
        "layer.layerStyle.blendingOptions" => (Some(&["opacity", "fillOpacity", "blend", "extra"]), false),
        "layer.layerStyle.copyLayerStyle" | "layer.layerStyle.pasteLayerStyle" => (Some(STYLE), false),
        "layer.layerMask.enabled" => (Some(&["mask.props.enabled"]), false),
        "layer.layerMask.linked" => (Some(&["mask.props.linked"]), false),
        "layer.layerMask.delete"
        | "layer.layerMask.revealAll"
        | "layer.layerMask.hideAll"
        | "layer.layerMask.revealSelection"
        | "layer.layerMask.hideSelection" => (Some(MASK), false),
        "layer.layerMask.apply" | "layer.layerMask.fromTransparency" => (Some(MASK_PIXELS), false),
        "layer.rasterize.vectorMask" => (Some(&["kind", "mask", "extra"]), false),
        _ if command.starts_with("layer.vectorMask.") => (Some(VECTOR_MASK), false),
        _ if command.starts_with("layer.editableFilter.") || command.starts_with("layer.objectLayers.stackMode.") => (Some(&["kind", "data", "text"]), false),
        "layer.setAdjustment" | "gradient.fill.set" | "gradient.fill.stop" | "gradient.fill.get" => (Some(&["kind", "data", "text"]), false),
        "layer.fill.solidColor.set" => (Some(&["kind", "locks", "data", "text"]), false),
        _ if command.starts_with("layer.combineShapes.") || command == "shape.edit" => (Some(CONTENT), false),
        _ if command.starts_with("path.") && params.get("name").and_then(Value::as_str) == Some("layer") => (Some(SHAPE_PATH), false),
        "type.edit"
        | "type.setStyle"
        | "type.insertText"
        | "type.warpText"
        | "type.convertToParagraphText"
        | "type.convertToPointText"
        | "type.loadDefaultTypeStyles"
        | "type.pasteLoremIpsum" => (Some(CONTENT), false),
        _ if command.starts_with("type.antiAlias.") || command.starts_with("type.orientation.") || command.starts_with("type.openType.") => {
            (Some(CONTENT), false)
        }
        "path.fill" | "path.stroke" => (Some(&["kind", "locks", "data", "text", "tiles"]), false),
        "layer.renameLayer" => (Some(&["name"]), false),
        "layer.setLabelColor" => (Some(&["extra.label"]), false),
        "layer.showLayers" => (Some(&["visible"]), false),
        // Where the layer is, and nothing of what it holds.
        _ if command.starts_with("layer.arrange.") => (Some(&NONE), true),
        _ => (None, !crate::acts::position_independent(command)),
    }
}

/// What a command on the selected layers reads or replaces of each (`None`: every field), and
/// whether where each is matters; `None` when it does not act on the selected layers.
fn selected_fields(command: &str) -> Option<(Fields, bool)> {
    Some(match command {
        "layer.hideLayers" | "layer.showLayers" => (Some(&["visible"]), false),
        "layer.lockLayers" => (Some(&["locks"]), false),
        "layer.linkLayers" => (Some(&["extra.link_group"]), false),
        "layer.setLabelColor" => (Some(&["extra.label"]), false),
        "layer.createClippingMask" | "layer.releaseClippingMask" => (Some(&["clipped"]), true),
        "layer.translate" => return None,
        _ if command.starts_with("layer.align.") || command.starts_with("layer.distribute.") => return None,
        _ if crate::acts::reads_of(command).selection => (None, !crate::acts::position_independent(command)),
        _ => return None,
    })
}

fn premise(id: LayerId, fields: Fields, positions: bool) -> Premise {
    let mut premise = Premise::on(id.0);
    if let Some(fields) = fields {
        premise = premise.fields(fields.iter().copied());
    }
    if !positions {
        premise = premise.positions(NONE);
    }
    premise
}

/// The premise on the one layer a command acts on, when [`crate::place`]'s families state none.
/// `None` when the command does not act on that layer: a shape added to the `addTo` layer, a
/// saved or work path (`name` other than `"layer"`), or a command that only reads it into the
/// session (the default type styles).
pub fn layer_premise(command: &str, params: &Value, id: LayerId) -> Option<Premise> {
    let named_path = params.get("name").and_then(Value::as_str).is_some_and(|name| name != "layer");
    let on_a_path = matches!(command, "path.transform" | "path.moveAnchors" | "path.moveHandle" | "path.bendSegment" | "path.convertPoint");
    if (command == "shape.create" && params.get("addTo").is_some()) || (on_a_path && named_path) || command == "type.saveDefaultTypeStyles" {
        return None;
    }
    let (fields, positions) = layer_fields(command, params);
    Some(premise(id, fields, positions))
}

/// Premises on the selected layers of a command that acts on them, when the call names no
/// `layer` (a numeric `layer` is the command's object, which the guard holds itself).
pub fn selected_premises(doc: &Document, command: &str, params: &Value, selected: &[LayerId]) -> Vec<Premise> {
    if params.get("layer").is_some_and(Value::is_u64) {
        return Vec::new();
    }
    let Some((fields, positions)) = selected_fields(command) else { return Vec::new() };
    let mut out = Vec::new();
    for id in selected {
        out.push(premise(*id, fields, positions));
        if command == "layer.setLabelColor" {
            out.extend(other_layers(doc, command, *id));
        }
    }
    out
}

/// The image's fields and stored members a command reads, beyond [`crate::place`]'s inputs.
pub fn image_fields(command: &str, params: &Value) -> Vec<&'static str> {
    const CANVAS: [&str; 2] = ["width", "height"];
    const MODE: [&str; 5] = ["mode", "depth", "icc", "docState.color_table", "docState.duotone"];
    let mut out: Vec<&'static str> = Vec::new();
    let family = |prefix: &str| command.starts_with(prefix);
    if family("image.imageRotation.")
        || matches!(
            command,
            "image.rotation.arbitrary"
                | "image.canvasSize"
                | "image.crop"
                | "image.revealAll"
                | "image.trim"
                | "image.analysis.straightenLayer"
                | "view.newGuideLayout"
        )
    {
        out.extend(CANVAS);
    }
    if matches!(command, "image.imageSize" | "file.automate.fitImage") {
        out.extend(CANVAS);
        out.push("dpi");
    }
    if family("image.mode.") && command != "image.mode.colorTable" && command != "image.mode.duotone"
        || matches!(command, "edit.assignProfile" | "edit.convertToProfile" | "color.profileMismatch" | "file.automate.conditionalModeChange")
    {
        out.extend(MODE);
    }
    match command {
        "image.mode.colorTable" => out.push("docState.color_table"),
        "image.mode.duotone" => out.extend(["mode", "docState.duotone"]),
        "layer.layerStyle.globalLight" => out.push("docState.global_light"),
        "layer.linkLayers" => out.push("~linkGroups"),
        "file.fileInfo" => out.push("docState.xmp"),
        // Without `path`: the work path, else the first saved path.
        "filter.render.flame" | "shape.presets.new" if params.get("path").is_none_or(Value::is_null) => {
            out.extend(["paths", "docState.work_path"]);
        }
        "path.set" | "path.delete" | "path.rename" | "path.clippingPath.set" | "path.clippingPath.clear" => {
            out.extend(["paths", "docState.work_path", "docState.clipping_path"]);
        }
        _ => {}
    }
    if family("notes.") {
        out.push("notes");
    }
    if family("count.")
        || matches!(
            command,
            "image.analysis.setMeasurementScale" | "image.analysis.rulerTool" | "image.analysis.placeScaleMarker" | "image.analysis.straightenLayer"
        )
    {
        out.push("docState.measurement");
    }
    if family("slice.") || matches!(command, "view.clearSlices" | "layer.newLayerBasedSlice") {
        out.push("docState.slices");
    }
    if family("timeline.") || family("layer.videoLayers.") || command == "layer.rasterize.video" {
        out.push("docState.timeline");
    }
    if family("image.variables.") || matches!(command, "image.applyDataSet" | "file.import.variableDataSets") {
        out.push("docState.variables");
    }
    if matches!(
        command,
        "view.moveGuide" | "view.deleteGuide" | "view.clearGuides" | "view.clearCanvasGuides" | "view.clearSelectedArtboardGuides" | "slice.fromGuides"
    ) {
        out.push("guides");
    }
    if (family("channel.") && !matches!(command, "channel.target" | "channel.split" | "channel.merge")) || command == "select.saveSelection" {
        out.extend(["channels", "~channelContents"]);
    }
    if samples_composite(command, params) {
        out.extend(["layers", "width", "height", "mode", "depth", "icc", "patterns", "docState.global_light"]);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Commands whose result is drawn from every layer: they flatten, stamp or measure the
/// composite, or rewrite every layer of a kind.
fn samples_composite(command: &str, params: &Value) -> bool {
    matches!(
        command,
        "layer.flattenImage"
            | "layer.mergeVisible"
            | "layer.stampVisible"
            | "image.trim"
            | "image.revealAll"
            | "image.mode.indexedColor"
            | "image.mode.bitmap"
            | "image.mode.multichannel"
            | "image.applyDataSet"
            | "channel.split"
            | "channel.mergeSpot"
            | "layer.rasterize.allLayers"
            | "layer.delete.hiddenLayers"
            | "layer.layerStyle.hideAllEffects"
            | "layer.layerStyle.showAllEffects"
            | "file.scripts.flattenAllLayerEffects"
            | "file.scripts.flattenAllMasks"
            | "file.scripts.deleteAllEmptyLayers"
            | "file.automate.cropAndStraightenPhotos"
            | "type.updateAllTextLayers"
            | "type.replaceAllMissingFonts"
            | "type.resolveMissingFonts"
            | "edit.findAndReplaceText"
    ) || (command == "filter.vanishingPoint" && params.get("newLayer").and_then(Value::as_bool) == Some(true))
}

/// The layer below `id` in its parent's list (stored bottom first), if any.
fn below(doc: &Document, id: LayerId) -> Option<LayerId> {
    let mut path = doc.path_of(id)?;
    let last = path.last_mut()?;
    *last = last.checked_sub(1)?;
    doc.layer_at(&path).map(|layer| layer.id)
}

/// Other layers one layer's command reads: the layer below for a merge or a stamp down.
pub fn other_layers(doc: &Document, command: &str, id: LayerId) -> Vec<Premise> {
    match command {
        "layer.setLabelColor" if doc.layer(id).is_some_and(|layer| layer.is_group()) => {
            vec![Premise::on(id.0).fields(["~subtreeLabels"]).positions(NONE)]
        }
        "layer.mergeDown" | "layer.stampDown" => {
            let Some(lower) = below(doc, id) else { return Vec::new() };
            let mut out = vec![Premise::on(lower.0)];
            let paths: Vec<_> = [lower, id].into_iter().filter_map(|id| doc.path_of(id)).collect();
            let mut light = false;
            let mut patterns = false;
            for (path, _, layer) in doc.walk() {
                if !paths.iter().any(|root| path.starts_with(root)) {
                    continue;
                }
                patterns |= matches!(layer.content, LayerContent::Fill(Fill::Pattern { .. }));
                if !layer.effects.enabled {
                    continue;
                }
                for effect in layer.effects.items.iter().filter(|effect| effect.enabled()) {
                    match effect {
                        Effect::DropShadow(shadow) | Effect::InnerShadow(shadow) => light |= shadow.use_global_light,
                        Effect::BevelEmboss(bevel) => {
                            light |= bevel.use_global_light;
                            patterns |= bevel.texture.is_some();
                        }
                        Effect::PatternOverlay { .. } => patterns = true,
                        Effect::OuterGlow(glow) | Effect::InnerGlow(glow) => patterns |= matches!(glow.paint, FxPaint::Pattern { .. }),
                        Effect::Stroke(stroke) => patterns |= matches!(stroke.paint, FxPaint::Pattern { .. }),
                        _ => {}
                    }
                }
            }
            let mut fields = Vec::new();
            if light {
                fields.push("docState.global_light");
            }
            if patterns {
                fields.push("patterns");
            }
            if !fields.is_empty() {
                out.push(Premise::on(crate::place::IMAGE).fields(fields).positions(NONE));
            }
            out
        }
        _ => Vec::new(),
    }
}
