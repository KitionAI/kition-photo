//! Colour emoji draw in colour: a COLRv1 font (kition-fonts' Noto Color
//! Emoji) paints its glyphs' gradients and layers instead of leaving their space empty, as the
//! family asked for and as the fallback for emoji in other text.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

/// kition-fonts' Noto Color Emoji: `KITION_FONTS_DIR`, else the checkout next to this one.
fn noto_color_emoji() -> Option<Vec<u8>> {
    let dir = std::env::var_os("KITION_FONTS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../kition-fonts")));
    std::fs::read(dir.join("fonts/noto-color-emoji/Noto-COLRv1.ttf")).ok()
}

/// The image's pixels after setting `text` with `style`, on white.
fn pixels(text: &str, style: Value) -> Vec<[u8; 4]> {
    let mut image = PhotoDocument::create("emoji.psd", 200, 80).unwrap();
    let mut params = json!({ "x": 10, "y": 60, "text": text, "size": 40, "color": "#000000" });
    for (key, value) in style.as_object().unwrap() {
        params[key] = value.clone();
    }
    image.execute("text.add", params).unwrap();
    image.render(0.0, 0.0, 1.0, 200, 80).unwrap().as_chunks::<4>().0.iter().map(|px| [px[0], px[1], px[2], px[3]]).collect()
}

/// Pixels that are clearly coloured (not white, black or grey).
fn colourful(pixels: &[[u8; 4]]) -> usize {
    pixels.iter().filter(|p| p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]) > 80).count()
}

/// Distinct coloured shades (to 16 levels), so a gradient is more than one flat colour.
fn shades(pixels: &[[u8; 4]]) -> usize {
    let mut seen: Vec<[u8; 3]> =
        pixels.iter().filter(|p| p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]) > 80).map(|p| [p[0] / 16, p[1] / 16, p[2] / 16]).collect();
    seen.sort_unstable();
    seen.dedup();
    seen.len()
}

#[test]
fn colour_emoji_draw_in_colour() {
    let Some(bytes) = noto_color_emoji() else {
        eprintln!("skipping: kition-fonts' fonts/noto-color-emoji/Noto-COLRv1.ttf is not here (set KITION_FONTS_DIR)");
        return;
    };
    // As in the kit's process, which reads no font of the machine: a runner's installed
    // "Noto Color Emoji" (Ubuntu's bitmap CBDT build) must not stand in for the font the host
    // adds.
    kition_photo_text::no_system_access();
    kition_photo_kit::add_font(&bytes);
    // 😀 🌈 🔥 in the emoji family: yellow faces with radial gradients, a rainbow, a flame.
    let drawn = pixels("\u{1F600}\u{1F308}\u{1F525}", json!({ "font": "Noto Color Emoji" }));
    assert!(colourful(&drawn) > 1500, "{} coloured pixels", colourful(&drawn));
    assert!(shades(&drawn) > 40, "{} shades: gradients flattened", shades(&drawn));
    // The grinning face is yellow.
    let yellow = drawn[..].iter().filter(|p| p[0] > 200 && p[1] > 150 && p[2] < 100).count();
    assert!(yellow > 300, "{yellow} yellow pixels");
    // An emoji in Inter text falls back to the emoji font and draws in colour too.
    let mixed = pixels("Hi \u{1F600}", json!({ "font": "Inter" }));
    assert!(colourful(&mixed) > 300, "{} coloured pixels in mixed text", colourful(&mixed));
    // The text colour still draws the letters.
    assert!(mixed.iter().any(|p| p[0] < 40 && p[1] < 40 && p[2] < 40));
}
