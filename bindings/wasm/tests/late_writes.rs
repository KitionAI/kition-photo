//! C8-photo-3: a session journal is not a declared input to an agent's replay.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use kition_kit::KitDocument;
use kition_kit::dispatch::{Gate, Refusal, Role};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

struct Agent;
impl Gate for Agent {
    fn role(&self) -> Role {
        Role::Agent
    }
    fn check(&self, _: &str, _: &Value) -> Result<(), Refusal> {
        Ok(())
    }
}

fn replay(command: &str) {
    let mut doc = PhotoDocument::create("Late.kphoto", 64, 48).unwrap();
    doc.execute("layer.new.layer", json!({"name":"Ink"})).unwrap();
    doc.execute("select.rect", json!({"x":10,"y":10,"width":24,"height":20})).unwrap();
    doc.execute("edit.fill", json!({"color":"#ff0000"})).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.execute("filter.blur.gaussianBlur", json!({"radius":2})).unwrap();
    doc.execute("edit.transform", json!({"matrix":[1,0,0,1,4,0]})).unwrap();
    let version = doc.version().unwrap();
    let bytes = doc.save_sealed(Some("Late.kphoto")).unwrap();
    let mut colleague = PhotoDocument::open("Late.kphoto", &bytes).unwrap();
    colleague.execute("select.rect", json!({"x":20,"y":20,"width":4,"height":4})).unwrap();
    colleague.execute("edit.fill", json!({"color":"#0000ff"})).unwrap();
    colleague.execute("edit.deselectAll", json!({})).unwrap();
    doc.add_blobs(&colleague.blobs().unwrap()).unwrap();
    doc.import_changes(&colleague.export_changes(&version).unwrap()).unwrap();
    let before = doc.render(0.0, 0.0, 1.0, 64, 48).unwrap();
    let result = kition_kit::dispatch::with_gate(Arc::new(Agent), || doc.execute(command, json!({})));
    assert!(result.is_err(), "{command} replayed the unguarded session journal after a collaborator painted: {result:?}");
    assert_eq!(doc.render(0.0, 0.0, 1.0, 64, 48).unwrap(), before, "refused replay changed the image");
}

#[test]
fn last_filter_is_not_available_to_agents() {
    replay("filter.lastFilter");
}

#[test]
fn transform_again_is_not_available_to_agents() {
    replay("edit.transform.again");
}

#[test]
fn transform_again_copy_is_not_available_to_agents() {
    replay("edit.transform.againCopy");
}
