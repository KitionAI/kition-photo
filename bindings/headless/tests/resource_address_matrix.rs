//! C8-photo-3: non-layer addresses and outside-file rewrites, with independent necessities.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_engine::doc::{Document, NamedPath};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Resources.kphoto";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    ChannelName,
    ChannelIndex,
    PathName,
    CompId,
    CompName,
    NoteIndex,
    SliceId,
    GuideIndex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Same,
    Other,
    Rename,
    Delete,
    Swap,
    Before,
    After,
    Layer,
}

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    doc.as_object_mut().unwrap().remove("id");
    doc.as_object_mut().unwrap().remove("selection");
    doc
}

fn fixture() -> Vec<u8> {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    for i in 0..3 {
        for (command, p) in [
            ("channel.new", json!({"name":format!("C{i}")})),
            ("layerComp.new", json!({"name":format!("C{i}")})),
            ("note.add", json!({"x":i*24,"y":8,"author":format!("C{i}"),"text":"Before","open":false})),
            ("slice.new", json!({"rect":[i*24,8,16,16],"name":format!("C{i}")})),
            ("view.newGuide", json!({"orientation":"vertical","position":i*24})),
        ] {
            doc.execute(command, p).unwrap();
        }
    }
    let mut engine = kition_photo_format::load_from_bytes(&doc.save(Some(FILE)).unwrap()).unwrap();
    for i in 0..3 {
        engine.paths.push(NamedPath { name: format!("C{i}"), ..Default::default() });
    }
    kition_photo_format::save_to_bytes(&engine, &Default::default()).unwrap()
}

fn positional(form: Form) -> bool {
    matches!(form, Form::ChannelIndex | Form::NoteIndex | Form::GuideIndex)
}

// The addressed member starts at index 1. Names and ids survive other members' list moves;
// index addresses must still name the member observed at basis. Other properties are merged.
fn necessity(form: Form, change: Change, scope: usize) -> Option<&'static str> {
    let target = scope == 0;
    if target && change == Change::Delete {
        return Some("deleted");
    }
    // same-field decision: Same (and the five duplicate Rename cases) assigns the
    // very field the agent assigns. This includes a note's color on both sides (whether a
    // note is open is each person's own view, not the note's).
    if target
        && (change == Change::Same
            || change == Change::Rename && matches!(form, Form::ChannelName | Form::ChannelIndex | Form::PathName | Form::CompId | Form::CompName))
    {
        return Some("changed");
    }
    if positional(form)
        && match change {
            Change::Delete | Change::After => scope == 1,
            Change::Swap | Change::Before => scope < 2,
            _ => false,
        }
    {
        return Some("relocated");
    }
    None
}

fn change(doc: &mut Document, form: Form, edit: Change, scope: usize) {
    let i = [1, 0, 2][scope];
    if edit == Change::Layer {
        doc.layers[0].name = "Colleague".into();
        return;
    }
    // Inserting a distinct member keeps the old members' identities. No fixture id is reused.
    macro_rules! structure {
        ($list:expr,$new:expr) => {{
            let list = &mut $list;
            match edit {
                Change::Delete => {
                    list.remove(i);
                }
                Change::Swap => list.swap(i, if i == 0 { 1 } else { 0 }),
                Change::Before => list.insert(i, $new),
                Change::After => list.insert(i + 1, $new),
                _ => {}
            }
        }};
    }
    match form {
        Form::ChannelName | Form::ChannelIndex => {
            let mut new = doc.channels[i].clone();
            new.key = kition_photo_engine::doc::fresh_resource_key();
            new.name = "Inserted".into();
            match edit {
                Change::Same | Change::Rename => doc.channels[i].name = "Colleague".into(),
                Change::Other => doc.channels[i].opacity = 0.2,
                _ => structure!(doc.channels, new),
            }
        }
        Form::PathName => {
            let mut new = doc.paths[i].clone();
            new.key = kition_photo_engine::doc::fresh_resource_key();
            new.name = "Inserted".into();
            match edit {
                Change::Same | Change::Rename => doc.paths[i].name = "Colleague".into(),
                Change::Other => doc.paths[i].psd_raw = Some(std::sync::Arc::new(vec![1, 2, 3])),
                _ => structure!(doc.paths, new),
            }
        }
        Form::CompId | Form::CompName => {
            let mut new = doc.layer_comps[i].clone();
            new.id = 99;
            new.name = "Inserted".into();
            match edit {
                Change::Same | Change::Rename => doc.layer_comps[i].name = "Colleague".into(),
                Change::Other => doc.layer_comps[i].comment = "Colleague".into(),
                _ => structure!(doc.layer_comps, new),
            }
        }
        Form::NoteIndex => {
            let mut new = doc.notes[i].clone();
            new.key = kition_photo_engine::doc::fresh_resource_key();
            new.author = "Inserted".into();
            match edit {
                Change::Same => doc.notes[i].color = kition_photo_engine::doc::Color::rgb(0.0, 1.0, 0.0),
                Change::Other | Change::Rename => doc.notes[i].text = "Colleague".into(),
                _ => structure!(doc.notes, new),
            }
        }
        Form::SliceId => {
            let mut new = doc.slices.list[i].clone();
            new.id = 99;
            new.name = "Inserted".into();
            match edit {
                Change::Same => doc.slices.list[i].url = "Colleague".into(),
                Change::Other | Change::Rename => doc.slices.list[i].name = "Colleague".into(),
                _ => structure!(doc.slices.list, new),
            }
        }
        Form::GuideIndex => match edit {
            Change::Same => doc.guides.vertical[i].position += 1.0,
            Change::Other | Change::Rename => doc.guides.horizontal.push(kition_photo_engine::doc::Guide::new(5.0)),
            _ => structure!(doc.guides.vertical, kition_photo_engine::doc::Guide::new(48.0)),
        },
    }
}

fn indexed_or_identity_cases(form: Form, edit: Change, scope: usize) -> bool {
    // Stored-array premises currently report changed on Image for deleted members and
    // relocated index addresses. Keep the independently required reasons in necessity().
    if matches!(necessity(form, edit, scope), Some("deleted" | "relocated")) {
        return true;
    }
    match form {
        Form::ChannelName => {
            matches!(edit, Change::Same | Change::Rename | Change::Delete) && scope > 0 || matches!(edit, Change::Swap | Change::Before | Change::After)
        }
        Form::ChannelIndex => {
            matches!(edit, Change::Same | Change::Rename) && scope > 0
                || matches!(edit, Change::Delete | Change::Swap | Change::Before) && scope == 2
                || edit == Change::After && scope != 1
        }
        Form::PathName | Form::CompId | Form::CompName | Form::SliceId => edit == Change::Swap && scope < 2,
        Form::NoteIndex => {
            matches!(edit, Change::Other | Change::Rename) && scope == 0
                || matches!(edit, Change::Delete | Change::Before) && scope == 2
                || edit == Change::After && scope != 1
        }
        Form::GuideIndex => matches!(edit, Change::Delete | Change::Before) && scope == 2 || edit == Change::After && scope != 1,
    }
}

async fn matrix(waiting: bool) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let bytes = fixture();
    let engine = kition_photo_format::load_from_bytes(&bytes).unwrap();
    let mut wrong = vec![];
    let mut false_refusals = vec![];
    let mut failures = vec![];
    let mut merges = vec![];
    let mut reasons = vec![];
    let (mut cells, mut applied, mut necessary) = (0, 0, 0);
    for form in [Form::ChannelName, Form::ChannelIndex, Form::PathName, Form::CompId, Form::CompName, Form::NoteIndex, Form::SliceId, Form::GuideIndex] {
        let (command, params) = match form {
            Form::ChannelName => ("channel.rename", json!({"channel":"C1","name":"Mine"})),
            Form::ChannelIndex => ("channel.rename", json!({"channel":1,"name":"Mine"})),
            Form::PathName => ("path.rename", json!({"name":"C1","to":"Mine"})),
            Form::CompId => ("layerComp.rename", json!({"comp":engine.layer_comps[1].id,"name":"Mine"})),
            Form::CompName => ("layerComp.rename", json!({"comp":"C1","name":"Mine"})),
            Form::NoteIndex => ("note.set", json!({"index":1,"color":"#ff0000"})),
            Form::SliceId => ("slice.set", json!({"slice":engine.slices.list[1].id,"url":"Mine"})),
            Form::GuideIndex => ("view.moveGuide", json!({"orientation":"vertical","index":1,"position":32})),
        };
        for edit in [Change::Same, Change::Other, Change::Rename, Change::Delete, Change::Swap, Change::Before, Change::After, Change::Layer] {
            for scope in 0..3 {
                if edit == Change::Layer && scope > 0 {
                    continue;
                }
                if indexed_or_identity_cases(form, edit, scope) != waiting {
                    continue;
                }
                let expected = necessity(form, edit, scope);
                let label = format!("{form:?}/{edit:?}/{scope}");
                let f = Fixture::from_bytes(FILE, &bytes, CallOptions::default());
                let (_, basis) = f.inspect().await;
                let mut colleague = kition_photo_format::load_from_bytes(&std::fs::read(f.file()).unwrap()).unwrap();
                change(&mut colleague, form, edit, scope);
                std::fs::write(f.file(), kition_photo_format::save_to_bytes(&colleague, &Default::default()).unwrap()).unwrap();
                let before = std::fs::read(f.file()).unwrap();
                let result = f.call("run", json!({"basis":basis,"steps":[{"command":command,"params":params}]})).await;
                cells += 1;
                match (&result, expected) {
                    (Ok(_), Some(_)) => wrong.push(label),
                    (Err(e), Some(reason)) if e["error"]["code"] == "conflict" => {
                        necessary += 1;
                        if !e["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == reason) {
                            reasons.push(format!("{label} expected {reason}: {e}"));
                        }
                        assert_eq!(std::fs::read(f.file()).unwrap(), before);
                    }
                    (Err(e), None) if e["error"]["code"] == "conflict" => false_refusals.push(format!("{label}: {}", e["conflicts"])),
                    (Err(e), _) => failures.push(format!("{label}: {e}")),
                    (Ok(_), None) => {
                        let mut reference = PhotoDocument::open(FILE, &before).unwrap();
                        reference.execute(command, params.clone()).unwrap();
                        if saved(&std::fs::read(f.file()).unwrap()) != saved(&reference.save(Some(FILE)).unwrap()) {
                            merges.push(label);
                        }
                        applied += 1;
                    }
                }
                f.shutdown().await;
            }
        }
    }
    println!(
        "RESOURCE waiting={waiting} cells={cells} applied={applied} necessary={necessary} wrong={} false={} reason={} failed={} merge={}\nwrong={wrong:?}\nfalse={false_refusals:?}\nreason={reasons:?}\nfailed={failures:?}\nmerge={merges:?}",
        wrong.len(),
        false_refusals.len(),
        reasons.len(),
        failures.len(),
        merges.len()
    );
    (cells, applied, necessary, wrong.len(), false_refusals.len(), reasons.len(), failures.len(), merges.len())
}

#[tokio::test(flavor = "multi_thread")]
async fn resource_address_matrix() {
    assert_eq!(matrix(false).await, (108, 95, 13, 0, 0, 0, 0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn resource_array_addresses() {
    // Indexed continuity and resource reasons use persistent member identities.
    assert_eq!(matrix(true).await, (68, 42, 26, 0, 0, 0, 0, 0));
}
