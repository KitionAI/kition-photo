//! Every command declares the editor state it reads (task card C-photo; `src/acts.rs`): each
//! command runs on one image in editor states that differ in one member, and what it did (its
//! answer or error, the image's content as inspect lists it without the editor state, every
//! layer's pixels and the composite) is compared:
//!
//! - the active layer: layers A and B selected, A active, against the same two with B active;
//! - the selected layers: A and B selected against A alone (A active in both);
//! - the pixel selection: a rectangle over part of A and B against another one;
//! - the colors: another foreground, then another background.
//!
//! A command whose outcome differs only with the active layer reads it (`ACTIVE`), with the
//! selected layers `SELECTED`, with the pixel selection `REGION`, with a color `COLORS`
//! (`src/acts_tables.rs`), read on the first pair of layers (pixel, type, shape, object layer)
//! where its outcome depends on the state. The test fails when a table and the outcomes disagree.
//! A command whose outcome depended on nothing the probe varied (it needs parameters the probe
//! does not give, something the image lacks, or ran as a no-op) is declared from its notation and
//! description: `ACTIVE` when it takes an optional `layer` or says it acts on the active layer.
//! `ACTS_PRINT=1` prints the tables as found.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use kition_kit::{CallContext, KitDocument};
use kition_photo_kit::acts::{ACTIVE, COLORS, REGION, SELECTED};
use kition_photo_kit::{EDITOR_STATE, PhotoDocument};
use serde_json::{Value, json};

/// Parameters for commands that do nothing observable with `{}`.
const SAMPLES: &[(&str, &str)] = &[
    ("cloneSource.select", r#"{"index": 0}"#),
    ("color.profileMismatch", r#"{"action": "discard"}"#),
    ("document.activate", r#"{"document": 0}"#),
    ("paint.setSymmetry", r#"{"mode": "vertical"}"#),
    ("paint.stroke", r#"{"points": [[2, 2, 1], [60, 40, 1]], "size": 8}"#),
    ("paint.line", r#"{"x0": 2, "y0": 2, "x1": 60, "y1": 40, "width": 4}"#),
    ("eraser.stroke", r#"{"points": [[2, 2, 1], [60, 40, 1]], "size": 8}"#),
    ("edit.stroke", r#"{"width": 2}"#),
    ("layer.setProps", r#"{"opacity": 0.5}"#),
    ("layer.rename", r#"{"name": "Renamed"}"#),
    ("edit.transform.scale", r#"{"scaleX": 0.5, "scaleY": 0.5}"#),
    ("edit.transform.rotate", r#"{"angle": 30}"#),
    ("layer.move", r#"{"dx": 3, "dy": 2}"#),
    ("filter.blur.gaussianBlur", r#"{"radius": 2}"#),
    ("image.adjustments.brightnessContrast", r#"{"brightness": 40, "contrast": 10}"#),
    ("image.adjustments.hueSaturation", r#"{"hue": 40}"#),
    ("image.adjustments.levels", r#"{"gamma": 1.6}"#),
    // A fixed time: by default it records now, which differs between two runs a second apart.
    ("image.analysis.recordMeasurements", r#"{"dateTime": "2026-11-12T00:00:00Z"}"#),
    ("image.adjustments.curves", r#"{"points": [[0, 0], [128, 180], [255, 255]]}"#),
    ("image.adjustments.exposure", r#"{"exposure": 1}"#),
    ("image.adjustments.vibrance", r#"{"vibrance": 60, "saturation": 30}"#),
    ("image.adjustments.colorBalance", r#"{"midtones": [40, 0, -40]}"#),
    ("image.adjustments.channelMixer", r#"{"red": [50, 50, 0, 0]}"#),
    ("image.adjustments.posterize", r#"{"levels": 3}"#),
    ("image.adjustments.selectiveColor", r#"{"colors": "reds", "cyan": 50, "method": "absolute"}"#),
    ("gradient.apply", r#"{"x0": 0, "y0": 0, "x1": 60, "y1": 40}"#),
    ("paint.bucket", r#"{"x": 40, "y": 20}"#),
    ("type.create", r#"{"text": "Hi", "x": 4, "y": 30, "size": 12}"#),
    ("shape.rectangle", r#"{"x": 4, "y": 4, "width": 20, "height": 10}"#),
];

/// Editor state the independent reading of every write command (`declarations_independent.rs`,
/// task card C8-photo-read) found the handler reads where this probe's fixture and parameters show
/// no difference: no matte to remove, no clipboard to paste, no video layer, a paint stroke the
/// probe gives no points, a fill or a canvas extension whose color is given, a crop or a distribution
/// with nothing to move. Each is in the table its reading names.
pub const READ_ACTIVE: &[&str] = &[
    "edit.smartScale",
    "edit.paste",
    "edit.pasteSpecial.pasteInPlace",
    "edit.pasteSpecial.pasteInto",
    "edit.pasteSpecial.pasteOutside",
    "edit.transform.removeWarpSplit",
    "edit.transform.warpGrid",
    "file.import.videoFramesToLayers",
    "file.placeEmbedded",
    "file.placeLinked",
    "gradient.fill.create",
    "gradient.presets.select",
    "image.adjustments.replaceColor",
    "image.analysis.placeScaleMarker",
    "image.trap",
    "layer.matting.colorDecontaminate",
    "layer.matting.defringe",
    "layer.matting.removeBlackMatte",
    "layer.matting.removeWhiteMatte",
    "layer.rasterize.video",
    "layer.stampDown",
    "layer.videoLayers.deleteFrame",
    "layer.videoLayers.duplicateFrame",
    "layer.videoLayers.insertBlankFrame",
    "layer.videoLayers.interpretFootage",
    "layer.videoLayers.newVideoLayerFromFile",
    "layer.videoLayers.rasterize",
    "layer.videoLayers.reloadFrame",
    "layer.videoLayers.replaceFootage",
    "layer.videoLayers.restoreAllFrames",
    "layer.videoLayers.restoreFrame",
    "layer.videoLayers.showAlteredVideo",
    "pattern.presets.select",
    "plugin.run",
    "shape.create",
    "shape.presets.place",
];
pub const READ_SELECTED: &[&str] = &[
    "layer.distribute.bottomEdges",
    "layer.distribute.horizontalCenters",
    "layer.distribute.horizontally",
    "layer.distribute.leftEdges",
    "layer.distribute.rightEdges",
    "layer.distribute.topEdges",
    "layer.distribute.verticalCenters",
    "layer.distribute.vertically",
    "layer.setLabelColor",
    "layer.showLayers",
    "layer.translate",
    "style.presets.apply",
    "type.characterStyle.new",
    "type.characterStyle.redefine",
    "type.paragraphStyle.new",
];
pub const READ_REGION: &[&str] = &[
    "channel.new",
    "channel.newSpot",
    "edit.pasteSpecial.pasteInto",
    "edit.pasteSpecial.pasteOutside",
    "edit.perspectiveWarp",
    "edit.pinWarp",
    "edit.transform",
    "edit.transform.again",
    "edit.transform.againCopy",
    "edit.transform.warp",
    "filter.lastFilter",
    "filter.liquify",
    "gradient.fill.create",
    "image.adjustments.colorLookup",
    "image.adjustments.matchColor",
    "image.adjustments.replaceColor",
    "image.analysis.straightenLayer",
    "layer.align.bottomEdges",
    "layer.align.horizontalCenters",
    "layer.align.leftEdges",
    "layer.align.rightEdges",
    "layer.align.topEdges",
    "layer.align.verticalCenters",
    "layer.newAdjustmentLayer.brightnessContrast",
    "layer.newAdjustmentLayer.channelMixer",
    "layer.newAdjustmentLayer.colorBalance",
    "layer.newAdjustmentLayer.colorLookup",
    "layer.newAdjustmentLayer.curves",
    "layer.newAdjustmentLayer.exposure",
    "layer.newAdjustmentLayer.hueSaturation",
    "layer.newAdjustmentLayer.levels",
    "layer.newAdjustmentLayer.selectiveColor",
    "layer.newAdjustmentLayer.vibrance",
    "layer.objectLayers.perspectiveWarp",
    "layer.objectLayers.pinWarp",
    "paint.backgroundEraser",
    "paint.blur",
    "paint.burn",
    "paint.cloneStamp",
    "paint.colorReplacement",
    "paint.smartMove",
    "paint.dodge",
    "paint.gradient",
    "paint.healingBrush",
    "paint.historyBrush",
    "paint.magicEraser",
    "paint.mixerBrush",
    "paint.patch",
    "paint.patternStamp",
    "paint.pencil",
    "paint.redEye",
    "paint.sharpen",
    "paint.smudge",
    "paint.sponge",
    "paint.spotHealing",
    "path.stroke",
    "plugin.run",
];
pub const READ_COLORS: &[&str] = &[
    "edit.clear",
    "edit.cut",
    "filter.filterGallery",
    "filter.lastFilter",
    "gradient.fill.create",
    "gradient.fill.set",
    "gradient.fill.stop",
    "gradient.presets.select",
    "gradient.presets.stop",
    "image.adjustments.replaceColor",
    "image.analysis.straightenLayer",
    "image.canvasSize",
    "image.crop",
    "image.rotation.arbitrary",
    "layer.combineShapes.excludeOverlappingShapes",
    "layer.combineShapes.intersectShapeAreas",
    "layer.combineShapes.subtractFrontShape",
    "layer.combineShapes.unite",
    "layer.new.layerViaCut",
    "paint.backgroundEraser",
    "paint.colorReplacement",
    "paint.gradient",
    "paint.magicEraser",
    "paint.mixerBrush",
    "paint.pencil",
    "paint.smudge",
    "path.fill",
    "path.stroke",
    "shape.create",
    "shape.edit",
    "shape.presets.place",
];

const FILE: &str = "probe.kphoto";

/// White 64 x 48 with pairs of layers (mid-tone blocks, which every adjustment changes) of each kind the commands act on: A (a red block) and B (a
/// blue block over part of it), a group G holding C (a green block), two type layers, two shape
/// layers, two object layers; and a saved selection. The pairs' ids, A and B first.
fn fixture() -> (Vec<u8>, Vec<[u64; 2]>) {
    let mut doc = PhotoDocument::create(FILE, 64, 48).unwrap();
    let id = |answer: Value| answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer id in {answer}"));
    let layer = |doc: &mut PhotoDocument, name: &str| id(doc.execute("layer.new.layer", json!({ "name": name })).unwrap());
    let block = |doc: &mut PhotoDocument, rect: [i32; 4], color: &str| {
        doc.execute("select.rect", json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] })).unwrap();
        doc.execute("edit.fill", json!({ "color": color })).unwrap();
    };
    let a = layer(&mut doc, "A");
    block(&mut doc, [4, 4, 30, 24], "#c05040");
    let b = layer(&mut doc, "B");
    block(&mut doc, [20, 12, 30, 24], "#4060a0");
    doc.execute("select.saveSelection", json!({ "name": "Saved" })).unwrap();
    doc.execute("select.deselect", json!({})).unwrap();
    let t1 = id(doc.execute("type.create", json!({ "text": "One", "x": 4, "y": 20, "size": 12, "color": "#000000" })).unwrap());
    let t2 = id(doc.execute("type.create", json!({ "text": "Two words", "x": 20, "y": 40, "size": 10, "color": "#ff0000" })).unwrap());
    let s1 = id(doc.execute("shape.create", json!({ "kind": "rect", "rect": [2, 30, 20, 12], "fill": "#00ff00" })).unwrap());
    let s2 = id(doc.execute("shape.create", json!({ "kind": "ellipse", "rect": [40, 20, 16, 16], "fill": "#ff00ff" })).unwrap());
    let o1 = layer(&mut doc, "O1");
    block(&mut doc, [10, 30, 12, 10], "#808000");
    doc.execute("layer.objectLayers.convertToObjectLayer", json!({ "layer": o1 })).unwrap();
    let o2 = layer(&mut doc, "O2");
    block(&mut doc, [44, 4, 12, 10], "#008080");
    doc.execute("layer.objectLayers.convertToObjectLayer", json!({ "layer": o2 })).unwrap();
    doc.execute("layer.new.group", json!({ "name": "G" })).unwrap();
    layer(&mut doc, "C");
    block(&mut doc, [40, 2, 20, 10], "#00ff00");
    doc.execute("select.deselect", json!({})).unwrap();
    let ids_by_name = |doc: &PhotoDocument, name: &str| -> u64 {
        fn find(layers: &Value, name: &str) -> Option<u64> {
            layers.as_array()?.iter().find_map(|layer| if layer["name"] == name { layer["id"].as_u64() } else { find(&layer["children"], name) })
        }
        find(&doc.inspect().unwrap()["layers"], name).unwrap_or_else(|| panic!("no layer {name}"))
    };
    // Converting gives the object layer an id of its own.
    let (o1, o2) = (ids_by_name(&doc, "O1"), ids_by_name(&doc, "O2"));
    (doc.save_sealed(Some(FILE)).unwrap(), vec![[a, b], [t1, t2], [s1, s2], [o1, o2]])
}

/// An editor state the probe runs a command in.
#[derive(Clone, Copy)]
struct State {
    on: [usize; 2],
    one: bool,
    region: [i32; 4],
    foreground: &'static str,
    background: &'static str,
}

const BASE: State = State { on: [0, 1], one: false, region: [2, 2, 30, 30], foreground: "#ff00ff", background: "#00ffff" };

fn variants() -> [(&'static str, State); 5] {
    [
        ("active", State { on: [1, 0], ..BASE }),
        ("selection", State { one: true, ..BASE }),
        ("region", State { region: [26, 8, 36, 36], ..BASE }),
        ("foreground", State { foreground: "#ffff00", ..BASE }),
        ("background", State { background: "#808080", ..BASE }),
    ]
}

/// What `command` with `params` did in `state`: its answer, the image's content, every layer's
/// pixels, the composite.
fn outcome(bytes: &[u8], ids: [u64; 2], state: State, command: &str, params: &Value) -> Value {
    let mut doc = PhotoDocument::open(FILE, bytes).unwrap();
    doc.execute("select.rect", json!({ "x": state.region[0], "y": state.region[1], "width": state.region[2], "height": state.region[3] })).unwrap();
    doc.execute("tools.setColors", json!({ "foreground": state.foreground, "background": state.background })).unwrap();
    let on: Vec<Value> = if state.one { vec![json!(ids[state.on[0]])] } else { state.on.iter().map(|i| json!(ids[*i])).collect() };
    doc.set_context(&CallContext { on, part: None, time: None }).unwrap();
    let answer = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| doc.execute(command, params.clone()))) {
        Ok(Ok(answer)) => json!({ "ok": answer }),
        Ok(Err(error)) => json!({ "error": error }),
        Err(_) => json!({ "panic": true }),
    };
    let Ok(inspected) = doc.inspect() else { return json!({ "answer": answer, "closed": true }) };
    let content = kition_kit::document_content(inspected.clone(), EDITOR_STATE);
    let mut layers = Vec::new();
    fn walk(layers: &Value, out: &mut Vec<u64>) {
        for layer in layers.as_array().into_iter().flatten() {
            out.extend(layer["id"].as_u64());
            walk(&layer["children"], out);
        }
    }
    walk(&inspected["layers"], &mut layers);
    let pixels: Vec<Value> =
        layers.iter().map(|id| doc.layer_thumbnail(u32::try_from(*id).unwrap_or(0), 64).map(|bytes| json!(digest(&bytes))).unwrap_or(Value::Null)).collect();
    let (w, h) = (doc.width().max(1), doc.height().max(1));
    let composite = doc.render(0.0, 0.0, 1.0, w.min(256), h.min(256)).map(|bytes| digest(&bytes)).unwrap_or_default();
    json!({ "answer": answer, "content": content, "pixels": pixels, "composite": composite })
}

fn digest(bytes: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Commands the probe leaves alone: they reach files or the machine, or exist to fail.
fn skipped(id: &str) -> bool {
    id.starts_with("file.") || id.starts_with("plugins.") || id.starts_with("prefs.") || id.starts_with("presets.import") || id == "test.panic"
}

#[derive(Default)]
struct Found {
    active: BTreeSet<String>,
    selection: BTreeSet<String>,
    region: BTreeSet<String>,
    colors: BTreeSet<String>,
    ran: BTreeSet<String>,
    unsteady: BTreeSet<String>,
    by_notation: BTreeSet<String>,
    observed: BTreeSet<String>,
}

fn probe() -> Found {
    let (bytes, pairs) = fixture();
    let doc = PhotoDocument::open(FILE, &bytes).unwrap();
    let mut found = Found::default();
    for command in doc.commands() {
        let id = command.id.as_str();
        if skipped(id) {
            continue;
        }
        let params: Value = SAMPLES.iter().find(|(sample, _)| *sample == id).map_or_else(|| json!({}), |(_, params)| serde_json::from_str(params).unwrap());
        // The first pair of layers on which the command's outcome depends on the state (a command
        // that runs as a no-op on one kind of layer, a type option on pixel layers, goes on to the
        // next pair).
        let mut differs = Vec::new();
        let mut ran = false;
        let mut unsteady = false;
        for pair in &pairs {
            let base = outcome(&bytes, *pair, BASE, id, &params);
            if outcome(&bytes, *pair, BASE, id, &params) != base {
                unsteady = true;
                break;
            }
            ran |= base["answer"].get("ok").is_some();
            for (what, state) in variants() {
                let other = outcome(&bytes, *pair, state, id, &params);
                ran |= other["answer"].get("ok").is_some();
                if other != base {
                    differs.push(what);
                }
            }
            if !differs.is_empty() {
                break;
            }
        }
        if unsteady {
            found.unsteady.insert(id.to_owned());
            continue;
        }
        if ran {
            found.ran.insert(id.to_owned());
        }
        if !differs.is_empty() {
            found.observed.insert(id.to_owned());
        }
        let has = |what: &str| differs.contains(&what);
        if has("selection") {
            found.selection.insert(id.to_owned());
        }
        if has("active") && !has("selection") {
            found.active.insert(id.to_owned());
        }
        if has("region") {
            found.region.insert(id.to_owned());
        }
        if has("foreground") || has("background") {
            found.colors.insert(id.to_owned());
        }
    }
    // A command whose outcome the probe saw depend on nothing (it did not run, or ran as a no-op
    // on every pair of layers): it reads the active layer when its notation takes an optional
    // `layer` or its description says it acts on the active one.
    for command in doc.commands() {
        if skipped(&command.id) || found.observed.contains(&command.id) || found.unsteady.contains(&command.id) {
            continue;
        }
        let notation = command.params.clone().unwrap_or_default();
        let text = command.description.to_lowercase();
        // These implementations address a clone slot or the document, not an active layer.
        let document_or_slot = matches!(
            command.id.as_str(),
            "cloneSource.select"
                | "color.profileMismatch"
                | "document.activate"
                | "image.measure"
                | "paint.setSymmetry"
                | "paint.symmetryDisable"
                | "layer.selectTop"
                | "layer.selectBottom"
        );
        if !document_or_slot
            && (notation.contains(r#""layer":id?"#) || text.contains("active layer") || text.contains("else the active") || text.contains("the active one"))
        {
            found.active.insert(command.id.clone());
            found.by_notation.insert(command.id.clone());
        }
    }
    // filters::run_filter and lens_cmds::run_filter explicitly read the active layer and
    // doc.selection. A no-op or an elapsed-time answer cannot establish those reads by probing.
    for (set, read) in
        [(&mut found.active, READ_ACTIVE), (&mut found.selection, READ_SELECTED), (&mut found.region, READ_REGION), (&mut found.colors, READ_COLORS)]
    {
        set.extend(read.iter().map(|id| (*id).to_owned()));
    }
    for command in doc.commands() {
        if kition_photo_engine::filters::params_for(&command.id, &json!({})).is_some()
            || matches!(command.id.as_str(), "filter.lensCorrection" | "filter.adaptiveWideAngle" | "filter.rawDevelop")
        {
            found.active.insert(command.id.clone());
            found.region.insert(command.id);
        }
    }
    found
}

fn print(name: &str, doc: &str, set: &BTreeSet<String>) {
    println!("/// {doc}\npub const {name}: &[&str] = &[");
    for id in set {
        println!("    \"{id}\",");
    }
    println!("];");
}

#[test]
fn every_command_declares_the_editor_state_its_outcome_depends_on() {
    let found = probe();
    if std::env::var("ACTS_PRINT").is_ok() {
        print("ACTIVE", "Commands that act on the active layer when given no `layer`.", &found.active);
        print("SELECTED", "Commands that act on the selected layers.", &found.selection);
        print("REGION", "Commands that act inside the pixel selection.", &found.region);
        print("COLORS", "Commands that use the foreground or background color.", &found.colors);
        println!("// ran: {}, unsteady: {:?}, active by notation: {}", found.ran.len(), found.unsteady, found.by_notation.len());
        let all: Vec<String> = PhotoDocument::open(FILE, &fixture().0)
            .unwrap()
            .commands()
            .into_iter()
            .map(|c| c.id)
            .filter(|id| !skipped(id) && !found.ran.contains(id))
            .collect();
        println!("// did not run: {}", all.join(" "));
    }
    let mut wrong = Vec::new();
    for (name, table, set) in
        [("ACTIVE", ACTIVE, &found.active), ("SELECTED", SELECTED, &found.selection), ("REGION", REGION, &found.region), ("COLORS", COLORS, &found.colors)]
    {
        let declared: BTreeSet<String> = table.iter().map(|id| (*id).to_owned()).collect();
        for id in set.difference(&declared) {
            wrong.push(format!("{id}: its outcome depends on what {name} lists, and {name} does not list it"));
        }
        for id in declared.difference(set) {
            wrong.push(format!("{id}: {name} lists it, and its outcome did not depend on it where it ran"));
        }
        assert!(table.windows(2).all(|pair| pair[0] < pair[1]), "{name} is sorted, each once");
    }
    assert!(wrong.is_empty(), "{} declarations disagree with what the commands do:\n{}", wrong.len(), wrong.join("\n"));
    assert!(found.unsteady.len() <= 8, "commands whose outcome differs between two runs in one state: {:?}", found.unsteady);
}
