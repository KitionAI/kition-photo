//! The automation session ([`Headless`]) the headless kit serves: documents, file I/O through the
//! granted workspace, previews and errors, driven by its requests the way an agent drives it.

use std::io::Write;

use base64::Engine as _;
use kition_photo_automation::{AuthorizedWorkspace, Headless};
use serde_json::{Value, json};

fn ok(h: &mut Headless, method: &str, params: Value) -> Value {
    h.handle(method, params).unwrap_or_else(|e| panic!("{method} failed: {e}"))
}

fn err(h: &mut Headless, method: &str, params: Value) -> String {
    match h.handle(method, params) {
        Ok(v) => panic!("{method} succeeded: {v}"),
        Err(e) => e.to_string(),
    }
}

fn run(h: &mut Headless, id: &str, params: Value) -> Value {
    ok(h, "engine.execute", json!({"command": id, "params": params}))
}

fn run_err(h: &mut Headless, id: &str, params: Value) -> String {
    err(h, "engine.execute", json!({"command": id, "params": params}))
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("pc-headless-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn workspace(root: &std::path::Path) -> Headless {
    Headless::with_workspace(AuthorizedWorkspace::new(Some(root), Some(root)).expect("test workspace"))
}

fn write_image(dir: &std::path::Path, name: &str, format: kition_photo_codecs::Format) -> Vec<u8> {
    let img = kition_photo_codecs::Image::from_u8(16, 8, kition_photo_codecs::ChannelLayout::Rgb, (0..384).map(|i| (i * 7 % 251) as u8).collect()).unwrap();
    let bytes = kition_photo_codecs::encode(&img, format, &Default::default()).unwrap();
    std::fs::write(dir.join(name), &bytes).unwrap();
    bytes
}

fn decode_render(reply: &Value) -> kition_photo_codecs::Image {
    assert_eq!(reply["mime"], "image/png", "{reply}");
    let png = base64::engine::general_purpose::STANDARD.decode(reply["base64"].as_str().expect("base64 PNG")).unwrap();
    kition_photo_codecs::decode(&png).unwrap()
}

#[test]
fn edit_render_save_roundtrip() {
    let dir = tmp("edit");
    let mut h = workspace(&dir);
    ok(&mut h, "doc.new", json!({"width": 64, "height": 48, "background": "white", "name": "Agent"}));
    run(&mut h, "layer.new.layer", json!({"name": "Ink"}));
    run(&mut h, "paint.stroke", json!({"points": [[5, 5, 1.0], [50, 40, 1.0]], "size": 6, "color": "#ff0000"}));

    let doc = ok(&mut h, "doc.inspect", json!({}));
    assert_eq!(doc["width"], 64);
    let names: Vec<&str> = doc["layers"].as_array().unwrap().iter().filter_map(|l| l["name"].as_str()).collect();
    assert_eq!(names, ["Ink", "Background"]);

    let preview = decode_render(&ok(&mut h, "doc.render", json!({"maxSide": 32})));
    assert_eq!(preview.dimensions(), (32, 24));

    assert_eq!(ok(&mut h, "doc.save", json!({"path": "agent.kphoto"}))["path"], "agent.kphoto");
    ok(&mut h, "doc.save", json!({"path": "agent.png"}));
    ok(&mut h, "doc.save", json!({"path": "agent.jpg", "quality": 70}));
    assert!(kition_photo_codecs::decode(&std::fs::read(dir.join("agent.png")).unwrap()).is_ok());
    assert!(std::fs::metadata(dir.join("agent.jpg")).unwrap().len() > 100);

    // Re-open the native file: identical layer tree.
    assert_eq!(ok(&mut h, "doc.open", json!({"path": "agent.kphoto"}))["index"], 1);
    assert_eq!(ok(&mut h, "doc.inspect", json!({"index": 1}))["layers"].as_array().unwrap().len(), 2);
    assert_eq!(ok(&mut h, "session.list", json!({}))["documents"].as_array().unwrap().len(), 2);
    ok(&mut h, "doc.close", json!({"index": 1}));
    ok(&mut h, "doc.select", json!({"index": 0}));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn command_list_filters_and_reports_enabled() {
    let mut h = Headless::new();
    let all = ok(&mut h, "engine.commands", json!({}));
    let n_all = all.as_array().unwrap().len();
    assert!(n_all > 20);
    let blur = ok(&mut h, "engine.commands", json!({"filter": "blur"}));
    assert!(!blur.as_array().unwrap().is_empty() && blur.as_array().unwrap().len() < n_all);
    assert!(blur.as_array().unwrap().iter().all(|c| c["params"].is_string()));
    let enabled = |id: &str| all.as_array().unwrap().iter().find(|c| c["id"] == id).map(|c| c["enabled"].clone());
    assert_eq!(enabled("file.new"), Some(json!(true)));
    assert_eq!(enabled("layer.new.layer"), Some(json!(false)), "needs a document");
}

#[test]
fn errors_are_errors_not_crashes() {
    let mut h = Headless::new();
    assert!(run_err(&mut h, "no.such.command", json!({})).contains("unknown command"));
    err(&mut h, "doc.inspect", json!({"index": 0}));
    err(&mut h, "doc.open", json!({"path": "/definitely/missing.png"}));
    err(&mut h, "doc.save", json!({}));
    // The session still serves.
    assert_eq!(ok(&mut h, "session.list", json!({}))["documents"], json!([]));
}

#[test]
fn open_png_and_inspect() {
    let dir = tmp("open");
    let img = kition_photo_codecs::Image::from_u8(8, 4, kition_photo_codecs::ChannelLayout::Rgb, vec![200; 96]).unwrap();
    std::fs::File::create(dir.join("in.png"))
        .unwrap()
        .write_all(&kition_photo_codecs::encode(&img, kition_photo_codecs::Format::Png, &Default::default()).unwrap())
        .unwrap();
    let mut h = workspace(&dir);
    let o = ok(&mut h, "doc.open", json!({"path": "in.png"}));
    assert_eq!((o["width"].as_u64(), o["height"].as_u64()), (Some(8), Some(4)));
    let px = run(&mut h, "document.pixel", json!({"x": 1, "y": 1}));
    assert!(px.to_string().contains("0.78"), "{px}");
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

/// #518, #523: `doc.open` returns why an opened image is incomplete.
#[test]
fn open_reports_decode_warnings() {
    let dir = tmp("open-warnings");
    let jpeg = write_image(&dir, "whole.jpg", kition_photo_codecs::Format::Jpeg);
    std::fs::write(dir.join("cut.jpg"), &jpeg[..jpeg.len() - 4]).unwrap();
    // Two 1x1 frames.
    let mut gif = b"GIF89a\x01\0\x01\0\x80\0\0\0\0\0\xFF\xFF\xFF".to_vec();
    for _ in 0..2 {
        gif.extend_from_slice(b"\x2C\0\0\0\0\x01\0\x01\0\0\x02\x02\x44\x01\0");
    }
    gif.push(0x3B);
    std::fs::write(dir.join("anim.gif"), gif).unwrap();
    let mut h = workspace(&dir);
    let mut warnings = |path: &str| ok(&mut h, "doc.open", json!({"path": path}))["warnings"].clone();
    assert_eq!(warnings("whole.jpg"), json!([]));
    assert_eq!(warnings("cut.jpg"), json!(["JPEG data ends early (the file is truncated or damaged); part of the image is missing"]));
    assert_eq!(warnings("anim.gif"), json!(["only the first of 2 frames was imported"]));
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn affinity_preview_warns_and_requires_a_new_save_path() {
    let dir = tmp("affinity-preview");
    let png = write_image(&dir, "preview.png", kition_photo_codecs::Format::Png);
    // Synthetic v12 envelope; the native document graph is intentionally absent.
    let mut bytes = vec![0; 72];
    bytes[..4].copy_from_slice(b"\x00\xffKA");
    bytes[4..6].copy_from_slice(&12u16.to_le_bytes());
    bytes[8..12].copy_from_slice(b"nsrP");
    bytes[12..16].copy_from_slice(b"#Inf");
    bytes[24..32].copy_from_slice(&72u64.to_le_bytes());
    bytes[64..68].copy_from_slice(b"Prot");
    bytes.extend(b"\xff\xff\xff\xffThmb");
    bytes.extend(1u32.to_le_bytes());
    bytes.extend((png.len() as u32 + 13).to_le_bytes());
    bytes.extend(29u32.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend((png.len() as u32).to_le_bytes());
    bytes.push(1);
    bytes.extend(png);
    std::fs::write(dir.join("source.af"), &bytes).unwrap();
    // A renamed layered extension must not bypass source protection.
    std::fs::write(dir.join("renamed.psd"), &bytes).unwrap();
    let mut h = workspace(&dir);
    for name in ["source.af", "renamed.psd"] {
        let opened = ok(&mut h, "doc.open", json!({"path": name}));
        assert!(opened["warnings"].to_string().contains("only its embedded 16×8 PNG preview"));
        assert_eq!(opened["width"], 16);
        let refused = err(&mut h, "doc.save", json!({}));
        assert!(refused.contains("pass `path`"), "{refused}");
        assert!(err(&mut h, "doc.save", json!({"path": "source.af"})).contains("Affinity export"));
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), bytes);
    }
    ok(&mut h, "doc.save", json!({"path": "copy.kphoto"}));
    assert_eq!(ok(&mut h, "doc.save", json!({}))["path"], "copy.kphoto");
    assert_eq!(std::fs::read(dir.join("source.af")).unwrap(), bytes);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

/// A save without `path` writes back only to a layered file in its own format (#416).
#[test]
fn save_without_path_never_flattens_over_the_opened_file() {
    let dir = tmp("save-in-place");
    let png = write_image(&dir, "seed.png", kition_photo_codecs::Format::Png);
    let jpg = write_image(&dir, "seed.jpg", kition_photo_codecs::Format::Jpeg);
    let mut h = workspace(&dir);
    let refused = |h: &mut Headless, params: Value| {
        let e = err(h, "doc.save", params);
        assert!(e.contains("pass `path`"), "{e}");
    };

    // A flat file, edited or not, is left unchanged.
    ok(&mut h, "doc.open", json!({"path": "seed.png"}));
    run(&mut h, "layer.newAdjustmentLayer.curves", json!({"points": [[0, 0], [128, 170], [255, 255]]}));
    refused(&mut h, json!({}));
    assert_eq!(std::fs::read(dir.join("seed.png")).unwrap(), png);
    ok(&mut h, "doc.open", json!({"path": "seed.jpg"}));
    refused(&mut h, json!({}));
    assert_eq!(std::fs::read(dir.join("seed.jpg")).unwrap(), jpg);

    // An explicit path, even the opened file's own, still writes and reports what was lost.
    let r = ok(&mut h, "doc.save", json!({"path": "seed.jpg"}));
    assert!(r["warnings"].to_string().contains("lossy"), "{r}");

    // A layered file saves in place in its own format, but not converted over itself.
    ok(&mut h, "doc.select", json!({"index": 0}));
    ok(&mut h, "doc.save", json!({"path": "layered.psd"}));
    ok(&mut h, "doc.open", json!({"path": "layered.psd"}));
    let psd = std::fs::read(dir.join("layered.psd")).unwrap();
    refused(&mut h, json!({"format": "png"}));
    assert_eq!(std::fs::read(dir.join("layered.psd")).unwrap(), psd);
    assert_eq!(ok(&mut h, "doc.save", json!({}))["path"], "layered.psd");
    assert!(kition_photo_io::is_psd(&std::fs::read(dir.join("layered.psd")).unwrap()));

    // Once saved as .kphoto, a flat-born document saves in place there.
    ok(&mut h, "doc.select", json!({"index": 0}));
    ok(&mut h, "doc.save", json!({"path": "work.kphoto"}));
    assert_eq!(ok(&mut h, "doc.save", json!({}))["path"], "work.kphoto");
    assert_eq!(std::fs::read(dir.join("seed.png")).unwrap(), png);

    // A template opens untitled, without its path.
    std::fs::write(dir.join("card.psdt"), &psd).unwrap();
    assert_eq!(ok(&mut h, "doc.open", json!({"path": "card.psdt"}))["name"], "Untitled-1");
    refused(&mut h, json!({}));
    assert_eq!(std::fs::read(dir.join("card.psdt")).unwrap(), psd);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn filesystem_policy_rejects_absolute_and_escaping_paths_before_effects() {
    let base = tmp("filesystem-policy");
    let root = base.join("workspace");
    let outside = base.join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    write_image(&root, "inside.png", kition_photo_codecs::Format::Png);
    let mut h = workspace(&root);

    ok(&mut h, "doc.open", json!({"path": "inside.png"}));
    let absolute = outside.join("absolute.png");
    let e = err(&mut h, "doc.save", json!({"path": absolute.to_string_lossy()}));
    assert!(e.contains("automation path rejected"), "{e}");
    assert!(!absolute.exists());

    for path in ["../outside/traversal.png", "..\\outside\\mixed.png", "C:/outside/prefix.png", ""] {
        err(&mut h, "doc.save", json!({"path": path}));
    }
    assert!(!outside.join("traversal.png").exists());
    assert!(!outside.join("mixed.png").exists());

    let e = run_err(&mut h, "layer.objectLayers.exportContents", json!({"path": "outside.bin"}));
    assert!(e.contains("ambient filesystem paths"), "{e}");
    drop(h);
    std::fs::remove_dir_all(base).unwrap();
}

/// The template workflow stays in memory, including saving nested contents back to the parent.
#[test]
fn embedded_object_layers_can_be_edited_saved_and_unpacked_without_file_authority() {
    let mut h = Headless::new();
    ok(&mut h, "doc.new", json!({"width": 16, "height": 12, "background": "white"}));
    run(&mut h, "layer.objectLayers.convertToObjectLayer", json!({}));
    let outer = run(&mut h, "layer.objectLayers.convertToObjectLayer", json!({}))["layer"].clone();
    // An explicit target works even with a regular pixel layer selected.
    run(&mut h, "layer.new.layer", json!({"name": "Other layer"}));
    assert_eq!(run(&mut h, "layer.objectLayers.editContents", json!({"layer": outer}))["document"], 1);
    assert_eq!(run(&mut h, "layer.objectLayers.editContents", json!({}))["document"], 2);
    run(&mut h, "layer.setProps", json!({"name": "Edited photo", "opacity": 0.5}));
    assert_eq!(run(&mut h, "layer.objectLayers.saveContents", json!({}))["updated"], true);
    ok(&mut h, "doc.close", json!({}));
    run(&mut h, "layer.objectLayers.saveContents", json!({}));
    ok(&mut h, "doc.close", json!({}));
    assert_eq!(ok(&mut h, "session.list", json!({}))["documents"].as_array().unwrap().len(), 1);

    // Reopening the saved nested source proves both in-memory parent updates survived closing.
    run(&mut h, "layer.objectLayers.editContents", json!({"layer": outer}));
    run(&mut h, "layer.objectLayers.editContents", json!({}));
    let doc = ok(&mut h, "doc.inspect", json!({}));
    assert_eq!(doc["layers"][0]["name"], "Edited photo");
    assert_eq!(doc["layers"][0]["opacity"], 0.5);
    ok(&mut h, "doc.close", json!({}));
    ok(&mut h, "doc.close", json!({}));
    let unpacked = run(&mut h, "layer.objectLayers.convertToLayers", json!({"layer": outer}))["layer"].clone();
    run(&mut h, "layer.objectLayers.convertToLayers", json!({"layer": unpacked}));
    let doc = ok(&mut h, "doc.inspect", json!({}));
    assert!(doc["layers"].as_array().unwrap().iter().all(|l| l["kind"] == "Pixel"));
}

#[test]
fn object_layer_commands_refuse_real_linked_files_without_changing_the_session() {
    use kition_photo_engine::doc::{LayerContent, SmartSource};

    let dir = tmp("linked-object-layer");
    let original = write_image(&dir, "external.png", kition_photo_codecs::Format::Png);
    let mut session = kition_photo_engine::Session::new();
    session.execute("file.new", json!({"width": 16, "height": 8, "background": "white"})).unwrap();
    let layer = session.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap()["layer"].clone();
    let state = session.active_mut().unwrap();
    let LayerContent::Smart(smart) = &mut std::sync::Arc::make_mut(&mut state.doc).layers[0].content else { panic!("object layer") };
    smart.source = SmartSource::Linked { path: dir.join("external.png").to_string_lossy().into_owned() };
    std::fs::write(dir.join("parent.kphoto"), kition_photo_engine::object_layer_cmds::encode_source(&state.doc).unwrap()).unwrap();
    let mut h = workspace(&dir);
    ok(&mut h, "doc.open", json!({"path": "parent.kphoto"}));
    let before = ok(&mut h, "doc.inspect", json!({}));
    for id in ["layer.objectLayers.editContents", "layer.objectLayers.convertToLayers"] {
        let e = run_err(&mut h, id, json!({"layer": layer}));
        assert!(e.contains("ambient filesystem paths"), "{e}");
        assert_eq!(ok(&mut h, "doc.inspect", json!({})), before);
        assert_eq!(ok(&mut h, "session.list", json!({}))["documents"].as_array().unwrap().len(), 1);
    }
    assert_eq!(std::fs::read(dir.join("external.png")).unwrap(), original);
    drop(h);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn previews_preserve_document_indices() {
    let mut h = Headless::new();
    ok(&mut h, "doc.new", json!({"width": 12, "height": 6, "background": "white"}));
    ok(&mut h, "doc.new", json!({"width": 6, "height": 12, "background": "black"}));
    for (args, dimensions, pixel) in [(json!({"index": 0, "maxSide": 6}), (6, 3), [255, 255, 255, 255]), (json!({"maxSide": 6}), (3, 6), [0, 0, 0, 255])] {
        let image = decode_render(&ok(&mut h, "doc.render", args));
        assert_eq!(image.dimensions(), dimensions);
        assert!(image.to_rgba8().as_chunks::<4>().0.iter().all(|actual| actual[..] == pixel[..]));
    }
    assert_eq!(ok(&mut h, "session.list", json!({}))["active"], 1);
}

/// #368: agents can open the clipboard as a document of its own.
#[test]
fn new_from_clipboard_opens_the_copy_as_a_document() {
    let mut h = Headless::new();
    ok(&mut h, "doc.new", json!({"width": 40, "height": 30}));
    let r = ok(
        &mut h,
        "batch",
        json!({"steps": [
            {"command": "select.rect", "params": {"x": 5, "y": 5, "width": 12, "height": 7}},
            {"command": "edit.copy"},
            {"command": "file.newFromClipboard"}
        ]}),
    );
    assert_eq!(r["completed"], 3, "{r}");
    let doc = ok(&mut h, "doc.inspect", json!({}));
    assert_eq!((doc["width"].as_u64(), doc["height"].as_u64()), (Some(12), Some(7)), "{doc}");
    assert_eq!(ok(&mut h, "session.list", json!({}))["documents"].as_array().unwrap().len(), 2);
}
