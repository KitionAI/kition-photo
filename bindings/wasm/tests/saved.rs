//! In memory = saved (task card C-photo): after each of a representative set of agent commands,
//! what the image holds is what its saved file opens as, in both formats an agent saves to (PSD
//! and `.kphoto`). The premise guard compares the document the kit holds with the document it
//! reads from a file another program saved, so every difference between the two would read as a
//! change by others, and a write on a layer that differs only so would be refused. Two things are
//! compared after each command: the object model (the replica, paired as the guard pairs two reads
//! of a file, `kition_kdm::reread::diff_paired`), and the image as inspect lists it without the
//! editor state.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_kit::kdm::reread;
use kition_kit::session::KitModel;
use kition_kit::session::model::Exchange;
use kition_photo_kit::{EDITOR_STATE, PhotoDocument};
use serde_json::{Value, json};

fn state(doc: &mut PhotoDocument) -> Value {
    Exchange::new().replica(doc).unwrap().state_json().unwrap()
}

/// The leaves where two JSON values differ, `path: before -> after`.
fn leaves(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|key| !x.contains_key(*key))) {
                leaves(&format!("{path}.{key}"), x.get(key).unwrap_or(&Value::Null), y.get(key).unwrap_or(&Value::Null), out);
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (index, (p, q)) in x.iter().zip(y).enumerate() {
                leaves(&format!("{path}.{index}"), p, q, out);
            }
        }
        _ if a != b => {
            let cut = |value: &Value| value.to_string().chars().take(160).collect::<String>();
            out.push(format!("{path}: {} -> {}", cut(a), cut(b)));
        }
        _ => {}
    }
}

/// The mismatches between `doc` and its saved file (as `name`) opened again.
fn mismatches(doc: &mut PhotoDocument, name: &str) -> Vec<String> {
    let bytes = doc.save(Some(name)).unwrap();
    let mut again = PhotoDocument::open(name, &bytes).unwrap();
    let schema = kdm_model::PhotoModel::schema();
    let diff = reread::diff_paired(&schema, &state(doc), &state(&mut again)).to_json();
    let mut out = Vec::new();
    for list in ["created", "changed", "moved", "deleted"] {
        for entry in diff[list].as_array().into_iter().flatten() {
            let edits = entry["edits"].to_string().chars().take(300).collect::<String>();
            let edits = if list == "moved" { entry.to_string().chars().take(400).collect::<String>() } else { edits };
            out.push(format!("model {list} {} {} {} {edits}", entry["id"], entry["type"], entry["fields"]));
        }
    }
    let content = |doc: &PhotoDocument| kition_kit::document_content(doc.inspect().unwrap(), EDITOR_STATE);
    let mut image = Vec::new();
    leaves("inspect", &content(doc), &content(&again), &mut image);
    out.extend(image.into_iter().filter(|line| !line.starts_with("inspect.name")).map(|line| format!("engine {line}")));
    out
}

/// The representative commands, in order, on one image: what agents do most.
fn steps() -> Vec<(&'static str, Value)> {
    vec![
        ("layer.new.layer", json!({ "name": "Ink" })),
        ("paint.stroke", json!({ "points": [[10, 10, 1], [300, 200, 0.6]], "size": 12, "color": "#336699" })),
        ("edit.fill", json!({ "color": "#ff8800" })),
        ("layer.setProps", json!({ "opacity": 0.37, "blend": "Multiply", "name": "Renamed" })),
        ("layer.translate", json!({ "dx": 7, "dy": -3 })),
        ("filter.blur.gaussianBlur", json!({ "radius": 2.5 })),
        ("image.adjustments.levels", json!({ "gamma": 1.3 })),
        ("type.create", json!({ "text": "Title", "x": 20, "y": 60, "size": 24, "color": "#202020" })),
        ("shape.create", json!({ "kind": "ellipse", "rect": [200, 40, 80, 60], "fill": "#00aa55" })),
        ("layer.new.group", json!({ "name": "Group" })),
        ("layer.new.layer", json!({ "name": "Inside" })),
        ("paint.stroke", json!({ "points": [[50, 150, 1], [120, 160, 1]], "size": 6, "color": "#aa0000" })),
        ("layer.layerMask.revealAll", json!({})),
        ("layer.layerStyle.dropShadow", json!({ "distance": 5, "size": 4 })),
        ("layer.newAdjustmentLayer.hueSaturation", json!({ "hue": 20 })),
        ("layer.duplicate", json!({})),
        ("layer.arrange.sendBackward", json!({})),
        ("select.rect", json!({ "x": 10, "y": 10, "width": 50, "height": 40 })),
        ("select.saveSelection", json!({ "name": "Saved" })),
        ("select.deselect", json!({})),
        ("view.newGuide", json!({ "orientation": "vertical", "position": 100 })),
        ("layer.delete", json!({})),
        ("image.canvasSize", json!({ "width": 420, "height": 300, "anchor": "center" })),
    ]
}

/// Runs the steps on a new image saved as `name`, and answers every mismatch after each step.
fn run(name: &str) -> Vec<String> {
    let mut doc = PhotoDocument::create(name, 400, 260).unwrap();
    let mut all = Vec::new();
    for line in mismatches(&mut doc, name) {
        all.push(format!("(new) {line}"));
    }
    for (command, params) in steps() {
        doc.execute(command, params.clone()).unwrap_or_else(|error| panic!("{command}: {error}"));
        for line in mismatches(&mut doc, name) {
            all.push(format!("{command}: {line}"));
        }
    }
    all
}

/// A mismatch line without the command, the object's id and the layer's index: what kind of
/// difference it is.
fn kind(line: &str) -> String {
    let rest = line.split_once(": ").map_or(line, |(_, rest)| rest);
    let words: Vec<&str> = rest.splitn(6, ' ').collect();
    match words.as_slice() {
        ["model", list, _id, ty, fields, ..] => format!("model {list} {ty} {fields}"),
        ["engine", path, ..] => {
            let path = path.trim_end_matches(':');
            let parts: Vec<String> = path.split('.').map(|part| if part.parse::<usize>().is_ok() { "*".to_owned() } else { part.to_owned() }).collect();
            format!("engine {}", parts.join("."))
        }
        _ => rest.to_owned(),
    }
}

/// What a PSD does not hold as the image held it after the representative commands, by kind, with
/// why. Wanted: nothing (as `.kphoto`). Each makes the premise guard read the difference as a
/// change by others after a file the kit saved as PSD is changed outside, and refuse a write on
/// that layer (`docs/agent.md`). Fixed (C-photo-2): a layer's opacity and fill opacity enter as the
/// byte PSD stores; an adjustment's own block is kept verbatim only when the export would not write
/// it again byte for byte; a deleted layer's tree node is no longer
/// told (kition-sdk C8-fix-10).
const PSD_KNOWN: &[(&str, &str)] = &[
    (
        r#"model changed "Layer" ["props.data.color.c.0","props.data.color.c.1","props.data.color.c.2","props.data.paragraphs.0.len","props.data.psd_raw","props.data.runs.0.style.color.c.0","props.data.runs.0.style.color.c.1","props.data.runs.0.style.color.c.2","props.data.runs.0.style.postscript_name","props.extra.psd_blocks"]"#,
        "a type layer read from PSD: its colors through the text engine's floats, its paragraph length with the closing return, the font's PostScript name filled in, and the TySh block kept verbatim",
    ),
    (
        r#"model changed "Layer" ["props"]"#,
        "a shape layer read from PSD: its path's anchors go through PSD's fixed-point coordinates (a fraction of the canvas in 8.24, read back through f32), and its SoCo and vmsk blocks are kept verbatim (`props.extra.psd_blocks`, `props.data.psd_raw`)",
    ),
    (
        r#"model changed "Layer" ["effects.psd_raw"]"#,
        "a layer style's lfx2 block kept verbatim when read (crates/formats/io/tests/flat.rs `effects_are_written_to_psd_and_read_back` requires it kept)",
    ),
    (r#"model changed "Image" ["docState.psd_global_blocks"]"#, "the global Txt2 block a PSD with type writes, kept verbatim when read"),
];

#[test]
fn a_psd_wrongly_differs_from_the_image_after_some_commands() {
    let found = run("art.psd");
    for line in &found {
        println!("MISMATCH psd {line}");
    }
    let kinds: std::collections::BTreeSet<String> = found.iter().map(|line| kind(line)).collect();
    let known: std::collections::BTreeSet<String> = PSD_KNOWN.iter().map(|(kind, _)| (*kind).to_owned()).collect();
    assert_eq!(kinds, known, "the kinds of difference a PSD shows changed (fewer is progress: drop them from PSD_KNOWN)");
}

#[test]
fn a_kphoto_holds_what_the_image_held_after_each_command() {
    let found = run("art.kphoto");
    for line in &found {
        println!("MISMATCH kphoto {line}");
    }
    assert!(found.is_empty(), "{} mismatches", found.len());
}

/// The fixture files: each opened, saved in its own format and opened again.
#[test]
fn the_fixture_files_hold_what_they_open_as() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    for file in [
        "conformance/basic.psd",
        "conformance/check-empty-layer.psd",
        "conformance/check-hidden-layer.psd",
        "conformance/check-off-canvas.psd",
        "conformance/check-empty-text.kphoto",
        "conformance/check-font-substituted.kphoto",
        "conformance/check-missing-object-source.kphoto",
        "cases/photo.agent.draft/title-layer/input/cover.psd",
        "web/public/sample/Sample.psd",
    ] {
        let name = file.rsplit('/').next().unwrap();
        let mut doc = PhotoDocument::open(name, &std::fs::read(root.join(file)).unwrap()).unwrap();
        for line in mismatches(&mut doc, name) {
            found.push(format!("{file}: {line}"));
        }
    }
    for line in &found {
        println!("MISMATCH {line}");
    }
    assert!(found.is_empty(), "{} mismatches", found.len());
}
