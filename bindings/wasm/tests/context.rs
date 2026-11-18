//! Explicit call context (Kit Process Protocol section 7a; task card C-photo): the editor state a
//! command reads besides its parameters (the active layer, the selected layers, the pixel
//! selection, the targeted channel, the foreground and background colors, the Layers panel's
//! view) is taken and put back without an edit, set for one call from the layers it acts on
//! (`on`), and an agent's command given `layer`, `region`, `foregroundColor` or
//! `backgroundColor` acts on what it was given and leaves the person's state as it was.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use kition_kit::dispatch::{Gate, Refusal, Role};
use kition_kit::failure::{CommandFailure, parse_failure};
use kition_kit::{CallContext, KitDocument};
use kition_photo_kit::{EDITOR_STATE, PhotoDocument};
use serde_json::{Value, json};

struct Agent;

impl Gate for Agent {
    fn role(&self) -> Role {
        Role::Agent
    }

    fn check(&self, _command: &str, _params: &Value) -> Result<(), Refusal> {
        Ok(())
    }
}

/// Runs `run` as the headless host runs an agent's command.
fn as_agent<T>(run: impl FnOnce() -> T) -> T {
    kition_kit::dispatch::with_gate(Arc::new(Agent), run)
}

struct Image {
    doc: PhotoDocument,
    a: u64,
    b: u64,
}

fn layer_id(answer: &Value) -> u64 {
    answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer id in {answer}"))
}

/// A white 64 x 48 image with two empty pixel layers A (below) and B (above). The person has B
/// active, a rectangular selection at (40, 30, 10, 8), and red over green as their colors.
fn image() -> Image {
    let mut doc = PhotoDocument::create("art.psd", 64, 48).unwrap();
    let a = layer_id(&doc.execute("layer.new.layer", json!({ "name": "A" })).unwrap());
    let b = layer_id(&doc.execute("layer.new.layer", json!({ "name": "B" })).unwrap());
    doc.execute("select.rect", json!({ "x": 40, "y": 30, "width": 10, "height": 8 })).unwrap();
    doc.execute("tools.setColors", json!({ "foreground": "#ff0000", "background": "#00ff00" })).unwrap();
    Image { doc, a, b }
}

/// The editor as the person sees it.
fn editor(doc: &PhotoDocument) -> Value {
    let inspected = doc.inspect().unwrap();
    json!({
        "active": inspected["activeLayer"],
        "selected": inspected["selectedLayers"],
        "selection": inspected["selectionBounds"],
        "state": doc.editor_state(),
    })
}

fn context(on: &[Value]) -> CallContext {
    CallContext { on: on.to_vec(), part: None, time: None }
}

/// A layer's non-transparent bounds, as inspect lists them; `null` for an empty layer.
fn bounds(doc: &PhotoDocument, id: u64) -> Value {
    fn find(layers: &Value, id: u64) -> Option<Value> {
        layers.as_array()?.iter().find_map(|layer| if layer["id"] == id { Some(layer["bounds"].clone()) } else { find(&layer["children"], id) })
    }
    let found = find(&doc.inspect().unwrap()["layers"], id).unwrap_or_else(|| panic!("no layer {id}"));
    if found[2] == 0 || found[3] == 0 { Value::Null } else { found }
}

#[test]
fn setting_the_layers_a_call_acts_on_and_putting_them_back_is_no_edit() {
    let Image { mut doc, a, b } = image();
    kition_kit::testing::check_context(&mut doc, &context(&[json!(a)]), EDITOR_STATE).unwrap();
    kition_kit::testing::check_context(&mut doc, &context(&[json!(a.to_string()), json!(b)]), EDITOR_STATE).unwrap();
}

#[test]
fn on_makes_the_layers_selected_and_the_first_active() {
    let Image { mut doc, a, b } = image();
    let saved = doc.editor_state();
    doc.set_context(&context(&[json!(a), json!(b)])).unwrap();
    let inspected = doc.inspect().unwrap();
    assert_eq!((inspected["activeLayer"].as_u64(), inspected["selectedLayers"].clone()), (Some(a), json!([a, b])));
    doc.restore_editor_state(&saved).unwrap();
    assert_eq!(doc.inspect().unwrap()["activeLayer"].as_u64(), Some(b));
}

#[test]
fn contexts_the_image_lacks_are_refused_with_their_reason() {
    let Image { mut doc, a, .. } = image();
    let before = editor(&doc);
    let reason = |error: String| parse_failure(&error).map(|(text, reason)| (text.to_owned(), reason)).unwrap_or_else(|| panic!("untyped: {error}"));
    let (text, why) = reason(doc.set_context(&context(&[json!(999)])).unwrap_err());
    assert_eq!(why, CommandFailure::InvalidParams, "{text}");
    assert!(text.contains("999"), "{text}");
    let (text, why) = reason(doc.set_context(&context(&[json!("#1")])).unwrap_err());
    assert_eq!(why, CommandFailure::InvalidParams, "{text}");
    let (text, why) = reason(doc.set_context(&CallContext { on: vec![json!(a)], part: Some(json!({ "page": 1 })), time: None }).unwrap_err());
    assert_eq!(why, CommandFailure::InvalidParams, "{text}");
    assert!(text.contains("no parts"), "{text}");
    let (text, why) = reason(doc.set_context(&CallContext { on: vec![], part: None, time: Some(1.0) }).unwrap_err());
    assert_eq!(why, CommandFailure::Disabled, "{text}");
    assert_eq!(editor(&doc), before, "a refused context changes nothing");
}

#[test]
fn the_pixel_selection_and_the_colors_come_back_after_an_agent_changed_them() {
    let Image { mut doc, .. } = image();
    let saved = doc.editor_state();
    let before = editor(&doc);
    let revision = doc.revision();
    as_agent(|| {
        doc.execute("select.rect", json!({ "x": 2, "y": 2, "width": 5, "height": 5 })).unwrap();
        doc.execute("tools.setColors", json!({ "foreground": "#0000ff" })).unwrap();
    });
    assert_ne!(editor(&doc), before);
    let edited = (doc.revision(), doc.dirty());
    doc.restore_editor_state(&saved).unwrap();
    assert_eq!(editor(&doc), before);
    assert_eq!((doc.revision(), doc.dirty()), edited, "putting the state back is no edit");
    assert!(edited.0 >= revision);
}

#[test]
fn an_agents_command_given_a_layer_acts_on_it_and_leaves_the_active_layer() {
    let Image { mut doc, a, b } = image();
    let before = editor(&doc);
    as_agent(|| doc.execute("layer.setProps", json!({ "layer": a, "opacity": 0.4 }))).unwrap();
    as_agent(|| doc.execute("filter.other.offset", json!({ "layer": a, "horizontal": 3, "vertical": 0 }))).unwrap();
    as_agent(|| doc.execute("edit.fill", json!({ "layer": a, "contents": "black", "region": { "rect": [4, 4, 6, 6] } }))).unwrap();
    assert_eq!(editor(&doc), before, "the person's active layer, selection and colors stay");
    assert_eq!(bounds(&doc, a), json!([4, 4, 6, 6]), "the fill went to A, inside the region");
    assert_eq!(bounds(&doc, b), Value::Null, "B is untouched");
}

#[test]
fn an_agents_fill_given_a_region_and_a_color_fills_there_with_it() {
    let Image { mut doc, a, .. } = image();
    let before = editor(&doc);
    as_agent(|| {
        doc.execute("edit.fill", json!({ "layer": a, "contents": "foreground", "foregroundColor": "#0000ff", "region": { "ellipse": [10, 10, 8, 8] } }))
    })
    .unwrap();
    assert_eq!(editor(&doc), before);
    assert_eq!(bounds(&doc, a), json!([10, 10, 8, 8]));
    let pixel = doc.render_area(14, 14, 15, 15, 1, 1).unwrap();
    assert_eq!(&pixel[..3], &[0, 0, 255], "the given color, not the person's red");
}

#[test]
fn a_region_names_a_layers_pixels_a_channel_or_a_polygon() {
    let Image { mut doc, a, b } = image();
    as_agent(|| doc.execute("edit.fill", json!({ "layer": a, "contents": "black", "region": { "rect": [0, 0, 8, 8] } }))).unwrap();
    as_agent(|| doc.execute("edit.fill", json!({ "layer": b, "contents": "black", "region": { "layer": a } }))).unwrap();
    assert_eq!(bounds(&doc, b), json!([0, 0, 8, 8]));
    as_agent(|| doc.execute("edit.fill", json!({ "layer": b, "contents": "black", "region": { "polygon": [[20, 20], [30, 20], [30, 30]] } }))).unwrap();
    assert_eq!(bounds(&doc, b), json!([0, 0, 30, 30]));
    for bad in [json!({ "rect": [0, 0] }), json!({ "circle": 3 }), json!("everything"), json!({ "layer": 999 }), json!({ "channel": "Nope" })] {
        let error = as_agent(|| doc.execute("edit.fill", json!({ "layer": b, "contents": "black", "region": bad.clone() }))).unwrap_err();
        assert_eq!(parse_failure(&error).map(|(_, reason)| reason), Some(CommandFailure::InvalidParams), "{bad}: {error}");
    }
}

#[test]
fn an_agents_command_that_moves_the_editor_keeps_its_effect() {
    let Image { mut doc, a, .. } = image();
    as_agent(|| doc.execute("layer.select", json!({ "layer": a }))).unwrap();
    assert_eq!(doc.inspect().unwrap()["activeLayer"].as_u64(), Some(a));
}

#[test]
fn a_layer_an_agents_command_makes_stays_active_for_the_next_step() {
    let Image { mut doc, a, .. } = image();
    let copy = layer_id(&as_agent(|| doc.execute("layer.duplicate", json!({ "layer": a }))).unwrap());
    assert_eq!(doc.inspect().unwrap()["activeLayer"].as_u64(), Some(copy));
}

#[test]
fn the_persons_commands_are_not_bracketed() {
    let Image { mut doc, a, .. } = image();
    doc.execute("edit.fill", json!({ "contents": "black" })).unwrap();
    assert!(bounds(&doc, a).is_null());
    assert_eq!(doc.inspect().unwrap()["selectionBounds"], json!([40, 30, 10, 8]));
}

/// The pixel selection is editor state a command reads (kition-sdk `EditorState::Area`, C8-fix-10):
/// every command that acts inside it declares `area`, the editor state carries it, and setting a
/// context and putting it back leaves it (the image keeps a selection at (40, 30, 10, 8)).
#[test]
fn the_commands_that_act_inside_the_selection_read_the_area() {
    let Image { mut doc, a, .. } = image();
    let commands = doc.commands();
    let reading: Vec<&str> =
        commands.iter().filter(|command| command.reads.contains(&kition_kit::EditorState::Area)).map(|command| command.id.as_str()).collect();
    assert_eq!(reading.len(), kition_photo_kit::acts::REGION.len(), "{reading:?}");
    for id in ["edit.fill", "edit.clear", "paint.stroke"] {
        assert!(reading.contains(&id), "{id} reads the area");
    }
    assert!(!reading.contains(&"layer.setProps"), "a property reads no area");
    assert_eq!(doc.editor_state()["area"], json!([40, 30, 10, 8]));
    kition_kit::testing::check_context(&mut doc, &context(&[json!(a)]), EDITOR_STATE).unwrap();
    assert_eq!(doc.inspect().unwrap()["selectionBounds"], json!([40, 30, 10, 8]));
}
