//! `PhotoSession`, the engine bridge's session: diff-recorded edits, undo steps, and a reload that
//! keeps local layer ids and per-person state.

use kdm_model::PhotoSession;
use kdm_model::kition_kit::kdm::{Origin, UndoScope, Version};
use kdm_model::kition_kit::session::MemoryBlobs;
use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{Document, Layer, Size};
use kition_photo_geom::Rect;
use kition_photo_raster::Surface;

fn document() -> Document {
    let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
    doc.layers.push(Layer::raster("Layer 1", PixelFormat::RGBA8));
    doc.layers.push(Layer::raster("Layer 2", PixelFormat::RGBA8));
    doc
}

fn me() -> Origin {
    Origin::person("local")
}

fn selection() -> Surface {
    let mut s = Surface::new(PixelFormat::GRAY8);
    s.fill_rect(Rect::new(0, 0, 10, 10), &[1.0]);
    s
}

#[test]
fn per_person_state_writes_nothing() {
    let doc = document();
    let mut session = PhotoSession::create(&doc).unwrap();
    let version = session.version();
    let mut after = doc.clone();
    after.selection = Some(selection());
    after.quick_mask = Some(kition_photo_doc::AlphaChannel::new("Quick Mask", selection()));
    after.name = "renamed.psd".into();
    assert_eq!(session.record(&me(), &after).unwrap(), 0);
    assert_eq!(session.version(), version, "no change was committed");
    assert!(!session.can_undo_in(UndoScope::Person));
}

#[test]
fn undo_of_a_delete_brings_the_layer_back_with_its_id_and_keeps_the_selection() {
    let doc = document();
    let removed = doc.layers[1].id;
    let mut session = PhotoSession::create(&doc).unwrap();
    let mut after = doc.clone();
    after.layers.pop();
    after.selection = Some(selection());
    assert_eq!(session.record(&me(), &after).unwrap(), 1);
    let (done, _) = session.undo_in(UndoScope::Person).unwrap();
    assert!(done);
    let view = session.document();
    assert_eq!(view.layers.len(), 2);
    assert_eq!(view.layers[1].id, removed, "the restored layer keeps its local id");
    assert_eq!(view.layers[0].id, doc.layers[0].id);
    assert_eq!(view.id, doc.id, "the view keeps its identity");
    assert!(view.selection.is_some(), "the reload keeps the selection");
    let (done, _) = session.redo_in(UndoScope::Person).unwrap();
    assert!(done);
    assert_eq!(session.document().layers.len(), 1);
}

#[test]
fn a_group_is_one_undo_step() {
    let doc = document();
    let mut session = PhotoSession::create(&doc).unwrap();
    session.begin_undo_group().unwrap();
    let mut steps = 0;
    let mut after = doc.clone();
    for opacity in [0.9, 0.8, 0.7] {
        after.layers[0].opacity = opacity;
        steps += session.record(&me(), &after).unwrap();
    }
    session.end_undo_group();
    assert_eq!(steps, 1);
    session.undo_in(UndoScope::Person).unwrap();
    assert_eq!(session.document().layers[0].opacity, 1.0);
    assert!(!session.can_undo_in(UndoScope::Person));
}

#[test]
fn a_solid_background_is_one_blob_and_reloads_shared_with_its_default_pixel() {
    let mut doc = Document::with_background("t", Size { width: 2000, height: 1500 }, ColorMode::Rgb, SampleType::U8, kition_photo_doc::Color::WHITE);
    let mut sparse = Surface::with_default(PixelFormat::RGBA8, &[1.0, 0.0, 0.0, 1.0]);
    sparse.fill_rect(Rect::new(0, 0, 4, 4), &[0.0, 0.0, 1.0, 1.0]);
    doc.layers.push(Layer::new("Sparse", kition_photo_doc::LayerContent::Raster(sparse)));
    let mut session = PhotoSession::create(&doc).unwrap();
    // Full tiles share one Arc; the right and bottom edge tiles are partial and the sparse
    // layer's single tile is its own: a handful of blobs, not one per tile.
    assert!(session.blobs().0.len() <= 6, "{} blobs", session.blobs().0.len());
    let mut after = doc.clone();
    after.layers.swap(0, 1);
    session.record(&me(), &after).unwrap();
    session.undo_in(UndoScope::Person).unwrap();
    assert_eq!(session.document().layers[0].name, "Background");
    // A replica opened from scratch decodes every tile: the shared ones share again.
    let fresh = PhotoSession::open_snapshot(&session.snapshot().unwrap(), session.blobs().clone()).unwrap();
    let view = fresh.document();
    let bg = view.layers[0].surface().unwrap();
    let pointers: std::collections::HashSet<_> = bg.tiles().map(|(_, t)| std::sync::Arc::as_ptr(t)).collect();
    assert!(pointers.len() <= 4, "a reload shares the solid tiles again ({} distinct)", pointers.len());
    assert_eq!(bg.pixel(10, 10), vec![1.0, 1.0, 1.0, 1.0]);
    let sparse = view.layers[1].surface().unwrap();
    assert_eq!(sparse.default_pixel(), vec![1.0, 0.0, 0.0, 1.0], "the default pixel is kept");
    assert_eq!(sparse.pixel(1000, 1000), vec![1.0, 0.0, 0.0, 1.0]);
    assert_eq!(sparse.pixel(1, 1), vec![0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn video_frames_round_trip_and_undo() {
    let mut doc = document();
    let mut frame = Surface::new(PixelFormat::RGBA8);
    frame.fill_rect(Rect::new(0, 0, 20, 20), &[0.0, 1.0, 0.0, 1.0]);
    doc.layers[1].video = Some(kition_photo_doc::VideoData::new(vec![frame.clone(), Surface::new(PixelFormat::RGBA8)], 24.0));
    let mut session = PhotoSession::create(&doc).unwrap();
    let fresh = PhotoSession::open_snapshot(&session.snapshot().unwrap(), session.blobs().clone()).unwrap();
    let video = fresh.document().layers[1].video.as_ref().unwrap();
    assert_eq!((video.frames.len(), video.fps), (2, 24.0));
    assert_eq!(video.frames[0].pixel(5, 5), vec![0.0, 1.0, 0.0, 1.0]);
    // A painted frame is a change; undo brings the old pixels back.
    let mut after = doc.clone();
    after.layers[1].video.as_mut().unwrap().frames[1].fill_rect(Rect::new(0, 0, 4, 4), &[1.0, 0.0, 0.0, 1.0]);
    assert_eq!(session.record(&me(), &after).unwrap(), 1);
    after.layers[1].video = None;
    assert_eq!(session.record(&me(), &after).unwrap(), 1);
    session.undo_in(UndoScope::Person).unwrap();
    let video = session.document().layers[1].video.as_ref().unwrap();
    assert_eq!(video.frames[1].pixel(1, 1), vec![1.0, 0.0, 0.0, 1.0]);
    session.undo_in(UndoScope::Person).unwrap();
    assert_eq!(session.document().layers[1].video.as_ref().unwrap().frames[1].pixel(1, 1), vec![0.0; 4]);
}

#[test]
fn blobs_only_older_steps_needed_are_dropped() {
    let doc = document();
    let mut session = PhotoSession::create(&doc).unwrap();
    let mut after = doc.clone();
    for i in 0..20 {
        let surface = after.layers[0].surface_mut().unwrap();
        surface.fill_rect(Rect::new(0, 0, 10, 10), &[i as f32 / 20.0, 0.0, 0.0, 1.0]);
        session.record(&me(), &after).unwrap();
    }
    assert!(session.blobs().0.len() >= 20, "every step's tile is still reachable");
    session.retain_steps(UndoScope::Person, 3, 0);
    // The current tile plus the three held steps' before-values.
    assert!(session.blobs().0.len() <= 5, "{} blobs", session.blobs().0.len());
    for _ in 0..3 {
        assert!(session.undo_in(UndoScope::Person).unwrap().0);
    }
    assert!(!session.can_undo_in(UndoScope::Person), "older steps are out of reach");
    assert!((session.document().layers[0].surface().unwrap().pixel(1, 1)[0] - 16.0 / 20.0).abs() < 0.01);
    while session.redo_in(UndoScope::Person).unwrap().0 {}
    assert!((session.document().layers[0].surface().unwrap().pixel(1, 1)[0] - 19.0 / 20.0).abs() < 0.01);
}

/// Three strokes on three different 256×256 tiles: `blobs_since` the version right after the
/// first answers exactly the other two's tiles, not the first's (already had) nor anything else.
#[test]
fn blobs_since_answers_only_the_tiles_a_later_version_added() {
    let doc = document();
    let mut session = PhotoSession::create(&doc).unwrap();
    let mut after = doc.clone();

    after.layers[0].surface_mut().unwrap().fill_rect(Rect::new(0, 0, 10, 10), &[1.0, 0.0, 0.0, 1.0]);
    session.record(&me(), &after).unwrap();
    let v1 = session.version();

    after.layers[0].surface_mut().unwrap().fill_rect(Rect::new(260, 0, 270, 10), &[0.0, 1.0, 0.0, 1.0]);
    session.record(&me(), &after).unwrap();

    after.layers[0].surface_mut().unwrap().fill_rect(Rect::new(0, 260, 10, 270), &[0.0, 0.0, 1.0, 1.0]);
    session.record(&me(), &after).unwrap();

    let since_v1: std::collections::HashMap<_, _> = session.blobs_since(&v1).into_iter().collect();
    assert_eq!(since_v1.len(), 2, "exactly strokes 2 and 3's tiles, not stroke 1's");
    for hash in since_v1.keys() {
        assert!(session.blobs().0.contains_key(hash), "still in the store");
    }
    // Stroke 1's tile is not among them.
    let since_empty: std::collections::HashSet<_> = session.blobs_since(&Version::empty()).into_iter().map(|(h, _)| h).collect();
    let since_v1_hashes: std::collections::HashSet<_> = since_v1.keys().cloned().collect();
    let stroke1_only: Vec<_> = since_empty.difference(&since_v1_hashes).collect();
    assert_eq!(stroke1_only.len(), 1, "exactly one tile (stroke 1's) is missing from blobs_since(v1)");
}

/// `blobs_since(empty)` answers every blob the *current* state references, not the whole store:
/// repeated edits to the same tile keep the old blobs around for undo, but only the latest one
/// is referenced now, and it changes with every edit.
#[test]
fn blobs_since_empty_answers_the_referenced_store_not_the_whole_blob_log() {
    let doc = document();
    let mut session = PhotoSession::create(&doc).unwrap();
    let mut after = doc.clone();
    let mut last_hash = None;
    for i in 0..5 {
        after.layers[0].surface_mut().unwrap().fill_rect(Rect::new(0, 0, 10, 10), &[i as f32 / 5.0, 0.0, 0.0, 1.0]);
        session.record(&me(), &after).unwrap();
        let referenced = session.blobs_since(&Version::empty());
        assert_eq!(referenced.len(), 1, "only the current tile is referenced by the current state");
        assert_ne!(Some(referenced[0].0.clone()), last_hash, "a new pixel value, a new hash");
        last_hash = Some(referenced[0].0.clone());
    }
    assert!(session.blobs().0.len() >= 5, "every step's tile is still kept in the store for undo");
}

/// A second session, opened from `a`'s snapshot at version `v`, converges to the same pixels as
/// `a` using only `export_changes(v)` and `blobs_since(v)` (not the whole store): the standard
/// catch-up a live sync peer that is behind performs.
#[test]
fn blobs_since_feeds_a_catching_up_peer_to_identical_pixels() {
    let doc = document();
    let mut a = PhotoSession::create(&doc).unwrap();
    let mut b = PhotoSession::open_snapshot(&a.snapshot().unwrap(), MemoryBlobs(a.blobs_since(&Version::empty()).into_iter().collect())).unwrap();
    let v = b.version();
    assert_eq!(v, a.version());

    let mut after = doc.clone();
    after.layers[0].surface_mut().unwrap().fill_rect(Rect::new(0, 0, 10, 10), &[1.0, 0.0, 0.0, 1.0]);
    a.record(&me(), &after).unwrap();
    after.layers[0].surface_mut().unwrap().fill_rect(Rect::new(260, 0, 270, 10), &[0.0, 1.0, 0.0, 1.0]);
    a.record(&me(), &after).unwrap();
    after.layers[1].surface_mut().unwrap().fill_rect(Rect::new(0, 260, 10, 270), &[0.0, 0.0, 1.0, 1.0]);
    a.record(&me(), &after).unwrap();

    let changes = a.export_changes(&v).unwrap();
    let new_blobs = MemoryBlobs(a.blobs_since(&v).into_iter().collect());
    assert!(!new_blobs.0.is_empty(), "the three new tiles need their blobs sent");
    b.add_blobs(&new_blobs);
    b.import_changes(&changes).unwrap();

    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    let (av, bv) = (a.document(), b.document());
    for (li, (x, y)) in [(0, (5, 5)), (0, (265, 5)), (1, (5, 265))] {
        assert_eq!(av.layers[li].surface().unwrap().pixel(x, y), bv.layers[li].surface().unwrap().pixel(x, y));
    }
}

/// Deleting a layer deletes its mask with it (task card C-photo): the mask's `Surface` is the
/// layer's own (`owns`), and one left behind made a replica differ from the same image read from a
/// PSD (`bindings/wasm/tests/saved.rs`), which the agents' premise guard reads as a change by others.
#[test]
fn deleting_a_layer_deletes_its_mask() {
    let mut doc = document();
    doc.layers[1].mask = Some(kition_photo_doc::LayerMask::reveal_all());
    let mut session = PhotoSession::create(&doc).unwrap();
    let surfaces = |session: &PhotoSession| session.replica().state_json().unwrap().to_string().matches("\"Surface\"").count();
    assert_eq!(surfaces(&session), 1, "the mask");
    let mut after = doc.clone();
    after.layers.remove(1);
    session.record(&me(), &after).unwrap();
    assert_eq!(surfaces(&session), 0, "no surface is left behind");
}
