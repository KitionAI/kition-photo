//! C8-photo-3: nested/selected addresses and recursive group writes.
//! Necessities below were written before the first run, independently of declarations.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod common;

use common::*;
use kition_kit::{CallContext, KitDocument};
use kition_kits::tools::CallOptions;
use kition_photo_engine::doc::{Document, Group, LabelColor, Layer, LayerContent, LayerId, LayerMask, PixelFormat, Rect, Surface};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Tree.kphoto";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    NestedId,
    NestedName,
    SelectedLeaves,
    GroupId,
    GroupName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Same,
    Other,
    Rename,
    Delete,
    Reorder,
    InsertBefore,
    InsertAfter,
    Translate,
    Hide,
    Lock,
    LeaveGroup,
    EnterGroup,
    PaintInside,
    PaintOutside,
    Mask,
    Selection,
    Resize,
    Crop,
    Depth,
    Mode,
}

fn group_form(form: Form) -> bool {
    matches!(form, Form::GroupId | Form::GroupName)
}

fn label_form(form: Form) -> bool {
    !matches!(form, Form::NestedId | Form::NestedName)
}

// Scopes are the nested target, its group, its sibling, and an unrelated root layer.
// Labeling a group writes the group and every descendant, including added descendants;
// labeling explicitly selected leaves only writes those ids. Order is irrelevant to labels.
fn necessity(form: Form, change: Change, scope: usize) -> Option<&'static str> {
    let affected = if group_form(form) {
        scope < 3
    } else if form == Form::SelectedLeaves {
        scope == 0 || scope == 2
    } else {
        scope == 0
    };
    if change == Change::Delete && (affected || scope == 1) {
        return Some(if scope == 1 || !group_form(form) { "deleted" } else { "changed" });
    }
    if change == Change::Same && affected {
        // same-field decision: opacity or every assigned descendant label is held.
        return Some("changed");
    }
    if change == Change::Rename && (form == Form::NestedName && scope == 0 || form == Form::GroupName && scope == 1) {
        return Some("relocated");
    }
    if group_form(form)
        && (matches!(change, Change::InsertBefore | Change::InsertAfter | Change::LeaveGroup) && (scope == 0 || scope == 2)
            || change == Change::EnterGroup && scope != 1)
    {
        return Some("changed");
    }
    None
}

fn fixture() -> Vec<u8> {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    for i in 0..4 {
        doc.execute("layer.new.layer", json!({"name":format!("L{i}")})).unwrap();
        doc.execute("select.rect", json!({"x":8+i*20,"y":12,"width":16,"height":24})).unwrap();
        doc.execute("edit.fill", json!({"color":"#dd3344"})).unwrap();
        doc.execute("select.deselect", json!({})).unwrap();
    }
    let ids = ids_by_name(&doc.inspect().unwrap());
    let group = doc.execute("layer.new.group", json!({"name":"G"})).unwrap()["layer"].clone();
    doc.execute("layer.moveTo", json!({"layers":[ids["L2"],ids["L3"]],"target":group,"position":"into"})).unwrap();
    doc.execute("layer.select", json!({"layer":ids["L0"]})).unwrap();
    let outer = doc.execute("layer.new.group", json!({"name":"Outer"})).unwrap()["layer"].clone();
    doc.execute("layer.moveTo", json!({"layer":group,"target":outer,"position":"into"})).unwrap();
    doc.save(Some(FILE)).unwrap()
}

fn siblings<'a>(doc: &'a mut Document, path: &[usize]) -> &'a mut Vec<Layer> {
    if path.len() == 1 { &mut doc.layers } else { doc.layer_at_mut(&path[..path.len() - 1]).unwrap().children_mut().unwrap() }
}

fn change(doc: &mut Document, form: Form, edit: Change, scope: usize, ids: &std::collections::BTreeMap<String, u64>) {
    let id = LayerId([ids["L3"], ids["G"], ids["L2"], ids["L0"]][scope]);
    let global = match edit {
        Change::Resize => Some(("image.canvasSize", json!({"width":106,"height":64,"anchor":"topLeft"}))),
        Change::Crop => Some(("image.crop", json!({"x":0,"y":0,"width":92,"height":60}))),
        Change::Depth => Some(("image.mode.bits16", json!({}))),
        Change::Mode => Some(("image.mode.grayscale", json!({}))),
        _ => None,
    };
    if let Some((command, params)) = global {
        let mut engine = PhotoDocument::open(FILE, &kition_photo_format::save_to_bytes(doc, &Default::default()).unwrap()).unwrap();
        engine.execute(command, params).unwrap();
        *doc = kition_photo_format::load_from_bytes(&engine.save(Some(FILE)).unwrap()).unwrap();
        return;
    }
    match edit {
        Change::Same => {
            let layer = doc.layer_mut(id).unwrap();
            if label_form(form) {
                layer.label = LabelColor::Green;
            } else {
                layer.opacity = 0.3;
            }
        }
        Change::Other => {
            let layer = doc.layer_mut(id).unwrap();
            if label_form(form) {
                layer.opacity = 0.3;
            } else {
                layer.fill_opacity = 0.4;
            }
        }
        Change::Rename => doc.layer_mut(id).unwrap().name = "Colleague".into(),
        Change::Delete => {
            doc.remove(id).unwrap();
        }
        Change::Reorder => {
            let path = doc.path_of(id).unwrap();
            let i = *path.last().unwrap();
            let list = siblings(doc, &path);
            let other = if i == 0 { list.len() - 1 } else { 0 };
            list.swap(i, other);
        }
        Change::InsertBefore | Change::InsertAfter => {
            let layer = Layer::raster("Inserted", doc.pixel_format());
            let path = doc.path_of(id).unwrap();
            let i = *path.last().unwrap() + usize::from(edit == Change::InsertAfter);
            siblings(doc, &path).insert(i, layer);
        }
        Change::Translate => {
            *doc = kition_photo_engine::layer_multi_cmds::moved(doc, &[id], 4, 0).unwrap();
        }
        Change::Hide => doc.layer_mut(id).unwrap().visible = false,
        Change::Lock => doc.layer_mut(id).unwrap().locks.all = true,
        Change::LeaveGroup => {
            let layer = doc.remove(id).unwrap();
            if scope == 3 {
                doc.layer_mut(LayerId(ids["Outer"])).unwrap().children_mut().unwrap().push(layer);
            } else {
                doc.layers.push(layer);
            }
        }
        Change::EnterGroup => {
            let layer = doc.remove(id).unwrap();
            if scope == 3 {
                doc.layer_mut(LayerId(ids["G"])).unwrap().children_mut().unwrap().push(layer);
            } else {
                let wrapper = Layer::new("Inner", LayerContent::Group(Group { children: vec![layer], expanded: true, artboard: None }));
                if scope == 1 {
                    doc.layers.push(wrapper);
                } else {
                    doc.layer_mut(LayerId(ids["G"])).unwrap().children_mut().unwrap().push(wrapper);
                }
            }
        }
        Change::PaintInside | Change::PaintOutside => {
            if let Some(surface) = doc.layer_mut(id).unwrap().surface_mut() {
                let x = 10 + [3, 3, 2, 0][scope] * 20;
                let y = if edit == Change::PaintOutside { 45 } else { 14 };
                surface.fill_rect(Rect::new(x, y, 6, 6), &[0.0, 0.0, 1.0, 1.0]);
            } else {
                doc.layer_mut(id).unwrap().mask = Some(LayerMask::hide_all());
            }
        }
        Change::Mask => doc.layer_mut(id).unwrap().mask = Some(LayerMask::hide_all()),
        Change::Selection => {
            let mut selection = Surface::new(PixelFormat::GRAY8);
            selection.fill_rect(Rect::new(1, 1, 3, 3), &[1.0]);
            doc.selection = Some(selection);
        }
        Change::Resize | Change::Crop | Change::Depth | Change::Mode => unreachable!(),
    }
}

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    doc.as_object_mut().unwrap().remove("id");
    doc.as_object_mut().unwrap().remove("selection");
    doc
}

fn precision_cases(form: Form, edit: Change, scope: usize) -> bool {
    if group_form(form) {
        return matches!(edit, Change::InsertBefore | Change::InsertAfter | Change::EnterGroup) && scope == 1
            || edit == Change::LeaveGroup && (scope == 1 || scope == 3);
    }
    form == Form::SelectedLeaves
        && match edit {
            Change::Other | Change::Rename | Change::Reorder | Change::InsertBefore | Change::Hide | Change::Lock | Change::Mask => scope == 0 || scope == 2,
            Change::InsertAfter => scope == 2,
            Change::Translate => scope < 3,
            Change::LeaveGroup => scope == 1 || scope == 2,
            Change::EnterGroup => scope < 3,
            Change::Depth | Change::Mode => true,
            _ => false,
        }
}

async fn matrix(waiting: bool, forms: &[Form]) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let bytes = fixture();
    let ids = ids_by_name(&PhotoDocument::open(FILE, &bytes).unwrap().inspect().unwrap());
    let (mut cells, mut applied, mut necessary) = (0, 0, 0);
    let (mut wrong, mut false_refusals, mut reasons, mut failures, mut merges) = (vec![], vec![], vec![], vec![], vec![]);
    for &form in forms {
        for edit in [
            Change::Same,
            Change::Other,
            Change::Rename,
            Change::Delete,
            Change::Reorder,
            Change::InsertBefore,
            Change::InsertAfter,
            Change::Translate,
            Change::Hide,
            Change::Lock,
            Change::LeaveGroup,
            Change::EnterGroup,
            Change::PaintInside,
            Change::PaintOutside,
            Change::Mask,
            Change::Selection,
            Change::Resize,
            Change::Crop,
            Change::Depth,
            Change::Mode,
        ] {
            for scope in 0..4 {
                if scope > 0 && matches!(edit, Change::Selection | Change::Resize | Change::Crop | Change::Depth | Change::Mode) {
                    continue;
                }
                if precision_cases(form, edit, scope) != waiting {
                    continue;
                }
                let label = format!("{form:?}/{edit:?}/{scope}");
                let expected = necessity(form, edit, scope);
                let f = Fixture::from_bytes(FILE, &bytes, CallOptions::default());
                let (_, basis) = f.inspect().await;
                let mut colleague = kition_photo_format::load_from_bytes(&std::fs::read(f.file()).unwrap()).unwrap();
                change(&mut colleague, form, edit, scope, &ids);
                std::fs::write(f.file(), kition_photo_format::save_to_bytes(&colleague, &Default::default()).unwrap()).unwrap();
                let before = std::fs::read(f.file()).unwrap();
                let command = if label_form(form) { "layer.setLabelColor" } else { "layer.setProps" };
                let mut params = if label_form(form) { json!({"color":"red"}) } else { json!({"opacity":0.6}) };
                let on = if form == Form::SelectedLeaves { vec![json!(ids["L3"]), json!(ids["L2"])] } else { vec![] };
                match form {
                    Form::NestedId => params["layer"] = json!(ids["L3"]),
                    Form::NestedName => params["layer"] = json!("L3"),
                    Form::GroupId => params["layer"] = json!(ids["G"]),
                    Form::GroupName => params["layer"] = json!("G"),
                    Form::SelectedLeaves => {}
                }
                let mut steps = vec![];
                if form == Form::SelectedLeaves {
                    steps.push(json!({"command":"layer.select","params":{"layer":ids["L3"]}}));
                    steps.push(json!({"command":"layer.select","params":{"layer":ids["L2"],"mode":"add"}}));
                }
                steps.push(json!({"command":command,"params":params}));
                let result = f.call("run", json!({"basis":basis,"steps":steps})).await;
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
                    (Err(e), None) if e["error"]["code"] == "conflict" => false_refusals.push(format!("{label}: {e}")),
                    (Err(e), _) => failures.push(format!("{label}: {e}")),
                    (Ok(_), None) => {
                        let mut reference = PhotoDocument::open(FILE, &before).unwrap();
                        let state = reference.editor_state();
                        reference.set_context(&CallContext { on, ..Default::default() }).unwrap();
                        // Names still resolve to the observed ids in every applying cell.
                        reference.execute(command, params).unwrap();
                        reference.restore_editor_state(&state).unwrap();
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
        "TREE waiting={waiting} forms={forms:?} cells={cells} applied={applied} necessary={necessary} wrong={} false={} reason={} failed={} merge={}\nwrong={wrong:?}\nfalse={false_refusals:?}\nreason={reasons:?}\nfailed={failures:?}\nmerge={merges:?}",
        wrong.len(),
        false_refusals.len(),
        reasons.len(),
        failures.len(),
        merges.len()
    );
    (cells, applied, necessary, wrong.len(), false_refusals.len(), reasons.len(), failures.len(), merges.len())
}

macro_rules! form_test {
    ($test:ident, $form:ident, $cells:literal, $applied:literal, $necessary:literal) => {
        #[tokio::test(flavor = "multi_thread")]
        async fn $test() {
            assert_eq!(matrix(false, &[Form::$form]).await, ($cells, $applied, $necessary, 0, 0, 0, 0, 0));
        }
    };
}

form_test!(tree_address_matrix_nested_id, NestedId, 65, 62, 3);
form_test!(tree_address_matrix_nested_name, NestedName, 65, 61, 4);
form_test!(tree_address_matrix_selected, SelectedLeaves, 40, 35, 5);
form_test!(tree_address_matrix_group_id, GroupId, 60, 45, 15);
form_test!(tree_address_matrix_group_name, GroupName, 60, 44, 16);

#[tokio::test(flavor = "multi_thread")]
async fn selection_step_and_group_fact_premises() {
    assert_eq!(matrix(true, &[Form::NestedId, Form::NestedName, Form::SelectedLeaves, Form::GroupId, Form::GroupName]).await, (35, 35, 0, 0, 0, 0, 0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn selection_steps_before_a_recursive_write_preserve_changed_child_labels() {
    let bytes = fixture();
    let f = Fixture::from_bytes(FILE, &bytes, CallOptions::default());
    let (details, basis) = f.inspect().await;
    let ids = ids_by_name(&details);
    f.colleague(&[("layer.setLabelColor", json!({"layer":ids["L3"],"color":"green"}))]);
    let before = std::fs::read(f.file()).unwrap();
    let result = f
        .call(
            "run",
            json!({"basis":basis,"steps":[
                {"command":"layer.select","params":{"layer":ids["G"]}},
                {"command":"layer.select","params":{"layer":ids["L0"],"mode":"add"}},
                {"command":"layer.setLabelColor","params":{"color":"red"}}
            ]}),
        )
        .await;
    // same-field decision: the selected group's child label is also assigned.
    let after = std::fs::read(f.file()).unwrap();
    f.shutdown().await;
    let error = result.expect_err("the recursive assignment must refuse the changed child label");
    assert!(error["conflicts"].as_array().unwrap().iter().any(|conflict| conflict["reason"] == "changed"), "{error}");
    assert_eq!(after, before, "a refused recursive write must leave the saved file intact");
    let before = kition_photo_format::load_from_bytes(&before).unwrap();
    let after = kition_photo_format::load_from_bytes(&after).unwrap();
    let child = LayerId(ids["L3"]);
    assert_eq!(before.layer(child).unwrap().label, LabelColor::Green);
    assert_eq!(after.layer(child).unwrap().label, LabelColor::Green);
}
