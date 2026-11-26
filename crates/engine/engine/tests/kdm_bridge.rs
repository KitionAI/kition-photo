//! The command bridge to the KDM replica (Kition phase 3a, Task 3): every edit is one
//! transaction, a second session that imports it reproduces the document, undo is per person, a
//! coalesced gesture is one undo step, the selection is never replicated, and changes made
//! elsewhere on the document's line of changes (in order) come in without taking back its own.

use kition_photo_doc::{Document, Layer, LayerContent, LayerId};
use kition_photo_engine::{Origin, Session, UndoScope, Version};
use kition_photo_geom::Rect;
use serde_json::{Value, json};

fn new_session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 600, "height": 400, "background": "white"})).unwrap();
    s
}

/// A second session holding a replica of `a`'s active document.
fn peer(a: &mut Session) -> Session {
    let mut b = Session::new();
    b.open_snapshot(&a.snapshot().unwrap(), a.blobs().unwrap().clone(), "peer").unwrap();
    b
}

/// Sends `from`'s changes that `to` lacks (and the blobs they need) to `to`.
fn send(from: &mut Session, to: &mut Session) {
    let since = to.version().unwrap();
    let bytes = from.export_changes(&since).unwrap();
    to.add_blobs(from.blobs().unwrap()).unwrap();
    to.import_changes(&bytes).unwrap();
}

fn doc(s: &Session) -> &Document {
    &s.active().unwrap().doc
}

fn active(s: &Session) -> LayerId {
    s.active().unwrap().active_layer.unwrap()
}

/// The layer tree and every layer's own state and pixels, without engine-local ids.
fn fingerprint(d: &Document) -> Vec<Value> {
    let bounds = Rect::new(0, 0, d.size.width as i32, d.size.height as i32);
    d.walk()
        .into_iter()
        .map(|(path, _, l)| {
            let pixels = match &l.content {
                LayerContent::Raster(s) => {
                    let px = s.read_region(bounds);
                    let sum: f64 = px.iter().map(|v| f64::from(*v)).sum();
                    json!([px.len(), sum, blake(&px)])
                }
                LayerContent::Group(g) => json!(g.children.len()),
                _ => Value::Null,
            };
            json!({
                "depth": path.len(), "name": l.name, "visible": l.visible, "opacity": l.opacity,
                "blend": format!("{:?}", l.blend), "pixels": pixels,
            })
        })
        .collect()
}

fn blake(px: &[f32]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in px {
        for b in v.to_bits().to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn assert_same(a: &mut Session, b: &mut Session, what: &str) {
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap(), "{what}: replicas differ");
    assert_eq!(fingerprint(doc(a)), fingerprint(doc(b)), "{what}: documents differ");
}

/// Leaf paths that differ between two JSON values.
fn changed_paths(a: &Value, b: &Value, path: String, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                changed_paths(x.get(key).unwrap_or(&Value::Null), y.get(key).unwrap_or(&Value::Null), format!("{path}/{key}"), out);
            }
        }
        _ if a != b => out.push(path),
        _ => {}
    }
}

fn state(s: &Session) -> Value {
    s.active().unwrap().kdm().unwrap().replica().state_json().unwrap()
}

fn object_of(s: &Session, id: LayerId) -> String {
    s.active().unwrap().kdm().unwrap().model().ids().object(id).unwrap().as_str().to_string()
}

#[test]
fn a_stroke_is_one_change_touching_only_its_tiles() {
    let mut s = new_session();
    s.execute("layer.new.layer", json!({})).unwrap();
    let layer = active(&s);
    let before = state(&s);
    let version = s.version().unwrap();
    s.execute("paint.stroke", json!({"points": [[10, 10], [60, 20]], "size": 8, "color": "#ff0000"})).unwrap();
    assert_eq!(s.active().unwrap().history.undo_external(), 1, "one replica undo step");
    assert_ne!(s.version().unwrap(), version);
    let mut paths = Vec::new();
    changed_paths(&before, &state(&s), String::new(), &mut paths);
    let tiles = format!("/objects/{}/tiles/", object_of(&s, layer));
    assert!(!paths.is_empty());
    assert!(paths.iter().all(|p| p.starts_with(&tiles)), "only the layer's tiles changed: {paths:?}");
    // The stroke spans tile (0,0) only.
    assert_eq!(paths, vec![format!("{tiles}0,0")]);

    // And the change alone, imported into a peer that had the layer, reproduces the pixels.
    let mut a = new_session();
    a.execute("layer.new.layer", json!({})).unwrap();
    let mut b = peer(&mut a);
    a.execute("paint.stroke", json!({"points": [[10, 10], [300, 200]], "size": 8, "color": "#ff0000"})).unwrap();
    send(&mut a, &mut b);
    assert_same(&mut a, &mut b, "after the stroke");
}

#[test]
fn layer_commands_reproduce_in_a_second_session() {
    let mut a = new_session();
    a.execute("layer.new.layer", json!({"name": "One"})).unwrap();
    a.execute("paint.stroke", json!({"points": [[10, 10], [300, 200]], "size": 12, "color": "#00ff00"})).unwrap();
    let mut b = peer(&mut a);
    assert_same(&mut a, &mut b, "opened from the snapshot");
    let one = active(&a);
    let steps: Vec<(&str, Value)> = vec![
        ("layer.setProps", json!({"opacity": 0.5, "blend": "multiply", "name": "Renamed"})),
        ("layer.duplicate", json!({})),
        ("layer.new.group", json!({"name": "Group"})),
        ("layer.moveTo", json!({"layer": one.0, "target": 0, "position": "into"})),
        ("layer.new.layer", json!({"name": "In group"})),
        ("arrange.sendBackward", json!({})),
        ("layer.delete", json!({"layer": one.0})),
    ];
    let mut group = None;
    for (cmd, mut params) in steps {
        if cmd == "layer.moveTo" {
            params["target"] = json!(group.unwrap_or(0));
        }
        let version = a.version().unwrap();
        let r = a.execute(cmd, params).unwrap_or_else(|e| panic!("{cmd}: {e}"));
        if cmd == "layer.new.group" {
            group = r["layer"].as_u64();
        }
        if cmd == "layer.moveTo" {
            // Select a layer inside the group so the next new layer lands there.
            a.execute("layer.select", json!({"layer": one.0})).unwrap();
        }
        assert_ne!(a.version().unwrap(), version, "{cmd} wrote a change");
        assert_eq!(a.active().unwrap().history.undo_external(), 1, "{cmd}: one transaction, one undo step");
        send(&mut a, &mut b);
        assert_same(&mut a, &mut b, cmd);
    }
    // The new layer went into the group.
    let in_group = doc(&a).walk().into_iter().find(|(_, _, l)| l.name == "In group").map(|(p, _, _)| p.len());
    assert_eq!(in_group, Some(2), "nested one level");
}

#[test]
fn undo_after_a_remote_edit_keeps_it_and_reverts_only_the_local_one() {
    let mut a = new_session();
    a.execute("layer.new.layer", json!({"name": "Shared"})).unwrap();
    let mut b = peer(&mut a);
    let shared_b = doc(&b).walk().into_iter().find(|(_, _, l)| l.name == "Shared").unwrap().2.id;
    a.execute("paint.stroke", json!({"points": [[10, 10], [100, 100]], "size": 10, "color": "#0000ff"})).unwrap();
    // One line of changes: b takes a's stroke, then edits the same layer and adds one.
    send(&mut a, &mut b);
    b.execute("layer.setProps", json!({"layer": shared_b.0, "opacity": 0.25})).unwrap();
    b.execute("layer.new.layer", json!({"name": "Theirs"})).unwrap();
    send(&mut b, &mut a);
    assert_same(&mut a, &mut b, "exchanged");
    let shared = active(&a);
    assert!(a.active().unwrap().doc.layer(shared).unwrap().surface().unwrap().tile_count() > 0);

    assert!(a.undo(), "the stroke undoes");
    let d = doc(&a);
    let layer = d.layer(shared).expect("the active layer keeps its id through the reload");
    assert_eq!(layer.surface().unwrap().tile_count(), 0, "the local stroke is gone");
    assert_eq!(layer.opacity, 0.25, "the remote opacity stays");
    assert!(d.walk().iter().any(|(_, _, l)| l.name == "Theirs"), "the remote layer stays");
    assert_eq!(active(&a), shared);

    // The next undo is A's own New Layer, not B's edits.
    assert!(a.undo());
    assert!(doc(&a).walk().iter().all(|(_, _, l)| l.name != "Shared"));
    assert!(doc(&a).walk().iter().any(|(_, _, l)| l.name == "Theirs"));
    send(&mut a, &mut b);
    assert_same(&mut a, &mut b, "after the undos");
    assert!(a.redo() && a.redo());
    send(&mut a, &mut b);
    assert_same(&mut a, &mut b, "after the redos");
    assert_eq!(doc(&a).layer(shared).unwrap().opacity, 0.25);
}

#[test]
fn a_coalesced_drag_is_one_undo_step() {
    let mut s = new_session();
    s.execute("layer.new.layer", json!({})).unwrap();
    let layer = active(&s);
    let past = s.active().unwrap().history.past_len();
    for opacity in [0.9, 0.7, 0.5, 0.3] {
        s.execute("layer.setProps", json!({"opacity": opacity, "coalesce": "opacity-drag"})).unwrap();
    }
    let st = s.active().unwrap();
    assert_eq!(st.history.past_len(), past + 1, "one history step");
    assert_eq!(st.history.undo_external(), 1, "one replica undo step");
    assert!(s.undo());
    assert_eq!(doc(&s).layer(layer).unwrap().opacity, 1.0, "one undo takes the whole drag back");
    assert!(doc(&s).layer(layer).is_some(), "and not the new layer");
    assert!(s.redo());
    assert_eq!(doc(&s).layer(layer).unwrap().opacity, 0.3);
}

#[test]
fn selection_changes_write_nothing_to_the_replica() {
    let mut s = new_session();
    let b = peer(&mut s);
    let version = s.version().unwrap();
    let hash = s.state_hash().unwrap();
    s.execute("edit.selectAll", json!({})).unwrap();
    s.execute("select.rect", json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
    s.execute("edit.deselectAll", json!({})).unwrap();
    s.execute("select.rect", json!({"x": 5, "y": 5, "width": 20, "height": 20})).unwrap();
    assert_eq!(s.version().unwrap(), version, "no transaction");
    assert_eq!(s.state_hash().unwrap(), hash);
    assert!(doc(&s).selection.is_some());
    assert!(doc(&b).selection.is_none());
    // The selection still undoes locally, without a replica step.
    assert_eq!(s.active().unwrap().history.undo_external(), 0);
    assert!(s.undo());
    assert!(doc(&s).selection.is_none(), "undo of the last selection restores the one before (none)");
    assert_eq!(s.version().unwrap(), version);
}

#[test]
fn a_remote_delete_of_the_active_layer_picks_a_neighbour() {
    let mut a = new_session();
    a.execute("layer.new.layer", json!({"name": "Below"})).unwrap();
    let below = active(&a);
    a.execute("layer.new.layer", json!({"name": "Doomed"})).unwrap();
    a.execute("layer.new.layer", json!({"name": "Above"})).unwrap();
    let mut b = peer(&mut a);
    let doomed_b = doc(&b).walk().into_iter().find(|(_, _, l)| l.name == "Doomed").unwrap().2.id;
    let doomed = doc(&a).walk().into_iter().find(|(_, _, l)| l.name == "Doomed").unwrap().2.id;
    a.execute("layer.select", json!({"layer": doomed.0})).unwrap();
    b.execute("layer.delete", json!({"layer": doomed_b.0})).unwrap();
    send(&mut b, &mut a);
    assert!(doc(&a).layer(doomed).is_none());
    assert_eq!(active(&a), below, "the layer below takes over");
    assert_same(&mut a, &mut b, "after the delete");
}

/// KitModel 1.1 M12a: on one replica, the person's undo and an agent's undo each take back only
/// their own steps and keep the other's.
#[test]
fn agent_edits_record_their_origin_and_undo_in_their_own_scope() {
    let mut s = new_session();
    let agent = Origin::agent("test");
    let names = |s: &Session| doc(s).walk().iter().map(|(_, _, l)| l.name.clone()).collect::<Vec<_>>();
    s.execute("layer.new.layer", json!({"name": "Person"})).unwrap();
    s.execute_as("layer.new.layer", json!({"name": "Agent"}), &agent).unwrap();
    assert_eq!(s.origin(), kition_photo_engine::kdm_bridge::local_person(), "the origin is restored");
    assert!(s.undo(), "the person's undo takes their own step");
    assert!(names(&s).contains(&"Agent".to_owned()), "and keeps the agent's: {:?}", names(&s));
    assert!(!names(&s).contains(&"Person".to_owned()), "{:?}", names(&s));
    s.execute_as("edit.undo", json!({}), &agent).unwrap();
    assert!(!names(&s).contains(&"Agent".to_owned()), "the agent's undo takes the agent's step: {:?}", names(&s));
    assert!(!s.undo(), "the person has nothing left to undo");
}

/// M12a: an agent's undo of a step under the person's later edit keeps the person's edit, and
/// `edit.undo`'s `scope` (engine rule E14a) names whose step to undo.
#[test]
fn an_agent_undo_under_the_persons_later_step_keeps_the_persons_edit() {
    let mut s = new_session();
    let agent = Origin::agent("test");
    let names = |s: &Session| doc(s).walk().iter().map(|(_, _, l)| l.name.clone()).collect::<Vec<_>>();
    s.execute_as("layer.new.layer", json!({"name": "Agent"}), &agent).unwrap();
    s.execute("layer.new.layer", json!({"name": "Person"})).unwrap();
    s.execute_as("edit.undo", json!({}), &agent).unwrap();
    assert!(names(&s).contains(&"Person".to_owned()), "the person's later edit stays: {:?}", names(&s));
    assert!(!names(&s).contains(&"Agent".to_owned()), "{:?}", names(&s));
    s.execute("edit.redo", json!({"scope": "agent"})).unwrap();
    assert!(names(&s).contains(&"Agent".to_owned()), "a person's redo in the agent scope brings it back: {:?}", names(&s));
    assert!(s.execute("edit.undo", json!({"scope": "other"})).is_err(), "an unknown scope is refused");
}

/// A tiny deterministic generator (no extra dependency).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

#[test]
fn undo_still_works_for_compound_and_folded_steps() {
    // Transform Again on a copy folds Duplicate + Transform into one step (purge_last).
    let mut s = new_session();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.execute("paint.stroke", json!({"points": [[10, 10], [60, 20]], "size": 8})).unwrap();
    s.execute("edit.transform", json!({"matrix": [1, 0, 0, 1, 10, 5]})).unwrap();
    let count = doc(&s).layer_count();
    s.execute("edit.transform.againCopy", json!({})).unwrap();
    assert_eq!(doc(&s).layer_count(), count + 1);
    assert_eq!(s.active().unwrap().history.undo_external(), 2, "the folded step undoes both transactions");
    assert!(s.undo());
    assert_eq!(doc(&s).layer_count(), count, "one undo takes the copy back");
    assert!(s.redo());
    assert_eq!(doc(&s).layer_count(), count + 1);
    // The whole change since the start reaches a peer that has nothing yet.
    let mut fresh = Session::new();
    fresh.open_snapshot(&s.snapshot().unwrap(), s.blobs().unwrap().clone(), "copy").unwrap();
    assert_eq!(fingerprint(doc(&fresh)), fingerprint(doc(&s)));
    assert!(!s.export_changes(&Version::empty()).unwrap().is_empty());
}

#[test]
fn layer_ids_survive_an_undo_that_reloads_the_view() {
    let mut s = new_session();
    s.execute("layer.new.layer", json!({"name": "Keep"})).unwrap();
    let keep = active(&s);
    s.execute("layer.new.group", json!({})).unwrap();
    let ids: Vec<LayerId> = doc(&s).walk().into_iter().map(|(_, _, l)| l.id).collect();
    s.execute("layer.delete", json!({"layer": keep.0})).unwrap();
    assert!(s.undo());
    let again: Vec<LayerId> = doc(&s).walk().into_iter().map(|(_, _, l)| l.id).collect();
    assert_eq!(ids, again);
    let _: Option<&Layer> = doc(&s).layer(keep);
}

#[test]
fn a_compound_command_is_one_undo_step_on_both_stacks() {
    // Crop with an angle rotates, then crops: one history step, one replica undo group.
    let mut a = new_session();
    a.execute("paint.stroke", json!({"points": [[100, 100], [400, 300]], "size": 20, "color": "#123456"})).unwrap();
    let mut b = peer(&mut a);
    let size = doc(&a).size;
    let past = a.active().unwrap().history.past_len();
    a.execute("image.crop", json!({"x": 50, "y": 40, "width": 300, "height": 200, "angle": 10})).unwrap();
    assert_eq!(a.active().unwrap().history.past_len(), past + 1);
    assert_eq!(a.active().unwrap().history.undo_external(), 1, "one replica step");
    send(&mut a, &mut b);
    assert_same(&mut a, &mut b, "cropped");
    assert!(a.undo());
    assert_eq!(doc(&a).size, size);
    send(&mut a, &mut b);
    assert_same(&mut a, &mut b, "crop undone");
}

#[test]
fn a_document_that_leaves_its_replica_gets_a_fresh_one_on_the_next_edit() {
    let mut s = new_session();
    s.execute("layer.new.layer", json!({"name": "Before"})).unwrap();
    assert!(s.active().unwrap().kdm().is_some());
    let mut old_peer = peer(&mut s);
    let old_snapshot = s.snapshot().unwrap();
    // Groups nested past the replica's depth limit: the write is refused and the document
    // leaves its replica (a reset hosts see), keeping the edit and its undo.
    s.edit("Deep", |doc, _| {
        let mut layer = Layer::raster("Leaf", doc.pixel_format());
        for i in 0..=kition_photo_doc::MAX_GROUP_DEPTH + 1 {
            layer = Layer::group(format!("G{i}"), vec![layer]);
        }
        doc.layers.push(layer);
        Ok(())
    })
    .unwrap();
    let st = s.active().unwrap();
    assert!(st.kdm().is_none(), "the document left the replica");
    assert_eq!(st.kdm_resets(), 1);
    let layers = doc(&s).layer_count();
    assert!(s.undo());
    assert!(doc(&s).layer_count() < layers, "undo works from the snapshots");
    assert!(s.redo());
    assert_eq!(doc(&s).layer_count(), layers, "the deep groups are back");
    // While the tree is too deep there is no replica to sync with.
    assert!(s.snapshot().is_err());
    // An edit that makes it fit again makes a fresh replica (a new KDM document).
    let deep = doc(&s).layers.last().unwrap().id;
    s.execute("layer.delete", json!({"layer": deep.0})).unwrap();
    assert!(s.active().unwrap().kdm().is_some(), "a fresh replica");
    s.execute("layer.setProps", json!({"opacity": 0.5})).unwrap();
    let mut fresh = peer(&mut s);
    assert_same(&mut s, &mut fresh, "a new peer joins from the fresh snapshot");
    // The old peer can't import the fresh replica's changes: it needs the full snapshot.
    let bytes = s.export_changes(&Version::empty()).unwrap();
    assert!(old_peer.import_changes(&bytes).is_err(), "a different KDM document");
    assert_ne!(s.snapshot().unwrap(), old_snapshot);
    // The steps taken before the fresh replica still undo, from their snapshots.
    assert!(s.undo(), "undo the opacity change");
    assert!(s.undo(), "undo the delete (a snapshot step)");
    assert!(doc(&s).layer(deep).is_some());
}

/// Every frame's pixels, for comparing video stacks.
fn frames(s: &Session, id: LayerId) -> Option<Vec<String>> {
    let d = doc(s);
    let bounds = Rect::new(0, 0, d.size.width as i32, d.size.height as i32);
    d.layer(id)?.video.as_ref().map(|v| v.frames.iter().map(|f| blake(&f.read_region(bounds))).collect())
}

/// A three-frame video layer with paint on frames 0 and 1.
fn video_session() -> (Session, LayerId) {
    let mut s = new_session();
    s.execute("timeline.add", json!({"duration": 3, "fps": 12})).unwrap();
    let id = LayerId(s.execute("layer.videoLayers.newBlankVideoLayer", json!({})).unwrap()["layer"].as_u64().unwrap());
    s.execute("paint.stroke", json!({"points": [[10, 10], [200, 100]], "size": 12, "color": "#ff0000"})).unwrap();
    s.execute("timeline.setFrame", json!({"frame": 1})).unwrap();
    s.execute("paint.stroke", json!({"points": [[300, 50], [500, 300]], "size": 12, "color": "#00ff00"})).unwrap();
    s.execute("timeline.setFrame", json!({"frame": 0})).unwrap();
    let f = frames(&s, id).unwrap();
    assert_eq!(f.len(), 3);
    assert_ne!(f[0], f[2], "frame 0 is painted");
    assert_ne!(f[1], f[2], "frame 1 is painted");
    (s, id)
}

#[test]
fn undoing_a_video_rasterize_brings_every_frame_back() {
    let (mut s, id) = video_session();
    let before = frames(&s, id);
    let mut b = peer(&mut s);
    assert_eq!(frames(&b, doc(&b).walk().into_iter().find(|(_, _, l)| l.video.is_some()).unwrap().2.id), before, "a peer has the frames");
    s.execute("layer.rasterize.video", json!({})).unwrap();
    assert!(frames(&s, id).is_none());
    send(&mut s, &mut b);
    assert!(s.undo());
    assert_eq!(frames(&s, id), before, "every frame is back");
    assert!(s.redo());
    assert!(frames(&s, id).is_none());
    assert!(s.undo());
    send(&mut s, &mut b);
    let theirs = doc(&b).walk().into_iter().find(|(_, _, l)| l.video.is_some()).map(|(_, _, l)| l.id);
    assert_eq!(theirs.and_then(|t| frames(&b, t)), before, "the peer gets the frames back too");
}

#[test]
fn undoing_a_video_layer_delete_brings_it_back_with_its_frames() {
    let (mut s, id) = video_session();
    let before = frames(&s, id);
    s.execute("layer.delete", json!({"layer": id.0})).unwrap();
    assert!(doc(&s).layer(id).is_none());
    assert!(s.undo());
    assert_eq!(frames(&s, id), before);
}

#[test]
fn undoing_insert_blank_frame_removes_the_frame() {
    let (mut s, id) = video_session();
    let before = frames(&s, id);
    s.execute("layer.videoLayers.insertBlankFrame", json!({})).unwrap();
    assert_eq!(frames(&s, id).unwrap().len(), 4);
    assert_eq!(s.active().unwrap().history.undo_external(), 1, "a replica step");
    assert!(s.undo());
    assert_eq!(frames(&s, id), before);
    assert!(s.redo());
    assert_eq!(frames(&s, id).unwrap().len(), 4);
}

#[test]
fn blobs_stay_bounded_by_the_history() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 1024, "height": 1024, "background": "white"})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    let limit = s.active().unwrap().history.max_states;
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut largest = 0usize;
    for i in 0..500 {
        let (x, y) = (rng.below(900) as i64, rng.below(900) as i64);
        s.execute("paint.stroke", json!({"points": [[x, y], [x + 60, y + 40]], "size": 10, "color": format!("#{:06x}", rng.below(0xffffff))})).unwrap();
        if i % 50 == 49 {
            largest = largest.max(s.blobs().unwrap().0.len());
        }
    }
    let blobs = s.blobs().unwrap();
    let (count, bytes) = (blobs.0.len(), blobs.0.values().map(|b| b.len()).sum::<usize>());
    // 16 tiles per layer: the current state plus what the held steps can bring back (a stroke
    // touches up to 4 tiles), far below the 500 strokes' worth.
    let bound = 2 * 16 + (limit + 1) * 4 * 2;
    assert!(count <= bound && largest <= bound, "{count} blobs (largest {largest}), bound {bound}");
    assert!(bytes < bound * 300 * 1024, "{bytes} bytes");
    let (undo, _) = s.active().unwrap().kdm().unwrap().steps(UndoScope::Person);
    assert!(undo <= limit, "{undo} replica undo steps held, history holds {limit}");
    // Every held step still undoes and redoes (none lost its blobs), and the oldest stops.
    let mut undone = 0;
    while s.undo() {
        undone += 1;
    }
    assert!(undone + 1 >= limit && undone <= limit, "{undone} undos");
    assert_eq!(s.active().unwrap().kdm_resets(), 0, "no undo failed");
    while s.redo() {}
    let mut b = peer(&mut s);
    assert_same(&mut s, &mut b, "a peer from the trimmed replica");
    // Purging the history drops what only undo needed.
    s.execute("edit.purge.histories", json!({})).unwrap();
    assert!(s.blobs().unwrap().0.len() <= 2 * 16, "{} blobs after the purge", s.blobs().unwrap().0.len());
}

#[test]
fn opening_a_document_makes_no_replica_until_it_is_edited() {
    use kition_photo_raster::Surface;
    // A large photo-like layer: making a replica means compressing every tile.
    let mut d = Document::new("big", kition_photo_doc::Size { width: 4000, height: 3000 }, kition_photo_doc::ColorMode::Rgb, kition_photo_doc::SampleType::U8);
    let fmt = d.pixel_format();
    let mut surface = Surface::new(fmt);
    let mut rng = Rng(99);
    let px: Vec<f32> = (0..4000 * 3000 * 4).map(|i| if i % 4 == 3 { 1.0 } else { (rng.below(256) as f32) / 255.0 }).collect();
    surface.write_region(Rect::new(0, 0, 4000, 3000), &px);
    d.layers.push(Layer::new("Photo", LayerContent::Raster(surface)));
    let mut s = Session::new();
    let t = std::time::Instant::now();
    s.add_document(d, None);
    let open = t.elapsed();
    assert!(s.active().unwrap().kdm().is_none(), "no replica yet");
    // Selecting writes nothing, so it makes none either.
    s.execute("edit.selectAll", json!({})).unwrap();
    assert!(s.active().unwrap().kdm().is_none());
    assert!(open < std::time::Duration::from_millis(100), "opening took {open:?}");
    // The sample PSD opens without one too.
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../web/public/sample/Sample.psd")).unwrap();
    let sample = kition_photo_io::import("Sample.psd", &bytes).unwrap().document;
    let t = std::time::Instant::now();
    s.add_document(sample, None);
    assert!(t.elapsed() < std::time::Duration::from_millis(50), "{:?}", t.elapsed());
    assert!(s.active().unwrap().kdm().is_none());
    // The first edit makes it, and it holds the document as opened plus the edit.
    s.execute("layer.new.layer", json!({"name": "First edit"})).unwrap();
    assert!(s.active().unwrap().kdm().is_some());
    let mut b = peer(&mut s);
    assert_same(&mut s, &mut b, "the lazily made replica");
    assert!(s.undo());
    assert!(doc(&s).walk().iter().all(|(_, _, l)| l.name != "First edit"));
}

/// `blobs_since` ships only a change's own blobs, not the whole store, and still converges a
/// peer that applies `export_changes` plus those blobs (the live-sync path, Kition phase 3c).
#[test]
fn blobs_since_ships_only_what_a_change_needs_and_still_converges_a_peer() {
    let mut a = new_session();
    a.execute("layer.new.layer", json!({})).unwrap();
    let mut b = peer(&mut a);
    let since = b.version().unwrap();

    // Several strokes on the same tile: the store keeps the older ones for undo, but the one
    // change being sent only needs its own new tile.
    for (x, color) in [(10, "#ff0000"), (12, "#00ff00"), (14, "#0000ff")] {
        a.execute("paint.stroke", json!({"points": [[x, 10], [x + 4, 14]], "size": 4, "color": color})).unwrap();
    }
    let new_blobs = a.blobs_since(&since).unwrap();
    assert!(!new_blobs.0.is_empty(), "the change needs at least its own tile");
    assert!(new_blobs.0.len() < a.blobs().unwrap().0.len(), "fewer than the whole store, which also holds undo history");

    let bytes = a.export_changes(&since).unwrap();
    b.add_blobs(&new_blobs).unwrap();
    b.import_changes(&bytes).unwrap();
    assert_same(&mut a, &mut b, "after importing only the change's own blobs");
}

/// `blobs_since(Version::empty())` answers every blob the current state references: enough on
/// its own to open a peer from nothing (no `export_changes` needed first).
#[test]
fn blobs_since_empty_is_enough_to_open_a_fresh_peer() {
    let mut a = new_session();
    a.execute("layer.new.layer", json!({})).unwrap();
    a.execute("paint.stroke", json!({"points": [[10, 10], [60, 20]], "size": 8, "color": "#ff0000"})).unwrap();
    let snapshot = a.snapshot().unwrap();
    let blobs = a.blobs_since(&Version::empty()).unwrap();

    let mut b = Session::new();
    b.open_snapshot(&snapshot, blobs, "peer").unwrap();
    assert_same(&mut a, &mut b, "opened from blobs_since(empty) alone");
}

/// `doc_id` is `None` before a replica exists (and never forces one), then the replica's own
/// identity once there is one, shared by a peer opened from its snapshot.
#[test]
fn doc_id_is_none_until_a_replica_exists_then_matches_a_peer() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 100, "height": 100, "background": "white"})).unwrap();
    assert!(s.doc_id().is_none(), "no replica yet");
    assert!(s.active().unwrap().kdm().is_none(), "doc_id did not force one");

    s.execute("layer.new.layer", json!({})).unwrap();
    let id = s.doc_id();
    assert!(id.is_some());
    // Stable across further calls and further edits.
    s.execute("paint.stroke", json!({"points": [[1, 1], [5, 5]], "size": 2, "color": "#000000"})).unwrap();
    assert_eq!(s.doc_id(), id);

    let b = peer(&mut s);
    assert_eq!(b.doc_id(), id, "a peer of the same KDM document shares its identity");
}

/// Changes the document cannot take (a field set to a value the image has no meaning for, as a
/// refused object-model batch hands it over) change nothing: the replica's version, its exports and
/// the image are as before, a later edit syncs, and the person's earlier steps still undo (KitAgent
/// A18). Before, the replica kept the value its view could not read, and every later import failed.
#[test]
fn refused_changes_leave_the_document_as_it_was() {
    use kition_photo_engine::kdm_model::kition_kit::kdm::{ActorId, ObjectId, Replica};
    use kition_photo_engine::kdm_model::kition_kit::session::KitModel;

    let mut s = new_session();
    s.execute("layer.new.layer", json!({"name": "Mine"})).unwrap();
    let version = s.version().unwrap();
    let state = s.state_hash().unwrap();
    let before = fingerprint(doc(&s));

    let mut script = Replica::load(kition_photo_engine::kdm_model::PhotoModel::schema(), &s.snapshot().unwrap(), ActorId::random()).unwrap();
    let since = script.version();
    let image = ObjectId::from_string("image");
    // The schema takes any JSON for the document state (it refuses a depth it does not list), the
    // image reads only that state's fields.
    script.transact(&Origin::system("script"), |tx| tx.set(&image, "docState", json!("khp check"))).unwrap();
    let refused = script.export_changes(&since).unwrap();
    assert!(s.import_changes(&refused).is_err());

    assert_eq!(s.version().unwrap(), version, "the replica did not take the changes");
    assert_eq!(s.state_hash().unwrap(), state);
    assert_eq!(fingerprint(doc(&s)), before);
    let mut joined = peer(&mut s);
    assert_same(&mut s, &mut joined, "a peer opens what the replica holds");

    s.execute("layer.new.layer", json!({"name": "After"})).unwrap();
    send(&mut s, &mut joined);
    assert_same(&mut s, &mut joined, "a later edit syncs");
    assert!(s.undo(), "undo the later edit");
    assert!(s.undo(), "undo the step taken before the refused changes");
    assert!(!doc(&s).walk().iter().any(|(_, _, l)| l.name == "Mine"));
}
