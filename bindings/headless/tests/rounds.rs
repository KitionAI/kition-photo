//! Rounds of a colleague's change and the agent's writes with `basis` (task card C-photo), for the
//! numbers in the report: each round opens a fresh image of 20 blocks (`common::image`), the agent
//! inspects it, a colleague's program (this kit's engine, its own instance) changes the file, and
//! the agent writes four ways on a target layer with the `basis` of its inspect:
//!
//! - A: the target's opacity by id (`layer: 7`);
//! - B: the target's opacity by its place in the stack (`layer: "#9"`, from the top);
//! - C: a fill of a 40 x 40 region of the target, by id and image coordinates;
//! - D: the opacity of the layer picked at the middle of the target's block (`at: [x, y]`).
//!
//! A write is *needed refused* when running it now would not do what it did at `basis`:
//! A when the target is gone or others changed its opacity; B when `"#n"` names another layer
//! now (or none); C when the target is gone, the image's size changed (its coordinates name
//! other pixels), or the target's stored pixels inside the region differ (compared byte
//! for byte); D when the layer picked at the point or its opacity differs. Otherwise a refusal is *not needed*
//! (false). Each outcome: applied, refused (`conflict`), or failed at the kit (any other error).
//!
//! The rounds print their tables (`--nocapture`); the assertions hold what the report states.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::collections::BTreeMap;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::{CallOptions, Guard};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Image.kphoto";
const LAYERS: usize = 20;
const ROUNDS: usize = 50;
const SEED: u64 = 0x9e37_79b9_7f4a_7c15;

const CHANGES: [&str; 9] = [
    "property of another layer",
    "paint on another layer",
    "paint on the target outside the region",
    "paint on the target inside the region",
    "insert a layer above the target",
    "delete another layer",
    "move the target",
    "crop the canvas",
    "rename swap",
];

const WRITES: [&str; 4] = ["A by id", "B by #n", "C region by id", "D by point"];

fn paint(id: u64, rect: [i32; 4], color: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("layer.select", json!({ "layer": id })),
        ("select.rect", json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] })),
        ("edit.fill", json!({ "color": color })),
        ("edit.deselectAll", json!({})),
    ]
}

/// The colleague's change `kind` with target `t` and another layer `o` (by block number), ids by
/// name in `names`.
fn change(kind: usize, t: usize, o: usize, names: &BTreeMap<String, u64>, rng: &mut Rng, width: i64, height: i64) -> Vec<(&'static str, Value)> {
    let (target, other) = (names[&format!("L{t}")], names[&format!("L{o}")]);
    match kind {
        0 => vec![("layer.setProps", json!({ "layer": other, "opacity": 0.5 }))],
        1 => {
            let [x, y, ..] = block(o);
            paint(other, [x + 30, y + 30, 20, 20], "#101010")
        }
        2 => {
            let [x, y, ..] = block((t + 1 + rng.below(LAYERS - 1)) % LAYERS);
            paint(target, [x, y, 12, 12], "#202020")
        }
        3 => {
            let [x, y, ..] = region(t);
            paint(target, [x + 5 + i32::try_from(rng.below(20)).unwrap(), y + 5 + i32::try_from(rng.below(20)).unwrap(), 8, 8], "#303030")
        }
        4 => vec![("layer.select", json!({ "layer": target })), ("layer.new.layer", json!({ "name": "Inserted" }))],
        5 => vec![("layer.delete", json!({ "layer": other }))],
        6 => {
            let step = |rng: &mut Rng| i64::try_from(rng.below(201)).unwrap() - 100;
            let (mut dx, dy) = (step(rng), step(rng));
            if dx == 0 && dy == 0 {
                dx = 1;
            }
            vec![("layer.translate", json!({ "layer": target, "dx": dx, "dy": dy }))]
        }
        7 => vec![("image.canvasSize", json!({ "width": width - 128, "height": height, "anchor": "center" }))],
        8 => vec![
            ("layer.setProps", json!({ "layer": target, "name": "Swap" })),
            ("layer.setProps", json!({ "layer": other, "name": format!("L{t}") })),
            ("layer.setProps", json!({ "layer": target, "name": format!("L{o}") })),
        ],
        _ => unreachable!(),
    }
}

/// The layer `"#n"` names in `doc`: the n-th of the top level as inspect lists it, from the top.
fn at_index(doc: &PhotoDocument, index: &str) -> Option<u64> {
    let n: usize = index.trim_start_matches('#').parse().ok()?;
    let top: Vec<u64> = doc.inspect().unwrap()["layers"].as_array().unwrap().iter().map(|layer| layer["id"].as_u64().unwrap()).collect();
    n.checked_sub(1).and_then(|from_top| top.get(from_top).copied())
}

/// The layer picked at `point` in `doc`.
fn picked(doc: &PhotoDocument, point: [i32; 2]) -> Option<u64> {
    doc.under("layer.setProps", &json!({ "at": point })).ok().flatten().and_then(|under| under.objects.first().and_then(Value::as_u64))
}

fn layer(doc: &PhotoDocument, id: u64) -> Option<Value> {
    layers_of(&doc.inspect().unwrap()).into_iter().find(|layer| layer["id"] == id)
}

/// The stored pixels the fill replaces, before display opacity, masks or blending.
fn pixels(doc: &mut PhotoDocument, id: u64, rect: [i32; 4]) -> Option<Value> {
    let raw = kition_photo_format::load_from_bytes(&doc.save_sealed(Some(FILE)).unwrap()).unwrap();
    let surface = raw.layer(kition_photo_engine::doc::LayerId(id))?.surface()?;
    let area = kition_photo_engine::doc::Rect::from_xywh(rect[0], rect[1], rect[2] as u32, rect[3] as u32);
    Some(json!({ "format": surface.format(), "pixels": surface.to_interleaved(area) }))
}

/// Which of the four writes is needed refused after the change from `before` to `after`.
fn needed(before: &mut PhotoDocument, after: &mut PhotoDocument, target: u64, index: &str, rect: [i32; 4], point: [i32; 2]) -> [bool; 4] {
    let gone = layer(after, target).is_none();
    let a = gone || layer(before, target).map(|l| l["opacity"].clone()) != layer(after, target).map(|l| l["opacity"].clone());
    let b = at_index(after, index) != Some(target) || a;
    let size = |doc: &PhotoDocument| (doc.width(), doc.height());
    let c = gone || size(before) != size(after) || pixels(before, target, rect) != pixels(after, target, rect);
    let opacity_at = |doc: &PhotoDocument| picked(doc, point).and_then(|id| layer(doc, id)).map(|l| l["opacity"].clone());
    let d = picked(before, point) != picked(after, point) || opacity_at(before) != opacity_at(after);
    [a, b, c, d]
}

#[test]
fn review_needed_counts_the_written_property_but_not_raw_pixel_appearance() {
    let bytes = image(FILE, LAYERS);
    let mut before = PhotoDocument::open(FILE, &bytes).unwrap();
    let target = ids_by_name(&before.inspect().unwrap())["L0"];
    let mut after = PhotoDocument::open(FILE, &bytes).unwrap();
    after.execute("layer.setProps", json!({ "layer": target, "opacity": 0.5 })).unwrap();
    assert_eq!(needed(&mut before, &mut after, target, "#20", region(0), middle(0)), [true, true, false, true]);
}

#[derive(Default, Clone, Copy)]
struct Tally {
    rounds: usize,
    applied: usize,
    refused_needed: usize,
    refused_false: usize,
    applied_wrongly: usize,
    failed: usize,
}

impl Tally {
    fn add(&mut self, outcome: &Result<Value, Value>, needed: bool) {
        self.rounds += 1;
        match outcome {
            Ok(_) => {
                self.applied += 1;
                if needed {
                    self.applied_wrongly += 1;
                }
            }
            Err(error) if error["error"]["code"] == "conflict" => {
                if needed {
                    self.refused_needed += 1;
                } else {
                    self.refused_false += 1;
                }
            }
            Err(_) => self.failed += 1,
        }
    }
}

fn steps(target: u64, index: &str, rect: [i32; 4], point: [i32; 2]) -> [Value; 4] {
    [
        json!({ "command": "layer.setProps", "params": { "layer": target, "opacity": 0.25 } }),
        json!({ "command": "layer.setProps", "params": { "layer": index, "opacity": 0.375 } }),
        json!({ "command": "edit.fill", "params": { "layer": target, "color": "#00ff00", "region": { "rect": rect } } }),
        json!({ "command": "layer.setProps", "params": { "at": point, "opacity": 0.625 } }),
    ]
}

/// One round: a fresh image, the agent's inspect, the colleague's change `kind`, the four writes.
async fn round(bytes: &[u8], options: CallOptions, kind: usize, rng: &mut Rng) -> ([Result<Value, Value>; 4], [bool; 4], String) {
    let t = rng.below(LAYERS);
    let o = (t + 1 + rng.below(LAYERS - 1)) % LAYERS;
    let f = Fixture::from_bytes(FILE, bytes, options);
    let (details, basis) = f.inspect().await;
    let names = ids_by_name(&details);
    let target = names[&format!("L{t}")];
    let top: Vec<u64> = details["layers"].as_array().unwrap().iter().map(|layer| layer["id"].as_u64().unwrap()).collect();
    let index = format!("#{}", top.iter().position(|id| *id == target).unwrap() + 1);
    let (rect, point) = (region(t), middle(t));
    let mut before = on_disk(&f.file());
    let (width, height) = (details["width"].as_i64().unwrap(), details["height"].as_i64().unwrap());
    f.colleague(&change(kind, t, o, &names, rng, width, height));
    let mut after = on_disk(&f.file());
    let needed = needed(&mut before, &mut after, target, &index, rect, point);
    let mut outcomes = Vec::new();
    for step in steps(target, &index, rect, point) {
        outcomes.push(f.call("run", json!({ "basis": basis, "steps": [step] })).await);
    }
    f.shutdown().await;
    (outcomes.try_into().unwrap(), needed, format!("L{t} {index} other L{o}"))
}

fn print_table(title: &str, rows: &BTreeMap<(usize, usize), Tally>, labels: &[&str]) {
    println!(
        "\n{title}\n| change | write | rounds | applied | refused needed | refused false | applied wrongly | failed at kit |\n|---|---|---|---|---|---|---|---|"
    );
    for ((change, write), tally) in rows {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            labels[*change], WRITES[*write], tally.rounds, tally.applied, tally.refused_needed, tally.refused_false, tally.applied_wrongly, tally.failed
        );
    }
}

/// (11) 50 rounds on 20 layers, a random unrelated change (another layer's property or paint):
/// with the premise guard every write applies; with `--guard epoch` every write is refused.
#[tokio::test(flavor = "multi_thread")]
async fn unrelated_rounds_premise_against_epoch() {
    let bytes = image(FILE, LAYERS);
    for (guard, name) in [(Guard::Premise, "premise"), (Guard::Epoch, "epoch")] {
        let mut rng = Rng(SEED);
        let mut rows: BTreeMap<(usize, usize), Tally> = BTreeMap::new();
        for _ in 0..ROUNDS {
            let kind = rng.below(2);
            let (outcomes, needed, _) = round(&bytes, CallOptions { guard, ..CallOptions::default() }, kind, &mut rng).await;
            for (write, outcome) in outcomes.iter().enumerate() {
                rows.entry((0, write)).or_default().add(outcome, needed[write]);
            }
        }
        print_table(&format!("(11) unrelated change, guard {name}, seed {SEED:#x}, {ROUNDS} rounds, {LAYERS} layers"), &rows, &["unrelated"]);
        for write in 0..4 {
            let tally = rows[&(0, write)];
            assert_eq!(tally.failed, 0, "{name} {}", WRITES[write]);
            match guard {
                Guard::Premise => assert_eq!(tally.applied, ROUNDS, "premise {}: every write applies", WRITES[write]),
                _ => assert_eq!(tally.refused_false, ROUNDS, "epoch {}: every write is refused", WRITES[write]),
            }
        }
    }
}

/// (12) 50 mixed rounds, seed 0x9e3779b97f4a7c15: the colleague does one of nine changes; the
/// agent writes A, B, C and D with `basis`. Nothing applies wrongly; the totals are pinned (the
/// refusals not needed are the report's).
#[tokio::test(flavor = "multi_thread")]
async fn mixed_rounds_where_nothing_applies_wrongly() {
    let bytes = image(FILE, LAYERS);
    let mut rng = Rng(SEED);
    let mut rows: BTreeMap<(usize, usize), Tally> = BTreeMap::new();
    let mut wrong = Vec::new();
    for number in 0..ROUNDS {
        let kind = rng.below(CHANGES.len());
        let (outcomes, needed, what) = round(&bytes, CallOptions::default(), kind, &mut rng).await;
        for (write, outcome) in outcomes.iter().enumerate() {
            rows.entry((kind, write)).or_default().add(outcome, needed[write]);
            let note = match outcome {
                Ok(_) if needed[write] => Some("applied wrongly".to_owned()),
                Err(error) if error["error"]["code"] != "conflict" => Some(format!("failed: {error}")),
                Err(error) if !needed[write] => Some(format!("refused, not needed: {}", error["conflicts"])),
                _ => None,
            };
            if let Some(note) = note {
                wrong.push(format!("round {number} {} ({what}) {}: {note}", CHANGES[kind], WRITES[write]));
            }
        }
    }
    print_table(&format!("(12) mixed, seed {SEED:#x}, {ROUNDS} rounds, {LAYERS} layers"), &rows, &CHANGES);
    let mut total = Tally::default();
    for tally in rows.values() {
        total.rounds += tally.rounds;
        total.applied += tally.applied;
        total.refused_needed += tally.refused_needed;
        total.refused_false += tally.refused_false;
        total.applied_wrongly += tally.applied_wrongly;
        total.failed += tally.failed;
    }
    println!(
        "| all | all | {} | {} | {} | {} | {} | {} |",
        total.rounds, total.applied, total.refused_needed, total.refused_false, total.applied_wrongly, total.failed
    );
    for line in &wrong {
        println!("{line}");
    }
    assert_eq!(total.failed, 0, "no write fails at the kit");
    assert_eq!(total.applied_wrongly, 0, "nothing applies wrongly");
    assert_eq!(
        (total.rounds, total.applied, total.refused_needed, total.refused_false, total.applied_wrongly),
        (200, 165, 35, 0, 0),
        "the pinned totals (applied, refused needed, refused not needed, applied wrongly)"
    );
}
