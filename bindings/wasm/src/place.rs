//! What one call depends on (kition-sdk `KitDocument::under`, `covered`, `derived`; task card
//! C-photo), for the premise guard of an agent's write with `basis`.
//!
//! **Addresses.** A command that reads the active layer ([`crate::acts::ACTIVE`]) names its layer
//! by id (`layer: 12`, or `on`), by its place in the image's top-level stack counted from the top
//! as inspect and the Layers panel list it (`layer: "#3"`; the replica's `layers` tree lists the
//! stack bottom first, so the commands declare `index_reversed` and [`PhotoDocument::under_call`]
//! answers the window in that stored order), or by a point
//! (`at: [x, y]`: the topmost visible layer, not fully locked, with a pixel there, as the Move
//! tool's Auto-Select picks it; the guard asks [`PhotoDocument::covered_at`] what was there).
//!
//! **Premises** ([`premises`]), for the families whose code was read:
//! - `layer.setProps`: the fields it sets (`name`, `visible`, `opacity`, `fillOpacity`, `blend`,
//!   `clipped`, `locks`, `extra.excluded_channels`), no position.
//! - pixel writes in an area of one layer (`edit.fill` and `edit.delete` on the layer's pixels,
//!   `paint.stroke`, `eraser.stroke`, the per-pixel `image.adjustments.*`): the layer's `kind`,
//!   effective `locks`, `data`, `text` and the 256-pixel tiles of its `tiles` map that the area
//!   touches (resolved stroke dabs intersected with the region/selection, else the region or
//!   selection, else every tile), and the image's size, mode and depth; no position.
//! - whole-layer pixel commands (`filter.*`, the other `image.adjustments.*`): the layer's
//!   `kind`, effective `locks`, `tiles`, `data`, `text` and the image's size, mode and depth.
//!   A mask is held when targeted or read as Lens Blur depth, not merely displayed.
//! - [`inputs`]: region sources, stored presets, composite sources and linked move participants.
//!
//! Every other command keeps kition-sdk's default: every field and position of what it names.
//!
//! **Derived** ([`PhotoDocument::derived_values`]): a layer's `bounds`, its non-transparent extent
//! as inspect lists it (`null` when it draws nothing, and for a fill layer, whose extent is the
//! canvas, not a value of its own fields); Image channel content fingerprints and link groups.

use kition_kit::{ContentPremise, Covered, CoveredAsk, CoveredAt, Covers, DerivedValues, Premise, Under, Within};
use kition_photo_engine::doc::{Document, LayerContent, LayerId};
use kition_photo_engine::inspect::xywh;
use kition_photo_geom::Rect;
use serde_json::{Value, json};

use crate::PhotoDocument;
use crate::context::{disabled, invalid, layer_of};

/// The image's object in the replica: the `layers` tree's owner, and the holder of `width` and
/// `height`.
pub const IMAGE: &str = "image";

/// The fields `covers` names for a pick by point: what besides the stack may move a layer into or
/// out of the point.
const PICK_FIELDS: [&str; 11] = ["visible", "tiles", "mask", "data", "extra", "frames", "text", "kind", "locks", "~bounds", "~pickState"];

/// The per-pixel destructive adjustments: each output pixel depends on that pixel alone.
const PER_PIXEL: [&str; 16] = [
    "image.adjustments.blackWhite",
    "image.adjustments.brightnessContrast",
    "image.adjustments.channelMixer",
    "image.adjustments.colorBalance",
    "image.adjustments.curves",
    "image.adjustments.desaturate",
    "image.adjustments.exposure",
    "image.adjustments.gradientMap",
    "image.adjustments.hueSaturation",
    "image.adjustments.invert",
    "image.adjustments.levels",
    "image.adjustments.photoFilter",
    "image.adjustments.posterize",
    "image.adjustments.selectiveColor",
    "image.adjustments.threshold",
    "image.adjustments.vibrance",
];

/// How a command's premise on its layer is narrowed ([`premises`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `layer.setProps`.
    Props,
    /// Pixels in an area of one layer.
    Area,
    /// All of one layer's pixels.
    Whole,
}

/// The family whose premise `command` states, if its code was read.
pub fn family(command: &str) -> Option<Family> {
    match command {
        "layer.setProps" => Some(Family::Props),
        "edit.fill"
        | "edit.delete"
        | "edit.cut"
        | "edit.fillForeground"
        | "edit.fillBackground"
        | "edit.fillForegroundPreserve"
        | "edit.fillBackgroundPreserve"
        | "paint.stroke"
        | "eraser.stroke" => Some(Family::Area),
        _ if PER_PIXEL.contains(&command) => Some(Family::Area),
        _ if !crate::acts::reads_of(command).active => None,
        _ if command.starts_with("image.adjustments.") || command.starts_with("filter.") || command.starts_with("layer.matting.") => Some(Family::Whole),
        // Paint tools other than a stroke: what they paint follows the layer's pixels (a fill's
        // extent, a clone's source, a healing's surroundings) beyond the brush's own dabs.
        _ if command.starts_with("paint.") && !matches!(command, "paint.symmetryFromPath" | "paint.setSymmetry" | "paint.symmetryDisable") => {
            Some(Family::Whole)
        }
        "image.autoTone"
        | "image.autoContrast"
        | "image.autoColor"
        | "image.trap"
        | "image.applyImage"
        | "edit.stroke"
        | "edit.smartFill"
        | "edit.deleteAndFillSelection"
        | "edit.smartScale"
        | "plugin.run" => Some(Family::Whole),
        _ => None,
    }
}

/// The top-level layer `"#n"` names: the n-th of the image's top-level stack from the top, with its
/// index in stored order (`doc.layers`, bottom first).
pub fn index_layer(doc: &Document, text: &str) -> Result<Option<(usize, LayerId)>, String> {
    let Some(rest) = text.trim().strip_prefix('#') else { return Ok(None) };
    let n: usize = rest
        .parse()
        .map_err(|_| invalid(format!("`{text}` is no place in the stack: \"#n\" is the n-th layer of the image's top level from the top, n from 1")))?;
    let count = doc.layers.len();
    let index = n.checked_sub(1).filter(|from_top| *from_top < count).map(|from_top| count - 1 - from_top).ok_or_else(|| {
        invalid(format!("`{text}` is past the stack: the image's top level holds {count} layer(s), \"#1\" the top one, \"#{count}\" the bottom one"))
    })?;
    Ok(doc.layers.get(index).map(|layer| (index, layer.id)))
}

/// The one layer named `name` (a whole name, not an id's digits nor `"#n"`): `Ok(None)` for a text
/// that is an id or an index.
pub fn named_layer(doc: &Document, text: &str) -> Result<Option<LayerId>, String> {
    let trimmed = text.trim();
    if trimmed.starts_with('#') || trimmed.parse::<u64>().is_ok() {
        return Ok(None);
    }
    let found: Vec<LayerId> = doc.walk().iter().filter(|(_, _, layer)| layer.name == text).map(|(_, _, layer)| layer.id).collect();
    match found.as_slice() {
        [one] => Ok(Some(*one)),
        [] => Err(invalid(format!("no layer is named \"{text}\": name a layer by its id, \"#n\" or its whole name as inspect lists them"))),
        many => Err(invalid(format!("{} layers are named \"{text}\": name the one you mean by its id", many.len()))),
    }
}

/// The layer a `layer` value names: an id, `"#n"`, or a whole name.
pub fn layer_value(doc: &Document, value: &Value) -> Result<LayerId, String> {
    if let Some(text) = value.as_str() {
        if let Some((_, id)) = index_layer(doc, text)? {
            return Ok(id);
        }
        if let Some(id) = named_layer(doc, text)? {
            return Ok(id);
        }
    }
    let id = layer_of(value).ok_or_else(|| {
        invalid(format!("`layer` is {value}: a layer's id as inspect lists it, \"#n\" (the n-th of the top level from the top), or its whole name"))
    })?;
    doc.layer(id).map(|_| id).ok_or_else(|| invalid(format!("the image has no layer {}", id.0)))
}

/// The layer a pick at `(x, y)` names: the topmost visible layer, not fully locked, with a pixel
/// there (the Move tool's Auto-Select).
pub fn pick(doc: &Document, x: i32, y: i32) -> Option<LayerId> {
    kition_photo_engine::pick_cmds::layers_at(doc, x, y).into_iter().find(|id| !doc.effective_locks(*id).all)
}

/// `[x, y]` as whole pixels.
pub fn point(value: &Value) -> Result<(i32, i32), String> {
    let items = value.as_array().filter(|items| items.len() == 2);
    let coordinate = |item: &Value| item.as_f64().filter(|v| v.is_finite() && v.abs() < 1e9).map(|v| v.floor() as i32);
    match items.map(|items| (coordinate(&items[0]), coordinate(&items[1]))) {
        Some((Some(x), Some(y))) => Ok((x, y)),
        _ => Err(invalid(format!("`at` is {value}: a point is [x, y] in image pixels"))),
    }
}

/// The area a pixel write in an area acts in, in image pixels: its `region`, its stroke's points
/// widened by the brush, the current selection, or `None` for every pixel.
fn area(doc: &PhotoDocument, command: &str, params: &Value) -> Result<Option<Rect>, String> {
    let st = doc.session.active().ok_or_else(|| disabled("no image is open"))?;
    let selection = if let Some(region) = params.get("region") {
        crate::context::region_mask(st, region)?.map(|mask| mask.content_bounds())
    } else {
        st.doc.selection.as_ref().map(|mask| mask.content_bounds())
    };
    if command.ends_with(".stroke") && command != "edit.stroke" {
        use kition_photo_engine::paint::{BrushContext, dynamics::DabGenerator};
        let brush = kition_photo_engine::brush_cmds::resolve_brush(&doc.session, params, command).map_err(|e| invalid(e.to_string()))?;
        let points = kition_photo_engine::brush_cmds::parse_points(params, command).map_err(|e| invalid(e.to_string()))?;
        let zoom = params.get("zoom").and_then(Value::as_f64).unwrap_or(1.0) as f32;
        let ctx = BrushContext::new(&brush);
        let mut passes = st.symmetry.as_ref().map(|axis| axis.reflected_passes(&points)).unwrap_or_default();
        passes.push(points);
        let mut out = Rect::EMPTY;
        for points in passes {
            let mut generator = DabGenerator::new(&brush, zoom);
            let (mut primary, mut dual) = (Vec::new(), Vec::new());
            generator.push(&points, &mut primary, &mut dual);
            generator.finish(&mut primary, &mut dual);
            for dab in primary {
                out = out.union(&ctx.dab_rect(&dab, false));
            }
        }
        return Ok(Some(selection.map_or(out, |sel| out.intersect(&sel))));
    }
    // `"all"` resolves to a full-canvas mask, while absence of a selection means every pixel.
    if params.get("region").is_some_and(|region| !region.is_null()) && selection.is_none() {
        return Ok(Some(st.doc.bounds()));
    }
    Ok(selection)
}

/// A call can depend on the same object in several ways (a source can also be the target).
/// The guard takes the last premise for an id, so combine their fields before answering it.
pub(crate) fn hold(under: &mut Under, premise: Premise) {
    let Some(kept) = under.premises.iter_mut().find(|p| p.id == premise.id) else {
        under.premises.push(premise);
        return;
    };
    fn union(a: &mut Option<Vec<String>>, b: Option<Vec<String>>) {
        match (a.as_mut(), b) {
            (Some(a), Some(b)) => {
                for field in b {
                    if !a.contains(&field) {
                        a.push(field);
                    }
                }
            }
            (_, None) => *a = None,
            _ => {}
        }
    }
    union(&mut kept.fields, premise.fields);
    union(&mut kept.positions, premise.positions);
    kept.identities.extend(premise.identities);
}

fn image_fields(under: &mut Under, fields: impl IntoIterator<Item = impl Into<String>>) {
    hold(under, Premise::on(IMAGE).fields(fields).positions([] as [&str; 0]));
}

/// The image's fields and members [`crate::rests::image_fields`] says `command` reads.
fn rested_image(under: &mut Under, command: &str, params: &Value) {
    let fields = crate::rests::image_fields(command, params);
    if !fields.is_empty() {
        image_fields(under, fields);
    }
}

fn composite(under: &mut Under) {
    image_fields(under, ["layers", "width", "height", "mode", "depth", "icc", "patterns", "docState.global_light"]);
}

fn active_document(doc: &PhotoDocument, selector: &Value) -> bool {
    let index = match selector {
        Value::String(name) => doc.session.documents().iter().position(|st| st.doc.name == *name || st.path.as_deref() == Some(name.as_str())),
        value => value.as_u64().and_then(|i| usize::try_from(i).ok()),
    };
    index.is_some() && index == doc.session.active_index()
}

/// These tools sample the rendered document only in the modes their implementations resolve.
fn samples_composite(doc: &PhotoDocument, command: &str, params: &Value) -> Result<bool, String> {
    let pixels = !matches!(params.get("target").and_then(Value::as_str), Some("mask" | "quickMask"))
        && params.get("target").and_then(|target| target.get("channel")).and_then(Value::as_u64).is_none();
    Ok(match command {
        "paint.mixerBrush" => {
            let brush = kition_photo_engine::brush_cmds::resolve_brush(&doc.session, params, command).map_err(|e| invalid(e.to_string()))?;
            params.get("sampleAllLayers").and_then(Value::as_bool).unwrap_or(brush.mixer.sample_all_layers)
        }
        // Clone and Healing Brush sample the composite even when painting a mask/channel.
        "paint.cloneStamp" | "paint.healingBrush" => matches!(params.get("sampleLayer").and_then(Value::as_str), Some("all" | "currentAndBelow")),
        "paint.bucket" | "paint.spotHealing" | "paint.smartMove" | "paint.blur" | "paint.sharpen" | "paint.smudge" => {
            params.get("sampleAllLayers").and_then(Value::as_bool).unwrap_or(false) && pixels
        }
        _ => false,
    })
}

/// Mirror channel_cmds::inject_target after the call's layer has become active.
fn routed_params(doc: &PhotoDocument, command: &str, params: &Value, layer: Option<LayerId>) -> Result<Value, String> {
    if params.get("target").is_some() || !kition_photo_engine::channel_cmds::follows_target(command) {
        return Ok(params.clone());
    }
    use kition_photo_engine::channel_cmds::ChannelTarget;
    let st = doc.state()?;
    let target = match st.channel_view.target {
        ChannelTarget::Alpha(i) if i < st.doc.channels.len() => json!({ "channel": i }),
        ChannelTarget::Composite if st.doc.quick_mask.is_some() => json!("quickMask"),
        ChannelTarget::Composite
            if st.channel_view.layer_mask.is_some_and(|view| Some(view.layer) == layer && st.doc.layer(view.layer).is_some_and(|l| l.mask.is_some())) =>
        {
            json!("mask")
        }
        _ => return Ok(params.clone()),
    };
    let mut resolved = params.clone();
    if let Some(fields) = resolved.as_object_mut() {
        fields.insert("target".to_owned(), target);
    }
    Ok(resolved)
}

/// The nested form wins over the flat form, as channel_cmds::source_spec resolves it.
pub(crate) fn source_spec(params: &Value, key: &str, prefix: &str) -> Value {
    if let Some(source) = params.get(key).filter(|source| source.is_object()) {
        return source.clone();
    }
    let mut out = serde_json::Map::new();
    for (field, suffix) in [("layer", "Layer"), ("channel", "Channel"), ("document", "Document")] {
        if let Some(value) = params.get(format!("{prefix}{suffix}")) {
            out.insert(field.to_owned(), value.clone());
        }
    }
    Value::Object(out)
}

fn channel_source(doc: &PhotoDocument, under: &mut Under, source: &Value) -> Result<(), String> {
    let st = doc.state()?;
    let channel = source.get("channel");
    if channel.is_some_and(crate::resource_rests::alpha_address) {
        image_fields(under, [] as [&str; 0]);
        under.objects.push(json!(IMAGE));
        under.covers.get_or_insert_with(Covers::default).fields.push("~channelContents".to_owned());
    } else if matches!(channel.and_then(Value::as_str), Some("transparency" | "mask" | "vectorMask")) {
        if let Some(id) = source.get("layer").and_then(layer_of).or(st.active_layer) {
            hold(under, Premise::on(id.0));
        }
    } else if source.get("layer").and_then(layer_of).is_none() && !matches!(channel.and_then(Value::as_str), Some("selection" | "quickMask")) {
        composite(under);
    }
    image_fields(under, ["width", "height", "mode", "depth"]);
    Ok(())
}

/// Inputs that are not the active layer. These are also needed for commands with no layer
/// target, such as applying a comp or creating a fill layer from a stored pattern.
fn inputs(doc: &PhotoDocument, under: &mut Under, command: &str, params: &Value) -> Result<(), String> {
    let mut named = crate::names::stored_fields(command, params);
    if !crate::resource_rests::source_channels(command, params).is_empty() {
        named.retain(|field| !matches!(*field, "channels" | "~channelContents"));
    }
    if crate::resource_rests::stored_pattern(command, params).is_some() {
        named.retain(|field| *field != "patterns");
    }
    if !named.is_empty() {
        image_fields(under, named);
    }
    let st = doc.session.active().ok_or_else(|| disabled("no image is open"))?;
    if kition_photo_engine::channel_cmds::follows_target(command)
        && params.get("target").and_then(|target| target.get("channel")).and_then(Value::as_u64).is_some()
    {
        image_fields(under, ["channels", "~channelContents", "width", "height", "mode", "depth"]);
    }
    if command == "layer.translate" || command.starts_with("layer.align.") || command.starts_with("layer.distribute.") {
        let roots = if command == "layer.translate" {
            params.get("layer").map(|value| layer_value(&st.doc, value)).transpose()?.map(|id| vec![id]).unwrap_or_else(|| st.selected_layers())
        } else {
            st.selected_layers()
        };
        for id in kition_photo_engine::layer_multi_cmds::move_targets(&st.doc, &roots) {
            hold(under, Premise::on(id.0));
        }
        image_fields(under, ["~linkGroups"]);
    }
    if crate::acts::reads_of(command).region {
        if let Some(id) = params.get("region").and_then(|r| r.get("layer")).and_then(layer_of) {
            hold(under, Premise::on(id.0).fields(["kind", "tiles", "data", "text"]).positions([] as [&str; 0]));
            under.objects.push(json!(id.0));
            under.covers.get_or_insert_with(Covers::default).fields.push("~bounds".to_owned());
        }
        if let Some(channel) = params.get("region").and_then(|r| r.get("channel")) {
            let index = st.doc.channels.iter().enumerate().find(|(i, c)| channel.as_u64() == Some(*i as u64) || channel.as_str() == Some(c.name.as_str()));
            if index.is_some() {
                channel_source(doc, under, &json!({"channel":channel}))?;
            }
        }
    }
    let pattern = match command {
        "edit.fill" => params.get("contents").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("pattern")),
        "paint.bucket" => params.get("contents").or_else(|| params.get("source")).and_then(Value::as_str) == Some("pattern"),
        "layer.layerStyle.bevelEmboss" => params.get("texture").and_then(Value::as_str).is_some_and(|s| !s.is_empty()),
        "layer.layerStyle.replace" => params
            .get("effects")
            .and_then(Value::as_array)
            .is_some_and(|effects| effects.iter().any(|e| e.get("kind").and_then(Value::as_str) == Some("patternOverlay"))),
        "layer.newFillLayer.pattern"
        | "layer.layerStyle.patternOverlay"
        | "paint.patternStamp"
        | "pattern.rename"
        | "pattern.presets.apply"
        | "pattern.presets.select"
        | "brush.texturePattern" => true,
        _ => false,
    };
    if pattern {
        if crate::resource_rests::stored_pattern(command, params).is_some() {
            // The pattern it names at basis, told through the image's `~patternContents`
            // ([`crate::resource_rests::sources_covered`]).
            hold(under, Premise::on(IMAGE).fields([] as [&str; 0]).positions([] as [&str; 0]));
            if !under.objects.contains(&json!(IMAGE)) {
                under.objects.push(json!(IMAGE));
            }
            under.covers.get_or_insert_with(Covers::default).fields.push("~patternContents".to_owned());
        } else {
            image_fields(under, ["patterns"]);
        }
    }
    if command.starts_with("type.characterStyle.") || command.starts_with("type.paragraphStyle.") {
        image_fields(under, ["docState.text_styles"]);
    }
    if command.starts_with("layerComp.") {
        // Which comp was applied last, and the Last Document State, are each person's own.
        image_fields(under, ["comps"]);
        // Applying captures the previous appearance too; new/update capture the live stack.
        if matches!(
            command,
            "layerComp.new" | "layerComp.update" | "layerComp.apply" | "layerComp.next" | "layerComp.previous" | "layerComp.restoreLastDocumentState"
        ) {
            image_fields(under, ["layers", "width", "height"]);
        }
    }
    let sampled_composite = params.get("eyedropper").is_some_and(|e| e.get("at").is_some() && e.get("color").is_none())
        && matches!(command, "image.adjustments.curves" | "layer.newAdjustmentLayer.curves" | "layer.setAdjustment");
    if command == "image.adjustments.hdrToning" || sampled_composite || samples_composite(doc, command, params)? {
        composite(under);
    }
    if command == "filter.distort.displace"
        && params.get("mapPath").and_then(Value::as_str).is_none_or(str::is_empty)
        && params.get("mapDocument").is_some_and(|value| active_document(doc, value))
        && params.get("mapLayer").and_then(Value::as_u64).is_none_or(|id| st.doc.layer(LayerId(id)).is_none())
    {
        composite(under);
    }
    if command == "image.adjustments.matchColor"
        && params.get("source").filter(|v| v.as_i64().is_some_and(|i| i >= 0)).is_some_and(|value| active_document(doc, value))
        && params.get("sourceLayer").and_then(Value::as_u64).is_none()
    {
        composite(under);
    }
    if command == "image.applyImage" {
        channel_source(doc, under, &source_spec(params, "source", "source"))?;
    } else if command == "image.calculations" {
        for key in ["source1", "source2"] {
            channel_source(doc, under, &source_spec(params, key, key))?;
        }
    }
    if matches!(command, "image.applyImage" | "image.calculations")
        && (params.get("mask").is_some() || params.get("maskLayer").is_some() || params.get("maskChannel").is_some())
    {
        channel_source(doc, under, &source_spec(params, "mask", "mask"))?;
    }
    if command == "filter.lensCorrection" {
        image_fields(under, ["docState.exif"]);
    }
    Ok(())
}

/// What `command` reads or replaces of layer `id` ([`Family`]), and of the image.
fn premises(doc: &PhotoDocument, command: &str, params: &Value, id: LayerId) -> Result<Vec<Premise>, String> {
    let Some(family) = family(command) else { return Ok(Vec::new()) };
    let none: [&str; 0] = [];
    let to_mask = crate::context::targets_mask(params);
    let bounded = family == Family::Area
        && !to_mask
        && area(doc, command, params)?.is_some()
        && params.get("target").is_none_or(|target| target == "pixels")
        && !params.get("contents").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("smart"));
    let image_size = || {
        let mut fields = if bounded { vec!["mode", "depth"] } else { vec!["width", "height", "mode", "depth"] };
        if !to_mask {
            fields.push("~inheritedLocks");
        }
        Premise::on(IMAGE).fields(fields).positions(none)
    };
    Ok(match family {
        Family::Props => {
            let mut fields = Vec::new();
            for (key, field) in [
                ("name", "name"),
                ("visible", "visible"),
                ("opacity", "opacity"),
                ("fill", "fillOpacity"),
                ("blend", "blend"),
                ("clipped", "clipped"),
                ("locked", "locks"),
                ("locks", "locks"),
                ("channels", "extra.excluded_channels"),
            ] {
                if params.get(key).is_some() && !fields.contains(&field) {
                    fields.push(field);
                }
            }
            vec![Premise::on(id.0).fields(fields).positions(none)]
        }
        Family::Area
            if !to_mask
                && params.get("target").is_none_or(|target| target == "pixels")
                && !params.get("contents").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("smart")) =>
        {
            let mut fields = vec!["kind".to_owned(), "locks".to_owned(), "data".to_owned(), "text".to_owned()];
            match area(doc, command, params)? {
                Some(_) => {}
                None => fields.push("tiles".to_owned()),
            }
            vec![Premise::on(id.0).fields(fields).positions(none), image_size()]
        }
        Family::Area | Family::Whole => {
            let fields = if to_mask {
                vec!["kind", "mask"]
            } else {
                let mut fields = vec!["kind", "locks", "tiles", "data", "text"];
                if command == "filter.blur.lensBlur" && params.get("depthMap").and_then(Value::as_str) == Some("layerMask") {
                    fields.push("mask");
                }
                fields
            };
            vec![Premise::on(id.0).fields(fields).positions(none), image_size()]
        }
    })
}

impl PhotoDocument {
    /// `context.under` ([`kition_kit::KitDocument::under`]): for a command that reads the active
    /// layer, the layer its call names (by id, by `"#n"`, by `at`, else the active one) and what the
    /// call reads of it; `None` for other commands.
    pub(crate) fn under_call(&self, command: &str, params: &Value) -> Result<Option<Under>, String> {
        let st = self.session.active().ok_or_else(|| disabled("no image is open"))?;
        let doc = &st.doc;
        if command == "layer.select" && matches!(params.get("mode").and_then(Value::as_str).unwrap_or("replace"), "replace" | "add" | "toggle") {
            let id = params.get("layer").and_then(layer_of).ok_or_else(|| invalid("layer.select requires a layer id"))?;
            return Ok(Some(Under { premises: vec![Premise::on(id.0).fields([] as [&str; 0]).positions([] as [&str; 0])], ..Default::default() }));
        }
        if let Some(under) = crate::resource_rests::under(doc, command, params) {
            return Ok(Some(under));
        }
        let mut under = Under::default();
        if !crate::acts::reads_of(command).active {
            for premise in crate::rests::selected_premises(doc, command, params, &st.selected_layers()) {
                hold(&mut under, premise);
            }
            inputs(self, &mut under, command, params)?;
            rested_image(&mut under, command, params);
            return Ok((!under.premises.is_empty()).then_some(under));
        }
        let by_point = crate::acts::picks_by_point(command);
        let mut named = None;
        let target = if let Some(at) = params.get("at").filter(|at| by_point && !at.is_null()) {
            let (x, y) = point(at)?;
            under.covers = Some(Covers { fields: PICK_FIELDS.iter().map(|field| (*field).to_owned()).collect() });
            let picked = pick(doc, x, y);
            under.objects.extend(picked.map(|id| json!(id.0)));
            picked
        } else if let Some(text) = params.get("layer").and_then(Value::as_str).filter(|text| text.trim_start().starts_with('#')) {
            let (index, id) = index_layer(doc, text)?.ok_or_else(|| invalid(format!("`{text}` is no place in the stack")))?;
            under.within.push(Within { parent: json!(IMAGE), field: "layers".to_owned(), from: index, to: index + 1, ..Default::default() });
            under.objects.push(json!(id.0));
            // The image is where the index counts: it must exist, nothing of it is read.
            image_fields(&mut under, [] as [&str; 0]);
            Some(id)
        } else if let Some(name) = params.get("layer").and_then(Value::as_str).map(|text| named_layer(doc, text)).transpose()?.flatten() {
            // A name is a place: what it names now, and that it is still that layer's name.
            under.objects.push(json!(name.0));
            named = Some(name);
            Some(name)
        } else if let Some(value) = params.get("layer").filter(|value| !value.is_null()) {
            Some(layer_of(value).ok_or_else(|| invalid(format!("`layer` is {value}: a layer id, \"#n\" or a name")))?)
        } else {
            st.active_layer
        };
        if let Some(id) = target.filter(|id| doc.layer(*id).is_none()) {
            hold(&mut under, Premise::on(id.0).fields([] as [&str; 0]).positions([] as [&str; 0]));
        }
        if let Some(id) = target.filter(|id| doc.layer(*id).is_some()) {
            let params = routed_params(self, command, params, Some(id))?;
            let params = &params;
            if kition_photo_engine::channel_cmds::follows_target(command)
                && params.get("target").and_then(|t| t.get("channel")).and_then(Value::as_u64).is_some()
            {
                hold(&mut under, Premise::on(id.0).fields([] as [&str; 0]).positions([] as [&str; 0]));
                image_fields(&mut under, ["channels", "~channelContents", "width", "height", "mode", "depth"]);
            } else {
                for premise in premises(self, command, params, id)? {
                    hold(&mut under, premise);
                }
                if family(command) == Some(Family::Area)
                    && !crate::context::targets_mask(params)
                    && params.get("target").is_none_or(|target| target == "pixels")
                    && !params.get("contents").and_then(Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case("smart"))
                    && let Some(area) = area(self, command, params)?
                {
                    let region = json!([area.x0, area.y0, area.x1, area.y1]);
                    under.content.push(ContentPremise { id: json!(id.0), field: "tiles".to_owned(), region: region.clone() });
                    under.content.push(ContentPremise { id: json!(IMAGE), field: "~canvasCoordinates".to_owned(), region: json!(id.0) });
                    for field in ["width", "height"] {
                        under.content.push(ContentPremise { id: json!(IMAGE), field: field.to_owned(), region: region.clone() });
                    }
                }
            }
            // The layer the command acts on, by `layer`, `at` or as the active one: what its
            // handler reads or replaces of it, when no pixel family says it.
            if !under.premises.iter().any(|premise| premise.id == json!(id.0))
                && let Some(premise) = crate::rests::layer_premise(command, params, id)
            {
                hold(&mut under, premise);
            }
            for premise in crate::rests::other_layers(doc, command, id) {
                hold(&mut under, premise);
            }
            if crate::acts::reads_of(command).selection {
                for premise in crate::rests::selected_premises(doc, command, params, &st.selected_layers()) {
                    hold(&mut under, premise);
                }
            }
        }
        let mut input_params = routed_params(self, command, params, target)?;
        if command == "layer.translate"
            && let Some(id) = target
            && let Some(fields) = input_params.as_object_mut()
        {
            fields.insert("layer".to_owned(), json!(id.0));
        }
        inputs(self, &mut under, command, &input_params)?;
        rested_image(&mut under, command, params);
        if let Some(id) = named
            && let Some(premise) = under.premises.iter_mut().find(|premise| premise.id == json!(id.0))
            && let Some(fields) = premise.fields.as_mut()
            && !fields.iter().any(|field| field == "name")
        {
            fields.push("name".to_owned());
        }
        Ok(Some(under))
    }

    /// `context.covered` ([`kition_kit::KitDocument::covered`]): for a pick by point, what the
    /// point named at `basis`. Exact where the layers' values then are known: a layer whose
    /// visibility changed is evaluated with its visibility then; one whose pixels changed is known
    /// not to have covered the point when its bounds then (`~bounds`) did not hold it, and otherwise
    /// unknown (`covered: None`, which the guard reads as a refusal); a deleted layer likewise from
    /// its bounds then; a moved one (its place in the stack then is not given) is unknown when it
    /// covers the point now or covered it then.
    pub(crate) fn covered_at(&self, command: &str, params: &Value, objects: &[CoveredAsk]) -> Result<Option<CoveredAt>, String> {
        let Some(at) = params.get("at").filter(|at| !at.is_null()) else { return Ok(None) };
        if !crate::acts::picks_by_point(command) {
            return Ok(None);
        }
        let (x, y) = point(at)?;
        let st = self.session.active().ok_or_else(|| disabled("no image is open"))?;
        let mut then = (*st.doc).clone();
        let mut unknown = false;
        let mut covered = Vec::new();
        // The entries told after the pick at basis is known.
        let mut pending = Vec::new();
        for ask in objects {
            let Some(id) = layer_of(&ask.id) else {
                covered.push(Covered { id: ask.id.clone(), covered: None, ..Default::default() });
                unknown = true;
                continue;
            };
            if ask.created {
                if let Some(layer) = then.layer_mut(id) {
                    layer.visible = false;
                }
                covered.push(Covered { id: ask.id.clone(), covered: Some(false), ..Default::default() });
                continue;
            }
            // Its bounds then: as told when they changed, else as they are now (`~bounds` is one of
            // the fields `covers` names, so a change of them is told).
            let bounds_now = then.layer(id).map(bounds);
            let bounds_then = match ask.before.get("~bounds") {
                Some(bounds) => Some(bounds.clone()),
                None if ask.deleted || ask.unknown.iter().any(|u| u == "~bounds") => None,
                None => bounds_now,
            };
            let fill = then.layer(id).is_some_and(|layer| matches!(layer.content, LayerContent::Fill(_)));
            // Null is also used for fills, groups and artboards, including deleted ones.
            // It cannot establish that their pixels (or descendants) missed the point.
            let outside_then = !fill && bounds_then.as_ref().is_some_and(|bounds| bounds.is_array() && !holds(bounds, x, y));
            let pixels_changed = ["tiles", "mask", "data", "extra", "frames", "text", "kind"]
                .iter()
                .any(|field| ask.before.contains_key(*field) || ask.unknown.iter().any(|u| u == field));
            if ask.deleted || pixels_changed || !ask.unknown.is_empty() {
                if outside_then || ask.before.get("visible") == Some(&json!(false)) {
                    covered.push(Covered { id: ask.id.clone(), covered: Some(false), ..Default::default() });
                } else {
                    covered.push(Covered { id: ask.id.clone(), covered: None, ..Default::default() });
                    unknown = true;
                }
                continue;
            }
            if let Some(visible) = ask.before.get("visible").and_then(Value::as_bool)
                && let Some(layer) = then.layer_mut(id)
            {
                layer.visible = visible;
            }
            if let Some(locks) = ask.before.get("locks")
                && let Ok(locks) = serde_json::from_value(locks.clone())
                && let Some(layer) = then.layer_mut(id)
            {
                layer.locks = locks;
            }
            pending.push(covered.len());
            covered.push(Covered { id: ask.id.clone(), covered: None, ..Default::default() });
        }
        let picked = pick(&then, x, y);
        let moved_and_there = |id: LayerId| kition_photo_engine::pick_cmds::layers_at(&then, x, y).contains(&id);
        for at in pending {
            let Some(entry) = covered.get_mut(at) else { continue };
            let Some(id) = layer_of(&entry.id) else { continue };
            let ask = objects.iter().find(|ask| ask.id == entry.id);
            let only_moved = ask.is_some_and(|ask| ask.before.is_empty() && ask.unknown.is_empty() && !ask.deleted && !ask.created);
            if only_moved && (moved_and_there(id) || picked.is_some_and(|p| contains(&then, id, p))) {
                unknown = true;
                continue;
            }
            entry.covered = Some(picked == Some(id) || picked.is_some_and(|p| contains(&then, id, p)));
        }
        let under = if unknown { Vec::new() } else { picked.map(|id| vec![json!(id.0)]).unwrap_or_default() };
        Ok(Some(CoveredAt { under, covered }))
    }

    /// `model.derived`: layer bounds and Image channel/link/lock inputs; every object's when `ids`
    /// is empty, and every value when `names` is.
    pub(crate) fn derived_values(&self, ids: &[Value], names: &[String]) -> Result<Vec<DerivedValues>, String> {
        let st = self.session.active().ok_or_else(|| disabled("no image is open"))?;
        let wanted: Vec<LayerId> =
            if ids.is_empty() { st.doc.walk().iter().map(|(_, _, layer)| layer.id).collect() } else { ids.iter().filter_map(layer_of).collect() };
        let bounds_wanted = names.is_empty() || names.iter().any(|name| name == "bounds");
        let mut out = Vec::new();
        if (ids.is_empty() || ids.iter().any(|id| id == IMAGE)) && (names.is_empty() || names.iter().any(|name| name == "channelContents")) {
            let mut values = serde_json::Map::new();
            let channels: Vec<Value> = st
                .doc
                .channels
                .iter()
                .map(|channel| {
                    let surface = &channel.surface;
                    let tiles: Vec<Value> =
                        surface.tiles().map(|(coord, tile)| json!([coord.tx, coord.ty, kition_kit::session::sha256_hex(tile.bytes())])).collect();
                    json!({ "key":channel.key, "name": channel.name, "format": surface.format(), "default": surface.default_bytes(), "tiles": tiles })
                })
                .collect();
            values.insert("channelContents".to_owned(), json!(channels));
            out.push(DerivedValues { id: json!(IMAGE), values });
        }
        // The image's stored lists as their items' identities and the members a call addressing
        // one assigns or reads (a pattern's pixels by digest).
        for name in ["noteContents", "compContents", "pathContents", "patternContents"] {
            if !((ids.is_empty() || ids.iter().any(|id| id == IMAGE)) && (names.is_empty() || names.iter().any(|wanted| wanted == name))) {
                continue;
            }
            let value = match name {
                "noteContents" => json!(
                    st.doc
                        .notes
                        .iter()
                        .map(|n| json!({ "key": n.key, "author": n.author, "color": n.color, "text": n.text, "position": n.position, "popup": n.popup, "modified": n.modified }))
                        .collect::<Vec<_>>()
                ),
                "compContents" => json!(st.doc.layer_comps.iter().map(|c| json!({ "id": c.id, "name": c.name, "comment": c.comment })).collect::<Vec<_>>()),
                "pathContents" => json!(st.doc.paths.iter().map(|p| json!({ "key": p.key, "name": p.name })).collect::<Vec<_>>()),
                _ => json!(
                    st.doc
                        .patterns
                        .iter()
                        .map(|p| {
                            let mut bytes = Vec::new();
                            for (coord, tile) in p.surface.tiles() {
                                bytes.extend_from_slice(&coord.tx.to_le_bytes());
                                bytes.extend_from_slice(&coord.ty.to_le_bytes());
                                bytes.extend_from_slice(tile.bytes());
                            }
                            json!({ "key": p.id, "name": p.name, "width": p.width, "height": p.height, "pixels": kition_kit::session::sha256_hex(&bytes) })
                        })
                        .collect::<Vec<_>>()
                ),
            };
            if let Some(image) = out.iter_mut().find(|object| object.id == IMAGE) {
                image.values.insert(name.to_owned(), value);
            } else {
                out.push(DerivedValues { id: json!(IMAGE), values: serde_json::Map::from_iter([(name.to_owned(), value)]) });
            }
        }
        if (ids.is_empty() || ids.iter().any(|id| id == IMAGE)) && (names.is_empty() || names.iter().any(|name| name == "linkGroups")) {
            let mut groups: Vec<(u64, String)> =
                st.doc.walk().iter().filter_map(|(_, _, layer)| layer.link_group.map(|group| (layer.id.0, group.to_string()))).collect();
            groups.sort_unstable();
            let mut values = serde_json::Map::new();
            values.insert("linkGroups".to_owned(), json!(groups));
            if let Some(image) = out.iter_mut().find(|object| object.id == IMAGE) {
                image.values.extend(values);
            } else {
                out.push(DerivedValues { id: json!(IMAGE), values });
            }
        }
        if (ids.is_empty() || ids.iter().any(|id| id == IMAGE)) && (names.is_empty() || names.iter().any(|name| name == "inheritedLocks")) {
            let mut locks: Vec<_> = st
                .doc
                .walk()
                .iter()
                .filter_map(|(path, _, layer)| {
                    let effective = st.doc.locks_at(path);
                    (effective != layer.locks).then_some((layer.id.0, effective))
                })
                .collect();
            locks.sort_unstable_by_key(|(id, _)| *id);
            let mut values = serde_json::Map::new();
            values.insert("inheritedLocks".to_owned(), json!(locks));
            if let Some(image) = out.iter_mut().find(|object| object.id == IMAGE) {
                image.values.extend(values);
            } else {
                out.push(DerivedValues { id: json!(IMAGE), values });
            }
        }
        if (ids.is_empty() || ids.iter().any(|id| id == IMAGE)) && (names.is_empty() || names.iter().any(|name| name == "canvasCoordinates")) {
            let value = crate::point::canvas_fact(self).unwrap_or(Value::Null);
            if let Some(image) = out.iter_mut().find(|object| object.id == IMAGE) {
                image.values.insert("canvasCoordinates".to_owned(), value);
            } else {
                out.push(DerivedValues { id: json!(IMAGE), values: serde_json::Map::from_iter([("canvasCoordinates".to_owned(), value)]) });
            }
        }
        for id in wanted {
            let Some(layer) = st.doc.layer(id) else { continue };
            let mut values = serde_json::Map::new();
            if names.is_empty() || names.iter().any(|name| name == "pickState") {
                values.insert("pickState".to_owned(), crate::point::fact(self, id).unwrap_or(Value::Null));
            }
            if names.is_empty() || names.iter().any(|name| name == "subtreeLabels") {
                let mut labels: Vec<_> = st
                    .doc
                    .walk()
                    .iter()
                    .filter(|(_, _, child)| child.id != id && contains(&st.doc, id, child.id))
                    .map(|(_, _, child)| (child.id.0, child.label))
                    .collect();
                labels.sort_unstable_by_key(|(id, _)| *id);
                values.insert("subtreeLabels".to_owned(), json!(labels));
            }
            if bounds_wanted {
                values.insert("bounds".to_owned(), bounds(layer));
            }
            out.push(DerivedValues { id: json!(id.0), values });
        }
        Ok(out)
    }
}

/// A layer's non-transparent bounds as a function of its own fields: `[x, y, width, height]`, or
/// `null` when it draws nothing, and for groups, adjustment and fill layers.
pub fn bounds(layer: &kition_photo_engine::doc::Layer) -> Value {
    if matches!(layer.content, LayerContent::Group(_) | LayerContent::Adjustment(_) | LayerContent::Fill(_)) {
        return Value::Null;
    }
    match layer.surface().map(|surface| surface.content_bounds()) {
        Some(rect) if !rect.is_empty() => json!(xywh(rect)),
        _ => Value::Null,
    }
}

/// Whether `[x, y, width, height]` holds the pixel `(px, py)`; `null` holds nothing.
fn holds(bounds: &Value, px: i32, py: i32) -> bool {
    let get = |i: usize| bounds.get(i).and_then(Value::as_i64);
    match (get(0), get(1), get(2), get(3)) {
        (Some(x), Some(y), Some(w), Some(h)) => {
            let (px, py) = (i64::from(px), i64::from(py));
            px >= x && py >= y && px < x.saturating_add(w) && py < y.saturating_add(h)
        }
        _ => false,
    }
}

/// Whether group `group` holds layer `id` at any depth.
fn contains(doc: &Document, group: LayerId, id: LayerId) -> bool {
    doc.layer(group).is_some_and(|layer| holds_layer(layer, id))
}

/// Whether `layer`, a group, holds layer `id` at any depth.
pub fn holds_layer(layer: &kition_photo_engine::doc::Layer, id: LayerId) -> bool {
    let LayerContent::Group(g) = &layer.content else { return false };
    let mut stack: Vec<&kition_photo_engine::doc::Layer> = g.children.iter().collect();
    while let Some(next) = stack.pop() {
        if next.id == id {
            return true;
        }
        if let LayerContent::Group(inner) = &next.content {
            stack.extend(inner.children.iter());
        }
    }
    false
}
