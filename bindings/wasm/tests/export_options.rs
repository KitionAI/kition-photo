//! Export As's options through `file.exportImage` (plan 2026-11-24, stage 6): format, quality,
//! size with or without its aspect ratio, transparency kept or flattened onto a matte, and the
//! engine's clipboard as a PNG for the system clipboard. Nothing is written and the document is
//! unchanged.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

/// A transparent 200 x 100 image with an opaque red block at (20, 10, 60, 40).
fn image() -> PhotoDocument {
    let mut doc = PhotoDocument::create("art.psd", 200, 100).unwrap();
    let background = doc.execute("file.inspect", json!({})).unwrap()["layers"][0]["id"].clone();
    let block = doc.execute("layer.new.layer", json!({ "name": "Block" })).unwrap()["layer"].clone();
    doc.execute("layer.delete", json!({ "layer": background })).unwrap();
    doc.execute("layer.select", json!({ "layer": block })).unwrap();
    doc.execute("select.rect", json!({ "x": 20, "y": 10, "width": 60, "height": 40 })).unwrap();
    doc.execute("edit.fill", json!({ "color": "#ff0000" })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc
}

fn bytes(answer: &Value) -> Vec<u8> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(answer["data"].as_str().unwrap()).unwrap()
}

fn decoded(answer: &Value) -> (u32, u32, Vec<u8>) {
    let image = kition_photo_codecs::decode(&bytes(answer)).unwrap();
    (image.width(), image.height(), image.to_rgba8())
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

#[test]
fn each_format_answers_its_bytes_and_size() {
    let mut doc = image();
    let revision = doc.revision();
    for (format, mime, magic) in [
        ("png", "image/png", &b"\x89PNG"[..]),
        ("jpg", "image/jpeg", &b"\xff\xd8"[..]),
        ("webp", "image/webp", &b"RIFF"[..]),
        ("tiff", "image/tiff", &b"II"[..]),
    ] {
        let answer = doc.execute("file.exportImage", json!({ "format": format })).unwrap();
        assert_eq!(answer["mime"], mime, "{format}");
        assert_eq!((answer["width"].as_u64(), answer["height"].as_u64()), (Some(200), Some(100)), "{format}");
        let data = bytes(&answer);
        assert!(data.starts_with(magic), "{format}");
        assert_eq!(answer["bytes"].as_u64(), Some(data.len() as u64));
    }
    assert_eq!(doc.revision(), revision, "exporting changes nothing");
}

#[test]
fn size_keeps_the_aspect_ratio_of_one_side_or_takes_both() {
    let mut doc = image();
    let half = doc.execute("file.exportImage", json!({ "format": "png", "width": 100 })).unwrap();
    let (w, h, rgba) = decoded(&half);
    assert_eq!((w, h), (100, 50));
    assert_eq!(pixel(&rgba, w, 25, 15), [255, 0, 0, 255], "the block, halved");
    assert_eq!(pixel(&rgba, w, 80, 40)[3], 0, "transparent around it");
    let stretched = doc.execute("file.exportImage", json!({ "format": "png", "width": 40, "height": 40 })).unwrap();
    let (w, h, _) = decoded(&stretched);
    assert_eq!((w, h), (40, 40));
    assert!(doc.execute("file.exportImage", json!({ "width": 0 })).is_err());
}

#[test]
fn quality_trades_bytes_for_fidelity() {
    let mut doc = image();
    doc.execute("filter.noise.addNoise", json!({ "amount": 40 })).ok();
    let low = doc.execute("file.exportImage", json!({ "format": "jpg", "quality": 10 })).unwrap();
    let high = doc.execute("file.exportImage", json!({ "format": "jpg", "quality": 95 })).unwrap();
    assert!(low["bytes"].as_u64() < high["bytes"].as_u64(), "{} vs {}", low["bytes"], high["bytes"]);
    let lossy = doc.execute("file.exportImage", json!({ "format": "webp", "quality": 50 })).unwrap();
    let lossless = doc.execute("file.exportImage", json!({ "format": "webp", "lossless": true })).unwrap();
    assert_eq!(decoded(&lossless).2, decoded(&doc.execute("file.exportImage", json!({ "format": "png" })).unwrap()).2, "lossless WebP keeps every pixel");
    assert_ne!(lossy["data"], lossless["data"]);
}

#[test]
fn transparency_is_kept_or_flattened_onto_the_matte() {
    let mut doc = image();
    let kept = doc.execute("file.exportImage", json!({ "format": "png" })).unwrap();
    let (w, _, rgba) = decoded(&kept);
    assert_eq!(pixel(&rgba, w, 150, 80)[3], 0);
    let matted = doc.execute("file.exportImage", json!({ "format": "png", "transparency": false, "matte": "#00ff00" })).unwrap();
    let (w, _, rgba) = decoded(&matted);
    assert_eq!(pixel(&rgba, w, 150, 80), [0, 255, 0, 255], "the matte under transparent pixels");
    assert_eq!(pixel(&rgba, w, 40, 20), [255, 0, 0, 255], "opaque pixels as they are");
    let jpeg = doc.execute("file.exportImage", json!({ "format": "jpg", "matte": "#0000ff", "quality": 100 })).unwrap();
    let (w, _, rgba) = decoded(&jpeg);
    let blue = pixel(&rgba, w, 150, 80);
    assert!(blue[2] > 240 && blue[0] < 16 && blue[1] < 16, "JPEG takes the matte: {blue:?}");
    let white = doc.execute("file.exportImage", json!({ "format": "jpg" })).unwrap();
    let (w, _, rgba) = decoded(&white);
    assert!(pixel(&rgba, w, 150, 80).iter().take(3).all(|v| *v > 240), "and white without one");
    assert!(doc.execute("file.exportImage", json!({ "matte": "green" })).is_err());
}

#[test]
fn the_clipboard_is_a_png_the_size_of_what_was_copied() {
    let mut doc = image();
    assert_eq!(doc.clipboard_image().unwrap(), Value::Null);
    doc.execute("select.rect", json!({ "x": 10, "y": 5, "width": 30, "height": 20 })).unwrap();
    doc.execute("edit.copyMerged", json!({})).unwrap();
    let answer = doc.clipboard_image().unwrap();
    let (w, h, rgba) = decoded(&answer);
    // The engine's clipboard holds the pixels the selection took: the block's part, (20, 10)–(40, 25).
    assert_eq!((w, h), (20, 15));
    assert!((0..h).all(|y| (0..w).all(|x| pixel(&rgba, w, x, y) == [255, 0, 0, 255])));
}
