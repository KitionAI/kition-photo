//! The independent reading of every write command (task card C8-photo-read): for each of the 672
//! commands the kit offers that change the document, its handler and the engine operation behind
//! it were read — before the declarations (`src/acts_tables.rs`, `src/place.rs`, `src/rests.rs`)
//! were looked at — for which parameters it resolves and how, what it reads of the document and of
//! the editor, and what it writes. Each command's reading is the comment before its rows, in the
//! words of the reading (`layer`: how the engine resolves the parameter, `id-else-active` an id
//! and otherwise the active layer, `ignored` always the active layer, `id-else-selected` an id
//! and otherwise the selected layers; `T` the layer it acts on). Each row is one code path of
//! the command, on [`fixture`], with what a write with `basis` holds: the editor state the command
//! declares (`active`, `selection`, `area`, `colors`) and, for each object, the fields held
//! (`*`: every field, `-`: none, `tiles@`: the 256-pixel tiles of the area) and whether where it is
//! matters. What is held is computed as the guard does: the command's `layer` target and the
//! objects its declared parameters name by id (every field), replaced by `under`'s premises.
//!
//! Where the declarations before this card differed, the row says so (`was`). Of the 672
//! commands, 369 differed (55%): 274 held less than the handler reads (a write that can land on
//! what the agent did not observe), 128 held more (a refusal no change needs), 33 both. By class:
//! - a read that is not held, of the image (its canvas, mode, stored notes, counts, slices,
//!   timeline, variables, guides, channels, metadata, or the composite): 148 commands;
//! - a read that is not held, of the layer it acts on (the active layer, unnamed, held nothing
//!   outside the pixel families; the selected layers; the layer below a merge): 115;
//! - editor state read but not declared (the active layer 36, the selection area 57, the colors
//!   31, the selected layers 15): 119;
//! - a parameter resolved differently from its name (`layer` given as a name or `"#n"` to a
//!   command that does not read the active layer: the engine read it as no layer and wrote the
//!   active one): 12 (the host refuses the string by the schema; the in-page `execute` did not);
//! - a default taken from elsewhere (Flame's path: the work path, else the first saved path): 1;
//! - held more than the handler reads (every field of a layer whose effects, mask, vector mask,
//!   content, name or visibility alone it reads or replaces): 123;
//! - a value naming another object (a saved path's command held the active layer): 5.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};

use kition_kit::{KitCommand, KitDocument};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

#[path = "declarations_independent/facts.rs"]
mod facts;

fn id(answer: &Value) -> u64 {
    answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer id in {answer}"))
}

/// An 8-bit RGB 400 x 300 image, top first: `$T` type "Title", `$ADJ` Curves, `$SH` shape, `$G`
/// group holding `$C` (with a mask) and `$C2`, `$SO` object layer, `$P2` clipped to `$P`, `$P`
/// "Paint" (active, and the only selected layer), `$FILL` a solid color fill, `$BG` Background;
/// a saved channel "Alpha 1", a saved path "Path 1", a layer comp, a guide, a note and a slice.
pub fn fixture() -> (PhotoDocument, Vec<(&'static str, u64)>) {
    let mut d = PhotoDocument::create("fx.psd", 400, 300).unwrap();
    let bg = d.inspect().unwrap()["layers"][0]["id"].as_u64().unwrap();
    let mut ex = |c: &str, p: Value| d.execute(c, p.clone()).unwrap_or_else(|e| panic!("{c} {p}: {e}"));
    let fill = id(&ex("layer.newFillLayer.solidColor", json!({ "color": "#336699" })));
    let block = |ex: &mut dyn FnMut(&str, Value) -> Value, rect: [i32; 4], color: &str| {
        ex("select.rect", json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] }));
        ex("edit.fill", json!({ "color": color }));
        ex("select.deselect", json!({}));
    };
    let p = id(&ex("layer.new.layer", json!({ "name": "Paint" })));
    block(&mut ex, [50, 50, 200, 150], "#c05040");
    let p2 = id(&ex("layer.new.layer", json!({ "name": "Second" })));
    block(&mut ex, [100, 80, 100, 100], "#4060a0");
    ex("layer.createClippingMask", json!({ "layer": p2 }));
    let so = id(&ex("layer.new.layer", json!({ "name": "Smart" })));
    block(&mut ex, [300, 20, 60, 60], "#40a060");
    let so = ex("layer.objectLayers.convertToObjectLayer", json!({ "layer": so }))["layer"].as_u64().unwrap_or(so);
    let g = id(&ex("layer.new.group", json!({ "name": "Group 1" })));
    let c2 = id(&ex("layer.new.layer", json!({ "name": "Child 2" })));
    block(&mut ex, [20, 200, 50, 50], "#808020");
    let c = id(&ex("layer.new.layer", json!({ "name": "Child" })));
    block(&mut ex, [30, 210, 50, 50], "#208080");
    ex("layer.layerMask.revealAll", json!({ "layer": c }));
    ex("layer.select", json!({ "layer": g }));
    let sh = id(&ex("shape.create", json!({ "kind": "rect", "rect": [260, 150, 80, 60], "fill": "#aa3388", "name": "Shape 1" })));
    let adj = id(&ex("layer.newAdjustmentLayer.curves", json!({ "points": [[0, 0], [128, 160], [255, 255]] })));
    let t = id(&ex("type.create", json!({ "x": 20, "y": 40, "text": "Title", "size": 24 })));
    for (layer, under) in [(sh, g), (adj, sh), (t, adj)] {
        ex("layer.moveTo", json!({ "layer": layer, "target": under, "position": "above" }));
    }
    ex("select.rect", json!({ "x": 60, "y": 60, "width": 40, "height": 40 }));
    ex("select.saveSelection", json!({ "name": "Alpha 1" }));
    ex("select.deselect", json!({}));
    ex("path.set", json!({ "name": "Path 1", "path": { "subpaths": [{ "closed": true, "knots": [[10, 10], [90, 10], [90, 90], [10, 90]] }] } }));
    ex("layerComp.new", json!({ "name": "Comp 1" }));
    ex("view.newGuide", json!({ "orientation": "vertical", "position": 100 }));
    ex("notes.add", json!({ "x": 5, "y": 5, "text": "n" }));
    ex("slice.new", json!({ "rect": [0, 0, 50, 50] }));
    let bytes = d.save(None).unwrap();
    let mut d = PhotoDocument::open("fx.psd", &bytes).unwrap();
    d.execute("layer.select", json!({ "layer": p })).unwrap();
    let roles =
        vec![("$T", t), ("$ADJ", adj), ("$SH", sh), ("$G", g), ("$C", c), ("$C2", c2), ("$SO", so), ("$P2", p2), ("$P", p), ("$FILL", fill), ("$BG", bg)];
    (d, roles)
}

/// One code path of a command: its parameters (layers by role), the editor state it declares,
/// and what a write holds of each object: (object, fields, whether where it is matters).
struct Row {
    command: &'static str,
    params: &'static str,
    editor: &'static str,
    holds: &'static [(&'static str, &'static str, bool)],
}

const fn r(command: &'static str, params: &'static str, editor: &'static str, holds: &'static [(&'static str, &'static str, bool)]) -> Row {
    Row { command, params, editor, holds }
}

fn with_roles(value: &Value, roles: &[(&str, u64)]) -> Value {
    match value {
        Value::String(text) => roles.iter().find(|(role, _)| role == text).map_or_else(|| value.clone(), |(_, id)| json!(id)),
        Value::Array(items) => Value::Array(items.iter().map(|item| with_roles(item, roles)).collect()),
        Value::Object(fields) => Value::Object(fields.iter().map(|(key, item)| (key.clone(), with_roles(item, roles))).collect()),
        _ => value.clone(),
    }
}

/// The ids a declared object parameter (`a.b`, `items[].c`) names in `params`.
fn ids_at(params: &Value, path: &str) -> Vec<u64> {
    let mut current = vec![params.clone()];
    for part in path.split('.') {
        let key = part.trim_end_matches("[]");
        let mut next = Vec::new();
        for value in &current {
            match value.get(key) {
                Some(Value::Array(items)) if part.ends_with("[]") => next.extend(items.iter().cloned()),
                Some(item) => next.push(item.clone()),
                None => {}
            }
        }
        current = next;
    }
    current
        .iter()
        .flat_map(|value| if let Value::Array(items) = value { items.clone() } else { vec![value.clone()] })
        .filter_map(|value| value.as_u64())
        .collect()
}

/// The fields as a row writes them: sorted, `*` for every field, `-` for none, tile keys as `tiles@`.
fn spelled(fields: Option<&Vec<String>>) -> String {
    let Some(fields) = fields else { return "*".to_owned() };
    let set: BTreeSet<String> = fields.iter().map(|field| if field.starts_with("tiles.") { "tiles@".to_owned() } else { field.clone() }).collect();
    if set.is_empty() { "-".to_owned() } else { set.into_iter().collect::<Vec<_>>().join(",") }
}

/// What a write with `basis` of `command` with `params` holds, as the guard computes it.
fn held(doc: &PhotoDocument, command: &KitCommand, params: &Value, names: &BTreeMap<u64, &str>) -> (String, BTreeMap<String, (String, bool)>) {
    let mut editor: Vec<&str> = command.reads.iter().map(|state| state.as_str()).collect();
    if kition_photo_kit::acts::reads_of(&command.id).colors {
        editor.push("colors");
    }
    editor.sort_unstable();
    let name = |id: u64| names.get(&id).map_or_else(|| id.to_string(), |name| (*name).to_owned());
    let mut holds = BTreeMap::new();
    if command.targets == Some("layer")
        && let Some(id) = params.get("layer").and_then(Value::as_u64)
    {
        holds.insert(name(id), ("*".to_owned(), !command.position_independent));
    }
    for path in command.objects {
        for id in ids_at(params, path) {
            holds.insert(name(id), ("*".to_owned(), true));
        }
    }
    if let Some(under) = doc.under(&command.id, params).unwrap_or_else(|e| panic!("{}: {e}", command.id)) {
        let mut explicit = BTreeMap::<String, (BTreeSet<String>, bool)>::new();
        for premise in &under.premises {
            let object = premise.id.as_u64().map_or_else(|| premise.id.as_str().unwrap_or_default().to_owned(), name);
            let entry = explicit.entry(object).or_default();
            entry.0.extend(facts::split_fields(&spelled(premise.fields.as_ref())));
            entry.1 |= premise.positions.as_ref().is_none_or(|positions| !positions.is_empty());
        }
        for (object, (fields, at)) in explicit {
            let fields = if fields.contains("*") {
                "*".to_owned()
            } else if fields.is_empty() {
                "-".to_owned()
            } else {
                fields.into_iter().collect::<Vec<_>>().join(",")
            };
            holds.insert(object, (fields, at));
        }
    }
    (editor.join(","), holds)
}

/// Every write command's rows hold what its reading says, and every write command has a row.
#[test]
fn every_write_rests_on_what_its_handler_reads() {
    let (mut doc, roles) = fixture();
    facts::compare(&mut doc, &roles, ROWS);
    let commands = doc.commands();
    let mut wrong = Vec::new();
    let rows: BTreeSet<&str> = ROWS.iter().map(|row| row.command).collect();
    let writes: BTreeSet<&str> = commands.iter().filter(|c| c.mutates).map(|c| c.id.as_str()).collect();
    for missing in writes.difference(&rows) {
        wrong.push(format!("{missing}: a write command with no reading"));
    }
    for gone in rows.difference(&writes) {
        wrong.push(format!("{gone}: a reading of no write command"));
    }
    assert_eq!(writes.len(), 672, "the write commands read");
    // The path comparison above retains the frozen rows and checks the source-backed
    // refinements at field leaves, including content and historical coverage callbacks.
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Class "parameter resolved differently from its name": the engine reads `layer` as an id only
/// and takes any other value for the active layer, so a command that does not read the active
/// layer refuses a name or `"#n"` there instead of acting on the active one (the host refuses it
/// by the schema too; the in-page `execute` did not).
#[test]
fn a_layer_by_name_where_the_command_takes_an_id_is_refused() {
    for command in ["layer.delete", "layer.duplicate", "layer.hideLayers", "layer.createClippingMask"] {
        for value in [json!("Title"), json!("#1"), json!(3.0)] {
            let (mut doc, _) = fixture();
            let before = doc.inspect().unwrap()["layers"].clone();
            let error = doc.execute(command, json!({ "layer": value })).expect_err(&format!("{command} with layer {value}"));
            assert!(error.contains("takes a layer by its id"), "{command}: {error}");
            assert_eq!(doc.inspect().unwrap()["layers"], before, "{command}: nothing changed");
        }
    }
}

const ROWS: &[Row] = &[
    // filter.blur.gaussianBlur: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.gaussianBlur",
        r##"{"radius": 2, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.gaussianBlur",
        r##"{"radius": 2, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.gaussianBlur",
        r##"{"radius": 2, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.blur.gaussianBlur",
        r##"{"radius": 2, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.blur.blur: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.blur",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.blur",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r("filter.blur.blur", r##"{"layer": "$C", "target": "mask"}"##, "active,area", &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)]),
    r(
        "filter.blur.blur",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.blur.blurMore: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.blurMore",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.blurMore",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.blurMore",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.blur.blurMore",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.sharpen.sharpen: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.sharpen.sharpen",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.sharpen",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.sharpen",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.sharpen.sharpen",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.sharpen.sharpenMore: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.sharpen.sharpenMore",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.sharpenMore",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.sharpenMore",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.sharpen.sharpenMore",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.sharpen.sharpenEdges: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.sharpen.sharpenEdges",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.sharpenEdges",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.sharpenEdges",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.sharpen.sharpenEdges",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.noise.despeckle: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.noise.despeckle",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.despeckle",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.despeckle",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.noise.despeckle",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.blur.boxBlur: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.boxBlur",
        r##"{"radius": 3, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.boxBlur",
        r##"{"radius": 3, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.boxBlur",
        r##"{"radius": 3, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.blur.boxBlur",
        r##"{"radius": 3, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.blur.motionBlur: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.motionBlur",
        r##"{"angle": 30, "distance": 10, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.motionBlur",
        r##"{"angle": 30, "distance": 10, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.motionBlur",
        r##"{"angle": 30, "distance": 10, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.blur.motionBlur",
        r##"{"angle": 30, "distance": 10, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.blur.radialBlur: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.radialBlur",
        r##"{"amount": 10, "method": "zoom", "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.radialBlur",
        r##"{"amount": 10, "method": "zoom", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.radialBlur",
        r##"{"amount": 10, "method": "zoom", "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.blur.radialBlur",
        r##"{"amount": 10, "method": "zoom", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.blur.surfaceBlur: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.blur.surfaceBlur",
        r##"{"radius": 5, "threshold": 15, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.surfaceBlur",
        r##"{"radius": 5, "threshold": 15, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.blur.surfaceBlur",
        r##"{"radius": 5, "threshold": 15, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.blur.surfaceBlur",
        r##"{"radius": 5, "threshold": 15, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.sharpen.unsharpMask: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.sharpen.unsharpMask",
        r##"{"amount": 50, "radius": 1, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.unsharpMask",
        r##"{"amount": 50, "radius": 1, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.unsharpMask",
        r##"{"amount": 50, "radius": 1, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.sharpen.unsharpMask",
        r##"{"amount": 50, "radius": 1, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.sharpen.smartSharpen: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.sharpen.smartSharpen",
        r##"{"amount": 100, "radius": 1, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.smartSharpen",
        r##"{"amount": 100, "radius": 1, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.sharpen.smartSharpen",
        r##"{"amount": 100, "radius": 1, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.sharpen.smartSharpen",
        r##"{"amount": 100, "radius": 1, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.noise.addNoise: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.noise.addNoise",
        r##"{"amount": 12.5, "seed": 3, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.addNoise",
        r##"{"amount": 12.5, "seed": 3, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.addNoise",
        r##"{"amount": 12.5, "seed": 3, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.noise.addNoise",
        r##"{"amount": 12.5, "seed": 3, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.noise.median: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.noise.median",
        r##"{"radius": 2, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.median",
        r##"{"radius": 2, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.median",
        r##"{"radius": 2, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.noise.median",
        r##"{"radius": 2, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.noise.dustAndScratches: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.noise.dustAndScratches",
        r##"{"radius": 2, "threshold": 0, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.dustAndScratches",
        r##"{"radius": 2, "threshold": 0, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.noise.dustAndScratches",
        r##"{"radius": 2, "threshold": 0, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.noise.dustAndScratches",
        r##"{"radius": 2, "threshold": 0, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.pixelate.mosaic: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.pixelate.mosaic",
        r##"{"cellSize": 10, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.pixelate.mosaic",
        r##"{"cellSize": 10, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.pixelate.mosaic",
        r##"{"cellSize": 10, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.pixelate.mosaic",
        r##"{"cellSize": 10, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.stylize.emboss: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.stylize.emboss",
        r##"{"angle": 135, "height": 3, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.stylize.emboss",
        r##"{"angle": 135, "height": 3, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.stylize.emboss",
        r##"{"angle": 135, "height": 3, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.stylize.emboss",
        r##"{"angle": 135, "height": 3, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.stylize.findEdges: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.stylize.findEdges",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.stylize.findEdges",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.stylize.findEdges",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.stylize.findEdges",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.stylize.solarize: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.stylize.solarize",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.stylize.solarize",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.stylize.solarize",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.stylize.solarize",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.distort.twirl: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.distort.twirl",
        r##"{"angle": 50, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.twirl",
        r##"{"angle": 50, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.twirl",
        r##"{"angle": 50, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.distort.twirl",
        r##"{"angle": 50, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.distort.pinch: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.distort.pinch",
        r##"{"amount": 50, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.pinch",
        r##"{"amount": 50, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.pinch",
        r##"{"amount": 50, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.distort.pinch",
        r##"{"amount": 50, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.distort.spherize: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.distort.spherize",
        r##"{"amount": 100, "mode": "normal", "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.spherize",
        r##"{"amount": 100, "mode": "normal", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.spherize",
        r##"{"amount": 100, "mode": "normal", "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.distort.spherize",
        r##"{"amount": 100, "mode": "normal", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.distort.wave: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.distort.wave",
        r##"{"generators": 5, "seed": 1, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.wave",
        r##"{"generators": 5, "seed": 1, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.wave",
        r##"{"generators": 5, "seed": 1, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.distort.wave",
        r##"{"generators": 5, "seed": 1, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.distort.ripple: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.distort.ripple",
        r##"{"amount": 100, "size": "medium", "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.ripple",
        r##"{"amount": 100, "size": "medium", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.ripple",
        r##"{"amount": 100, "size": "medium", "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.distort.ripple",
        r##"{"amount": 100, "size": "medium", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.distort.polarCoordinates: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.distort.polarCoordinates",
        r##"{"mode": "rectangularToPolar", "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.polarCoordinates",
        r##"{"mode": "rectangularToPolar", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.distort.polarCoordinates",
        r##"{"mode": "rectangularToPolar", "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.distort.polarCoordinates",
        r##"{"mode": "rectangularToPolar", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.other.highPass: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.other.highPass",
        r##"{"radius": 10, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.highPass",
        r##"{"radius": 10, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.highPass",
        r##"{"radius": 10, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.other.highPass",
        r##"{"radius": 10, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.other.minimum: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.other.minimum",
        r##"{"radius": 1, "preserve": "roundness", "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.minimum",
        r##"{"radius": 1, "preserve": "roundness", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.minimum",
        r##"{"radius": 1, "preserve": "roundness", "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.other.minimum",
        r##"{"radius": 1, "preserve": "roundness", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.other.maximum: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.other.maximum",
        r##"{"radius": 1, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.maximum",
        r##"{"radius": 1, "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.maximum",
        r##"{"radius": 1, "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.other.maximum",
        r##"{"radius": 1, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.other.offset: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.other.offset",
        r##"{"horizontal": 5, "vertical": 2, "undefinedAreas": "wrap", "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.offset",
        r##"{"horizontal": 5, "vertical": 2, "undefinedAreas": "wrap", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.other.offset",
        r##"{"horizontal": 5, "vertical": 2, "undefinedAreas": "wrap", "layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.other.offset",
        r##"{"horizontal": 5, "vertical": 2, "undefinedAreas": "wrap", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.lastFilter: layer active-only; editor active,area,channeltarget,colors; reads -
    // was held less: editor:colors; editor:area
    r(
        "filter.lastFilter",
        r##"{}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.gallery.coloredPencil: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.coloredPencil",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.coloredPencil",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.coloredPencil",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.coloredPencil",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.cutout: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.cutout",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.cutout",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.cutout",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.cutout",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.dryBrush: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.dryBrush",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.dryBrush",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.dryBrush",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.dryBrush",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.filmGrain: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.filmGrain",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.filmGrain",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.filmGrain",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.filmGrain",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.fresco: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.fresco",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.fresco",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.fresco",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.fresco",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.neonGlow: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.neonGlow",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.neonGlow",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.neonGlow",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.neonGlow",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.paintDaubs: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.paintDaubs",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.paintDaubs",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.paintDaubs",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.paintDaubs",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.paletteKnife: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.paletteKnife",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.paletteKnife",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.paletteKnife",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.paletteKnife",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.plasticWrap: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.plasticWrap",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.plasticWrap",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.plasticWrap",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.plasticWrap",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.posterEdges: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.posterEdges",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.posterEdges",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.posterEdges",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.posterEdges",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.roughPastels: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.roughPastels",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.roughPastels",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.roughPastels",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.roughPastels",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.smudgeStick: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.smudgeStick",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.smudgeStick",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.smudgeStick",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.smudgeStick",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.sponge: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.sponge",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.sponge",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.sponge",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.sponge",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.underpainting: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.underpainting",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.underpainting",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.underpainting",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.underpainting",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.watercolor: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.watercolor",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.watercolor",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.watercolor",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.watercolor",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.accentedEdges: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.accentedEdges",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.accentedEdges",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.accentedEdges",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.accentedEdges",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.angledStrokes: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.angledStrokes",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.angledStrokes",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.angledStrokes",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.angledStrokes",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.crosshatch: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.crosshatch",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.crosshatch",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.crosshatch",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.crosshatch",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.darkStrokes: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.darkStrokes",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.darkStrokes",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.darkStrokes",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.darkStrokes",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.inkOutlines: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.inkOutlines",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.inkOutlines",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.inkOutlines",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.inkOutlines",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.spatter: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.spatter",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.spatter",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.spatter",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.spatter",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.sprayedStrokes: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.sprayedStrokes",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.sprayedStrokes",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.sprayedStrokes",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.sprayedStrokes",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.sumiE: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.sumiE",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.sumiE",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.sumiE",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.sumiE",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.diffuseGlow: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.diffuseGlow",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.diffuseGlow",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.diffuseGlow",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.diffuseGlow",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.glass: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.glass",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.glass",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.glass",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.glass",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.oceanRipple: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.oceanRipple",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.oceanRipple",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.oceanRipple",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.oceanRipple",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.basRelief: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.basRelief",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.basRelief",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.basRelief",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.basRelief",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.chalkCharcoal: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.chalkCharcoal",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.chalkCharcoal",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.chalkCharcoal",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.chalkCharcoal",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.charcoal: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.charcoal",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.charcoal",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.charcoal",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.charcoal",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.chrome: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.chrome",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.chrome",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.chrome",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.chrome",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.conteCrayon: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.conteCrayon",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.conteCrayon",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.conteCrayon",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.conteCrayon",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.graphicPen: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.graphicPen",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.graphicPen",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.graphicPen",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.graphicPen",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.halftonePattern: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.halftonePattern",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.halftonePattern",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.halftonePattern",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.halftonePattern",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.notePaper: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.notePaper",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.notePaper",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.notePaper",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.notePaper",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.photocopy: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.photocopy",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.photocopy",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.photocopy",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.photocopy",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.plaster: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.plaster",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.plaster",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.plaster",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.plaster",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.reticulation: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.reticulation",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.reticulation",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.reticulation",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.reticulation",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.stamp: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.stamp",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.stamp",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.stamp",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.stamp",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.tornEdges: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.tornEdges",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.tornEdges",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.tornEdges",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.tornEdges",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.waterPaper: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.waterPaper",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.waterPaper",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.waterPaper",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.waterPaper",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.glowingEdges: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.glowingEdges",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.glowingEdges",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.glowingEdges",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.glowingEdges",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.craquelure: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.craquelure",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.craquelure",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.craquelure",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.craquelure",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.grain: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.grain",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.grain",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.grain",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.grain",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.mosaicTiles: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.mosaicTiles",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.mosaicTiles",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.mosaicTiles",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.mosaicTiles",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.patchwork: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.patchwork",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.patchwork",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.patchwork",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.patchwork",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.stainedGlass: layer id-else-active,ignored; editor active,area,channeltarget,colors,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.stainedGlass",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.stainedGlass",
        r##"{"layer": "$SO"}"##,
        "active,area,colors",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.stainedGlass",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.stainedGlass",
        r##"{"target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.gallery.texturizer: layer id-else-active,ignored; editor active,area,channeltarget,quickmask; reads T.content,T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "filter.gallery.texturizer",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.texturizer",
        r##"{"layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.gallery.texturizer",
        r##"{"layer": "$C", "target": "mask"}"##,
        "active,area",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "filter.gallery.texturizer",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // filter.pixelate.colorHalftone: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.pixelate.colorHalftone",
        r##"{"layer": "$P", "maxRadius": 8}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.pixelate.crystallize: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.pixelate.crystallize",
        r##"{"layer": "$P", "cellSize": 10}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.pixelate.facet: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.pixelate.facet",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.pixelate.fragment: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.pixelate.fragment",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.pixelate.mezzotint: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.pixelate.mezzotint",
        r##"{"layer": "$P", "type": "fineDots"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.pixelate.pointillize: layer ignored; editor area,channeltarget,colors,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.pixelate.pointillize",
        r##"{"layer": "$P", "cellSize": 5}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.stylize.diffuse: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.stylize.diffuse",
        r##"{"layer": "$P", "mode": "normal"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.stylize.extrude: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.stylize.extrude",
        r##"{"layer": "$P", "size": 30, "depth": 30}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.stylize.oilPaint: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.stylize.oilPaint",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.stylize.tiles: layer ignored; editor area,channeltarget,colors,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.stylize.tiles",
        r##"{"layer": "$P", "count": 10}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.stylize.traceContour: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.stylize.traceContour",
        r##"{"layer": "$P", "level": 128}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.stylize.wind: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.stylize.wind",
        r##"{"layer": "$P", "method": "wind"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.distort.displace: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.distort.displace",
        r##"{"layer": "$P", "mapDocument": 0, "mapLayer": "$C"}"##,
        "active,area",
        &[("$C", "*", true), ("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.distort.shear: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.distort.shear",
        r##"{"layer": "$P", "amount": 20}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.distort.zigZag: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.distort.zigZag",
        r##"{"layer": "$P", "amount": 10}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.fibers: layer ignored; editor area,channeltarget,colors,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.render.fibers",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.lensFlare: layer ignored; editor area,channeltarget,quickmask; reads T.content,T.locks,T.pixels,image.mode,image.size
    r(
        "filter.render.lensFlare",
        r##"{"layer": "$P", "centerX": 0.5}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.lightingEffects: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.render.lightingEffects",
        r##"{"layer": "$P", "texture": "none"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.relight: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.render.relight",
        r##"{"layer": "$P", "angle": 45}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.noise.reduceNoise: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.noise.reduceNoise",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blur.smartBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blur.smartBlur",
        r##"{"layer": "$P", "radius": 3}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blur.lensBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.mode,image.size
    r(
        "filter.blur.lensBlur",
        r##"{"layer": "$P", "radius": 15}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blur.shapeBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blur.shapeBlur",
        r##"{"layer": "$P", "radius": 10}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blurGallery.tiltShift: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blurGallery.tiltShift",
        r##"{"layer": "$P", "blur": 15}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blurGallery.irisBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blurGallery.irisBlur",
        r##"{"layer": "$P", "blur": 15}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blurGallery.fieldBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blurGallery.fieldBlur",
        r##"{"layer": "$P", "blur": 15}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blurGallery.spinBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blurGallery.spinBlur",
        r##"{"layer": "$P", "blurAngle": 15}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blurGallery.pathBlur: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.blurGallery.pathBlur",
        r##"{"layer": "$P", "speed": 50}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.other.custom: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.other.custom",
        r##"{"layer": "$P", "kernel": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.other.hsbHsl: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.other.hsbHsl",
        r##"{"layer": "$P", "inputMode": "rgb", "rowOrder": "hsb"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.video.deInterlace: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.video.deInterlace",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.video.ntscColors: layer ignored; editor area,channeltarget,quickmask; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.video.ntscColors",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.other.colorToAlpha: layer ignored; editor area,channeltarget,quickmask; reads T.attr,T.locks,T.pixels,T.position,image.mode,image.size
    r(
        "filter.other.colorToAlpha",
        r##"{"layer": "$P", "color": "#ffffff"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.filterGallery: layer ignored,none; editor area,channeltarget,colors,quickmask; reads T.locks,T.pixels,image.mode,image.size
    // was held less: editor:colors
    r(
        "filter.filterGallery",
        r##"{"layer": "$P", "effects": [{"filter": "cutout", "params": {}}]}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: editor:colors
    r(
        "filter.filterGallery",
        r##"{"list": true}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.blur.average: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.pixels,image.size
    r(
        "filter.blur.average",
        r##"{}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.clouds: layer ignored; editor active,area,channeltarget,colors,quickmask; reads T.locks,T.pixels,image.size
    r(
        "filter.render.clouds",
        r##"{"seed": 0}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.differenceClouds: layer ignored; editor active,area,channeltarget,colors,quickmask; reads T.locks,T.pixels,image.size
    r(
        "filter.render.differenceClouds",
        r##"{"seed": 0}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.convertForEditableFilters: layer id-else-selected; editor selected; reads T.attr,T.content,T.effects,T.locks,T.mask,T.pixels,T.position,T.vmask,docState.global_light,docState.metadata,docState.patterns,image.dpi,image.mode,image.size,others
    r("filter.convertForEditableFilters", r##"{"layer": "$P"}"##, "selection", &[("$P", "*", true)]),
    // was held less: $P not held
    r("filter.convertForEditableFilters", r##"{}"##, "selection", &[("$P", "*", true)]),
    // filter.lensCorrection: layer id-else-active; editor area,channeltarget; reads T.attr,T.locks,T.pixels,T.position,docState.metadata,image.mode,image.size
    r(
        "filter.lensCorrection",
        r##"{"layer": "$P", "distortion": -20}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,docState.exif,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.adaptiveWideAngle: layer id-else-active; editor area,channeltarget; reads T.attr,T.locks,T.pixels,T.position,docState.metadata,image.mode,image.size
    r(
        "filter.adaptiveWideAngle",
        r##"{"layer": "$P", "model": "perspective"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.rawDevelop: layer id-else-active; editor area,channeltarget; reads T.content,T.locks,T.pixels,image.mode,image.size
    r(
        "filter.rawDevelop",
        r##"{"layer": "$P", "exposure": 0.5}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.vanishingPoint: layer ignored; editor clipboard; reads T.locks,T.pixels,composite,image.mode,image.size,others
    r(
        "filter.vanishingPoint",
        r##"{"planes": [{"corners": [[120, 120], [280, 120], [360, 280], [40, 280]]}], "paste": [{"plane": 0, "layer": "$C"}]}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.vanishingPoint",
        r##"{"planes": [{"corners": [[120, 120], [280, 120], [360, 280], [40, 280]]}], "paste": [{"plane": 0}]}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.vanishingPoint",
        r##"{"planes": [{"corners": [[120, 120], [280, 120], [360, 280], [40, 280]]}], "clone": [{"source": [200, 257], "points": [[200, 160]]}]}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: image: docState.global_light,docState.patterns,icc,layers not held
    r(
        "filter.vanishingPoint",
        r##"{"planes": [{"corners": [[120, 120], [280, 120], [360, 280], [40, 280]]}], "clone": [{"source": [200, 257], "points": [[200, 160]]}], "newLayer": true}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width,~inheritedLocks", false)],
    ),
    // filter.liquify: layer id-else-active; editor area,channeltarget; reads T.locks,T.pixels,image.mode,image.size
    // was held less: editor:area
    r(
        "filter.liquify",
        r##"{"layer": "$P", "strokes": [{"tool": "forwardWarp", "size": 50, "points": [[100, 100], [120, 100]]}]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.flame: layer id-else-active; editor active,area; reads T.locks,T.pixels,image.mode,image.paths,image.size
    // was held less: image: docState.paths,docState.work_path not held
    r(
        "filter.render.flame",
        r##"{"layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,paths,docState.work_path,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "filter.render.flame",
        r##"{"layer": "$P", "path": "Path 1"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,paths,docState.work_path,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.pictureFrame: layer id-else-active; editor active,area; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.render.pictureFrame",
        r##"{"layer": "$P", "frame": "roses"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // filter.render.tree: layer id-else-active; editor active,area; reads T.locks,T.pixels,image.mode,image.size
    r(
        "filter.render.tree",
        r##"{"layer": "$P", "baseTreeType": 1}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // image.imageRotation.flipCanvasHorizontal: layer none; editor quickmask; reads docState.measurement,docState.notes,docState.slices,image.channels,image.dpi,image.guides,image.paths,image.size,others
    // was held less: image not held
    r("image.imageRotation.flipCanvasHorizontal", r##"{}"##, "", &[("image", "height,width", false)]),
    // image.imageRotation.flipCanvasVertical: layer none; editor quickmask; reads docState.measurement,docState.notes,docState.slices,image.channels,image.dpi,image.guides,image.paths,image.size,others
    // was held less: image not held
    r("image.imageRotation.flipCanvasVertical", r##"{}"##, "", &[("image", "height,width", false)]),
    // image.imageRotation.180: layer none; editor quickmask; reads docState.measurement,docState.notes,docState.slices,image.channels,image.dpi,image.guides,image.paths,image.size,others
    // was held less: image not held
    r("image.imageRotation.180", r##"{}"##, "", &[("image", "height,width", false)]),
    // image.imageRotation.90cw: layer none; editor quickmask; reads docState.measurement,docState.notes,docState.slices,image.channels,image.dpi,image.guides,image.paths,image.size,others
    // was held less: image not held
    r("image.imageRotation.90cw", r##"{}"##, "", &[("image", "height,width", false)]),
    // image.imageRotation.90ccw: layer none; editor quickmask; reads docState.measurement,docState.notes,docState.slices,image.channels,image.dpi,image.guides,image.paths,image.size,others
    // was held less: image not held
    r("image.imageRotation.90ccw", r##"{}"##, "", &[("image", "height,width", false)]),
    // image.adjustments.desaturate: layer ignored; editor active,area,channeltarget; reads T.locks,T.pixels,image.mode
    r(
        "image.adjustments.desaturate",
        r##"{}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // image.adjustments.brightnessContrast: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.brightnessContrast",
        r##"{"brightness": 20, "contrast": 10}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.brightnessContrast",
        r##"{"brightness": 20, "contrast": 10, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.brightnessContrast",
        r##"{"brightness": 20, "contrast": 10, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.levels: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.levels",
        r##"{"inBlack": 10, "gamma": 1.2}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.levels",
        r##"{"inBlack": 10, "gamma": 1.2, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.levels",
        r##"{"inBlack": 10, "gamma": 1.2, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.curves: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,composite,image.channels,image.mode,image.size
    r(
        "image.adjustments.curves",
        r##"{"points": [[0, 0], [128, 150], [255, 255]]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.curves",
        r##"{"points": [[0, 0], [128, 150], [255, 255]], "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.curves",
        r##"{"points": [[0, 0], [128, 150], [255, 255]], "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.exposure: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.exposure",
        r##"{"exposure": 0.5}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.exposure",
        r##"{"exposure": 0.5, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.exposure",
        r##"{"exposure": 0.5, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.vibrance: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.vibrance",
        r##"{"vibrance": 30}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.vibrance",
        r##"{"vibrance": 30, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.vibrance",
        r##"{"vibrance": 30, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.hueSaturation: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.hueSaturation",
        r##"{"hue": 20, "saturation": 10}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.hueSaturation",
        r##"{"hue": 20, "saturation": 10, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.hueSaturation",
        r##"{"hue": 20, "saturation": 10, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.colorBalance: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.colorBalance",
        r##"{"midtones": [10, 0, -10]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.colorBalance",
        r##"{"midtones": [10, 0, -10], "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.colorBalance",
        r##"{"midtones": [10, 0, -10], "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.blackWhite: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.blackWhite",
        r##"{"reds": 40, "tint": true}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.blackWhite",
        r##"{"reds": 40, "tint": true, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.blackWhite",
        r##"{"reds": 40, "tint": true, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.photoFilter: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.photoFilter",
        r##"{"filter": "warming85", "density": 25}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.photoFilter",
        r##"{"filter": "warming85", "density": 25, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.photoFilter",
        r##"{"filter": "warming85", "density": 25, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.channelMixer: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.channelMixer",
        r##"{"red": [100, 0, 0, 0]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.channelMixer",
        r##"{"red": [100, 0, 0, 0], "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.channelMixer",
        r##"{"red": [100, 0, 0, 0], "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.invert: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.invert",
        r##"{}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r("image.adjustments.invert", r##"{"target": "mask"}"##, "active,area", &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)]),
    r(
        "image.adjustments.invert",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.posterize: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.posterize",
        r##"{"levels": 4}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.posterize",
        r##"{"levels": 4, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.posterize",
        r##"{"levels": 4, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.threshold: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.threshold",
        r##"{"level": 128}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.threshold",
        r##"{"level": 128, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.threshold",
        r##"{"level": 128, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.gradientMap: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.gradientMap",
        r##"{"stops": [[0, "#000000"], [1, "#ffffff"]]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.gradientMap",
        r##"{"stops": [[0, "#000000"], [1, "#ffffff"]], "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.gradientMap",
        r##"{"stops": [[0, "#000000"], [1, "#ffffff"]], "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.selectiveColor: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    r(
        "image.adjustments.selectiveColor",
        r##"{"colors": "reds", "cyan": 10}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.selectiveColor",
        r##"{"colors": "reds", "cyan": 10, "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "image.adjustments.selectiveColor",
        r##"{"colors": "reds", "cyan": 10, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.adjustments.colorLookup: layer ignored; editor active,area,channeltarget,quickmask; reads T.locks,T.mask,T.pixels,image.channels,image.mode,image.size
    // was held less: editor:area
    r(
        "image.adjustments.colorLookup",
        r##"{"lut": "warm"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: editor:area
    r(
        "image.adjustments.colorLookup",
        r##"{"lut": "warm", "target": "mask"}"##,
        "active,area",
        &[("$P", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    // was held less: editor:area
    r(
        "image.adjustments.colorLookup",
        r##"{"lut": "warm", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.autoTone: layer ignored; editor active,area; reads T.locks,T.pixels,image.mode,image.size
    // was held less: $P not held; image not held
    r("image.autoTone", r##"{}"##, "active,area", &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)]),
    // image.autoContrast: layer ignored; editor active,area; reads T.locks,T.pixels,image.mode,image.size
    // was held less: $P not held; image not held
    r(
        "image.autoContrast",
        r##"{}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // image.autoColor: layer ignored; editor active,area; reads T.locks,T.pixels,image.mode,image.size
    // was held less: $P not held; image not held
    r("image.autoColor", r##"{}"##, "active,area", &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)]),
    // image.imageSize: layer none; editor area,quickmask; reads docState.notes,image.channels,image.dpi,image.guides,image.mode,image.paths,image.size,others
    // was held less: image not held
    r("image.imageSize", r##"{"width": 200}"##, "", &[("image", "dpi,height,width", false)]),
    // was held less: image not held
    r("image.imageSize", r##"{"resolution": 300, "resample": "none"}"##, "", &[("image", "dpi,height,width", false)]),
    // image.canvasSize: layer none; editor area,colors,quickmask; reads docState.notes,image.channels,image.paths,image.size,others
    // was held less: editor:colors; image not held
    r("image.canvasSize", r##"{"width": 500, "height": 400, "anchor": "center"}"##, "colors", &[("image", "height,width", false)]),
    // image.crop: layer none; editor area,colors,quickmask; reads docState.notes,image.channels,image.paths,others
    // was held less: editor:colors; image not held
    r("image.crop", r##"{"x": 10, "y": 10, "width": 200, "height": 150}"##, "area,colors", &[("image", "height,width", false)]),
    // was held less: editor:colors; image not held
    r("image.crop", r##"{}"##, "area,colors", &[("image", "height,width", false)]),
    // was held less: editor:colors; image not held
    r("image.crop", r##"{"x": 10, "y": 10, "width": 200, "height": 150, "angle": 5}"##, "area,colors", &[("image", "height,width", false)]),
    // image.trim: layer none; editor quickmask; reads composite,docState.notes,image.channels,image.paths,image.size,others
    // was held less: image not held
    r("image.trim", r##"{"basedOn": "transparent"}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // image.mode.rgb: layer none; editor prefs; reads composite,image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.rgb", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.mode.grayscale: layer none; editor prefs; reads composite,image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.grayscale", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.mode.cmyk: layer none; editor prefs; reads composite,image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.cmyk", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.mode.lab: layer none; editor prefs; reads composite,image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.lab", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.mode.bits8: layer none; editor quickmask; reads image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.bits8", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.mode.bits16: layer none; editor quickmask; reads image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.bits16", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.mode.bits32: layer none; editor quickmask; reads image.channels,image.mode,others
    // was held less: image not held
    r("image.mode.bits32", r##"{}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // image.duplicate: layer none; editor area; reads composite,docState.name
    r("image.duplicate", r##"{"name": "Copy"}"##, "", &[]),
    // image.adjustments.equalize: layer ignored,none; editor active,area,channeltarget; reads T.locks,T.pixels,image.channels
    r(
        "image.adjustments.equalize",
        r##"{}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "image.adjustments.equalize",
        r##"{"target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // image.revealAll: layer none; editor quickmask; reads docState.notes,image.channels,image.paths,image.size,others
    // was held less: image not held
    r("image.revealAll", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // image.applyImage: layer active-only,none; editor active,area,channeltarget,quickmask; reads T.locks,T.pixels,image.channels,image.mode,image.size
    // was held less: $P not held; image: ~inheritedLocks not held
    r(
        "image.applyImage",
        r##"{"source": {"layer": "$P2", "channel": "composite"}, "blending": "multiply", "opacity": 50}"##,
        "active,area",
        &[
            ("$P", "kind,locks,data,text,tiles", false),
            ("$P2", "*", true),
            ("image", "channels,depth,height,mode,width,~channelContents,~inheritedLocks", false),
        ],
    ),
    r(
        "image.applyImage",
        r##"{"source": {"channel": "composite"}, "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,docState.global_light,patterns,height,icc,layers,mode,width,~channelContents", false)],
    ),
    // image.calculations: layer none; editor active,area,quickmask; reads image.channels,image.dpi,image.mode,image.size
    r(
        "image.calculations",
        r##"{"source1": {"channel": "red"}, "source2": {"channel": "Alpha 1"}, "blending": "multiply"}"##,
        "",
        &[("image", "channels,depth,docState.global_light,patterns,height,icc,layers,mode,width,~channelContents", false)],
    ),
    r(
        "image.calculations",
        r##"{"source1": {"channel": "red"}, "result": "selection"}"##,
        "",
        &[("image", "channels,depth,docState.global_light,patterns,height,icc,layers,mode,width,~channelContents", false)],
    ),
    r(
        "image.calculations",
        r##"{"source1": {"channel": "red"}, "result": "newDocument"}"##,
        "",
        &[("image", "channels,depth,docState.global_light,patterns,height,icc,layers,mode,width,~channelContents", false)],
    ),
    // image.adjustments.shadowsHighlights: layer ignored; editor active,area,channeltarget; reads T.content,T.locks,T.pixels
    r(
        "image.adjustments.shadowsHighlights",
        r##"{"shadowAmount": 35}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // image.adjustments.replaceColor: layer ignored; editor active,area,channeltarget,colors; reads T.locks,T.pixels
    // was held less: editor:colors; editor:area; editor:active; $P not held; image not held
    r(
        "image.adjustments.replaceColor",
        r##"{"color": "#ff0000", "fuzziness": 40, "hue": 30}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // image.adjustments.matchColor: layer ignored; editor active,area,channeltarget; reads T.content,T.locks,T.pixels
    // was held less: editor:area
    r(
        "image.adjustments.matchColor",
        r##"{"source": 0, "luminance": 100}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width,~inheritedLocks", false)],
    ),
    // image.adjustments.hdrToning: layer ignored; editor -; reads composite,image.mode,image.size
    r(
        "image.adjustments.hdrToning",
        r##"{"radius": 30, "strength": 0.5}"##,
        "",
        &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // image.rotation.arbitrary: layer none; editor area,colors,quickmask; reads docState.notes,image.channels,image.dpi,image.paths,image.size,others
    // was held less: editor:colors; image not held
    r("image.rotation.arbitrary", r##"{"angle": 15, "direction": "cw"}"##, "colors", &[("image", "height,width", false)]),
    // image.mode.indexedColor: layer none; editor quickmask; reads composite,image.channels,image.mode
    // was held less: image not held
    r(
        "image.mode.indexedColor",
        r##"{"palette": "selective", "colors": 256}"##,
        "",
        &[("image", "depth,docState.color_table,docState.duotone,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // image.mode.colorTable: layer none; editor -; reads docState.color_table
    // was held less: image not held
    r("image.mode.colorTable", r##"{"entries": {"0": "#ff0000"}}"##, "", &[("image", "docState.color_table", false)]),
    // image.mode.bitmap: layer none; editor quickmask; reads composite,image.channels,image.dpi,image.mode
    // was held less: image not held
    r(
        "image.mode.bitmap",
        r##"{"method": "diffusion"}"##,
        "",
        &[("image", "depth,docState.color_table,docState.duotone,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // image.mode.duotone: layer none; editor -; reads image.mode
    // was held less: image not held
    r("image.mode.duotone", r##"{"type": "duotone"}"##, "", &[("image", "docState.duotone,mode", false)]),
    // image.mode.multichannel: layer none; editor quickmask; reads composite,docState.duotone,image.channels,image.mode
    // was held less: image not held
    r(
        "image.mode.multichannel",
        r##"{}"##,
        "",
        &[("image", "depth,docState.color_table,docState.duotone,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // image.analysis.setMeasurementScale: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("image.analysis.setMeasurementScale", r##"{"pixelLength": 100, "logicalLength": 10, "units": "mm"}"##, "", &[("image", "docState.measurement", false)]),
    // was held less: image not held
    r("image.analysis.setMeasurementScale", r##"{}"##, "", &[("image", "docState.measurement", false)]),
    // image.analysis.selectDataPoints: layer none; editor prefs; reads -
    r("image.analysis.selectDataPoints", r##"{"selection": ["area", "grayMean"]}"##, "", &[]),
    // image.analysis.recordMeasurements: layer none; editor area,library,prefs; reads composite,docState.measurement,docState.name,image.mode,image.size
    r("image.analysis.recordMeasurements", r##"{"source": "selection"}"##, "area", &[]),
    // image.analysis.rulerTool: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("image.analysis.rulerTool", r##"{"start": [10, 10], "end": [100, 20]}"##, "", &[("image", "docState.measurement", false)]),
    // was held less: image not held
    r("image.analysis.rulerTool", r##"{"protractor": [50, 80]}"##, "", &[("image", "docState.measurement", false)]),
    // was held less: image not held
    r("image.analysis.rulerTool", r##"{"clear": true}"##, "", &[("image", "docState.measurement", false)]),
    // image.analysis.placeScaleMarker: layer none; editor active,library; reads docState.measurement,image.dpi,image.mode,image.size
    // was held less: editor:active; $P not held; image not held
    r("image.analysis.placeScaleMarker", r##"{"length": 10}"##, "active", &[("$P", "clipped,kind", true), ("image", "docState.measurement", false)]),
    // image.analysis.straightenLayer: layer active-only,none; editor active,area,colors; reads T.content,T.locks,T.mask,T.pixels,T.position,T.vmask,docState.measurement,image.size,others
    // was held less: editor:colors; editor:area; $P not held; image not held
    r(
        "image.analysis.straightenLayer",
        r##"{"crop": true}"##,
        "active,area,colors",
        &[("$P", "*", true), ("image", "docState.measurement,height,width", false)],
    ),
    // was held less: editor:colors; editor:area; $P not held; image not held
    r(
        "image.analysis.straightenLayer",
        r##"{"crop": false}"##,
        "active,area,colors",
        &[("$P", "*", true), ("image", "docState.measurement,height,width", false)],
    ),
    // image.trap: layer ignored; editor active; reads T.pixels,image.mode
    // was held less: editor:active; $P not held; image not held
    r("image.trap", r##"{"width": 1}"##, "active", &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)]),
    // image.variables.define: layer none; editor -; reads others
    // was held less: image not held
    r("image.variables.define", r##"{"defs": [{"name": "title", "layer": "$T", "type": "textReplacement"}]}"##, "", &[("image", "docState.variables", false)]),
    // image.variables.dataSets: layer none; editor -; reads docState.variables
    // was held less: image not held
    r(
        "image.variables.dataSets",
        r##"{"dataSets": [{"name": "A", "values": [{"variable": "title", "kind": "text", "value": "Hi"}]}]}"##,
        "",
        &[("image", "docState.variables", false)],
    ),
    // image.applyDataSet: layer none; editor -; reads docState.variables,image.mode,others
    // was held less: image not held
    r(
        "image.applyDataSet",
        r##"{"name": "A"}"##,
        "",
        &[("image", "depth,docState.global_light,patterns,docState.variables,height,icc,layers,mode,width", false)],
    ),
    // layer.rasterize.vectorMask: layer id-else-active; editor active; reads T.mask,T.vmask,image.mode,image.size
    // was held more: $SH: every field held, reads kind,mask,props.extra; $SH: position held
    r("layer.rasterize.vectorMask", r##"{"layer": "$SH"}"##, "active", &[("$SH", "kind,mask,extra", false)]),
    // layer.rasterize.shape: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("layer.rasterize.shape", r##"{"layer": "$SH"}"##, "active", &[("$SH", "*", true)]),
    // layer.rasterize.layer: layer id-else-active,other; editor active; reads T.content,image.mode,image.size
    r("layer.rasterize.layer", r##"{"layer": "$T"}"##, "active", &[("$T", "*", true)]),
    r("layer.rasterize.layer", r##"{"layer": "$SH"}"##, "active", &[("$SH", "*", true)]),
    r("layer.rasterize.layer", r##"{"layer": "$FILL"}"##, "active", &[("$FILL", "*", true)]),
    // layer.rasterize.allLayers: layer none; editor active; reads image.mode,image.size,others
    // was held less: image not held
    r("layer.rasterize.allLayers", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // layer.rasterize.type: layer id-else-active; editor active; reads T.content,image.mode
    r("layer.rasterize.type", r##"{"layer": "$T"}"##, "active", &[("$T", "*", true)]),
    // layer.rasterize.fillContent: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("layer.rasterize.fillContent", r##"{"layer": "$FILL"}"##, "active", &[("$FILL", "*", true)]),
    // layer.rasterize.objectLayer: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("layer.rasterize.objectLayer", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.convertToObjectLayer: layer id-else-selected; editor active,selected; reads T.attr,T.content,T.effects,T.locks,T.vmask,docState.global_light,docState.patterns,image.dpi,image.mode,image.size,others
    r("layer.objectLayers.convertToObjectLayer", r##"{"layer": "$P"}"##, "selection", &[("$P", "*", true)]),
    // was held less: $P not held
    r("layer.objectLayers.convertToObjectLayer", r##"{"on": ["$C2", "$P"]}"##, "selection", &[("$P", "*", true)]),
    // layer.objectLayers.newObjectLayerViaCopy: layer id-else-active; editor active; reads T.content,others
    r("layer.objectLayers.newObjectLayerViaCopy", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.rasterize: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("layer.objectLayers.rasterize", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.editContents: layer id-else-active; editor active; reads T.content
    r("layer.objectLayers.editContents", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.saveContents: layer none; editor -; reads -
    r("layer.objectLayers.saveContents", r##"{}"##, "", &[]),
    // layer.objectLayers.replaceContents: layer id-else-active; editor active; reads T.content,T.mask,image.mode,image.size
    r("layer.objectLayers.replaceContents", r##"{"layer": "$SO", "path": "/tmp/new.png"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.exportContents: layer id-else-active; editor active; reads T.content
    r("layer.objectLayers.exportContents", r##"{"layer": "$SO", "path": "/tmp/out.kphoto"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.relinkToFile: layer id-else-active; editor active; reads T.content,T.mask,image.mode,image.size
    r("layer.objectLayers.relinkToFile", r##"{"layer": "$SO", "path": "/tmp/linked.psd"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.updateModifiedContent: layer id-else-active; editor active; reads T.content,T.mask,image.mode,image.size
    r("layer.objectLayers.updateModifiedContent", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.updateAllModifiedContent: layer none; editor -; reads image.mode,image.size,others
    r("layer.objectLayers.updateAllModifiedContent", r##"{}"##, "", &[]),
    // layer.objectLayers.convertToLayers: layer id-else-active; editor active,prefs; reads T.attr,T.content,T.effects,T.locks,T.mask,T.vmask,image.mode
    r("layer.objectLayers.convertToLayers", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.convertToEmbedded: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("layer.objectLayers.convertToEmbedded", r##"{"layer": "$SO"}"##, "active", &[("$SO", "*", true)]),
    // layer.objectLayers.convertToLinked: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("layer.objectLayers.convertToLinked", r##"{"layer": "$SO", "path": "/tmp/so.kphoto"}"##, "active", &[("$SO", "*", true)]),
    // layer.editableFilter.disableEditableFilters: layer id-else-active; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.disableEditableFilters", r##"{"layer": "$SO", "enabled": false}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.deleteFilterMask: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.deleteFilterMask", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.disableFilterMask: layer id-else-active; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.disableFilterMask", r##"{"layer": "$SO", "enabled": false}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.blendingOptions: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r(
        "layer.editableFilter.blendingOptions",
        r##"{"layer": "$SO", "index": 0, "blend": "multiply", "opacity": 50}"##,
        "active",
        &[("$SO", "kind,data,text", false)],
    ),
    // layer.editableFilter.clearEditableFilters: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.clearEditableFilters", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.setVisible: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.setVisible", r##"{"layer": "$SO", "index": 0, "visible": false}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.setParams: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.setParams", r##"{"layer": "$SO", "index": 0, "params": {"radius": 4}}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.delete: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.delete", r##"{"layer": "$SO", "index": 0}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.editableFilter.move: layer id-else-active; editor active; reads T.content,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.editableFilter.move", r##"{"layer": "$SO", "index": 0, "to": 1}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.entropy: layer id-else-active; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.entropy", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.kurtosis: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.kurtosis", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.maximum: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.maximum", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.mean: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.mean", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.median: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.median", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.minimum: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.minimum", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.range: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.range", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.skewness: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.skewness", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.standardDeviation: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.standardDeviation", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.summation: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.summation", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.variance: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.variance", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.stackMode.none: layer other; editor active; reads T.content,T.mask,image.mode,image.size
    // was held more: $SO: every field held, reads kind,props.data; $SO: position held
    r("layer.objectLayers.stackMode.none", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,data,text", false)]),
    // layer.objectLayers.revealInFinder: layer ignored; editor active; reads -
    r("layer.objectLayers.revealInFinder", r##"{"dryRun": true}"##, "", &[]),
    // layer.objectLayers.warp: layer id-else-active; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,image.mode,image.size
    r("layer.objectLayers.warp", r##"{"layer": "$SO", "style": "arc", "bend": 30}"##, "active,area", &[("$SO", "*", true)]),
    r("layer.objectLayers.warp", r##"{"layer": "$P", "style": "flag"}"##, "active,area", &[("$P", "*", true)]),
    // layer.objectLayers.pinWarp: layer id-else-active; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,image.mode,image.size
    // was held less: editor:area
    r("layer.objectLayers.pinWarp", r##"{"layer": "$SO", "pins": [{"src": [100, 100], "dst": [120, 110]}]}"##, "active,area", &[("$SO", "*", true)]),
    // was held less: editor:area
    r("layer.objectLayers.pinWarp", r##"{"layer": "$P", "pins": [{"src": [100, 100], "dst": [120, 110]}]}"##, "active,area", &[("$P", "*", true)]),
    // layer.objectLayers.perspectiveWarp: layer id-else-active; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,image.mode,image.size
    // was held less: editor:area
    r(
        "layer.objectLayers.perspectiveWarp",
        r##"{"layer": "$SO", "planes": [{"src": [[0, 0], [400, 0], [400, 300], [0, 300]], "dst": [[10, 0], [390, 10], [400, 300], [0, 290]]}]}"##,
        "active,area",
        &[("$SO", "*", true)],
    ),
    // was held less: editor:area
    r(
        "layer.objectLayers.perspectiveWarp",
        r##"{"layer": "$P", "planes": [{"src": [[0, 0], [400, 0], [400, 300], [0, 300]], "dst": [[10, 0], [390, 10], [400, 300], [0, 290]]}]}"##,
        "active,area",
        &[("$P", "*", true)],
    ),
    // layer.videoLayers.newBlankVideoLayer: layer ignored; editor active; reads docState.timeline,image.mode,others
    // was held less: $P not held; image not held
    r("layer.videoLayers.newBlankVideoLayer", r##"{}"##, "active", &[("$P", "clipped,kind", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.insertBlankFrame: layer ignored; editor active; reads docState.timeline,image.mode
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.insertBlankFrame", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.duplicateFrame: layer ignored; editor active; reads T.pixels,docState.timeline
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.duplicateFrame", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.deleteFrame: layer ignored; editor active; reads docState.timeline
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.deleteFrame", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.restoreFrame: layer ignored; editor active; reads docState.timeline,image.mode
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.restoreFrame", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.restoreAllFrames: layer ignored; editor active; reads docState.timeline,image.mode
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.restoreAllFrames", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.showAlteredVideo: layer ignored; editor active; reads docState.timeline
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.showAlteredVideo", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.rasterize: layer ignored; editor active; reads -
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.rasterize", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.rasterize.video: layer ignored; editor active; reads -
    // was held less: editor:active; $P not held; image not held
    r("layer.rasterize.video", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.newVideoLayerFromFile: layer ignored; editor active; reads docState.timeline,image.mode,image.size,others
    // was held less: editor:active; $P not held; image not held
    r(
        "layer.videoLayers.newVideoLayerFromFile",
        r##"{"path": "/tmp/clip_frames", "fps": 24}"##,
        "active",
        &[("$P", "clipped,kind", true), ("image", "docState.timeline", false)],
    ),
    // layer.videoLayers.replaceFootage: layer ignored; editor active; reads docState.timeline,image.mode
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.replaceFootage", r##"{"path": "/tmp/clip2.png"}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.interpretFootage: layer ignored; editor active; reads docState.timeline
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.interpretFootage", r##"{"fps": 24}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.videoLayers.reloadFrame: layer ignored; editor active; reads docState.timeline,image.mode
    // was held less: editor:active; $P not held; image not held
    r("layer.videoLayers.reloadFrame", r##"{}"##, "active", &[("$P", "*", true), ("image", "docState.timeline", false)]),
    // layer.layerMask.revealAll: layer id-else-active; editor active; reads T.attr,T.locks,T.position
    // was held more: $P: every field held, reads kind,locks,mask,name; $P: position held
    r("layer.layerMask.revealAll", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,mask,name", false)]),
    // layer.layerMask.hideAll: layer id-else-active; editor active; reads T.attr,T.locks,T.position
    // was held more: $P: every field held, reads kind,locks,mask,name; $P: position held
    r("layer.layerMask.hideAll", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,mask,name", false)]),
    // layer.layerMask.revealSelection: layer id-else-active; editor active,area; reads -
    // was held more: $P: every field held, reads kind,locks,mask,name; $P: position held
    r("layer.layerMask.revealSelection", r##"{"layer": "$P", "region": [50, 50, 100, 80]}"##, "active,area", &[("$P", "kind,locks,mask,name", false)]),
    // layer.layerMask.delete: layer id-else-active; editor active; reads -
    // was held more: $C: every field held, reads kind,locks,mask,name; $C: position held
    r("layer.layerMask.delete", r##"{"layer": "$C"}"##, "active", &[("$C", "kind,locks,mask,name", false)]),
    // layer.newFillLayer.solidColor: layer active-only; editor active,area,colors; reads image.mode,image.paths,others
    // was held less: $P not held
    r("layer.newFillLayer.solidColor", r##"{"color": "#ff00ff"}"##, "active,area,colors", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newFillLayer.solidColor",
        r##"{"color": "#ff00ff", "path": "Path 1"}"##,
        "active,area,colors",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newFillLayer.gradient: layer active-only; editor active,area,colors; reads image.mode,image.paths,others
    // was held less: $P not held
    r("layer.newFillLayer.gradient", r##"{"from": "#000000", "to": "#ffffff", "angle": 0}"##, "active,area,colors", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newFillLayer.gradient",
        r##"{"path": "work"}"##,
        "active,area,colors",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.brightnessContrast: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.brightnessContrast", r##"{"brightness": 20, "contrast": 10}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.brightnessContrast",
        r##"{"brightness": 20, "contrast": 10, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.levels: layer active-only; editor active,area; reads image.mode,image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.levels", r##"{"input": [10, 245]}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.levels",
        r##"{"input": [10, 245], "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.curves: layer active-only; editor active,area; reads composite,image.mode,image.paths,image.size,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.curves", r##"{"master": [[0, 0], [128, 150], [255, 255]]}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.curves",
        r##"{"eyedropper": {"point": "white", "at": [60, 60]}}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.curves",
        r##"{"master": [[0, 0], [128, 150], [255, 255]], "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.exposure: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.exposure", r##"{"exposure": 1}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.exposure",
        r##"{"exposure": 1, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.vibrance: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.vibrance", r##"{"vibrance": 30}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.vibrance",
        r##"{"vibrance": 30, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.hueSaturation: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.hueSaturation", r##"{"hue": 40, "saturation": 20}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.hueSaturation",
        r##"{"hue": 40, "saturation": 20, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.colorBalance: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.colorBalance", r##"{"midtones": [10, 0, -10]}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.colorBalance",
        r##"{"midtones": [10, 0, -10], "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.blackWhite: layer active-only; editor active,area; reads image.paths,others
    // was held less: $P not held
    r("layer.newAdjustmentLayer.blackWhite", r##"{"reds": 40, "tint": true}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newAdjustmentLayer.blackWhite",
        r##"{"reds": 40, "tint": true, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.photoFilter: layer active-only; editor active,area; reads image.paths,others
    // was held less: $P not held
    r("layer.newAdjustmentLayer.photoFilter", r##"{"filter": "warming85", "density": 25}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newAdjustmentLayer.photoFilter",
        r##"{"filter": "warming85", "density": 25, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.channelMixer: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.channelMixer", r##"{"red": [100, 0, 0, 0]}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.channelMixer",
        r##"{"red": [100, 0, 0, 0], "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.invert: layer active-only; editor active,area; reads image.paths,others
    // was held less: $P not held
    r("layer.newAdjustmentLayer.invert", r##"{}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newAdjustmentLayer.invert",
        r##"{"path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.posterize: layer active-only; editor active,area; reads image.paths,others
    // was held less: $P not held
    r("layer.newAdjustmentLayer.posterize", r##"{"levels": 4}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newAdjustmentLayer.posterize",
        r##"{"levels": 4, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.threshold: layer active-only; editor active,area; reads image.paths,others
    // was held less: $P not held
    r("layer.newAdjustmentLayer.threshold", r##"{"level": 128}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newAdjustmentLayer.threshold",
        r##"{"level": 128, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.gradientMap: layer active-only; editor active,area; reads image.paths,others
    // was held less: $P not held
    r("layer.newAdjustmentLayer.gradientMap", r##"{"stops": [[0, "#000000"], [1, "#ffffff"]]}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r(
        "layer.newAdjustmentLayer.gradientMap",
        r##"{"stops": [[0, "#000000"], [1, "#ffffff"]], "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.selectiveColor: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.selectiveColor", r##"{"colors": "reds", "cyan": 10}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.selectiveColor",
        r##"{"colors": "reds", "cyan": 10, "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.newAdjustmentLayer.colorLookup: layer active-only; editor active,area; reads image.paths,others
    // was held less: editor:area; $P not held
    r("layer.newAdjustmentLayer.colorLookup", r##"{"lut": "warm"}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; $P not held
    r(
        "layer.newAdjustmentLayer.colorLookup",
        r##"{"lut": "warm", "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,docState.work_path", false)],
    ),
    // layer.layerStyle.dropShadow: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.dropShadow", r##"{"layer": "$P", "distance": 8, "size": 6}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.innerShadow: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.innerShadow", r##"{"layer": "$P", "choke": 10}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.outerGlow: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.outerGlow", r##"{"layer": "$P", "size": 10}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.innerGlow: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.innerGlow", r##"{"layer": "$P", "source": "center"}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.stroke: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.stroke", r##"{"layer": "$P", "size": 3, "color": "#00ff00"}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.colorOverlay: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.colorOverlay", r##"{"layer": "$P", "color": "#0000ff"}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.gradientOverlay: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.gradientOverlay", r##"{"layer": "$P", "from": "#000000", "to": "#ffffff"}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.patternOverlay: layer id-else-active; editor active,library,prefs; reads T.effects,docState.patterns,image.mode
    // was held more: $P: every field held, reads effects
    r(
        "layer.layerStyle.patternOverlay",
        r##"{"layer": "$P", "pattern": "Dots"}"##,
        "active",
        &[("$P", "styleSettings,styles", false), ("image", "patterns", false)],
    ),
    // layer.layerStyle.bevelEmboss: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r(
        "layer.layerStyle.bevelEmboss",
        r##"{"layer": "$P", "style": "inner", "texture": "Dots"}"##,
        "active",
        &[("$P", "styleSettings,styles", false), ("image", "patterns", false)],
    ),
    // layer.layerStyle.satin: layer id-else-active; editor active; reads T.effects,image.mode
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.satin", r##"{"layer": "$P", "distance": 11}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.replace: layer id-else-active; editor active,library; reads docState.patterns,image.mode
    // was held more: $P: every field held, reads effects
    r(
        "layer.layerStyle.replace",
        r##"{"layer": "$P", "effects": [{"kind": "dropShadow", "params": {"distance": 4}}]}"##,
        "active",
        &[("$P", "styleSettings,styles", false), ("image", "patterns", false)],
    ),
    // layer.layerStyle.clear: layer id-else-active; editor active; reads -
    // was held more: $P: every field held, reads effects
    r("layer.layerStyle.clear", r##"{"layer": "$P"}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.vectorMask.add: layer id-else-active; editor active; reads T.locks,image.paths
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r("layer.vectorMask.add", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,extra", false)]),
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r(
        "layer.vectorMask.add",
        r##"{"layer": "$P", "name": "Path 1"}"##,
        "active",
        &[("$P", "kind,locks,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // layer.vectorMask.fromPath: layer id-else-active; editor active; reads T.locks,image.paths
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r(
        "layer.vectorMask.fromPath",
        r##"{"layer": "$P", "name": "Path 1"}"##,
        "active",
        &[("$P", "kind,locks,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // layer.vectorMask.edit: layer id-else-active; editor active; reads T.vmask
    // was held more: $SO: every field held, reads kind,locks,props.extra; $SO: position held
    r("layer.vectorMask.edit", r##"{"layer": "$SO", "density": 50}"##, "active", &[("$SO", "kind,locks,extra", false)]),
    // layer.vectorMask.delete: layer id-else-active; editor active; reads T.vmask
    // was held more: $SO: every field held, reads kind,locks,props.extra; $SO: position held
    r("layer.vectorMask.delete", r##"{"layer": "$SO"}"##, "active", &[("$SO", "kind,locks,extra", false)]),
    // layer.vectorMask.revealAll: layer id-else-active; editor active; reads T.locks
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r("layer.vectorMask.revealAll", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,extra", false)]),
    // layer.vectorMask.hideAll: layer id-else-active; editor active; reads T.locks
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r("layer.vectorMask.hideAll", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,extra", false)]),
    // layer.vectorMask.currentPath: layer id-else-active; editor active; reads T.locks,image.paths
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r("layer.vectorMask.currentPath", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,extra", false)]),
    // layer.vectorMask.enabled: layer id-else-active; editor active; reads T.vmask
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r("layer.vectorMask.enabled", r##"{"layer": "$P", "enabled": false}"##, "active", &[("$P", "kind,locks,extra", false)]),
    // layer.vectorMask.linked: layer id-else-active; editor active; reads T.vmask
    // was held more: $P: every field held, reads kind,locks,props.extra; $P: position held
    r("layer.vectorMask.linked", r##"{"layer": "$P", "linked": false}"##, "active", &[("$P", "kind,locks,extra", false)]),
    // layer.combineShapes.unite: layer id-else-active; editor active,colors; reads T.content,image.mode
    // was held less: editor:colors
    // was held more: $SH: every field held, reads kind,name,props.data; $SH: position held
    r("layer.combineShapes.unite", r##"{"layer": "$SH", "subpath": 1}"##, "active,colors", &[("$SH", "kind,name,data,text", false)]),
    // layer.combineShapes.subtractFrontShape: layer id-else-active; editor active,colors; reads T.content,image.mode
    // was held less: editor:colors
    // was held more: $SH: every field held, reads kind,name,props.data; $SH: position held
    r("layer.combineShapes.subtractFrontShape", r##"{"layer": "$SH", "subpath": 1}"##, "active,colors", &[("$SH", "kind,name,data,text", false)]),
    // layer.combineShapes.intersectShapeAreas: layer id-else-active; editor active,colors; reads T.content,image.mode
    // was held less: editor:colors
    // was held more: $SH: every field held, reads kind,name,props.data; $SH: position held
    r("layer.combineShapes.intersectShapeAreas", r##"{"layer": "$SH", "subpath": 1}"##, "active,colors", &[("$SH", "kind,name,data,text", false)]),
    // layer.combineShapes.excludeOverlappingShapes: layer id-else-active; editor active,colors; reads T.content,image.mode
    // was held less: editor:colors
    // was held more: $SH: every field held, reads kind,name,props.data; $SH: position held
    r("layer.combineShapes.excludeOverlappingShapes", r##"{"layer": "$SH", "subpath": 1}"##, "active,colors", &[("$SH", "kind,name,data,text", false)]),
    // layer.combineShapes.mergeShapeComponents: layer id-else-active; editor active; reads T.content
    // was held more: $SH: every field held, reads kind,name,props.data; $SH: position held
    r("layer.combineShapes.mergeShapeComponents", r##"{"layer": "$SH"}"##, "active", &[("$SH", "kind,name,data,text", false)]),
    // layer.layerStyle.copyLayerStyle: layer id-else-active; editor active; reads T.attr,T.effects,T.mask
    // was held more: $P: every field held, reads blend,effects,fillOpacity,props.extra
    r("layer.layerStyle.copyLayerStyle", r##"{"layer": "$P"}"##, "active", &[("$P", "blend,styleSettings,styles,fillOpacity,extra", false)]),
    // layer.layerStyle.pasteLayerStyle: layer id-else-active; editor active,clipboard; reads image.mode
    // was held more: $C: every field held, reads blend,effects,fillOpacity,props.extra
    r("layer.layerStyle.pasteLayerStyle", r##"{"layer": "$C"}"##, "active", &[("$C", "blend,styleSettings,styles,fillOpacity,extra", false)]),
    // layer.layerStyle.hideAllEffects: layer ignored; editor -; reads others
    // was held less: image not held
    r("layer.layerStyle.hideAllEffects", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // layer.layerStyle.showAllEffects: layer ignored; editor -; reads others
    // was held less: image not held
    r("layer.layerStyle.showAllEffects", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // layer.layerMask.enabled: layer id-else-active; editor active; reads T.mask
    // was held more: $C: every field held, reads kind,locks,mask,name; $C: position held
    r("layer.layerMask.enabled", r##"{"layer": "$C", "enabled": false}"##, "active", &[("$C", "kind,locks,mask,name", false)]),
    // layer.layerMask.linked: layer id-else-active; editor active; reads T.mask
    // was held more: $C: every field held, reads kind,locks,mask,name; $C: position held
    r("layer.layerMask.linked", r##"{"layer": "$C", "linked": false}"##, "active", &[("$C", "kind,locks,mask,name", false)]),
    // layer.layerMask.apply: layer id-else-active; editor active; reads T.content,T.locks,T.mask,T.pixels
    // was held more: $C: every field held, reads kind,locks,mask,props.data,tiles; $C: position held
    r("layer.layerMask.apply", r##"{"layer": "$C"}"##, "active", &[("$C", "kind,locks,mask,data,text,tiles", false)]),
    // layer.layerMask.fromTransparency: layer id-else-active; editor active; reads T.locks,T.pixels
    // was held more: $P: every field held, reads kind,locks,mask,props.data,tiles; $P: position held
    r("layer.layerMask.fromTransparency", r##"{"layer": "$P"}"##, "active", &[("$P", "kind,locks,mask,data,text,tiles", false)]),
    // layer.layerMask.hideSelection: layer id-else-active; editor active,area; reads image.size
    // was held more: $P: every field held, reads kind,locks,mask,name; $P: position held
    r("layer.layerMask.hideSelection", r##"{"layer": "$P", "region": [50, 50, 100, 80]}"##, "active,area", &[("$P", "kind,locks,mask,name", false)]),
    // layer.matting.defringe: layer ignored; editor active; reads T.content,T.locks,T.pixels
    // was held less: editor:active; $P not held; image not held
    r(
        "layer.matting.defringe",
        r##"{"width": 2}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // layer.matting.removeBlackMatte: layer ignored; editor active; reads T.locks,T.pixels,T.position
    // was held less: editor:active; $P not held; image not held
    r(
        "layer.matting.removeBlackMatte",
        r##"{}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // layer.matting.removeWhiteMatte: layer ignored; editor active; reads T.locks,T.pixels,T.position
    // was held less: editor:active; $P not held; image not held
    r(
        "layer.matting.removeWhiteMatte",
        r##"{}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // layer.matting.colorDecontaminate: layer ignored; editor active; reads T.content,T.locks,T.pixels
    // was held less: editor:active; $P not held; image not held
    r(
        "layer.matting.colorDecontaminate",
        r##"{"amount": 100, "radius": 4}"##,
        "active",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // layer.layerStyle.blendingOptions: layer id-else-active; editor active; reads T.attr,image.mode
    // was held more: $P: every field held, reads blend,fillOpacity,opacity,props.extra
    r(
        "layer.layerStyle.blendingOptions",
        r##"{"layer": "$P", "opacity": 50, "blendIf": {"channel": "gray", "thisLayer": [0, 200]}}"##,
        "active",
        &[("$P", "blend,fillOpacity,opacity,extra", false)],
    ),
    // layer.layerStyle.globalLight: layer none; editor -; reads docState.global_light
    // was held less: image not held
    r("layer.layerStyle.globalLight", r##"{"angle": 90}"##, "", &[("image", "docState.global_light", false)]),
    // layer.layerStyle.createLayer: layer id-else-active; editor active; reads T.attr,T.content,T.effects,T.pixels,T.position,T.vmask,docState.global_light,docState.patterns,image.size,others
    r("layer.layerStyle.createLayer", r##"{"layer": "$P"}"##, "active", &[("$P", "*", false)]),
    // layer.layerStyle.scaleEffects: layer id-else-active; editor active; reads T.effects
    // was held less: $P not held
    r("layer.layerStyle.scaleEffects", r##"{"scale": 200}"##, "active", &[("$P", "styleSettings,styles", false)]),
    // layer.layerStyle.transferEffects: layer none; editor -; reads others
    r("layer.layerStyle.transferEffects", r##"{"from": "$P", "to": "$C", "copy": true}"##, "", &[("$C", "*", true), ("$P", "*", true)]),
    // layer.newFillLayer.pattern: layer active-only,none; editor active,area,library,prefs; reads docState.patterns,image.paths,others
    // was held less: $P not held
    r("layer.newFillLayer.pattern", r##"{"pattern": "pat1", "scale": 100}"##, "active,area", &[("$P", "clipped,kind", true), ("image", "patterns", false)]),
    // was held less: $P not held
    r(
        "layer.newFillLayer.pattern",
        r##"{"pattern": "pat1", "path": "Path 1"}"##,
        "active,area",
        &[("$P", "clipped,kind", true), ("image", "paths,patterns,docState.work_path", false)],
    ),
    // layer.new.layer: layer active-only; editor active; reads image.mode,others
    // was held less: $P not held
    r("layer.new.layer", r##"{"name": "New"}"##, "active", &[("$P", "clipped,kind", true)]),
    // layer.new.group: layer active-only; editor active; reads others
    // was held less: $P not held
    r("layer.new.group", r##"{"name": "G2"}"##, "active", &[("$P", "clipped,kind", true)]),
    // layer.groupLayers: layer id-else-selected; editor selected; reads T.position,others
    r("layer.groupLayers", r##"{"layer": "$P", "name": "G"}"##, "selection", &[("$P", "*", true)]),
    // was held less: $P not held
    r("layer.groupLayers", r##"{}"##, "selection", &[("$P", "*", true)]),
    // layer.duplicate: layer id-else-active,id-else-selected; editor active,selected; reads T.content,T.effects,T.locks,T.vmask,others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("layer.duplicate", r##"{"layer": "$P", "name": "Paint copy"}"##, "selection", &[("$P", "*", true)]),
    // was held less: $P not held
    r("layer.duplicate", r##"{}"##, "selection", &[("$P", "*", true)]),
    // layer.delete: layer id-else-active,id-else-selected; editor active,selected; reads others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("layer.delete", r##"{"layer": "$P"}"##, "selection", &[("$P", "*", true)]),
    // was held less: $P not held
    r("layer.delete", r##"{}"##, "selection", &[("$P", "*", true)]),
    // layer.setProps: layer id-else-active; editor active; reads -
    r("layer.setProps", r##"{"layer": "$P", "opacity": 0.5, "blend": "Multiply", "visible": true}"##, "active", &[("$P", "blend,opacity,visible", false)]),
    // layer.arrange.bringForward: layer id-else-active; editor active; reads T.position
    // was held more: $SH: every field held, reads -
    r("layer.arrange.bringForward", r##"{"layer": "$SH"}"##, "active", &[("$SH", "-", true)]),
    // layer.arrange.sendBackward: layer id-else-active; editor active; reads T.position
    // was held more: $SH: every field held, reads -
    r("layer.arrange.sendBackward", r##"{"layer": "$SH"}"##, "active", &[("$SH", "-", true)]),
    // layer.arrange.bringToFront: layer id-else-active; editor active; reads T.position
    // was held more: $SH: every field held, reads -
    r("layer.arrange.bringToFront", r##"{"layer": "$SH"}"##, "active", &[("$SH", "-", true)]),
    // layer.arrange.sendToBack: layer id-else-active; editor active; reads T.position
    // was held more: $SH: every field held, reads -
    r("layer.arrange.sendToBack", r##"{"layer": "$SH"}"##, "active", &[("$SH", "-", true)]),
    // layer.createClippingMask: layer id-else-active,id-else-selected; editor selected; reads others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("layer.createClippingMask", r##"{"layer": "$SH"}"##, "selection", &[("$SH", "*", true)]),
    // was held less: $P not held
    r("layer.createClippingMask", r##"{}"##, "selection", &[("$P", "clipped", true)]),
    // layer.releaseClippingMask: layer id-else-active,id-else-selected; editor selected; reads others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("layer.releaseClippingMask", r##"{"layer": "$P2"}"##, "selection", &[("$P2", "*", true)]),
    // was held less: $P not held
    r("layer.releaseClippingMask", r##"{}"##, "selection", &[("$P", "clipped", true)]),
    // layer.mergeDown: layer id-else-active; editor active; reads T.content,T.effects,T.pixels,T.vmask,image.mode,image.size,others
    // was held less: $P not held
    r("layer.mergeDown", r##"{"layer": "$P2"}"##, "active", &[("$P", "*", true), ("$P2", "*", true)]),
    // layer.flattenImage: layer none; editor -; reads composite,docState.global_light,image.mode
    // was held less: image not held
    r("layer.flattenImage", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // layer.setAdjustment: layer id-else-active; editor active; reads T.content,image.mode
    // was held more: $ADJ: every field held, reads kind,props.data; $ADJ: position held
    r("layer.setAdjustment", r##"{"layer": "$ADJ", "midtones": [10, 10, 10]}"##, "active", &[("$ADJ", "kind,data,text", false)]),
    // was held more: $ADJ: every field held, reads kind,props.data; $ADJ: position held
    r("layer.setAdjustment", r##"{"layer": "$ADJ"}"##, "active", &[("$ADJ", "kind,data,text", false)]),
    // layer.moveTo: layer id-else-active,none; editor active; reads others
    r("layer.moveTo", r##"{"layer": "$SH", "target": "$G", "position": "into"}"##, "active", &[("$G", "*", true), ("$SH", "*", true)]),
    r(
        "layer.moveTo",
        r##"{"layers": ["$P", "$C"], "target": "$T", "position": "below"}"##,
        "active",
        &[("$C", "*", true), ("$P", "*", true), ("$T", "*", true)],
    ),
    r("layer.moveTo", r##"{"layer": "$P", "target": "$T", "copy": true}"##, "active", &[("$P", "*", true), ("$T", "*", true)]),
    // layer.translate: layer id-else-selected; editor active,selected; reads T.attr,T.content,T.locks,T.position,T.vmask,others
    // was held less: editor:selection
    r("layer.translate", r##"{"layer": "$P", "dx": 5, "dy": -3}"##, "active,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // was held less: editor:selection
    r("layer.translate", r##"{"dx": 5}"##, "active,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.fill.solidColor.set: layer id-strict; editor active; reads T.content,T.locks,T.pixels,image.mode
    // was held more: $FILL: every field held, reads kind,locks,props.data; $FILL: position held
    r("layer.fill.solidColor.set", r##"{"layer": "$FILL", "color": "#ff0000"}"##, "active", &[("$FILL", "kind,locks,data,text", false)]),
    // layer.removeBackground: layer id-else-active; editor -; reads T.effects,T.locks,T.mask,T.pixels,image.size
    r("layer.removeBackground", r##"{"layer": "$P"}"##, "active", &[("$P", "*", true)]),
    // layer.new.layerViaCopy: layer ignored; editor active,area; reads T.effects,T.mask,T.pixels,image.size,others
    // was held less: $P not held
    r("layer.new.layerViaCopy", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // was held less: $P not held
    r("layer.new.layerViaCopy", r##"{"region": [50, 50, 100, 80]}"##, "active,area", &[("$P", "*", true)]),
    // layer.new.layerViaCut: layer ignored; editor active,area,colors; reads T.content,T.locks,T.pixels,others
    // was held less: editor:colors; $P not held
    r("layer.new.layerViaCut", r##"{"region": [50, 50, 100, 80]}"##, "active,area,colors", &[("$P", "*", true)]),
    // layer.mergeVisible: layer none; editor -; reads composite,image.mode
    // was held less: image not held
    r("layer.mergeVisible", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // layer.new.layerFromBackground: layer none; editor -; reads -
    r("layer.new.layerFromBackground", r##"{}"##, "", &[]),
    // layer.delete.hiddenLayers: layer none; editor active; reads others
    // was held less: image not held
    r("layer.delete.hiddenLayers", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // layer.ungroupLayers: layer id-else-active; editor -; reads T.position,others
    r("layer.ungroupLayers", r##"{"layer": "$G"}"##, "active", &[("$G", "*", true)]),
    // layer.hideLayers: layer id-else-active,id-else-selected; editor active,selected; reads others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("layer.hideLayers", r##"{"layer": "$SH"}"##, "selection", &[("$SH", "*", true)]),
    // was held less: $P not held
    r("layer.hideLayers", r##"{}"##, "selection", &[("$P", "visible", false)]),
    // layer.showLayers: layer id-else-active,id-else-selected; editor active,selected; reads others
    // was held less: editor:selection
    // was held more: $SH: every field held, reads visible; $SH: position held
    r("layer.showLayers", r##"{"layer": "$SH"}"##, "active,selection", &[("$SH", "visible", false)]),
    // was held less: editor:selection; $P not held
    r("layer.showLayers", r##"{}"##, "active,selection", &[("$P", "visible", false)]),
    // layer.showOnly: layer id-else-active; editor active; reads others
    r("layer.showOnly", r##"{"layer": "$P"}"##, "active", &[("$P", "*", true)]),
    // layer.align.topEdges: layer selected; editor area,selected; reads image.size,others
    // was held less: editor:area
    r("layer.align.topEdges", r##"{"to": "canvas"}"##, "area,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.align.verticalCenters: layer selected; editor area,selected; reads image.size,others
    // was held less: editor:area
    r("layer.align.verticalCenters", r##"{"to": "canvas"}"##, "area,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.align.bottomEdges: layer selected; editor area,selected; reads image.size,others
    // was held less: editor:area
    r("layer.align.bottomEdges", r##"{"to": "canvas"}"##, "area,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.align.leftEdges: layer selected; editor area,selected; reads image.size,others
    // was held less: editor:area
    r("layer.align.leftEdges", r##"{"to": "canvas"}"##, "area,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.align.horizontalCenters: layer selected; editor area,selected; reads image.size,others
    // was held less: editor:area
    r("layer.align.horizontalCenters", r##"{"to": "canvas"}"##, "area,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.align.rightEdges: layer selected; editor area,selected; reads image.size,others
    // was held less: editor:area
    r("layer.align.rightEdges", r##"{"to": "canvas"}"##, "area,selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.topEdges: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.topEdges", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.verticalCenters: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.verticalCenters", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.bottomEdges: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.bottomEdges", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.leftEdges: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.leftEdges", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.horizontalCenters: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.horizontalCenters", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.rightEdges: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.rightEdges", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.horizontally: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.horizontally", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.distribute.vertically: layer selected; editor selected; reads others
    // was held less: editor:selection
    r("layer.distribute.vertically", r##"{}"##, "selection", &[("$P", "*", true), ("image", "~linkGroups", false)]),
    // layer.linkLayers: layer selected; editor selected; reads others
    // was held less: $P not held
    r("layer.linkLayers", r##"{}"##, "selection", &[("$P", "extra", false)]),
    // layer.mergeLayers: layer ignored; editor active,selected; reads image.mode,others
    // was held less: $P not held
    r("layer.mergeLayers", r##"{}"##, "selection", &[("$P", "*", true)]),
    // layer.new.groupFromLayers: layer id-else-active; editor selected; reads others
    // was held less: $P not held; resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("layer.new.groupFromLayers", r##"{"name": "Pair"}"##, "selection", &[("$P", "*", true)]),
    // layer.arrange.reverse: layer selected; editor selected; reads others
    // was held less: $P not held
    r("layer.arrange.reverse", r##"{}"##, "selection", &[("$P", "*", true)]),
    // layer.lockLayers: layer ignored; editor selected; reads others
    // was held less: $P not held
    r("layer.lockLayers", r##"{"position": true}"##, "selection", &[("$P", "locks", false)]),
    // layer.renameLayer: layer id-else-active; editor active; reads -
    // was held more: $P: every field held, reads name; $P: position held
    r("layer.renameLayer", r##"{"layer": "$P", "name": "Paint 2"}"##, "active", &[("$P", "name", false)]),
    // layer.copyToDocument: layer active-only; editor prefs; reads -
    r("layer.copyToDocument", r##"{"document": 1, "layers": ["$P"]}"##, "", &[("$P", "*", true)]),
    // layer.stampVisible: layer active-only; editor active; reads composite,image.mode,others
    // was held less: $P not held; image not held
    r(
        "layer.stampVisible",
        r##"{}"##,
        "active",
        &[("$P", "clipped,kind", true), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // layer.stampDown: layer ignored; editor active,selected; reads T.attr,others
    // was held less: editor:active; $P not held; $FILL not held
    r("layer.stampDown", r##"{}"##, "active,selection", &[("$FILL", "*", true), ("$P", "*", true)]),
    // layer.maskAllObjects: layer id-else-active; editor active; reads image.size
    r("layer.maskAllObjects", r##"{"layer": "$P"}"##, "active", &[("$P", "*", true)]),
    // layer.quickExportAsPng: layer id-else-active; editor active; reads T.content,T.effects,T.vmask,image.size
    r("layer.quickExportAsPng", r##"{"layer": "$SH", "path": "/tmp/s.png"}"##, "active", &[("$SH", "*", true)]),
    // layer.exportAs: layer id-else-active; editor active; reads T.content,T.effects,T.vmask,image.size
    r("layer.exportAs", r##"{"layer": "$SH", "path": "/tmp/s.jpg", "scale": 50}"##, "active", &[("$SH", "*", true)]),
    // layer.setLabelColor: layer id-else-selected; editor selected; writes every descendant label
    // was held less: editor:selection
    // was held more: $G: every field held, reads props.extra; $G: position held
    r(
        "layer.setLabelColor",
        r##"{"layer": "$G", "color": "red"}"##,
        "active,selection",
        &[("$G", "extra.label", false), ("image", "~subtreeLabels[id=\"$G\"].labels", false)],
    ),
    // layer.new.artboard: layer none; editor -; reads image.size
    r("layer.new.artboard", r##"{"preset": "iPhone 14"}"##, "", &[]),
    // layer.new.artboardFromGroup: layer id-else-active; editor -; reads image.size,others
    r("layer.new.artboardFromGroup", r##"{"layer": "$G"}"##, "active", &[("$G", "*", true)]),
    // layer.new.artboardFromLayers: layer ignored; editor selected; reads image.size,others
    // was held less: $P not held
    r("layer.new.artboardFromLayers", r##"{"name": "AB"}"##, "selection", &[("$P", "*", true)]),
    // layer.artboard.set: layer id-else-active; editor active; reads T.content
    // was held less: $P not held
    r("layer.artboard.set", r##"{"x": 10, "width": 300}"##, "active", &[("$P", "*", true)]),
    // layer.newLayerBasedSlice: layer id-else-active; editor active,prefs; reads T.effects,T.pixels,T.position,docState.slices
    // was held less: image not held
    r("layer.newLayerBasedSlice", r##"{"layer": "$SH", "name": "btn"}"##, "active", &[("$SH", "*", true), ("image", "docState.slices", false)]),
    // layer.new.frameFromLayers: layer other; editor selected; reads -
    // was held less: $P not held
    r("layer.new.frameFromLayers", r##"{"name": "Frame 1"}"##, "selection", &[("$P", "*", true)]),
    // layer.setEffectsVisible: layer id-strict; editor active; reads T.effects
    // was held more: $SH: every field held, reads effects; $SH: position held
    r("layer.setEffectsVisible", r##"{"layer": "$SH", "index": 0, "visible": false}"##, "active", &[("$SH", "styleSettings,styles", false)]),
    // type.create: layer active-only,ignored; editor active,colors,library; reads image.dpi,image.mode,others
    // was held less: $P not held
    r("type.create", r##"{"x": 20, "y": 60, "text": "Hello", "size": 24}"##, "active,colors", &[("$P", "clipped,kind", true)]),
    // was held less: $P not held
    r("type.create", r##"{"box": [10, 10, 200, 80], "text": "Para"}"##, "active,colors", &[("$P", "clipped,kind", true)]),
    // type.edit: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.edit", r##"{"layer": "$T", "text": "New title"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.edit", r##"{"layer": "$T", "kernPair": {"at": 1, "by": 20}}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.setStyle: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.setStyle", r##"{"layer": "$T", "range": [0, 3], "size": 30, "align": "center"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.rasterize: layer id-else-active; editor active; reads T.content,image.mode
    r("type.rasterize", r##"{"layer": "$T"}"##, "active", &[("$T", "*", true)]),
    // shape.create: layer ignored; editor active,colors; reads image.mode,image.size,others
    // was held less: editor:colors; editor:active; $P not held
    r("shape.create", r##"{"kind": "rect", "rect": [10, 10, 40, 20], "fill": "#ff0000"}"##, "active,colors", &[("$P", "clipped,kind", true)]),
    // was held less: editor:colors; editor:active
    r("shape.create", r##"{"kind": "ellipse", "rect": [0, 0, 20, 20], "addTo": "$SH", "op": "subtract"}"##, "active,colors", &[("$SH", "*", true)]),
    // shape.edit: layer id-else-active; editor active,colors; reads T.content,image.mode,image.size
    // was held less: editor:colors
    // was held more: $SH: every field held, reads kind,name,props.data; $SH: position held
    r("shape.edit", r##"{"layer": "$SH", "rect": [20, 20, 60, 40], "fill": "#00ff00"}"##, "active,colors", &[("$SH", "kind,name,data,text", false)]),
    // shape.rasterize: layer id-else-active; editor active; reads T.content,image.mode,image.size
    r("shape.rasterize", r##"{"layer": "$SH"}"##, "active", &[("$SH", "*", true)]),
    // path.set: layer none; editor -; reads image.paths
    // was held less: image: docState.clipping_path not held
    r(
        "path.set",
        r##"{"name": "Path 1", "path": {"subpaths": [{"knots": [[0, 0], [10, 0], [10, 10]]}]}}"##,
        "",
        &[("image", "docState.clipping_path,paths,docState.work_path", false)],
    ),
    // path.delete: layer none; editor -; reads image.paths
    // was held less: image: docState.clipping_path not held
    r("path.delete", r##"{"name": "Path 1"}"##, "", &[("image", "docState.clipping_path,paths,docState.work_path", false)]),
    // path.transform: layer id-else-active,ignored; editor active; reads T.content,T.vmask,image.paths,image.size
    r("path.transform", r##"{"name": "Path 1", "translateX": 5}"##, "active", &[("image", "paths,docState.work_path", false)]),
    // was held more: $SH: every field held, reads kind,props.data,props.extra; $SH: position held
    r(
        "path.transform",
        r##"{"name": "layer", "layer": "$SH", "scaleX": 2}"##,
        "active",
        &[("$SH", "kind,data,text,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // path.clippingPath.set: layer none; editor -; reads image.paths
    // was held less: image: docState.clipping_path not held
    r("path.clippingPath.set", r##"{"name": "Path 1", "flatness": 2}"##, "", &[("image", "docState.clipping_path,paths,docState.work_path", false)]),
    // path.clippingPath.clear: layer none; editor -; reads image.paths
    // was held less: image not held
    r("path.clippingPath.clear", r##"{}"##, "", &[("image", "docState.clipping_path,paths,docState.work_path", false)]),
    // path.style.copyFill: layer ignored; editor active; reads others
    r("path.style.copyFill", r##"{}"##, "", &[]),
    // path.style.copyStroke: layer ignored; editor active; reads others
    r("path.style.copyStroke", r##"{}"##, "", &[]),
    // path.style.pasteFill: layer id-else-active; editor clipboard; reads T.content,image.size
    r("path.style.pasteFill", r##"{"layer": "$SH"}"##, "active", &[("$SH", "*", true)]),
    // path.style.pasteStroke: layer id-else-active; editor clipboard; reads T.content,image.size
    r("path.style.pasteStroke", r##"{"layer": "$SH"}"##, "active", &[("$SH", "*", true)]),
    // path.rename: layer none; editor -; reads image.paths
    // was held less: image: docState.clipping_path not held
    r("path.rename", r##"{"name": "Path 1", "to": "Outline"}"##, "", &[("image", "docState.clipping_path,~pathContents[name=\"Path 1\"].name", false)]),
    // was held less: image: docState.clipping_path not held
    r("path.rename", r##"{"name": "work", "to": "Saved"}"##, "", &[("image", "docState.clipping_path,paths,docState.work_path", false)]),
    // path.toSelection: layer ignored; editor active,area; reads image.paths,image.size
    r("path.toSelection", r##"{"name": "Path 1", "mode": "add", "feather": 2}"##, "", &[("image", "paths,docState.work_path", false)]),
    // path.fill: layer id-else-active; editor active,colors; reads T.locks,T.pixels,image.paths,image.size
    // was held less: editor:colors
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "path.fill",
        r##"{"name": "Path 1", "layer": "$P", "color": "#0000ff"}"##,
        "active,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "paths,docState.work_path", false)],
    ),
    // path.stroke: layer id-else-active; editor active,area,colors,tool; reads T.locks,T.pixels,image.paths
    // was held less: editor:colors; editor:area
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "path.stroke",
        r##"{"name": "Path 1", "layer": "$P", "tool": "pencil"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "paths,docState.work_path", false)],
    ),
    // path.moveAnchors: layer id-else-active; editor active; reads T.content,T.vmask,image.paths,image.size
    r("path.moveAnchors", r##"{"name": "Path 1", "anchors": [[0, 1]], "move": [5, 0]}"##, "active", &[("image", "paths,docState.work_path", false)]),
    // was held more: $SH: every field held, reads kind,props.data,props.extra; $SH: position held
    r(
        "path.moveAnchors",
        r##"{"name": "layer", "anchors": [[0, 1]], "move": [5, 0], "layer": "$SH"}"##,
        "active",
        &[("$SH", "kind,data,text,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // path.moveHandle: layer id-else-active; editor active; reads T.content,T.vmask,image.paths,image.size
    r(
        "path.moveHandle",
        r##"{"name": "Path 1", "subpath": 0, "knot": 1, "handle": "out", "to": [20, 5]}"##,
        "active",
        &[("image", "paths,docState.work_path", false)],
    ),
    // was held more: $SH: every field held, reads kind,props.data,props.extra; $SH: position held
    r(
        "path.moveHandle",
        r##"{"name": "layer", "subpath": 0, "knot": 1, "handle": "out", "to": [20, 5], "layer": "$SH"}"##,
        "active",
        &[("$SH", "kind,data,text,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // path.bendSegment: layer id-else-active; editor active; reads T.content,T.vmask,image.paths,image.size
    r("path.bendSegment", r##"{"name": "Path 1", "subpath": 0, "knot": 0, "move": [0, 5]}"##, "active", &[("image", "paths,docState.work_path", false)]),
    // was held more: $SH: every field held, reads kind,props.data,props.extra; $SH: position held
    r(
        "path.bendSegment",
        r##"{"name": "layer", "subpath": 0, "knot": 0, "move": [0, 5], "layer": "$SH"}"##,
        "active",
        &[("$SH", "kind,data,text,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // path.convertPoint: layer id-else-active; editor active; reads T.content,T.vmask,image.paths,image.size
    r("path.convertPoint", r##"{"name": "Path 1", "subpath": 0, "knot": 1}"##, "active", &[("image", "paths,docState.work_path", false)]),
    // was held more: $SH: every field held, reads kind,props.data,props.extra; $SH: position held
    r(
        "path.convertPoint",
        r##"{"name": "layer", "subpath": 0, "knot": 1, "layer": "$SH"}"##,
        "active",
        &[("$SH", "kind,data,text,extra", false), ("image", "paths,docState.work_path", false)],
    ),
    // type.antiAlias.none: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.none", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.antiAlias.sharp: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.sharp", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.antiAlias.crisp: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.crisp", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.antiAlias.strong: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.strong", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.antiAlias.smooth: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.smooth", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.antiAlias.windowsLcd: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.windowsLcd", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.antiAlias.windows: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.antiAlias.windows", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.orientation.horizontal: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.orientation.horizontal", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.orientation.vertical: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.orientation.vertical", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.standardLigatures: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.standardLigatures", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.contextualAlternates: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.contextualAlternates", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.discretionaryLigatures: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.discretionaryLigatures", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.swash: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.swash", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.oldstyle: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.oldstyle", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.stylisticAlternates: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.stylisticAlternates", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.titlingAlternates: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.titlingAlternates", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.ornaments: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.ornaments", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.ordinals: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.ordinals", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.openType.fractions: layer id-else-active; editor active; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.openType.fractions", r##"{"layer": "$T", "on": true}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.createWorkPath: layer id-else-active; editor -; reads T.content,image.dpi
    r("type.createWorkPath", r##"{"layer": "$T"}"##, "active", &[("$T", "*", true)]),
    // type.convertToShape: layer id-else-active; editor -; reads T.content,image.dpi,image.mode,image.size
    r("type.convertToShape", r##"{"layer": "$T"}"##, "active", &[("$T", "*", true)]),
    // type.rasterizeTypeLayer: layer id-else-active; editor -; reads T.content,image.mode
    r("type.rasterizeTypeLayer", r##"{"layer": "$T"}"##, "active", &[("$T", "*", true)]),
    // type.convertToParagraphText: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.convertToParagraphText", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.convertToPointText: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.convertToPointText", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.warpText: layer id-else-active; editor -; reads T.attr,T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.warpText", r##"{"layer": "$T", "style": "arc", "bend": 30}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.updateAllTextLayers: layer ignored; editor -; reads image.dpi,image.mode,others
    // was held less: image not held
    r("type.updateAllTextLayers", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // type.replaceAllMissingFonts: layer none; editor -; reads image.dpi,image.mode,others
    // was held less: image not held
    r("type.replaceAllMissingFonts", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // type.resolveMissingFonts: layer none; editor -; reads image.dpi,others
    // was held less: image not held
    r(
        "type.resolveMissingFonts",
        r##"{"map": {"Old Font": "Inter"}}"##,
        "",
        &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // was held less: image not held
    r("type.resolveMissingFonts", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // type.pasteLoremIpsum: layer id-else-active,ignored; editor active,colors,library; reads T.attr,T.content,image.dpi,image.mode,image.size,others
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.pasteLoremIpsum", r##"{"layer": "$T", "at": 3}"##, "active,colors", &[("$T", "kind,name,data,text", false)]),
    // was held less: $P not held
    r("type.pasteLoremIpsum", r##"{"new": true}"##, "active,colors", &[("$P", "kind,name,data,text", false)]),
    // type.saveDefaultTypeStyles: layer ignored; editor active; reads others
    r("type.saveDefaultTypeStyles", r##"{}"##, "active", &[]),
    // type.loadDefaultTypeStyles: layer id-else-active; editor library; reads T.attr,T.content,image.dpi
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.loadDefaultTypeStyles", r##"{"layer": "$T"}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // type.characterStyle.new: layer id-else-active; editor active,selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: editor:selection
    r(
        "type.characterStyle.new",
        r##"{"name": "Head", "attrs": {"size": 36}, "layer": "$T"}"##,
        "active,selection",
        &[("$T", "*", true), ("image", "docState.text_styles", false)],
    ),
    // type.characterStyle.duplicate: layer none; editor -; reads docState.text_styles,image.dpi,image.mode,others
    r("type.characterStyle.duplicate", r##"{"id": 1}"##, "", &[("image", "docState.text_styles", false)]),
    // type.characterStyle.delete: layer none; editor -; reads docState.text_styles,image.dpi,others
    r("type.characterStyle.delete", r##"{"id": 1}"##, "", &[("image", "docState.text_styles", false)]),
    // type.characterStyle.rename: layer none; editor -; reads docState.text_styles,image.dpi,image.mode,others
    r("type.characterStyle.rename", r##"{"id": 1, "name": "Body"}"##, "", &[("image", "docState.text_styles", false)]),
    // type.characterStyle.set: layer none; editor -; reads docState.text_styles,image.dpi,image.mode,others
    r("type.characterStyle.set", r##"{"id": 1, "attrs": {"size": 20}}"##, "", &[("image", "docState.text_styles", false)]),
    // type.characterStyle.apply: layer id-else-active; editor selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r(
        "type.characterStyle.apply",
        r##"{"id": 1, "layer": "$T", "range": [0, 5]}"##,
        "selection",
        &[("$T", "*", true), ("image", "docState.text_styles", false)],
    ),
    // type.characterStyle.redefine: layer id-else-active; editor selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: editor:selection
    r("type.characterStyle.redefine", r##"{"layer": "$T"}"##, "active,selection", &[("$T", "*", true), ("image", "docState.text_styles", false)]),
    // type.characterStyle.clearOverride: layer id-else-active; editor selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("type.characterStyle.clearOverride", r##"{"layer": "$T"}"##, "selection", &[("$T", "*", true), ("image", "docState.text_styles", false)]),
    // type.paragraphStyle.new: layer id-else-active; editor active,selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: editor:selection
    r(
        "type.paragraphStyle.new",
        r##"{"name": "Head", "attrs": {"size": 36}, "layer": "$T"}"##,
        "active,selection",
        &[("$T", "*", true), ("image", "docState.text_styles", false)],
    ),
    // type.paragraphStyle.duplicate: layer none; editor -; reads docState.text_styles,image.dpi,image.mode,others
    r("type.paragraphStyle.duplicate", r##"{"id": 1}"##, "", &[("image", "docState.text_styles", false)]),
    // type.paragraphStyle.delete: layer none; editor -; reads docState.text_styles,image.dpi,others
    r("type.paragraphStyle.delete", r##"{"id": 1}"##, "", &[("image", "docState.text_styles", false)]),
    // type.paragraphStyle.rename: layer none; editor -; reads docState.text_styles,image.dpi,image.mode,others
    r("type.paragraphStyle.rename", r##"{"id": 1, "name": "Body"}"##, "", &[("image", "docState.text_styles", false)]),
    // type.paragraphStyle.set: layer none; editor -; reads docState.text_styles,image.dpi,image.mode,others
    r("type.paragraphStyle.set", r##"{"id": 1, "attrs": {"size": 20}}"##, "", &[("image", "docState.text_styles", false)]),
    // type.paragraphStyle.apply: layer id-else-active; editor selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r(
        "type.paragraphStyle.apply",
        r##"{"id": 1, "layer": "$T", "range": [0, 5]}"##,
        "selection",
        &[("$T", "*", true), ("image", "docState.text_styles", false)],
    ),
    // type.paragraphStyle.redefine: layer id-else-active; editor selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("type.paragraphStyle.redefine", r##"{"layer": "$T"}"##, "selection", &[("$T", "*", true), ("image", "docState.text_styles", false)]),
    // type.paragraphStyle.clearOverride: layer id-else-active; editor selected; reads docState.text_styles,image.dpi,image.mode,others
    // was held less: resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r("type.paragraphStyle.clearOverride", r##"{"layer": "$T"}"##, "selection", &[("$T", "*", true), ("image", "docState.text_styles", false)]),
    // type.insertText: layer id-else-active; editor -; reads T.content,image.dpi,image.mode
    // was held more: $T: every field held, reads kind,name,props.data; $T: position held
    r("type.insertText", r##"{"layer": "$T", "text": "!", "at": 2}"##, "active", &[("$T", "kind,name,data,text", false)]),
    // shape.presets.place: layer active-only; editor active,colors,library; reads image.mode,image.size,others
    // was held less: editor:colors; editor:active; $P not held
    r("shape.presets.place", r##"{"preset": "Cloud", "rect": [10, 10, 100, 80], "fill": "#ff0000"}"##, "active,colors", &[("$P", "clipped,kind", true)]),
    // shape.presets.new: layer none; editor active,library; reads image.paths
    r("shape.presets.new", r##"{"name": "Mine", "path": "Path 1"}"##, "", &[("image", "paths,docState.work_path", false)]),
    // shape.presets.edit: layer none; editor library; reads -
    r("shape.presets.edit", r##"{"action": "rename", "preset": "Cloud", "name": "Cloud 2"}"##, "", &[]),
    // shape.presets.reset: layer none; editor library; reads -
    r("shape.presets.reset", r##"{}"##, "", &[]),
    // edit.undo: layer none; editor -; reads -
    r("edit.undo", r##"{}"##, "", &[]),
    // edit.redo: layer none; editor -; reads -
    r("edit.redo", r##"{}"##, "", &[]),
    // edit.fill: layer id-else-active; editor area,channeltarget,colors,library,prefs,quickmask; reads T.locks,T.mask,T.pixels,docState.patterns,image.channels,image.size
    r(
        "edit.fill",
        r##"{"layer": "$P", "contents": "color", "color": "#ff0000", "mode": "multiply", "opacity": 50}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "edit.fill",
        r##"{"contents": "pattern", "pattern": "pat1", "scale": 100, "angle": 0}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,patterns,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "edit.fill",
        r##"{"contents": "history", "state": 0}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "edit.fill",
        r##"{"layer": "$P", "contents": "smart", "colorAdaptation": true, "seed": 1}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.clear: layer id-else-active; editor area,colors; reads T.attr,T.locks,T.pixels,T.position,image.size
    // was held less: editor:colors
    r(
        "edit.clear",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.transform: layer id-else-active; editor area,quickmask; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,T.vmask,image.dpi,others
    // was held less: editor:area
    r("edit.transform", r##"{"layer": "$P", "matrix": [1, 0, 0, 1, 10, 0]}"##, "active,area", &[("$P", "*", true)]),
    // was held less: editor:area; $P not held
    r("edit.transform", r##"{"target": {"channel": 0}, "matrix": [1, 0, 0, 1, 10, 0]}"##, "active,area", &[("$P", "*", true)]),
    // select.float: layer ignored,none; editor active,area,colors; reads T.attr,T.content,T.locks,T.pixels,T.position
    // was held less: $P not held
    r("select.float", r##"{"dx": 5, "dy": 0}"##, "active", &[("$P", "*", true)]),
    // was held less: $P not held
    r("select.float", r##"{"dx": 5}"##, "active", &[("$P", "*", true)]),
    // select.drop: layer none; editor area; reads T.pixels
    r("select.drop", r##"{}"##, "", &[]),
    // select.toWorkPath: layer none; editor area; reads image.size
    r("select.toWorkPath", r##"{"tolerance": 2}"##, "", &[]),
    // select.convertToShape: layer active-only; editor active,area,colors; reads image.size
    // was held less: $P not held
    r("select.convertToShape", r##"{"fill": "#00ff00", "name": "Sel Shape"}"##, "active,area,colors", &[("$P", "*", true)]),
    // edit.cut: layer active-only; editor active,area,colors; reads T.attr,T.content,T.locks,T.pixels
    // was held less: editor:colors; $P not held; image not held
    r("edit.cut", r##"{}"##, "active,area,colors", &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)]),
    // edit.paste: layer active-only,ignored; editor active,channeltarget,clipboard; reads image.mode,image.size
    // was held less: editor:active; $P not held
    r("edit.paste", r##"{"center": [200, 150]}"##, "active", &[("$P", "clipped,kind", true)]),
    // was held less: editor:active; $P not held
    r("edit.paste", r##"{"target": "mask"}"##, "active", &[("$P", "clipped,kind", true)]),
    // edit.pasteSpecial.pasteInPlace: layer active-only,none; editor active,channeltarget,clipboard; reads image.mode
    // was held less: editor:active; $P not held
    r("edit.pasteSpecial.pasteInPlace", r##"{}"##, "active", &[("$P", "clipped,kind", true)]),
    // was held less: editor:active; $P not held
    r(
        "edit.pasteSpecial.pasteInPlace",
        r##"{"target": {"channel": 0}}"##,
        "active",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // edit.toggleLastState: layer none; editor -; reads -
    r("edit.toggleLastState", r##"{}"##, "", &[]),
    // edit.transform.again: layer active-only; editor active,area; reads T.pixels
    // was held less: editor:area; $P not held
    r("edit.transform.again", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.transform.againCopy: layer active-only; editor active,area; reads T.attr
    // was held less: editor:area; $P not held
    r("edit.transform.againCopy", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.assignProfile: layer none; editor prefs; reads image.mode
    // was held less: image not held
    r("edit.assignProfile", r##"{"profile": "display-p3"}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // edit.convertToProfile: layer none; editor prefs; reads image.mode,others
    // was held less: image not held
    r(
        "edit.convertToProfile",
        r##"{"profile": "srgb", "intent": "relative", "bpc": true}"##,
        "",
        &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)],
    ),
    // edit.stroke: layer id-else-active; editor area,colors; reads T.locks,T.pixels,image.size
    // was held less: $P not held; image not held
    r(
        "edit.stroke",
        r##"{"width": 3, "color": "#000000", "location": "outside"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.transform.rotate180: layer selected; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,T.vmask
    // was held less: $P not held
    r("edit.transform.rotate180", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.transform.rotate90Cw: layer selected; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,T.vmask
    // was held less: $P not held
    r("edit.transform.rotate90Cw", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.transform.rotate90Ccw: layer selected; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,T.vmask
    // was held less: $P not held
    r("edit.transform.rotate90Ccw", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.transform.flipHorizontal: layer selected; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,T.vmask
    // was held less: $P not held
    r("edit.transform.flipHorizontal", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.transform.flipVertical: layer selected; editor active,area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,T.vmask
    // was held less: $P not held
    r("edit.transform.flipVertical", r##"{}"##, "active,area", &[("$P", "*", true)]),
    // edit.pasteSpecial.pasteInto: layer none; editor active,area,channeltarget,clipboard; reads image.mode
    // was held less: editor:area; editor:active; $P not held
    r("edit.pasteSpecial.pasteInto", r##"{}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; editor:active; $P not held
    r("edit.pasteSpecial.pasteInto", r##"{"target": "mask"}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // edit.pasteSpecial.pasteOutside: layer none; editor active,area,channeltarget,clipboard; reads image.mode
    // was held less: editor:area; editor:active; $P not held
    r("edit.pasteSpecial.pasteOutside", r##"{}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // was held less: editor:area; editor:active; $P not held
    r("edit.pasteSpecial.pasteOutside", r##"{"target": "mask"}"##, "active,area", &[("$P", "clipped,kind", true)]),
    // edit.checkSpelling: layer id-else-active,none; editor active,prefs; reads T.content,image.dpi,others
    // was held less: $P not held
    r("edit.checkSpelling", r##"{"action": "list"}"##, "active", &[("$P", "*", true)]),
    r(
        "edit.checkSpelling",
        r##"{"action": "change", "layer": "$T", "start": 0, "end": 5, "word": "Tilte", "replace": "Title"}"##,
        "active",
        &[("$T", "*", true)],
    ),
    // was held less: $P not held
    r("edit.checkSpelling", r##"{"action": "changeAll", "word": "Tilte", "replace": "Title"}"##, "active", &[("$P", "*", true)]),
    // was held less: $P not held
    r("edit.checkSpelling", r##"{"action": "addToDictionary", "word": "Kition"}"##, "active", &[("$P", "*", true)]),
    // edit.fade: layer ignored; editor -; reads -
    r("edit.fade", r##"{"opacity": 50, "mode": "normal"}"##, "", &[]),
    // edit.smartFill: layer active-only,ignored; editor active,area; reads T.locks,T.pixels,image.channels,image.size
    // was held less: $P not held; image not held
    r(
        "edit.smartFill",
        r##"{"sampling": "auto"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: $P not held; image not held
    r(
        "edit.smartFill",
        r##"{"output": "new"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.deleteAndFillSelection: layer active-only; editor active,area; reads T.locks,T.pixels
    // was held less: $P not held; image not held
    r(
        "edit.deleteAndFillSelection",
        r##"{}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.smartScale: layer ignored; editor active,area; reads T.content,T.locks,T.pixels,image.channels
    // was held less: editor:active; $P not held; image not held
    r(
        "edit.smartScale",
        r##"{"scaleX": 80}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.defineBrushPreset: layer none; editor active,area,library; reads T.pixels
    // was held less: $P not held
    r("edit.defineBrushPreset", r##"{"name": "Tip"}"##, "active,area", &[("$P", "*", true)]),
    // edit.defineCustomShape: layer none; editor active,library; reads image.paths
    r("edit.defineCustomShape", r##"{"name": "Mine", "path": "Path 1"}"##, "", &[("image", "paths,docState.work_path", false)]),
    // edit.findAndReplaceText: layer none; editor active; reads image.dpi,others
    // was held less: image not held
    r(
        "edit.findAndReplaceText",
        r##"{"find": "Title", "replace": "Heading"}"##,
        "",
        &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // was held less: image not held
    r(
        "edit.findAndReplaceText",
        r##"{"find": "Title", "replace": "Heading", "action": "changeFind"}"##,
        "",
        &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)],
    ),
    // edit.presets.presetManager: layer none; editor library; reads -
    r("edit.presets.presetManager", r##"{"action": "list"}"##, "", &[]),
    r("edit.presets.presetManager", r##"{"action": "rename", "kind": "patterns", "name": "Dots", "newName": "Spots"}"##, "", &[]),
    // edit.presets.exportImportPresets: layer none; editor library; reads -
    r("edit.presets.exportImportPresets", r##"{"action": "export"}"##, "", &[]),
    r("edit.presets.exportImportPresets", r##"{"action": "import", "data": {"format": "kition-photo-presets", "version": 1}}"##, "", &[]),
    // edit.fillForeground: layer id-else-active; editor area,channeltarget,colors; reads T.locks,T.pixels
    // was held less: image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "edit.fillForeground",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.fillBackground: layer id-else-active; editor area,channeltarget,colors; reads T.locks,T.pixels
    // was held less: image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "edit.fillBackground",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.fillForegroundPreserve: layer id-else-active; editor area,channeltarget,colors; reads T.locks,T.pixels
    // was held less: image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "edit.fillForegroundPreserve",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.fillBackgroundPreserve: layer id-else-active; editor area,channeltarget,colors; reads T.locks,T.pixels
    // was held less: image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "edit.fillBackgroundPreserve",
        r##"{"layer": "$P"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // edit.autoAlignLayers: layer selected; editor active,selected; reads image.size,others
    // was held less: $P not held
    r("edit.autoAlignLayers", r##"{"projection": "auto"}"##, "selection", &[("$P", "*", true)]),
    // edit.autoBlendLayers: layer selected; editor active,selected; reads others
    // was held less: $P not held
    r("edit.autoBlendLayers", r##"{"method": "stack", "seamlessTones": true}"##, "selection", &[("$P", "*", true)]),
    // select.saveSelection: layer none; editor area; reads image.channels,image.size
    // was held less: image not held
    r("select.saveSelection", r##"{"name": "Sel 1"}"##, "", &[("image", "channels,~channelContents", false)]),
    r("select.saveSelection", r##"{"channel": "Alpha 1", "operation": "add"}"##, "", &[("image", "channels,~channelContents", false)]),
    // channel.new: layer none; editor area; reads image.channels,image.size
    // was held less: editor:area; image not held
    r("channel.new", r##"{"name": "Alpha 2", "fill": "black"}"##, "area", &[("image", "channels,~channelContents", false)]),
    // channel.newSpot: layer none; editor area; reads image.channels,image.size
    // was held less: editor:area; image not held
    r("channel.newSpot", r##"{"name": "Spot Color 1", "color": "#00ff00", "solidity": 20}"##, "area", &[("image", "channels,~channelContents", false)]),
    // channel.duplicate: layer none; editor active,area,quickmask; reads image.size
    r("channel.duplicate", r##"{"channel": "Alpha 1"}"##, "", &[("image", "channels,~channelContents", false)]),
    r("channel.duplicate", r##"{"channel": "Alpha 1", "document": 1}"##, "", &[("image", "channels,~channelContents", false)]),
    r("channel.duplicate", r##"{"channel": "Alpha 1", "document": "new"}"##, "", &[("image", "channels,~channelContents", false)]),
    // channel.delete: layer none; editor channeltarget; reads image.channels
    r("channel.delete", r##"{"channel": "Alpha 1"}"##, "", &[("image", "channels,~channelContents", false)]),
    // channel.rename: layer none; editor -; reads image.channels
    r("channel.rename", r##"{"channel": "Alpha 1", "name": "Mask A"}"##, "", &[("image", "~channelContents[name=\"Alpha 1\"].name", false)]),
    // channel.move: layer none; editor -; reads image.channels
    r("channel.move", r##"{"channel": "Alpha 1", "to": 0}"##, "", &[("image", "channels,~channelContents", false)]),
    // channel.setVisible: layer none; editor -; reads image.channels
    r("channel.setVisible", r##"{"channel": "Alpha 1", "visible": true}"##, "", &[("image", "channels,~channelContents", false)]),
    // channel.options: layer none; editor prefs,quickmask; reads image.channels
    r("channel.options", r##"{"channel": "Alpha 1", "opacity": 60, "color": "#0000ff"}"##, "", &[("image", "channels,~channelContents", false)]),
    r("channel.options", r##"{"channel": "quickMask", "opacity": 40}"##, "", &[("image", "channels,~channelContents", false)]),
    // channel.mergeSpot: layer none; editor channeltarget; reads composite,image.channels,image.mode
    // was held less: image: depth,docState.global_light,docState.patterns,height,icc,layers,mode,width not held
    r(
        "channel.mergeSpot",
        r##"{"channel": "Spot Color 1"}"##,
        "",
        &[("image", "channels,depth,docState.global_light,patterns,height,icc,layers,mode,width,~channelContents", false)],
    ),
    // channel.split: layer none; editor -; reads composite,image.channels,image.mode,image.size
    // was held less: image not held
    r("channel.split", r##"{"closeOriginal": true}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // channel.merge: layer none; editor -; reads -
    r("channel.merge", r##"{"mode": "rgb"}"##, "", &[]),
    // edit.definePattern: layer none; editor area,library; reads composite,image.mode
    r("edit.definePattern", r##"{"name": "Red"}"##, "area", &[]),
    // edit.transform.warp: layer id-else-active; editor area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position,image.size,others
    // was held less: editor:area
    r("edit.transform.warp", r##"{"layer": "$P", "style": "flag", "bend": 50}"##, "active,area", &[("$P", "*", true)]),
    // was held less: editor:area
    r("edit.transform.warp", r##"{"layer": "$SO", "style": "arc", "bend": 30}"##, "active,area", &[("$SO", "*", true)]),
    // edit.transform.splitWarpCrosswise: layer ignored,none; editor active; reads T.content
    // was held less: $P not held
    r("edit.transform.splitWarpCrosswise", r##"{"style": "custom", "rect": [0, 0, 100, 100], "at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // was held less: $P not held
    r("edit.transform.splitWarpCrosswise", r##"{"at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // edit.transform.splitWarpHorizontally: layer ignored,none; editor active; reads T.content
    // was held less: $P not held
    r("edit.transform.splitWarpHorizontally", r##"{"style": "custom", "rect": [0, 0, 100, 100], "at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // was held less: $P not held
    r("edit.transform.splitWarpHorizontally", r##"{"at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // edit.transform.splitWarpVertically: layer ignored,none; editor active; reads T.content
    // was held less: $P not held
    r("edit.transform.splitWarpVertically", r##"{"style": "custom", "rect": [0, 0, 100, 100], "at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // was held less: $P not held
    r("edit.transform.splitWarpVertically", r##"{"at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // edit.transform.removeWarpSplit: layer ignored,none; editor active; reads T.content
    // was held less: editor:active; $P not held
    r("edit.transform.removeWarpSplit", r##"{"style": "custom", "rect": [0, 0, 100, 100], "at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // was held less: editor:active; $P not held
    r("edit.transform.removeWarpSplit", r##"{"at": [30, 20]}"##, "active", &[("$P", "*", true)]),
    // edit.transform.warpGrid: layer ignored,none; editor active; reads T.content
    // was held less: editor:active; $P not held
    r("edit.transform.warpGrid", r##"{"style": "custom", "rect": [0, 0, 100, 100], "size": 3}"##, "active", &[("$P", "*", true)]),
    // was held less: editor:active; $P not held
    r("edit.transform.warpGrid", r##"{"size": 4}"##, "active", &[("$P", "*", true)]),
    // edit.pinWarp: layer id-else-active; editor area; reads T.attr,T.content,T.locks,T.pixels,T.position,image.size
    // was held less: editor:area
    r("edit.pinWarp", r##"{"layer": "$P", "pins": [{"src": [60, 60], "dst": [70, 65]}]}"##, "active,area", &[("$P", "*", true)]),
    // was held less: editor:area
    r("edit.pinWarp", r##"{"layer": "$SO", "pins": [{"src": [60, 60], "dst": [70, 65]}]}"##, "active,area", &[("$SO", "*", true)]),
    // edit.perspectiveWarp: layer id-else-active; editor area; reads T.attr,T.content,T.locks,T.mask,T.pixels,T.position
    // was held less: editor:area
    r(
        "edit.perspectiveWarp",
        r##"{"layer": "$P", "planes": [{"src": [[50, 50], [200, 50], [200, 150], [50, 150]], "dst": [[40, 50], [210, 60], [200, 150], [50, 150]]}]}"##,
        "active,area",
        &[("$P", "*", true)],
    ),
    // was held less: editor:area
    r(
        "edit.perspectiveWarp",
        r##"{"layer": "$SO", "planes": [{"src": [[0, 0], [10, 0], [10, 10], [0, 10]], "dst": [[0, 0], [12, 0], [10, 10], [0, 10]]}]}"##,
        "active,area",
        &[("$SO", "*", true)],
    ),
    // paint.stroke: layer id-else-active,ignored; editor active,area,channeltarget,colors,library,quickmask,tool; reads T.attr,T.locks,T.mask,T.pixels,T.position,image.channels
    r(
        "paint.stroke",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "color": "#ff0000", "size": 10}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles@", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    r(
        "paint.stroke",
        r##"{"points": [[10, 10], [40, 40]], "layer": "$C", "target": "mask"}"##,
        "active,area,colors",
        &[("$C", "kind,mask", false), ("image", "depth,height,mode,width", false)],
    ),
    r(
        "paint.stroke",
        r##"{"points": [[10, 10], [40, 40]], "target": {"channel": 0}}"##,
        "active,area,colors",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // gradient.fill.create: layer active-only; editor active,area,colors,library; reads image.mode,image.size,others
    // was held less: editor:colors; editor:area; editor:active; $P not held
    r("gradient.fill.create", r##"{"from": [0, 0], "to": [400, 0], "style": "linear"}"##, "active,area,colors", &[("$P", "clipped,kind", true)]),
    // gradient.fill.set: layer id-else-active; editor colors; reads T.content,T.mask,T.vmask,image.mode,image.size
    // was held less: editor:colors
    // was held more: $FILL: every field held, reads kind,props.data; $FILL: position held
    r("gradient.fill.set", r##"{"layer": "$FILL", "from": [0, 0], "to": [200, 0]}"##, "active,colors", &[("$FILL", "kind,data,text", false)]),
    // gradient.fill.stop: layer id-else-active; editor colors; reads T.content
    // was held less: editor:colors
    // was held more: $FILL: every field held, reads kind,props.data; $FILL: position held
    r("gradient.fill.stop", r##"{"layer": "$FILL", "action": "add", "location": 0.5}"##, "active,colors", &[("$FILL", "kind,data,text", false)]),
    // paint.setSymmetry: layer none; editor -; reads image.size
    r("paint.setSymmetry", r##"{"mode": "vertical"}"##, "", &[]),
    // paint.symmetryFromPath: layer none; editor active; reads image.paths
    // was held less: $P not held
    r("paint.symmetryFromPath", r##"{"name": "Path 1"}"##, "active", &[("$P", "*", true), ("image", "paths,docState.work_path", false)]),
    // paint.symmetryDisable: layer none; editor tool; reads -
    r("paint.symmetryDisable", r##"{}"##, "", &[]),
    // color.profileMismatch: layer none; editor prefs; reads image.mode,others
    // was held less: image not held
    r("color.profileMismatch", r##"{"action": "preserve"}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // was held less: image not held
    r("color.profileMismatch", r##"{"action": "convert"}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // was held less: image not held
    r("color.profileMismatch", r##"{"action": "assignWorking"}"##, "", &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)]),
    // paint.pencil: layer id-else-active; editor active,area,channeltarget,colors,library,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:colors; editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.pencil",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "autoErase": true}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.mixerBrush: layer id-else-active; editor active,area,channeltarget,colors,library,tool; reads T.attr,T.locks,T.pixels,T.position,composite
    // was held less: editor:colors; editor:area; image: ~inheritedLocks not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.mixerBrush",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "sampleAllLayers": true}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width,~inheritedLocks", false)],
    ),
    // paint.colorReplacement: layer id-else-active; editor active,area,colors,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:colors; editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.colorReplacement",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "sampling": "backgroundSwatch"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // brush.presets.save: layer none; editor library,tool; reads -
    r("brush.presets.save", r##"{"name": "My Brush"}"##, "", &[]),
    // brush.presets.update: layer none; editor library,tool; reads -
    r("brush.presets.update", r##"{"name": "My Brush"}"##, "", &[]),
    // brush.presets.delete: layer none; editor library,tool; reads -
    r("brush.presets.delete", r##"{"name": "My Brush"}"##, "", &[]),
    // brush.presets.setCurrent: layer none; editor library; reads -
    r("brush.presets.setCurrent", r##"{"name": "My Brush"}"##, "", &[]),
    // brush.defineFromSelection: layer ignored; editor active,area,library; reads others
    r("brush.defineFromSelection", r##"{"name": "Sampled"}"##, "", &[]),
    // brush.presets.rename: layer none; editor library,tool; reads -
    r("brush.presets.rename", r##"{"name": "My Brush", "newName": "Other"}"##, "", &[]),
    // brush.presets.move: layer none; editor library; reads -
    r("brush.presets.move", r##"{"name": "My Brush", "group": "", "index": 0}"##, "", &[]),
    // brush.presets.moveGroup: layer none; editor library; reads -
    r("brush.presets.moveGroup", r##"{"group": "Imported", "index": 0}"##, "", &[]),
    // brush.presets.renameGroup: layer none; editor library; reads -
    r("brush.presets.renameGroup", r##"{"group": "Imported", "newName": "Mine"}"##, "", &[]),
    // brush.presets.deleteGroup: layer none; editor library,tool; reads -
    r("brush.presets.deleteGroup", r##"{"group": "Imported"}"##, "", &[]),
    // paint.magicEraser: layer ignored; editor active,area,colors; reads T.attr,T.locks,T.pixels,T.position,composite,image.size
    // was held less: editor:colors; editor:area; $P not held; image not held
    r(
        "paint.magicEraser",
        r##"{"x": 100, "y": 100, "tolerance": 32}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.backgroundEraser: layer ignored; editor active,area,colors,library,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:colors; editor:area; $P not held; image not held
    r(
        "paint.backgroundEraser",
        r##"{"points": [[60, 60], [120, 100]], "tolerance": 50}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // brush.presets.importAbr: layer none; editor library,tool; reads -
    r("brush.presets.importAbr", r##"{"path": "/tmp/x.abr", "select": true}"##, "", &[]),
    // gradient.presets.importGrd: layer none; editor library; reads -
    r("gradient.presets.importGrd", r##"{"path": "/tmp/x.grd"}"##, "", &[]),
    // swatches.add: layer none; editor colors,library; reads -
    r("swatches.add", r##"{"color": "#ff0000"}"##, "", &[]),
    // swatches.delete: layer none; editor library; reads -
    r("swatches.delete", r##"{"swatch": "Color Swatch 1"}"##, "", &[]),
    // swatches.rename: layer none; editor library; reads -
    r("swatches.rename", r##"{"swatch": "Color Swatch 1", "name": "Red"}"##, "", &[]),
    // swatches.move: layer none; editor library; reads -
    r("swatches.move", r##"{"swatch": "Red", "position": 0}"##, "", &[]),
    // swatches.newGroup: layer none; editor library; reads -
    r("swatches.newGroup", r##"{"name": "Mine", "swatches": ["Red"]}"##, "", &[]),
    // swatches.renameGroup: layer none; editor library; reads -
    r("swatches.renameGroup", r##"{"group": "Mine", "name": "Ours"}"##, "", &[]),
    // swatches.deleteGroup: layer none; editor library; reads -
    r("swatches.deleteGroup", r##"{"group": "Mine"}"##, "", &[]),
    // swatches.use: layer none; editor library,prefs; reads -
    r("swatches.use", r##"{"swatch": "Red", "target": "background"}"##, "", &[]),
    // swatches.reset: layer none; editor library; reads -
    r("swatches.reset", r##"{}"##, "", &[]),
    // swatches.import: layer none; editor library; reads -
    r("swatches.import", r##"{"path": "/tmp/x.aco"}"##, "", &[]),
    // swatches.export: layer none; editor library; reads -
    r("swatches.export", r##"{"path": "/tmp/out.ase"}"##, "", &[]),
    // paint.cloneStamp: layer active-only,id-else-active; editor active,area,channeltarget,library,tool; reads T.attr,T.locks,T.pixels,T.position,composite
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.cloneStamp",
        r##"{"points": [[120, 100], [140, 110]], "layer": "$P", "offset": [-40, 0]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: editor:area; image: ~inheritedLocks not held
    // was held more: $P2: every field held, reads kind,locks,props.data,tiles; $P2: position held
    r(
        "paint.cloneStamp",
        r##"{"points": [[120, 100]], "layer": "$P2", "offset": [-40, 0], "sampleLayer": "currentAndBelow"}"##,
        "active,area",
        &[("$P2", "kind,locks,data,text,tiles", false), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width,~inheritedLocks", false)],
    ),
    // paint.patternStamp: layer id-else-active; editor active,area,library,tool; reads T.locks,T.pixels,docState.patterns
    // was held less: editor:area; image: depth,height,mode,width,~inheritedLocks not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.patternStamp",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "pattern": "Dots"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,patterns,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.healingBrush: layer id-else-active; editor active,area,channeltarget,library,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.healingBrush",
        r##"{"points": [[120, 100], [140, 110]], "layer": "$P", "offset": [-40, 0]}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.spotHealing: layer id-else-active; editor active,area,channeltarget,tool; reads T.locks,T.pixels,composite,image.size
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.spotHealing",
        r##"{"points": [[100, 100]], "layer": "$P", "type": "smart"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.patch: layer id-else-active; editor active,area; reads T.attr,T.locks,T.pixels,T.position,image.size
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.patch",
        r##"{"offset": [100, 0], "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.smartMove: layer id-else-active; editor active,area; reads T.locks,T.pixels,image.size
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.smartMove",
        r##"{"offset": [100, 0], "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.dodge: layer id-else-active; editor active,area,channeltarget,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.dodge",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "range": "midtones"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.burn: layer id-else-active; editor active,area,channeltarget,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.burn",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "range": "shadows"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.sponge: layer id-else-active; editor active,area,channeltarget,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.sponge",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "mode": "saturate"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.blur: layer id-else-active; editor active,area,channeltarget,tool; reads T.attr,T.locks,T.pixels,T.position,composite
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.blur",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "strength": 50}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.sharpen: layer id-else-active; editor active,area,channeltarget,tool; reads T.attr,T.locks,T.pixels,T.position,composite
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.sharpen",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.smudge: layer id-else-active; editor active,area,channeltarget,colors,tool; reads T.attr,T.locks,T.pixels,T.position,composite
    // was held less: editor:colors; editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.smudge",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "fingerPainting": true}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.historyBrush: layer id-else-active; editor active,area,channeltarget,tool; reads T.attr,T.locks,T.pixels,T.position
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.historyBrush",
        r##"{"points": [[60, 60], [120, 100]], "layer": "$P", "state": 0}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.redEye: layer id-else-active; editor area,channeltarget; reads T.locks,T.pixels
    // was held less: editor:area; image not held
    // was held more: $P: every field held, reads kind,locks,props.data,tiles; $P: position held
    r(
        "paint.redEye",
        r##"{"x": 100, "y": 100, "layer": "$P"}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.bucket: layer active-only,ignored; editor active,area,channeltarget,colors,library; reads T.locks,T.pixels,composite,docState.patterns,image.size
    // was held less: $P not held; image not held
    r(
        "paint.bucket",
        r##"{"x": 100, "y": 100, "tolerance": 32}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: $P not held; image: ~inheritedLocks not held
    r(
        "paint.bucket",
        r##"{"x": 100, "y": 100, "sampleAllLayers": true}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width,~inheritedLocks", false)],
    ),
    // was held less: $P not held; image: depth,height,mode,width,~inheritedLocks not held
    r(
        "paint.bucket",
        r##"{"x": 100, "y": 100, "contents": "pattern", "pattern": "Dots"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,patterns,height,mode,width,~inheritedLocks", false)],
    ),
    // paint.gradient: layer ignored; editor active,area,channeltarget,colors,library; reads T.locks,T.pixels,image.size
    // was held less: editor:colors; editor:area; $P not held; image not held
    r(
        "paint.gradient",
        r##"{"from": [0, 0], "to": [400, 0], "gradient": "Black, White"}"##,
        "active,area,colors",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // pattern.rename: layer none; editor library; reads docState.patterns
    r("pattern.rename", r##"{"pattern": "Dots", "name": "Spots"}"##, "", &[("image", "patterns", false)]),
    // pattern.delete: layer none; editor library; reads -
    r("pattern.delete", r##"{"pattern": "Dots"}"##, "", &[("image", "patterns", false)]),
    // pattern.import: layer none; editor library; reads -
    r("pattern.import", r##"{"path": "/tmp/x.pat"}"##, "", &[]),
    // pattern.export: layer none; editor library; reads docState.patterns
    r("pattern.export", r##"{"path": "/tmp/out.pat", "patterns": ["Dots"]}"##, "", &[("image", "patterns", false)]),
    // brush.texturePattern: layer none; editor library,tool; reads docState.patterns
    r("brush.texturePattern", r##"{"pattern": "Dots", "scale": 50}"##, "", &[("image", "patterns", false)]),
    // gradient.presets.select: layer active-only; editor active,colors,library; reads image.mode,others
    // was held less: editor:colors; editor:active; $P not held
    r("gradient.presets.select", r##"{"preset": "Black, White"}"##, "active,colors", &[("$P", "*", true)]),
    // gradient.presets.stop: layer none; editor colors,library; reads -
    // was held less: editor:colors
    r("gradient.presets.stop", r##"{"action": "add", "location": 0.5}"##, "colors", &[]),
    // gradient.presets.apply: layer active-only; editor active,colors,library; reads image.mode,others
    // was held less: $P not held
    r("gradient.presets.apply", r##"{"preset": "Black, White", "angle": 90}"##, "active,colors", &[("$P", "clipped,kind", true)]),
    // gradient.presets.new: layer none; editor library; reads -
    r("gradient.presets.new", r##"{"name": "Mine"}"##, "", &[]),
    // gradient.presets.edit: layer none; editor library; reads -
    r("gradient.presets.edit", r##"{"action": "rename", "preset": "Black, White", "name": "BW"}"##, "", &[]),
    // gradient.presets.reset: layer none; editor library; reads -
    r("gradient.presets.reset", r##"{}"##, "", &[]),
    // pattern.presets.select: layer active-only; editor active,library; reads docState.patterns,others
    // was held less: editor:active; $P not held
    r("pattern.presets.select", r##"{"pattern": "Dots"}"##, "active", &[("$P", "*", true), ("image", "patterns", false)]),
    // pattern.presets.apply: layer active-only; editor active,area,library; reads docState.patterns,image.paths,others
    // was held less: $P not held
    r("pattern.presets.apply", r##"{"pattern": "Dots", "scale": 100}"##, "active,area", &[("$P", "clipped,kind", true), ("image", "patterns", false)]),
    // pattern.presets.new: layer none; editor area,library; reads composite,image.size
    r("pattern.presets.new", r##"{"name": "Mine"}"##, "area", &[]),
    // pattern.presets.edit: layer none; editor library; reads docState.patterns
    r("pattern.presets.edit", r##"{"action": "rename", "preset": "Dots", "name": "Spots"}"##, "", &[("image", "patterns", false)]),
    r("pattern.presets.edit", r##"{"action": "move", "preset": "Dots", "to": "Textures"}"##, "", &[("image", "patterns", false)]),
    // style.presets.apply: layer id-else-active; editor library,selected; reads T.attr,T.effects,T.locks,docState.patterns,image.mode
    // was held less: editor:selection
    r("style.presets.apply", r##"{"preset": "Drop Shadow", "layer": "$P"}"##, "active,selection", &[("$P", "*", true)]),
    // style.presets.new: layer id-else-active; editor active,library; reads T.attr,T.effects
    r("style.presets.new", r##"{"name": "Mine", "layer": "$T"}"##, "active", &[("$T", "*", true)]),
    // style.presets.edit: layer none; editor library; reads -
    r("style.presets.edit", r##"{"action": "rename", "preset": "Drop Shadow", "name": "DS"}"##, "", &[]),
    // style.presets.reset: layer none; editor library; reads -
    r("style.presets.reset", r##"{}"##, "", &[]),
    // tool.presets.new: layer none; editor colors,library,tool; reads -
    r("tool.presets.new", r##"{"tool": "brush", "name": "Mine", "includeColor": true}"##, "", &[]),
    // tool.presets.select: layer none; editor library,tool; reads -
    r("tool.presets.select", r##"{"preset": "Hard Round 12 px"}"##, "", &[]),
    // tool.presets.edit: layer none; editor library; reads -
    r("tool.presets.edit", r##"{"action": "rename", "preset": "Hard Round 12 px", "name": "Hard 12"}"##, "", &[]),
    // tool.presets.reset: layer none; editor -; reads -
    r("tool.presets.reset", r##"{}"##, "", &[]),
    // cloneSource.select: layer none; editor -; reads -
    r("cloneSource.select", r##"{"index": 1}"##, "", &[]),
    // cloneSource.set: layer id-else-active; editor active,library; reads -
    r("cloneSource.set", r##"{"source": [50, 50], "layer": "$P"}"##, "active", &[("$P", "*", true)]),
    // cloneSource.resetTransform: layer none; editor library; reads -
    r("cloneSource.resetTransform", r##"{}"##, "", &[]),
    // cloneSource.overlay: layer none; editor -; reads -
    r("cloneSource.overlay", r##"{"opacity": 50}"##, "", &[]),
    // file.new: layer none; editor colors; reads -
    r("file.new", r##"{"width": 400, "height": 300, "background": "white"}"##, "", &[]),
    r("file.new", r##"{"width": 400, "height": 300, "background": "backgroundColor"}"##, "", &[]),
    // file.newFromClipboard: layer none; editor clipboard; reads -
    r("file.newFromClipboard", r##"{}"##, "", &[]),
    // view.newGuide: layer none; editor -; reads -
    r("view.newGuide", r##"{"orientation": "vertical", "position": 100}"##, "", &[]),
    // view.moveGuide: layer none; editor -; reads image.guides
    // was held less: image not held
    r("view.moveGuide", r##"{"orientation": "vertical", "index": 0, "position": 120}"##, "", &[("image", "guides.vertical.0", false)]),
    // view.deleteGuide: layer none; editor -; reads image.guides
    // was held less: image not held
    r("view.deleteGuide", r##"{"orientation": "vertical", "index": 0}"##, "", &[("image", "guides", false)]),
    // view.clearGuides: layer none; editor -; reads -
    // was held less: image not held
    r("view.clearGuides", r##"{}"##, "", &[("image", "guides", false)]),
    // file.scripts.deleteAllEmptyLayers: layer none; editor active; reads others
    // was held less: image not held
    r("file.scripts.deleteAllEmptyLayers", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // file.revert: layer none; editor active; reads -
    r("file.revert", r##"{}"##, "", &[]),
    // file.openAs: layer none; editor prefs; reads -
    r("file.openAs", r##"{"path": "/tmp/a.png", "as": "png"}"##, "", &[]),
    // file.placeEmbedded: layer active-only; editor active; reads image.mode,image.size,others
    // was held less: editor:active; $P not held
    r("file.placeEmbedded", r##"{"path": "/tmp/logo.png"}"##, "active", &[("$P", "clipped,kind", true)]),
    // file.placeLinked: layer active-only; editor active; reads image.mode,image.size,others
    // was held less: editor:active; $P not held
    r("file.placeLinked", r##"{"path": "/tmp/logo.png", "fit": false}"##, "active", &[("$P", "clipped,kind", true)]),
    // file.fileInfo: layer none; editor -; reads docState.metadata
    // was held less: image not held
    r("file.fileInfo", r##"{"title": "Poster", "keywords": ["a", "b"]}"##, "", &[("image", "docState.xmp", false)]),
    // was held less: image not held
    r("file.fileInfo", r##"{}"##, "", &[("image", "docState.xmp", false)]),
    // file.automate.fitImage: layer none; editor area,quickmask; reads image.size
    // was held less: image not held
    r("file.automate.fitImage", r##"{"width": 200, "height": 200}"##, "", &[("image", "dpi,height,width", false)]),
    // file.automate.conditionalModeChange: layer none; editor -; reads image.mode
    // was held less: image not held
    r(
        "file.automate.conditionalModeChange",
        r##"{"from": ["rgb"], "to": "grayscale"}"##,
        "",
        &[("image", "depth,docState.color_table,docState.duotone,icc,mode", false)],
    ),
    // file.automate.batch: layer none; editor -; reads -
    r("file.automate.batch", r##"{"steps": [["image.invert", {}]], "input": ["/tmp/in/a.png"], "output": "/tmp/out"}"##, "", &[]),
    // file.scripts.imageProcessor: layer none; editor -; reads -
    r("file.scripts.imageProcessor", r##"{"input": ["/tmp/in/a.png"], "output": "/tmp/out", "width": 100}"##, "", &[]),
    // file.scripts.loadFilesIntoStack: layer none; editor -; reads -
    r("file.scripts.loadFilesIntoStack", r##"{"paths": ["/tmp/a.png", "/tmp/b.png"]}"##, "", &[]),
    r("file.scripts.loadFilesIntoStack", r##"{"paths": ["/tmp/a.png", "/tmp/b.png"], "createObjectLayer": true}"##, "", &[]),
    // file.scripts.flattenAllLayerEffects: layer none; editor -; reads image.mode,image.size,others
    // was held less: image not held
    r("file.scripts.flattenAllLayerEffects", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // file.scripts.flattenAllMasks: layer none; editor -; reads image.mode,image.size,others
    // was held less: image not held
    r("file.scripts.flattenAllMasks", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // view.newGuideLayout: layer none; editor -; reads image.guides,image.size
    // was held less: image not held
    r("view.newGuideLayout", r##"{"columns": 4, "gutter": 10, "margin": 20}"##, "", &[("image", "height,width", false)]),
    // view.newGuidesFromShape: layer id-else-active; editor active; reads image.guides,others
    r("view.newGuidesFromShape", r##"{"layer": "$SH"}"##, "active", &[("$SH", "*", true)]),
    // view.clearCanvasGuides: layer none; editor -; reads image.guides
    // was held less: image not held
    r("view.clearCanvasGuides", r##"{}"##, "", &[("image", "guides", false)]),
    // file.automate.photomerge: layer none; editor -; reads -
    r("file.automate.photomerge", r##"{"useOpenDocuments": true, "layout": "reposition"}"##, "", &[]),
    // file.automate.mergeToHdrPro: layer none; editor -; reads composite
    r("file.automate.mergeToHdrPro", r##"{"useOpenDocuments": true, "exposures": [0, 2, 4], "mode": "8"}"##, "", &[]),
    // file.automate.cropAndStraightenPhotos: layer none; editor active; reads composite,image.dpi,image.mode,image.size
    // was held less: image not held
    r("file.automate.cropAndStraightenPhotos", r##"{}"##, "", &[("image", "depth,docState.global_light,patterns,height,icc,layers,mode,width", false)]),
    // file.automate.lensCorrection: layer none; editor -; reads -
    r("file.automate.lensCorrection", r##"{"input": ["/tmp/a.jpg"], "output": "/tmp/out", "profile": "auto"}"##, "", &[]),
    // layerComp.new: layer none; editor -; reads docState.layer_comps,others
    r(
        "layerComp.new",
        r##"{"name": "Comp 2", "visibility": true, "position": true, "appearance": true}"##,
        "",
        &[("image", "comps,height,layers,width", false)],
    ),
    // layerComp.update: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps,others
    r("layerComp.update", r##"{"comp": 1, "what": "all"}"##, "", &[("image", "comps,height,layers,width", false)]),
    r("layerComp.update", r##"{"comp": "*", "what": "visibility"}"##, "", &[("image", "comps,height,layers,width", false)]),
    // layerComp.apply: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps,others
    r("layerComp.apply", r##"{"comp": "Comp 1"}"##, "", &[("image", "comps,height,layers,width", false)]),
    // layerComp.previous: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps,others
    r("layerComp.previous", r##"{}"##, "", &[("image", "comps,height,layers,width", false)]),
    // layerComp.next: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps,others
    r("layerComp.next", r##"{}"##, "", &[("image", "comps,height,layers,width", false)]),
    // layerComp.delete: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps
    r("layerComp.delete", r##"{"comp": 1}"##, "", &[("image", "comps", false)]),
    // layerComp.duplicate: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps
    r("layerComp.duplicate", r##"{"comp": 1}"##, "", &[("image", "comps", false)]),
    // layerComp.rename: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps
    r("layerComp.rename", r##"{"comp": 1, "name": "Renamed"}"##, "", &[("image", "~compContents[id=1].name", false)]),
    // layerComp.setComment: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps
    r("layerComp.setComment", r##"{"comp": 1, "comment": "note"}"##, "", &[("image", "comps", false)]),
    // layerComp.setOptions: layer none; editor -; reads docState.last_applied_comp,docState.layer_comps
    r("layerComp.setOptions", r##"{"comp": 1, "visibility": false}"##, "", &[("image", "comps", false)]),
    // layerComp.restoreLastDocumentState: layer none; editor -; reads docState.last_document_state,others
    r("layerComp.restoreLastDocumentState", r##"{}"##, "", &[("image", "comps,height,layers,width", false)]),
    // layerComp.updateWarnings: layer none; editor -; reads docState.layer_comps,others
    r("layerComp.updateWarnings", r##"{}"##, "", &[("image", "comps", false)]),
    r("layerComp.updateWarnings", r##"{"clear": true}"##, "", &[("image", "comps", false)]),
    // view.clearSelectedArtboardGuides: layer none; editor active; reads image.guides
    // was held less: image not held
    r("view.clearSelectedArtboardGuides", r##"{}"##, "", &[("image", "guides", false)]),
    // count.add: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.add", r##"{"x": 120, "y": 80}"##, "", &[("image", "docState.measurement", false)]),
    // count.remove: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.remove", r##"{"index": 0, "group": 0}"##, "", &[("image", "docState.measurement", false)]),
    // was held less: image not held
    r("count.remove", r##"{"x": 120, "y": 80, "radius": 6}"##, "", &[("image", "docState.measurement", false)]),
    // count.move: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.move", r##"{"index": 0, "group": 0, "to": [130, 90]}"##, "", &[("image", "docState.measurement", false)]),
    // was held less: image not held
    r("count.move", r##"{"x": 120, "y": 80, "to": [130, 90]}"##, "", &[("image", "docState.measurement", false)]),
    // count.clear: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.clear", r##"{"group": 0}"##, "", &[("image", "docState.measurement", false)]),
    // was held less: image not held
    r("count.clear", r##"{"group": "all"}"##, "", &[("image", "docState.measurement", false)]),
    // count.newGroup: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.newGroup", r##"{"name": "Cells", "color": "#00ff00"}"##, "", &[("image", "docState.measurement", false)]),
    // count.deleteGroup: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.deleteGroup", r##"{"group": 0}"##, "", &[("image", "docState.measurement", false)]),
    // count.setGroup: layer none; editor -; reads docState.measurement
    // was held less: image not held
    r("count.setGroup", r##"{"group": 0, "name": "Cells", "visible": false}"##, "", &[("image", "docState.measurement", false)]),
    // measurementLog.delete: layer none; editor -; reads -
    r("measurementLog.delete", r##"{"all": true}"##, "", &[]),
    // measurementLog.export: layer none; editor -; reads -
    r("measurementLog.export", r##"{"path": "/tmp/m.csv"}"##, "", &[]),
    r("measurementLog.export", r##"{}"##, "", &[]),
    // notes.add: layer none; editor -; reads docState.notes
    // was held less: image not held
    r("notes.add", r##"{"x": 10, "y": 12, "text": "fix this", "author": "QA", "color": "#ff0000"}"##, "", &[("image", "notes", false)]),
    // notes.set: layer none; editor -; reads docState.notes
    // was held less: image not held
    r(
        "notes.set",
        r##"{"index": 0, "text": "fixed", "x": 20}"##,
        "",
        &[("image", "~noteContents.0.popup,~noteContents.0.position,~noteContents.0.text", false)],
    ),
    // notes.delete: layer none; editor -; reads docState.notes
    // was held less: image not held
    r("notes.delete", r##"{"index": 1}"##, "", &[("image", "notes", false)]),
    // was held less: image not held
    r("notes.delete", r##"{"all": true}"##, "", &[("image", "notes", false)]),
    // file.import.notes: layer none; editor -; reads image.size
    r("file.import.notes", r##"{"path": "/tmp/other.psd"}"##, "", &[]),
    // history.goTo: layer none; editor -; reads -
    r("history.goTo", r##"{"state": 1}"##, "", &[]),
    // history.deleteState: layer none; editor -; reads -
    r("history.deleteState", r##"{}"##, "", &[]),
    // history.newDocument: layer none; editor -; reads -
    r("history.newDocument", r##"{"name": "Start"}"##, "", &[]),
    // document.presets.save: layer none; editor colors,library; reads -
    r("document.presets.save", r##"{"name": "Web", "settings": {"width": 800, "height": 600}}"##, "", &[]),
    // document.presets.delete: layer none; editor library; reads -
    r("document.presets.delete", r##"{"name": "Web"}"##, "", &[]),
    // slice.new: layer none; editor prefs; reads docState.slices,image.size
    // was held less: image not held
    r("slice.new", r##"{"rect": [10, 10, 100, 50], "name": "hdr", "url": "https://x"}"##, "", &[("image", "docState.slices", false)]),
    // slice.fromGuides: layer none; editor prefs; reads image.guides,image.size
    // was held less: image not held
    r("slice.fromGuides", r##"{}"##, "", &[("image", "docState.slices,guides", false)]),
    // slice.set: layer none; editor prefs; reads docState.slices
    // was held less: image not held
    r(
        "slice.set",
        r##"{"slice": 1, "url": "https://x", "rect": [0, 0, 50, 50]}"##,
        "",
        &[("image", "docState.slices.list[id=1].id,docState.slices.list[id=1].origin,docState.slices.list[id=1].rect,docState.slices.list[id=1].url", false)],
    ),
    // slice.promote: layer none; editor prefs; reads docState.slices
    // was held less: image not held
    r("slice.promote", r##"{"number": 2}"##, "", &[("image", "docState.slices", false)]),
    // slice.delete: layer none; editor prefs; reads docState.slices
    // was held less: image not held
    r("slice.delete", r##"{"slice": 1}"##, "", &[("image", "docState.slices", false)]),
    // was held less: image not held
    r("slice.delete", r##"{"slices": [1, 2]}"##, "", &[("image", "docState.slices", false)]),
    // slice.divide: layer none; editor prefs; reads docState.slices
    // was held less: image not held
    r("slice.divide", r##"{"slice": 1, "horizontal": 2, "vertical": 3}"##, "", &[("image", "docState.slices", false)]),
    // view.lockSlices: layer none; editor prefs; reads -
    r("view.lockSlices", r##"{"on": true}"##, "", &[]),
    // view.clearSlices: layer none; editor prefs; reads docState.slices
    // was held less: image not held
    r("view.clearSlices", r##"{}"##, "", &[("image", "docState.slices", false)]),
    // file.generate.imageAssets: layer none; editor prefs; reads image.dpi,image.mode,image.size,others
    r("file.generate.imageAssets", r##"{"on": true, "dir": "/tmp/assets"}"##, "", &[]),
    r("file.generate.imageAssets", r##"{"on": false}"##, "", &[]),
    // file.automate.contactSheetII: layer none; editor library,prefs; reads -
    r("file.automate.contactSheetII", r##"{"input": ["/tmp/a.png", "/tmp/b.jpg"], "columns": 2, "rows": 1, "caption": true}"##, "", &[]),
    // file.automate.createDroplet: layer none; editor -; reads -
    r("file.automate.createDroplet", r##"{"path": "/tmp/d", "steps": [["filter.blur.gaussianBlur", {"radius": 2}]]}"##, "", &[]),
    // file.automate.runDroplet: layer none; editor -; reads -
    r("file.automate.runDroplet", r##"{"droplet": "/tmp/d.pcdroplet", "input": ["/tmp/a.png"]}"##, "", &[]),
    // file.scripts.statistics: layer none; editor -; reads -
    r("file.scripts.statistics", r##"{"input": ["/tmp/a.png", "/tmp/b.png"], "mode": "median"}"##, "", &[]),
    // file.scripts.browse: layer none; editor -; reads -
    r("file.scripts.browse", r##"{"steps": [["layer.new.layer", {}]]}"##, "", &[]),
    // file.scripts.scriptEventsManager: layer none; editor prefs; reads -
    r("file.scripts.scriptEventsManager", r##"{"add": {"event": "newDocument", "steps": [["layer.new.layer", {}]]}}"##, "", &[]),
    // file.package: layer none; editor -; reads docState.path,others
    r("file.package", r##"{"dir": "/tmp/pkg", "format": "kphoto"}"##, "", &[]),
    // timeline.create: layer none; editor -; reads docState.timeline
    // was held less: image not held
    r("timeline.create", r##"{"duration": 24, "fps": 24}"##, "", &[("image", "docState.timeline", false)]),
    // timeline.delete: layer none; editor -; reads docState.timeline
    // was held less: image not held
    r("timeline.delete", r##"{}"##, "", &[("image", "docState.timeline", false)]),
    // timeline.setFrame: layer none; editor -; reads docState.timeline
    // was held less: image not held
    r("timeline.setFrame", r##"{"frame": 10}"##, "", &[("image", "docState.timeline", false)]),
    // timeline.nextFrame: layer none; editor -; reads docState.timeline
    // was held less: image not held
    r("timeline.nextFrame", r##"{}"##, "", &[("image", "docState.timeline", false)]),
    // timeline.previousFrame: layer none; editor -; reads docState.timeline
    // was held less: image not held
    r("timeline.previousFrame", r##"{}"##, "", &[("image", "docState.timeline", false)]),
    // timeline.setProps: layer none; editor -; reads docState.timeline
    // was held less: image not held
    r("timeline.setProps", r##"{"fps": 30, "workStart": 2, "workEnd": 10}"##, "", &[("image", "docState.timeline", false)]),
    // file.import.videoFramesToLayers: layer active-only; editor active; reads image.mode
    // was held less: editor:active; $P not held
    r("file.import.videoFramesToLayers", r##"{"path": "/tmp/frames"}"##, "active", &[("$P", "clipped,kind", true)]),
    // file.import.variableDataSets: layer none; editor -; reads docState.variables
    // was held less: image not held
    r("file.import.variableDataSets", r##"{"path": "/tmp/data.csv"}"##, "", &[("image", "docState.variables", false)]),
    // plugin.run: layer id-else-active,none,other; editor area,channeltarget,library,quickmask; reads T.content,T.mask,T.pixels,image.channels,image.size
    // was held less: editor:area; editor:active; $P not held; image not held; resolution: `layer` as a name or "#n" written to the active layer (in-page path)
    r(
        "plugin.run",
        r##"{"id": "org.example.tone", "layer": "$P", "params": {"amount": 50}}"##,
        "active,area",
        &[("$P", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: editor:area; editor:active; $SO not held; image not held
    r(
        "plugin.run",
        r##"{"id": "org.example.tone", "layer": "$SO"}"##,
        "active,area",
        &[("$SO", "kind,locks,data,text,tiles", false), ("image", "depth,height,mode,width,~inheritedLocks", false)],
    ),
    // was held less: editor:area; editor:active; $P not held
    r(
        "plugin.run",
        r##"{"id": "org.example.tone", "target": {"channel": 0}}"##,
        "active,area",
        &[("$P", "-", false), ("image", "channels,depth,height,mode,width,~channelContents", false)],
    ),
    // actions.play: layer none; editor library; reads -
    r("actions.play", r##"{"action": "Action 1", "from": 0}"##, "", &[]),
];
