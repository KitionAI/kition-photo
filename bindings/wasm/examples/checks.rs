//! Writes the conformance fixtures (KHP `spec/conformance.md`): `basic.psd`, which `photo_check`
//! finds clean, one `check-<code>` file per code of `agent/checks.json`, on which the check reports
//! that code, and `golden/` (the previews of `basic.psd`).
//! `cargo run -p kition-photo-kit --example checks -- conformance`. The images are made with the
//! engine's own commands (and, for states no command makes, its document model) and written by its
//! own PSD and Kition Photo writers, so they are the same on every run.

use std::path::Path;
use std::sync::Arc;

use kition_photo_engine::Session;
use kition_photo_engine::doc::{Document, Layer, LayerContent};
use serde_json::json;

fn image() -> Result<Session, String> {
    let mut session = Session::new();
    session.execute("file.new", json!({ "width": 320, "height": 200, "background": "white" })).map_err(|error| error.to_string())?;
    // Something to see: a filled rectangle on the background.
    session.execute("select.rect", json!({ "x": 40, "y": 40, "width": 160, "height": 100 })).map_err(|error| error.to_string())?;
    session.execute("edit.fill", json!({ "color": "#1f4e8c" })).map_err(|error| error.to_string())?;
    session.execute("select.deselect", json!({})).map_err(|error| error.to_string())?;
    Ok(session)
}

fn run(session: &mut Session, command: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    session.execute(command, params).map_err(|error| format!("{command}: {error}"))
}

fn doc(session: &mut Session) -> Result<&mut Document, String> {
    Ok(Arc::make_mut(&mut session.active_mut().ok_or("no image")?.doc))
}

fn top(session: &mut Session) -> Result<&mut Layer, String> {
    doc(session)?.layers.last_mut().ok_or_else(|| "no layer".to_owned())
}

fn fixtures() -> Result<Vec<(&'static str, Session)>, String> {
    let mut out = vec![("basic.psd", image()?)];

    let mut empty_layer = image()?;
    run(&mut empty_layer, "layer.new.layer", json!({ "name": "Empty" }))?;
    out.push(("check-empty-layer.psd", empty_layer));

    let mut hidden = image()?;
    run(&mut hidden, "layer.new.layer", json!({ "name": "Badge" }))?;
    run(&mut hidden, "select.rect", json!({ "x": 220, "y": 120, "width": 60, "height": 40 }))?;
    run(&mut hidden, "edit.fill", json!({ "color": "#c0392b" }))?;
    run(&mut hidden, "select.deselect", json!({}))?;
    top(&mut hidden)?.visible = false;
    out.push(("check-hidden-layer.psd", hidden));

    let mut off_canvas = image()?;
    run(&mut off_canvas, "layer.new.layer", json!({ "name": "Stray" }))?;
    run(&mut off_canvas, "select.rect", json!({ "x": 10, "y": 10, "width": 30, "height": 30 }))?;
    run(&mut off_canvas, "edit.fill", json!({ "color": "#27ae60" }))?;
    run(&mut off_canvas, "select.deselect", json!({}))?;
    if let LayerContent::Raster(surface) = &top(&mut off_canvas)?.content {
        let moved = surface.translated(900, 0, surface.content_bounds());
        top(&mut off_canvas)?.content = LayerContent::Raster(moved);
    }
    out.push(("check-off-canvas.psd", off_canvas));

    let mut empty_text = image()?;
    let made = run(&mut empty_text, "type.create", json!({ "x": 20, "y": 30, "text": "Title", "size": 18, "color": "#000000" }))?;
    run(&mut empty_text, "type.edit", json!({ "layer": made["layer"], "text": "" }))?;
    out.push(("check-empty-text.kphoto", empty_text));

    let mut substituted = image()?;
    run(&mut substituted, "type.create", json!({ "x": 20, "y": 30, "text": "Title", "size": 18, "color": "#000000", "font": "No Such Font Family 42" }))?;
    out.push(("check-font-substituted.kphoto", substituted));

    // An object layer linked to a picture that is gone: placed from a file that is then removed.
    let mut missing = image()?;
    let picture = std::env::temp_dir().join(format!("kition-photo-fixture-{}.png", std::process::id()));
    image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 60, 40, 255])).save(&picture).map_err(|error| error.to_string())?;
    let placed = run(&mut missing, "file.placeLinked", json!({ "path": picture.to_string_lossy(), "fit": false }));
    let _ = std::fs::remove_file(&picture);
    placed?;
    if let Some(layer) = doc(&mut missing)?.layers.last().map(|layer| layer.id) {
        run(&mut missing, "layer.rename", json!({ "layer": layer.0, "name": "Placed photo" })).ok();
    }
    out.push(("check-missing-object-source.kphoto", missing));

    let mut truncated = image()?;
    for _ in 0..103 {
        run(&mut truncated, "layer.new.layer", json!({}))?;
    }
    out.push(("check-truncated.psd", truncated));
    Ok(out)
}

fn main() -> Result<(), String> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "conformance".into());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    for (name, session) in fixtures()? {
        let state = session.active().ok_or("no image")?;
        // PSD keeps no linked object layer, empty type layer or font name, so those fixtures are
        // in the kit's own format, which keeps the whole document.
        let bytes = if name.ends_with(".kphoto") {
            state.save_kphoto().map_err(|error| error.to_string())?
        } else {
            kition_photo_io::export(&state.doc, name, &kition_photo_io::ExportOptions::default()).map_err(|error| error.to_string())?.bytes
        };
        std::fs::write(dir.join(name), &bytes).map_err(|error| format!("{name}: {error}"))?;
        println!("wrote {name}");
        if name == "basic.psd" {
            golden(dir, &bytes)?;
        }
    }
    Ok(())
}

/// `golden/basic.png` and `golden/basic-scale-2.png`: the engine's own render of `basic.psd` at 1 and
/// 2 pixels per pixel, which `<prefix>_inspect { preview }` must match.
fn golden(dir: &Path, bytes: &[u8]) -> Result<(), String> {
    use kition_kit::KitDocument;
    let doc = kition_photo_kit::PhotoDocument::open("basic.psd", bytes)?;
    let golden = dir.join("golden");
    std::fs::create_dir_all(&golden).map_err(|error| error.to_string())?;
    for (file, scale) in [("basic.png", 1.0), ("basic-scale-2.png", 2.0)] {
        let (width, height) = ((f64::from(doc.width()) * scale) as u32, (f64::from(doc.height()) * scale) as u32);
        let rgba = doc.render(0.0, 0.0, scale, width, height)?;
        let picture = image::RgbaImage::from_raw(width, height, rgba).ok_or("a render of the wrong size")?;
        picture.save(golden.join(file)).map_err(|error| error.to_string())?;
        println!("wrote golden/{file}");
    }
    Ok(())
}
