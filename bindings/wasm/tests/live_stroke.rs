//! Live strokes (plan 2026-11-24, stage 3; `src/stroke.rs`): a brush-family stroke shows while it
//! is drawn, without changing the document; ending it leaves exactly one history step with the
//! pixels the command itself paints; cancelling leaves the document as it was; hostile input is
//! an error, never a panic. And the Move tool's render that leaves one layer out.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const W: u32 = 160;
const H: u32 = 100;

/// White W x H with a textured "Paint" layer on top (four coloured blocks, so the retouching tools
/// have something to work on) and an empty "Ink" layer above it, active.
fn image() -> (PhotoDocument, u64, u64) {
    let mut doc = PhotoDocument::create("art.psd", W, H).unwrap();
    let paint = doc.execute("layer.new.layer", json!({ "name": "Paint" })).unwrap()["layer"].as_u64().unwrap();
    for (x, y, color) in [(0, 0, "#d04020"), (80, 0, "#2060c0"), (0, 50, "#30a050"), (80, 50, "#e0c040")] {
        doc.execute("select.rect", json!({ "x": x, "y": y, "width": 80, "height": 50 })).unwrap();
        doc.execute("edit.fill", json!({ "color": color })).unwrap();
    }
    doc.execute("edit.deselectAll", json!({})).unwrap();
    let ink = doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap()["layer"].as_u64().unwrap();
    (doc, paint, ink)
}

fn pixels(doc: &PhotoDocument) -> Vec<u8> {
    doc.render(0.0, 0.0, 1.0, W, H).unwrap()
}

fn steps(doc: &PhotoDocument) -> usize {
    doc.inspect().unwrap()["history"].as_array().unwrap().len()
}

/// The stroke's points after the first, as the editor sends them.
const REST: [[f64; 3]; 5] = [[40.0, 30.0, 1.0], [70.0, 60.0, 0.8], [100.0, 40.0, 1.0], [130.0, 70.0, 0.6], [140.0, 20.0, 1.0]];
const FIRST: [f64; 3] = [12.0, 20.0, 1.0];

fn flat(points: &[[f64; 3]]) -> Vec<f64> {
    points.iter().flatten().copied().collect()
}

fn all_points() -> Value {
    let mut all = vec![json!(FIRST)];
    all.extend(REST.iter().map(|p| json!(p)));
    Value::Array(all)
}

/// Draws a live stroke of `command` with `params` (the points come here) on `doc`, ends it, and
/// checks it against the command run directly on a second, identical image. Answers whether the
/// preview just before the release already showed the final pixels.
fn live_matches_the_command(command: &str, params: Value, select_paint: bool) -> bool {
    let (mut live, paint, _) = image();
    let (mut direct, ..) = image();
    let mut params = params;
    if select_paint {
        for doc in [&mut live, &mut direct] {
            doc.execute("layer.select", json!({ "layer": paint })).unwrap();
            if command == "paint.historyBrush" {
                // The History Brush paints back the state before this black stroke.
                params["state"] = doc.inspect().unwrap()["historyIndex"].clone();
                doc.execute("paint.stroke", json!({ "points": [[0, 25], [160, 75]], "size": 40, "color": "#000000" })).unwrap();
            }
        }
    }
    let before = pixels(&live);
    let (revision, history) = (live.revision(), steps(&live));
    let mut begin = params.clone();
    begin["points"] = json!([FIRST]);
    let answer = live.stroke_begin(command, begin).unwrap_or_else(|e| panic!("{command}: {e}"));
    let stroke = u32::try_from(answer["stroke"].as_u64().unwrap()).unwrap();
    for batch in REST.chunks(2) {
        live.stroke_push(stroke, &flat(batch)).unwrap();
    }
    let preview = pixels(&live);
    assert_ne!(preview, before, "{command}: the stroke shows before the release");
    assert_eq!((live.revision(), steps(&live)), (revision, history), "{command}: the document is untouched while the stroke is drawn");
    live.stroke_end(stroke).unwrap();
    assert_eq!(steps(&live), history + 1, "{command}: ending leaves exactly one history step");
    assert!(live.revision() > revision);
    let mut run = params;
    run["points"] = all_points();
    if !answer["seed"].is_null() {
        run["seed"] = answer["seed"].clone();
    }
    direct.execute(command, run).unwrap();
    assert_eq!(pixels(&live), pixels(&direct), "{command}: the same pixels as the command run directly");
    // A stroke that ended is gone.
    assert!(live.stroke_push(stroke, &flat(&REST[..1])).is_err());
    preview == pixels(&live)
}

#[test]
fn a_live_brush_ends_as_paint_stroke() {
    // Soft, smoothed and scattered: the seed and the catch-up tail must reach the commit.
    for params in [
        json!({ "size": 14, "color": "#ff0000" }),
        json!({ "size": 20, "hardness": 0.2, "opacity": 0.6, "smoothing": 0.4, "color": "#0000ff" }),
        json!({ "size": 16, "preset": "Spatter", "color": "#00aa00" }),
    ] {
        assert!(live_matches_the_command("paint.stroke", params.clone(), false), "the brush's preview is its result: {params}");
    }
}

#[test]
fn a_live_eraser_and_pencil_end_as_their_commands() {
    assert!(live_matches_the_command("paint.stroke", json!({ "size": 18, "erase": true }), true));
    assert!(live_matches_the_command("paint.pencil", json!({ "size": 6, "color": "#112233" }), false));
}

#[test]
fn live_retouching_strokes_end_as_their_commands() {
    for (command, params) in [
        ("paint.cloneStamp", json!({ "size": 16, "offset": [60, 30] })),
        ("paint.healingBrush", json!({ "size": 16, "offset": [60, 30] })),
        ("paint.historyBrush", json!({ "size": 16 })),
        ("paint.dodge", json!({ "size": 24, "exposure": 60 })),
        ("paint.burn", json!({ "size": 24 })),
        ("paint.sponge", json!({ "size": 24, "mode": "saturate" })),
        ("paint.blur", json!({ "size": 24, "strength": 80 })),
        ("paint.sharpen", json!({ "size": 24 })),
        ("paint.smudge", json!({ "size": 24, "strength": 70 })),
    ] {
        live_matches_the_command(command, params, true);
    }
}

#[test]
fn cancelling_leaves_the_document_as_it_was() {
    let (mut doc, ..) = image();
    let before = pixels(&doc);
    let (revision, history, dirty) = (doc.revision(), steps(&doc), doc.dirty());
    let answer = doc.stroke_begin("paint.stroke", json!({ "points": [FIRST], "size": 14, "color": "#ff0000" })).unwrap();
    let stroke = u32::try_from(answer["stroke"].as_u64().unwrap()).unwrap();
    let damage = doc.stroke_push(stroke, &flat(&REST)).unwrap();
    assert!(damage[2] > damage[0] && damage[3] > damage[1], "{damage:?}");
    assert_ne!(pixels(&doc), before);
    let area = doc.stroke_cancel(stroke);
    assert!(area[0] <= 5 && area[2] >= 147, "the area to draw again covers the stroke: {area:?}");
    assert_eq!(pixels(&doc), before, "the picture is back");
    assert_eq!((doc.revision(), steps(&doc), doc.dirty()), (revision, history, dirty));
    assert!(doc.stroke_push(stroke, &flat(&REST)).is_err(), "a cancelled stroke takes no more points");
    assert!(doc.stroke_end(stroke).is_err(), "nor ends");
    assert_eq!(doc.stroke_cancel(stroke), [0; 4]);
}

#[test]
fn hostile_input_is_an_error_and_never_a_panic() {
    let (mut doc, ..) = image();
    let before = pixels(&doc);
    // Begin: not a live command, unknown keys, missing or empty points, absurd coordinates.
    for (command, params) in [
        ("filter.blur.gaussianBlur", json!({ "points": [[1, 1]] })),
        ("paint.mixerBrush", json!({ "points": [[1, 1]] })),
        ("paint.spotHealing", json!({ "points": [[1, 1]] })),
        ("no.suchCommand", json!({ "points": [[1, 1]] })),
        ("paint.stroke", json!({ "points": [[1, 1]], "bogus": 1 })),
        ("paint.stroke", json!({})),
        ("paint.stroke", json!({ "points": [] })),
        ("paint.stroke", json!({ "points": "[[1, 1]]" })),
        ("paint.stroke", json!({ "points": [[1e300, 0]] })),
        ("paint.stroke", json!({ "points": [[0, -2_000_000]] })),
        ("paint.stroke", json!({ "points": [[1, 1]], "size": 1e30 })),
        ("paint.stroke", json!({ "points": [[1, 1]], "layer": 9999 })),
        ("paint.stroke", json!([[1, 1]])),
        ("paint.dodge", json!({ "points": [[1, 1]], "range": "sideways" })),
    ] {
        assert!(doc.stroke_begin(command, params.clone()).is_err(), "{command} {params}");
    }
    // A source far off the image is no error (it clones nothing), and no panic either.
    for offset in [json!([2_000_000_000, 0]), json!([-2_147_483_000, 2_147_483_000]), json!([1e300, 0])] {
        if let Ok(answer) = doc.stroke_begin("paint.cloneStamp", json!({ "points": [[1, 1]], "offset": offset })) {
            let stroke = u32::try_from(answer["stroke"].as_u64().unwrap()).unwrap();
            let _ = doc.stroke_push(stroke, &[100.0, 60.0, 1.0]);
            doc.stroke_cancel(stroke);
        }
    }
    // No stroke under way.
    assert!(doc.stroke_push(1, &[1.0, 1.0, 1.0]).is_err());
    assert!(doc.stroke_end(1).is_err());
    assert_eq!(doc.stroke_cancel(1), [0; 4]);
    assert_eq!(pixels(&doc), before);

    let answer = doc.stroke_begin("paint.stroke", json!({ "points": [FIRST], "size": 10, "color": "#000000" })).unwrap();
    let stroke = u32::try_from(answer["stroke"].as_u64().unwrap()).unwrap();
    let shown = pixels(&doc);
    // Points: not triples, not numbers, off the range, too many at once; another stroke's number.
    // `stroke::MAX_PUSH` is a million points.
    let too_many = vec![1.0; 3 * 1_000_001];
    for points in [
        vec![1.0, 2.0],
        vec![f64::NAN, 2.0, 1.0],
        vec![2.0, f64::INFINITY, 1.0],
        vec![1e300, 2.0, 1.0],
        vec![50.0, 50.0, f64::NAN],
        vec![50.0, 50.0, 1.0, 60.0, f64::NEG_INFINITY, 1.0],
        too_many,
    ] {
        assert!(doc.stroke_push(stroke, &points).is_err(), "{:?}", points.get(..6));
    }
    assert!(doc.stroke_push(stroke + 1, &[50.0, 50.0, 1.0]).is_err());
    assert_eq!(pixels(&doc), shown, "a refused call leaves the stroke as it was");
    // An empty call and a pressure out of range are fine.
    doc.stroke_push(stroke, &[]).unwrap();
    doc.stroke_push(stroke, &[80.0, 50.0, 7.0]).unwrap();
    let history = steps(&doc);
    doc.stroke_end(stroke).unwrap();
    assert_eq!(steps(&doc), history + 1);
    // A hidden layer is refused, as the command refuses it.
    let (mut hidden, _, ink) = image();
    hidden.execute("layer.setProps", json!({ "layer": ink, "visible": false })).unwrap();
    assert!(hidden.stroke_begin("paint.stroke", json!({ "points": [[5, 5]] })).is_err());
}

#[test]
fn a_change_under_the_stroke_drops_its_preview() {
    let (mut doc, ..) = image();
    let answer = doc.stroke_begin("paint.stroke", json!({ "points": [FIRST], "size": 14, "color": "#ff0000" })).unwrap();
    let stroke = u32::try_from(answer["stroke"].as_u64().unwrap()).unwrap();
    doc.stroke_push(stroke, &flat(&REST)).unwrap();
    doc.execute("layer.new.layer", json!({ "name": "Other" })).unwrap();
    let now = pixels(&doc);
    let (mut fresh, ..) = image();
    fresh.execute("layer.new.layer", json!({ "name": "Other" })).unwrap();
    assert_eq!(now, pixels(&fresh), "the canvas shows the document, not a stroke on a picture that is gone");
    assert!(doc.stroke_push(stroke, &flat(&REST)).is_err());
    // A new stroke begins as usual and replaces any other.
    let next = doc.stroke_begin("paint.stroke", json!({ "points": [FIRST], "size": 14 })).unwrap();
    let again = doc.stroke_begin("paint.stroke", json!({ "points": [[30, 30]], "size": 14 })).unwrap();
    assert_ne!(next["stroke"], again["stroke"]);
    assert!(doc.stroke_push(u32::try_from(next["stroke"].as_u64().unwrap()).unwrap(), &flat(&REST)).is_err());
}

#[test]
fn a_drag_leaves_its_layer_out_until_the_document_changes() {
    let (mut doc, paint, _) = image();
    let white = PhotoDocument::create("white.psd", W, H).unwrap();
    let full = pixels(&doc);
    doc.omit_layer(Some(u32::try_from(paint).unwrap())).unwrap();
    assert_eq!(pixels(&doc), pixels(&white), "without the Paint layer the picture is the white background");
    assert_eq!(doc.render_area(0, 0, 80, 50, 40, 25).unwrap(), white.render_area(0, 0, 80, 50, 40, 25).unwrap());
    assert!(!doc.inspect().unwrap().to_string().contains("\"visible\":false"), "the layer itself stays visible");
    doc.omit_layer(None).unwrap();
    assert_eq!(pixels(&doc), full);
    // The move's commit changes the document: the layer shows at its new place at once.
    doc.omit_layer(Some(u32::try_from(paint).unwrap())).unwrap();
    doc.execute("layer.select", json!({ "layer": paint })).unwrap();
    doc.execute("layer.translate", json!({ "dx": 10, "dy": 0 })).unwrap();
    let moved = pixels(&doc);
    assert_ne!(moved, pixels(&white));
    assert_ne!(moved, full);
    assert!(doc.omit_layer(Some(9999)).is_err());
}
