//! Regression: justified text with hyphenation fills every line but
//! a paragraph's last to the box's width, the lines that end in a hyphen too. A test binary of
//! its own: fonts are the process's.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn run(image: &mut PhotoDocument, command: &str, params: Value) -> Value {
    image.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

const TEXT: &str = "The ferry leaves at six, or a little after, depending on whether the baker from Holm has finished loading his crates. Nobody minds. The deck smells of diesel and cardamom buns, and the gulls follow the wake as far as the lighthouse before they lose interest.\nFor the first hour the islands slide past like sleeping animals, low and green and close enough to count the sheep. Fishermen wave from their sheds. A boy on the pier at Skarv holds up a hand-lettered sign with his grandmother\u{2019}s name on it, and the captain, who knows her, sounds the horn twice.";

#[test]
fn justified_hyphenated_lines_reach_the_edge() {
    let dir = std::env::var_os("KITION_FONTS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../kition-fonts")));
    let lora = std::fs::read(dir.join("fonts/lora/Lora-Variable.ttf")).expect("needs the kition-fonts checkout");
    kition_photo_kit::add_font(&lora);
    let mut image = PhotoDocument::create("column.psd", 400, 700).unwrap();
    let made = run(
        &mut image,
        "text.add",
        json!({ "x": 20, "y": 20, "box": [20, 20, 315, 640], "font": "Lora", "size": 15, "leading": 22, "color": "#000000", "align": "justify", "hyphenate": true, "text": TEXT }),
    );
    let info = run(&mut image, "text.info", json!({ "layer": made["layer"] }));
    let pixels = image.render(0.0, 0.0, 1.0, 400, 700).unwrap();
    let lines = info["lines"].as_array().unwrap();
    for (i, line) in lines.iter().enumerate() {
        let baseline = 20.0 + line["baseline"].as_f64().unwrap();
        let (top, bottom) = ((baseline - 10.0) as usize, (baseline + 2.0) as usize);
        let right = (top..bottom).filter_map(|y| (0..400usize).rev().find(|&x| pixels[(y * 400 + x) * 4] < 140)).max().unwrap_or(0);
        let (start, end) = (line["start"].as_u64().unwrap() as usize, line["end"].as_u64().unwrap() as usize);
        let text: String = TEXT.chars().skip(start).take(end - start).collect();
        // Every line but a paragraph's last ends at the box's right edge (335), hyphen included.
        let last = text.ends_with('.') && (i + 1 == lines.len() || TEXT.chars().nth(end) == Some('\n'));
        if !last {
            assert!((331..=336).contains(&right), "line {i} {text:?} ends at {right}, not at the edge");
        }
    }
}
