//! Each person's own view of a document, which the replica does not hold: whether a group is
//! expanded in the Layers panel, whether a note's popup is open, the timeline's playhead, and
//! which layer comp was applied last (with the Last Document State it saved). A read of the
//! replica keeps the view's own; a fresh read starts them as a new document would.

use std::collections::HashMap;

use kition_photo_doc::{Document, Layer, LayerContent, LayerId, MAX_GROUP_DEPTH};

fn groups(layers: &[Layer], out: &mut HashMap<LayerId, bool>, depth: usize) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for layer in layers {
        if let LayerContent::Group(group) = &layer.content {
            out.insert(layer.id, group.expanded);
            groups(&group.children, out, depth + 1);
        }
    }
}

fn set_groups(layers: &mut [Layer], expanded: &dyn Fn(LayerId) -> Option<bool>, depth: usize) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for layer in layers {
        let id = layer.id;
        if let LayerContent::Group(group) = &mut layer.content {
            if let Some(value) = expanded(id) {
                group.expanded = value;
            }
            set_groups(&mut group.children, expanded, depth + 1);
        }
    }
}

/// Puts `previous`'s view state on `document`, read again from the replica.
pub(crate) fn carry(previous: &Document, document: &mut Document) {
    let mut expanded = HashMap::new();
    groups(&previous.layers, &mut expanded, 0);
    set_groups(&mut document.layers, &|id| expanded.get(&id).copied(), 0);
    for (index, note) in document.notes.iter_mut().enumerate() {
        let before = match &note.key {
            Some(key) => previous.notes.iter().find(|other| other.key.as_ref() == Some(key)),
            None => previous.notes.get(index).filter(|other| other.key.is_none()),
        };
        if let Some(before) = before {
            note.open = before.open;
        }
    }
    if let (Some(timeline), Some(before)) = (document.timeline.as_mut(), previous.timeline.as_ref()) {
        timeline.current = before.current.min(timeline.duration.saturating_sub(1));
    }
    document.last_applied_comp = previous.last_applied_comp.filter(|id| document.layer_comps.iter().any(|comp| comp.id == *id));
    document.last_document_state.clone_from(&previous.last_document_state);
}

/// `document` with no view state of its own: what two views of one replica must agree on.
pub(crate) fn shared(document: &Document) -> Document {
    let mut document = document.clone();
    set_groups(&mut document.layers, &|_| Some(true), 0);
    for note in &mut document.notes {
        note.open = false;
    }
    if let Some(timeline) = document.timeline.as_mut() {
        timeline.current = 0;
    }
    document.last_applied_comp = None;
    document.last_document_state = None;
    document
}
