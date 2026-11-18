//! A variable font draws every weight its `wght` axis covers, and its variation axes, even when
//! static faces of the same family are registered too: Kition Photo
//! bundles Inter Regular, Medium and SemiBold, and kition-fonts serves Inter's variable font.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

/// kition-fonts' Inter variable font: `KITION_FONTS_DIR`, else the checkout next to this one.
fn inter_variable() -> Option<Vec<u8>> {
    let dir = std::env::var_os("KITION_FONTS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../kition-fonts")));
    std::fs::read(dir.join("fonts/inter/Inter-Variable.ttf")).ok()
}

/// The ink of "Weight" set in Inter with `style` (darkness summed over the image).
fn ink(style: Value) -> f64 {
    let mut image = PhotoDocument::create("weights.psd", 360, 90).unwrap();
    let mut params = json!({ "x": 10, "y": 64, "text": "Weight", "font": "Inter", "size": 48, "color": "#000000" });
    for (key, value) in style.as_object().unwrap() {
        params[key] = value.clone();
    }
    image.execute("type.create", params).unwrap();
    let rgba = image.render(0.0, 0.0, 1.0, 360, 90).unwrap();
    rgba.as_chunks::<4>().0.iter().map(|px| 255.0 - f64::from(px[0])).sum()
}

#[test]
fn inter_draws_every_weight_and_its_axes() {
    let Some(bytes) = inter_variable() else {
        eprintln!("skipping: kition-fonts' fonts/inter/Inter-Variable.ttf is not here (set KITION_FONTS_DIR)");
        return;
    };
    kition_photo_kit::add_font(&bytes);
    let weights: Vec<(u32, f64)> = [100, 200, 300, 400, 500, 600, 700, 800, 900].into_iter().map(|w| (w, ink(json!({ "weight": w })))).collect();
    for pair in weights.windows(2) {
        let ((light, a), (heavy, b)) = (pair[0], pair[1]);
        assert!(b > a * 1.04, "weight {heavy} draws no darker than {light}: {weights:?}");
    }
    // An explicit wght variation draws that weight too.
    let regular = ink(json!({ "weight": 400 }));
    let black = ink(json!({ "variations": { "wght": 900 } }));
    let thin = ink(json!({ "variations": { "wght": 100 } }));
    assert!(black > regular * 1.3 && thin < regular * 0.7, "wght variation ignored: thin {thin}, regular {regular}, black {black}");
}
