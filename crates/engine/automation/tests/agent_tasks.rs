//! M11 acceptance: an agent completes 10 scripted edit tasks through the automation session
//! (roadmap M11 DoD), the [`Headless`] that the headless kit (`bindings/headless`) serves.
//!
//! Each task uses only the session's requests (`doc.*`, `engine.execute`) the way an agent would,
//! and checks the outcome through `doc.inspect`, `file.pixel` or the exported files, never
//! engine internals.

use std::io::Write;

use kition_photo_automation::{AuthorizedWorkspace, Headless};
use serde_json::{Value, json};

/// A session with read and write authority over `root` only.
fn connect(root: &std::path::Path) -> Headless {
    Headless::with_workspace(AuthorizedWorkspace::new(Some(root), Some(root)).expect("test workspace"))
}

/// Send one request and return its result, failing the test on an error.
fn tool(h: &mut Headless, method: &str, params: Value) -> Value {
    h.handle(method, params).unwrap_or_else(|e| panic!("{method} failed: {e}"))
}

fn run(c: &mut Headless, id: &str, params: Value) -> Value {
    tool(c, "engine.execute", json!({"command": id, "params": params}))
}

fn inspect(c: &mut Headless) -> Value {
    tool(c, "doc.inspect", json!({}))
}

/// Composite RGBA at (x, y), whatever envelope `engine.execute` wraps the result in.
fn pixel(c: &mut Headless, x: i32, y: i32) -> Vec<f64> {
    let v = run(c, "file.pixel", json!({"x": x, "y": y}));
    let arr = if v.is_array() { v } else { v.get("result").cloned().unwrap_or(v) };
    arr.as_array().unwrap_or_else(|| panic!("pixel: {arr}")).iter().map(|x| x.as_f64().unwrap()).collect()
}

fn top_layer(doc: &Value) -> &Value {
    &doc["layers"][0]
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("pc-agent-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A 64×48 RGB gradient PNG (red ramps across, green down).
fn gradient_png(dir: &std::path::Path) -> &'static str {
    let (w, h) = (64u32, 48u32);
    let mut px = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            px.extend_from_slice(&[(x * 4) as u8, (y * 5) as u8, 128]);
        }
    }
    let img = kition_photo_codecs::Image::from_u8(w, h, kition_photo_codecs::ChannelLayout::Rgb, px).unwrap();
    let path = dir.join("gradient.png");
    std::fs::File::create(&path)
        .unwrap()
        .write_all(&kition_photo_codecs::encode(&img, kition_photo_codecs::Format::Png, &Default::default()).unwrap())
        .unwrap();
    "gradient.png"
}

fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
}

#[test]
fn agent_completes_ten_scripted_tasks() {
    let dir = tmp("tasks");
    let png = gradient_png(&dir);
    let mut c = connect(&dir);

    // 1. Make a title card: type layer with a drop shadow, exported as PNG.
    tool(&mut c, "doc.new", json!({"width": 200, "height": 120, "background": "white"}));
    run(&mut c, "text.add", json!({"x": 20, "y": 70, "text": "Hello", "size": 36}));
    run(&mut c, "layer.layerStyle.dropShadow", json!({"distance": 4, "size": 3}));
    let doc = inspect(&mut c);
    let t = top_layer(&doc);
    assert_eq!(t["kind"], "Type", "task 1: {t}");
    assert_eq!(t["text"]["text"], "Hello", "task 1: {t}");
    assert_eq!(t["effects"]["items"][0]["kind"], "Drop Shadow", "task 1: {t}");
    let out = dir.join("card.png");
    tool(&mut c, "doc.save", json!({"path": "card.png"}));
    let card = kition_photo_codecs::decode(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(card.dimensions(), (200, 120), "task 1");
    tool(&mut c, "doc.close", json!({}));

    // 2. Colour grade a photo non-destructively: Curves brightens the midtones, Vibrance on top.
    tool(&mut c, "doc.open", json!({"path": png}));
    let before = pixel(&mut c, 16, 24);
    run(&mut c, "layer.newAdjustmentLayer.curves", json!({"points": [[0, 0], [128, 200], [255, 255]]}));
    run(&mut c, "layer.newAdjustmentLayer.vibrance", json!({"vibrance": 40, "saturation": 0}));
    let after = pixel(&mut c, 16, 24);
    assert!(after[1] > before[1] + 0.05, "task 2: curves should brighten green {before:?} -> {after:?}");
    let doc = inspect(&mut c);
    let kinds: Vec<&str> = doc["layers"].as_array().unwrap().iter().filter_map(|l| l["kind"].as_str()).collect();
    assert_eq!(kinds, ["Adjustment", "Adjustment", "Pixel"], "task 2");

    // 3. Undo/redo: remove both adjustments, restore one.
    run(&mut c, "edit.undo", json!({}));
    run(&mut c, "edit.undo", json!({}));
    assert!(close(&pixel(&mut c, 16, 24), &before, 1e-6), "task 3: undo should restore the original");
    run(&mut c, "edit.redo", json!({}));
    assert_eq!(inspect(&mut c)["layers"].as_array().unwrap().len(), 2, "task 3: redo restores Curves");
    run(&mut c, "edit.undo", json!({}));

    // 4. Editable blur: object layer + Gaussian Blur, retune the radius, then hide the filter.
    run(&mut c, "layer.objectLayers.convertToObjectLayer", json!({}));
    run(&mut c, "filter.blur.gaussianBlur", json!({"radius": 2}));
    run(&mut c, "layer.editableFilter.setParams", json!({"index": 0, "params": {"radius": 6}}));
    let t = inspect(&mut c);
    let sf = &top_layer(&t)["editableFilters"][0];
    assert_eq!(sf["command"], "filter.blur.gaussianBlur", "task 4: {t}");
    assert_eq!(sf["params"]["radius"].as_f64(), Some(6.0), "task 4: {sf}");
    run(&mut c, "layer.editableFilter.setVisible", json!({"index": 0, "visible": false}));
    assert!(close(&pixel(&mut c, 16, 24), &before, 1.0 / 255.0), "task 4: hiding the filter restores the pixels");

    // 5. Selection → layer mask: an elliptical reveal on a red layer.
    run(&mut c, "layer.new.layer", json!({"name": "Red"}));
    run(&mut c, "edit.selectAll", json!({}));
    run(&mut c, "edit.fill", json!({"color": "#ff0000"}));
    run(&mut c, "select.rect", json!({"x": 16, "y": 12, "width": 32, "height": 24, "ellipse": true}));
    run(&mut c, "layer.layerMask.revealSelection", json!({}));
    assert_eq!(top_layer(&inspect(&mut c))["hasMask"], true, "task 5");
    let inside = pixel(&mut c, 32, 24);
    let outside = pixel(&mut c, 1, 1);
    assert!(inside[0] > 0.99 && inside[1] < 0.01, "task 5: centre is red {inside:?}");
    assert!(outside[0] < 0.1, "task 5: corner shows the photo {outside:?}");
    run(&mut c, "edit.deselectAll", json!({}));

    // 6. Save a selection to a channel and load it back.
    run(&mut c, "select.rect", json!({"x": 4, "y": 6, "width": 20, "height": 10}));
    run(&mut c, "select.saveSelection", json!({"name": "Keep"}));
    run(&mut c, "edit.deselectAll", json!({}));
    assert_eq!(inspect(&mut c)["hasSelection"], false, "task 6");
    run(&mut c, "select.loadSelection", json!({"channel": "Keep"}));
    let doc = inspect(&mut c);
    assert_eq!(doc["selectionBounds"], json!([4, 6, 20, 10]), "task 6: {}", doc["selectionBounds"]);
    assert_eq!(doc["channels"]["alpha"][0]["name"], "Keep", "task 6");
    run(&mut c, "edit.deselectAll", json!({}));
    tool(&mut c, "doc.close", json!({}));

    // 7. Lay out three swatches and align their left edges.
    tool(&mut c, "doc.new", json!({"width": 120, "height": 90, "background": "white"}));
    for (i, x) in [10, 40, 70].into_iter().enumerate() {
        run(&mut c, "layer.new.layer", json!({"name": format!("S{i}")}));
        run(&mut c, "select.rect", json!({"x": x, "y": 10 + 25 * i as i32, "width": 20, "height": 15}));
        run(&mut c, "edit.fill", json!({"color": "#3366cc"}));
    }
    run(&mut c, "edit.deselectAll", json!({}));
    run(&mut c, "select.allLayers", json!({}));
    run(&mut c, "layer.align.leftEdges", json!({}));
    let doc = inspect(&mut c);
    let xs: Vec<i64> = doc["layers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["name"].as_str().is_some_and(|n| n.starts_with('S')))
        .map(|l| l["bounds"][0].as_i64().unwrap())
        .collect();
    assert_eq!(xs, [10, 10, 10], "task 7: {}", doc["layers"]);

    // 8. Export through the capability-scoped document API.
    let layers_dir = dir.join("layers");
    std::fs::create_dir_all(&layers_dir).unwrap();
    tool(&mut c, "doc.save", json!({"path": "layers/swatch.png"}));
    let n = std::fs::read_dir(&layers_dir).unwrap().filter(|e| e.as_ref().is_ok_and(|e| e.path().extension().is_some_and(|x| x == "png"))).count();
    assert_eq!(n, 1, "task 8: capability-scoped export");

    // 9. Resize, then crop to a square.
    run(&mut c, "image.imageSize", json!({"width": 60, "height": 45}));
    run(&mut c, "image.crop", json!({"x": 5, "y": 0, "width": 45, "height": 45}));
    let doc = inspect(&mut c);
    assert_eq!((doc["width"].as_u64(), doc["height"].as_u64()), (Some(45), Some(45)), "task 9");

    // 10. Prepare for print: convert to CMYK and save natively, then reopen.
    run(&mut c, "image.mode.cmyk", json!({"intent": "perceptual"}));
    assert_eq!(inspect(&mut c)["mode"], "Cmyk", "task 10");
    tool(&mut c, "doc.save", json!({"path": "print.kphoto"}));
    tool(&mut c, "doc.close", json!({}));
    tool(&mut c, "doc.open", json!({"path": "print.kphoto"}));
    let doc = inspect(&mut c);
    assert_eq!((doc["mode"].as_str(), doc["width"].as_u64()), (Some("Cmyk"), Some(45)), "task 10");
    assert_eq!(doc["layers"].as_array().unwrap().len(), 4, "task 10");

    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

/// Click just after "Big", insert a word, then bold through the end of the line. Geometry comes
/// from `text.hitTest` / `text.caret` / `text.navigate`; the edit is the existing `text.edit`.
#[test]
fn agent_edits_text_by_position() {
    let dir = tmp("type-caret");
    let mut c = connect(&dir);
    tool(&mut c, "doc.new", json!({"width": 480, "height": 200, "background": "white"}));
    let listed = tool(&mut c, "engine.commands", json!({"filter": "text."}));
    let ids: Vec<&str> = listed.as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
    for id in ["text.hitTest", "text.caret", "text.navigate"] {
        assert!(ids.contains(&id), "engine.commands missing {id}: {ids:?}");
    }

    run(&mut c, "text.add", json!({"x": 40, "y": 90, "text": "Big Sale", "size": 48}));
    let before = inspect(&mut c)["history"].as_array().unwrap().len();
    // The caret after "Big" and the caret after the following space. A quarter of the way is
    // the left half of that space, which hit-tests as the caret after "Big".
    let after_big = run(&mut c, "text.caret", json!({"index": 3}));
    let after_space = run(&mut c, "text.caret", json!({"index": 4}));
    let seg = after_big["segment"].as_array().unwrap();
    let (x3, y0, y1) = (seg[0][0].as_f64().unwrap(), seg[0][1].as_f64().unwrap(), seg[1][1].as_f64().unwrap());
    let x4 = after_space["segment"][0][0].as_f64().unwrap();
    assert!(x4 > x3 + 0.5, "space should have width, caret x {x3} -> {x4}");
    let hit = run(&mut c, "text.hitTest", json!({"x": x3 + 0.25 * (x4 - x3), "y": (y0 + y1) / 2.0}));
    assert_eq!(hit["index"].as_u64(), Some(3), "{hit}");
    assert_eq!(hit["inside"], true, "{hit}");
    let quiet = inspect(&mut c)["history"].as_array().unwrap().len();
    assert_eq!(quiet, before, "caret queries are not history steps");

    run(&mut c, "text.edit", json!({"replace": {"start": hit["index"], "end": hit["index"], "text": " Summer"}}));
    let info = run(&mut c, "text.info", json!({}));
    assert_eq!(info["text"], "Big Summer Sale", "{info}");

    let end = run(&mut c, "text.navigate", json!({"index": 0, "move": "lineEnd"}));
    let n = info["text"].as_str().unwrap().chars().count() as u64;
    assert_eq!(end["index"].as_u64(), Some(n), "{end}");
    // "Summer" starts one character after the insertion caret (the space in " Summer").
    let summer = hit["index"].as_u64().unwrap() + 1;
    run(&mut c, "text.setStyle", json!({"range": [summer, end["index"]], "fauxBold": true}));
    let info = run(&mut c, "text.info", json!({}));
    assert_eq!(info["text"], "Big Summer Sale");
    let bold = info["runs"].as_array().unwrap().iter().find(|r| r["style"]["faux_bold"] == true);
    let bold = bold.unwrap_or_else(|| panic!("no bold run in {info}"));
    assert_eq!(bold["start"].as_u64(), Some(summer), "{bold}");
    assert_eq!(bold["end"].as_u64(), end["index"].as_u64(), "{bold}");

    tool(&mut c, "doc.close", json!({}));
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn agent_records_and_replays_an_action() {
    let dir = tmp("actions");
    let mut c = connect(&dir);
    let listed = tool(&mut c, "engine.commands", json!({"filter": "action."}));
    let ids: Vec<&str> = listed.as_array().unwrap().iter().filter_map(|item| item.get("id").and_then(Value::as_str)).collect();
    for id in ["action.list", "action.get", "action.record", "action.stop", "action.play", "action.delete"] {
        assert!(ids.contains(&id), "{id} missing from {ids:?}");
    }

    run(&mut c, "file.new", json!({"width": 32, "height": 32, "background": "white"}));
    run(&mut c, "action.record", json!({"name": "Red"}));
    run(&mut c, "layer.new.layer", json!({}));
    run(&mut c, "select.rect", json!({"x": 0, "y": 0, "width": 8, "height": 8}));
    run(&mut c, "edit.fill", json!({"color": "#ff0000"}));
    let stopped = run(&mut c, "action.stop", json!({}));
    assert_eq!(stopped["steps"], 3, "{stopped}");
    let got = run(&mut c, "action.get", json!({"action": "Red"}));
    assert_eq!(got["steps"][0][0], "layer.new.layer");
    assert_eq!(got["steps"][1][0], "select.rect");
    assert_eq!(got["steps"][2][0], "edit.fill");

    run(&mut c, "file.new", json!({"width": 32, "height": 32, "background": "white"}));
    let played = run(&mut c, "action.play", json!({"action": "Red"}));
    assert_eq!(played["ran"], 3, "{played}");
    assert!(played.get("failed").is_none(), "{played}");
    let px = pixel(&mut c, 2, 2);
    assert!(close(&px, &[1.0, 0.0, 0.0, 1.0], 1e-4), "{px:?}");

    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn agent_play_refuses_a_recorded_file_open() {
    let dir = tmp("actions-auth");
    let workspace = AuthorizedWorkspace::new(Some(&dir), Some(&dir)).expect("test workspace");
    let mut c = Headless::with_workspace(workspace);
    c.session.actions.list.push(kition_photo_engine::actions_cmds::Action {
        name: "Open".into(),
        steps: vec![("file.open".into(), json!({"path": "/etc/passwd"})), ("layer.new.layer".into(), json!({}))],
    });
    let played = run(&mut c, "action.play", json!({"action": "Open"}));
    assert_eq!(played["ran"], 0, "{played}");
    assert_eq!(played["failed"]["step"], 0, "{played}");
    assert_eq!(played["failed"]["id"], "file.open", "{played}");
    assert!(played["failed"]["error"].as_str().unwrap_or("").contains("disabled"), "{played}");
    let session = tool(&mut c, "session.list", json!({}));
    assert!(session["documents"].as_array().is_some_and(|d| d.is_empty()), "{session}");
    drop(c);
    std::fs::remove_dir_all(dir).unwrap();
}
