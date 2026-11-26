//! Schema freeze (task schema-freeze): every collection and every text of the `kit.photo` replica
//! is a container, so an agent's draft and the person's edit of different parts of it both stay
//! when the person accepts the draft (kition-sdk data model 6.6: lists by item, maps by key,
//! fields by field). Each test makes a draft of one document, edits the draft and the document
//! each its own way, merges the draft, and checks that both edits stay, that the view reads what a
//! fresh read gives, and that one undo of the person gives back the exact state before the merge.
//! Both sides changing one text is refused. What is each person's own view (a group's expanded
//! state, an open note, the timeline's playhead) is not replicated at all.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use kdm_model::PhotoSession;
use kdm_model::kition_kit::kdm::{ActorId, DocId, Origin, Replica, UndoScope};
use kdm_model::kition_kit::session::{IdMap, KitModel, MemoryBlobs};
use kition_photo_color::{BlendMode, ColorMode, PixelFormat, SampleType};
use kition_photo_doc::effects::{Effect, FxCommon};
use kition_photo_doc::text::{CharStyle, TextRun};
use kition_photo_doc::{
    Color, Document, Group, Guide, Layer, LayerComp, LayerContent, NamedPath, Note, Path, Pattern, Size, Subpath, TextLayer, Timeline, VideoData,
};
use kition_photo_geom::Rect;
use kition_photo_raster::Surface;

fn me() -> Origin {
    Origin::person("local")
}

fn base() -> Document {
    let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
    doc.layers.push(Layer::raster("Layer 1", PixelFormat::RGBA8));
    doc
}

fn agent() -> Origin {
    Origin::agent("agent:test")
}

/// `a` holds `doc`; `b` is an agent's draft of it (a replica opened from `a`'s snapshot, on its
/// line of changes).
fn pair(doc: &Document) -> (PhotoSession, PhotoSession) {
    let a = PhotoSession::create(doc).unwrap();
    let b = PhotoSession::open_snapshot(&a.snapshot().unwrap(), a.blobs().clone()).unwrap();
    (a, b)
}

/// The draft's changes since it was made (with the blobs they need) taken into `a` as the person
/// accepts them.
fn merge(a: &mut PhotoSession, b: &PhotoSession, since: &kdm_model::kition_kit::kdm::Version) -> kdm_model::kition_kit::session::Result<Option<UndoScope>> {
    let changes = b.export_changes(since).unwrap();
    a.add_blobs(b.blobs());
    a.merge_changes(&changes, &agent()).map(|(scope, _)| scope)
}

/// The agent edits its draft (`edit_b`) while the person edits the document (`edit_a`); the
/// person accepts the draft: both edits stay, the view is what a fresh read of the merged replica
/// gives, and the merge is one undo step of the person, exact both ways.
fn concurrently(a: &mut PhotoSession, b: &mut PhotoSession, edit_a: impl FnOnce(&mut Document), edit_b: impl FnOnce(&mut Document)) {
    let since = b.version();
    let mut doc_b = b.document().clone();
    edit_b(&mut doc_b);
    assert_eq!(b.record(&agent(), &doc_b).unwrap(), 1, "the draft's edit is one step");
    let mut doc_a = a.document().clone();
    edit_a(&mut doc_a);
    assert_eq!(a.record(&me(), &doc_a).unwrap(), 1, "the person's edit is one step");
    let before = a.state_hash().unwrap();
    assert_eq!(merge(a, b, &since).unwrap(), Some(UndoScope::Person), "the merge is the person's step");
    let after = a.state_hash().unwrap();
    let fresh = PhotoSession::open_snapshot(&a.snapshot().unwrap(), a.blobs().clone()).unwrap();
    assert_eq!(a.model(), fresh.model(), "the view reads the merged document");
    assert!(a.undo_in(UndoScope::Person).unwrap().0);
    assert_eq!(a.state_hash().unwrap(), before, "one undo gives back the exact state before the merge");
    assert!(a.redo_in(UndoScope::Person).unwrap().0);
    assert_eq!(a.state_hash().unwrap(), after, "one redo gives back the merge");
    assert_eq!(a.model(), fresh.model(), "and the view with it");
}

/// Both sides changing one text (a note's) collide: the merge is refused and changes nothing.
#[test]
fn a_draft_and_the_person_typing_in_one_note_is_refused() {
    let mut doc = base();
    doc.notes.push(Note { author: "Ann".into(), text: "Check the horizon".into(), ..Default::default() });
    let (mut a, mut b) = pair(&doc);
    let since = b.version();
    let mut doc_b = b.document().clone();
    doc_b.notes[0].text = "Check the horizon line".into();
    b.record(&agent(), &doc_b).unwrap();
    let mut doc_a = a.document().clone();
    doc_a.notes[0].text = "Please check the horizon".into();
    a.record(&me(), &doc_a).unwrap();
    let (before, version) = (a.state_hash().unwrap(), a.version());
    assert!(merge(&mut a, &b, &since).is_err(), "one text changed on both sides");
    assert_eq!(a.state_hash().unwrap(), before, "a refused merge changes nothing");
    assert_eq!(a.version(), version);
    assert_eq!(a.document().notes[0].text, "Please check the horizon");
}

#[test]
fn notes_added_at_once_are_both_kept() {
    let mut doc = base();
    doc.notes.push(Note { author: "Ann".into(), text: "first".into(), ..Default::default() });
    let (mut a, mut b) = pair(&doc);
    concurrently(
        &mut a,
        &mut b,
        |d| d.notes.push(Note { author: "Ann".into(), text: "from a".into(), ..Default::default() }),
        |d| {
            d.notes[0].author = "Bob".into();
            d.notes.push(Note { author: "Bob".into(), text: "from b".into(), ..Default::default() });
        },
    );
    let notes = &a.document().notes;
    assert_eq!(notes.len(), 3, "{notes:?}");
    assert_eq!(notes[0].author, "Bob", "b's edit of the first note survives");
    let texts: Vec<&str> = notes.iter().map(|note| note.text.as_str()).collect();
    assert!(texts.contains(&"from a") && texts.contains(&"from b"), "{texts:?}");
}

fn comp(id: u32, name: &str) -> LayerComp {
    LayerComp { id, name: name.into(), comment: String::new(), apply_visibility: true, apply_position: false, apply_appearance: false, states: Vec::new() }
}

#[test]
fn a_comp_renamed_while_another_is_added_keeps_both() {
    let mut doc = base();
    doc.layer_comps.push(comp(1, "Day"));
    let (mut a, mut b) = pair(&doc);
    concurrently(&mut a, &mut b, |d| d.layer_comps[0].name = "Morning".into(), |d| d.layer_comps.push(comp(2, "Night")));
    let names: Vec<&str> = a.document().layer_comps.iter().map(|comp| comp.name.as_str()).collect();
    assert_eq!(names, ["Morning", "Night"]);
}

fn path(name: &str, side: f64) -> NamedPath {
    NamedPath { name: name.into(), path: Path::new(vec![Subpath::polygon(&[(0.0, 0.0), (side, 0.0), (side, side)])]), ..Default::default() }
}

#[test]
fn two_saved_paths_edited_at_once_keep_both_edits() {
    let mut doc = base();
    doc.paths.push(path("Outline", 10.0));
    doc.paths.push(path("Horizon", 20.0));
    let (mut a, mut b) = pair(&doc);
    concurrently(&mut a, &mut b, |d| d.paths[0].name = "Edge".into(), |d| d.paths[1].path = path("", 40.0).path);
    let paths = &a.document().paths;
    assert_eq!(paths[0].name, "Edge");
    assert_eq!(paths[1].path, path("", 40.0).path);
}

fn pattern(name: &str, value: f32) -> Pattern {
    let mut surface = Surface::new(PixelFormat::RGBA8);
    surface.fill_rect(Rect::new(0, 0, 4, 4), &[value, 0.0, 0.0, 1.0]);
    Pattern::new(name, surface, 4, 4)
}

#[test]
fn patterns_defined_at_once_are_both_kept() {
    let (mut a, mut b) = pair(&base());
    concurrently(&mut a, &mut b, |d| d.patterns.push(pattern("Red", 1.0)), |d| d.patterns.push(pattern("Dark", 0.2)));
    let mut names: Vec<&str> = a.document().patterns.iter().map(|pattern| pattern.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Dark", "Red"]);
    let red = a.document().patterns.iter().find(|pattern| pattern.name == "Red").unwrap();
    assert_eq!(red.surface.pixel(1, 1), vec![1.0, 0.0, 0.0, 1.0], "the pattern's pixels came across");
}

#[test]
fn two_effects_of_one_layer_style_edited_at_once_keep_both_edits() {
    let mut doc = base();
    doc.layers[0].effects.enabled = true;
    doc.layers[0].effects.items =
        vec![Effect::default_drop_shadow(), Effect::ColorOverlay { common: FxCommon::new(BlendMode::Normal, 1.0), color: Color::rgb(1.0, 0.0, 0.0) }];
    let (mut a, mut b) = pair(&doc);
    concurrently(
        &mut a,
        &mut b,
        |d| {
            if let Effect::DropShadow(shadow) = &mut d.layers[0].effects.items[0] {
                shadow.distance = 12.0;
            }
        },
        |d| {
            if let Effect::ColorOverlay { color, .. } = &mut d.layers[0].effects.items[1] {
                *color = Color::rgb(0.0, 0.0, 1.0);
            }
        },
    );
    let items = &a.document().layers[0].effects.items;
    assert!(matches!(&items[0], Effect::DropShadow(shadow) if shadow.distance == 12.0), "{items:?}");
    assert!(matches!(&items[1], Effect::ColorOverlay { color, .. } if *color == Color::rgb(0.0, 0.0, 1.0)), "{items:?}");
}

fn frame() -> Surface {
    Surface::new(PixelFormat::RGBA8)
}

#[test]
fn two_frames_of_one_video_layer_painted_at_once_keep_both_strokes() {
    let mut doc = base();
    doc.layers[0].video = Some(VideoData::new(vec![frame(), frame()], 24.0));
    let (mut a, mut b) = pair(&doc);
    let paint = |frame: usize, value: f32| {
        move |d: &mut Document| d.layers[0].video.as_mut().unwrap().frames[frame].fill_rect(Rect::new(0, 0, 10, 10), &[value, 0.0, 0.0, 1.0])
    };
    concurrently(&mut a, &mut b, paint(0, 1.0), paint(1, 0.5));
    let frames = &a.document().layers[0].video.as_ref().unwrap().frames;
    assert_eq!(frames[0].pixel(5, 5), vec![1.0, 0.0, 0.0, 1.0]);
    assert!((frames[1].pixel(5, 5)[0] - 0.5).abs() < 0.01, "{:?}", frames[1].pixel(5, 5));
}

#[test]
fn a_guide_added_while_another_moves_keeps_both() {
    let mut doc = base();
    doc.guides.vertical.push(Guide::new(10.0));
    doc.guides.vertical.push(Guide::new(50.0));
    let (mut a, mut b) = pair(&doc);
    concurrently(&mut a, &mut b, |d| d.guides.vertical.insert(1, Guide::new(30.0)), |d| d.guides.vertical[1].position = 60.0);
    let positions: Vec<f32> = a.document().guides.vertical.iter().map(|guide| guide.position).collect();
    assert_eq!(positions, [10.0, 30.0, 60.0]);
    assert_eq!(a.document().guides.vertical[0].key, doc.guides.vertical[0].key, "a guide keeps its key");
}

#[test]
fn deleting_a_guide_rewrites_no_other() {
    let mut doc = base();
    for at in [10.0, 20.0, 30.0] {
        doc.guides.horizontal.push(Guide::new(at));
    }
    let (mut a, mut b) = pair(&doc);
    concurrently(&mut a, &mut b, |d| d.guides.horizontal[2].position = 35.0, |d| _ = d.guides.horizontal.remove(0));
    let positions: Vec<f32> = a.document().guides.horizontal.iter().map(|guide| guide.position).collect();
    assert_eq!(positions, [20.0, 35.0]);
}

fn type_layer(text: &str) -> Layer {
    let content = TextLayer {
        text: text.into(),
        runs: vec![TextRun { len: text.len(), style: CharStyle { size_pt: 18.0, ..CharStyle::default() } }],
        ..TextLayer::default()
    };
    Layer::new("Title", LayerContent::Text(content))
}

fn typed(doc: &Document) -> &TextLayer {
    match &doc.layers[1].content {
        LayerContent::Text(text) => text,
        other => panic!("not a type layer: {other:?}"),
    }
}

#[test]
fn style_runs_round_trip_as_marks() {
    let mut doc = base();
    let mut layer = type_layer("Héllo wörld");
    if let LayerContent::Text(text) = &mut layer.content {
        let bold = CharStyle { weight: 700, ..CharStyle::default() };
        // Two neighbouring runs of one style stay two runs; the second starts after a two-byte é.
        text.runs =
            vec![TextRun { len: 3, style: bold.clone() }, TextRun { len: 3, style: bold }, TextRun { len: text.text.len() - 6, style: CharStyle::default() }];
    }
    doc.layers.push(layer);
    let session = PhotoSession::create(&doc).unwrap();
    let fresh = PhotoSession::open_snapshot(&session.snapshot().unwrap(), session.blobs().clone()).unwrap();
    assert_eq!(typed(fresh.document()), typed(&doc));
}

#[test]
fn each_persons_view_is_not_replicated() {
    let mut doc = base();
    doc.layers
        .push(Layer::new("Group", LayerContent::Group(Group { children: vec![Layer::raster("Inside", PixelFormat::RGBA8)], expanded: true, artboard: None })));
    doc.notes.push(Note { text: "n".into(), ..Default::default() });
    doc.timeline = Some(Timeline::new(10, 24.0));
    let (mut a, mut b) = pair(&doc);
    let version = a.version();
    let mut view = a.document().clone();
    if let LayerContent::Group(group) = &mut view.layers[1].content {
        group.expanded = false;
    }
    view.notes[0].open = true;
    view.timeline.as_mut().unwrap().current = 7;
    assert_eq!(a.record(&me(), &view).unwrap(), 0, "a view change is no step");
    assert_eq!(a.version(), version, "nothing was written");
    // An agent's draft is accepted: the view keeps its own state.
    let mut other = b.document().clone();
    other.layers[0].name = "Renamed".into();
    b.record(&agent(), &other).unwrap();
    merge(&mut a, &b, &version).unwrap();
    let now = a.document();
    assert_eq!(now.layers[0].name, "Renamed");
    assert!(matches!(&now.layers[1].content, LayerContent::Group(group) if !group.expanded), "the group stays collapsed here");
    assert!(now.notes[0].open, "the note stays open here");
    assert_eq!(now.timeline.as_ref().unwrap().current, 7, "the playhead stays");
    assert_eq!(a.model(), b.model(), "the shared document is the same");
}

#[test]
fn a_replica_of_another_schema_version_is_refused() {
    let mut schema = (*kdm_model::PhotoModel::schema()).clone();
    schema.schema_version = 3;
    let mut blobs = MemoryBlobs::default();
    let (replica, ()) = Replica::create_with(Arc::new(schema), DocId::new(), ActorId::random(), &Origin::system("create"), |tx| {
        kdm_model::write_document(tx, &base(), &mut IdMap::default(), &mut kdm_model::Nodes::default(), &mut blobs).map(|_| ())
    })
    .unwrap();
    let snapshot = replica.snapshot().unwrap();
    assert!(!kdm_model::replica_matches(&snapshot));
    assert!(PhotoSession::open_snapshot(&snapshot, blobs).is_err());
    let current = PhotoSession::create(&base()).unwrap();
    assert!(kdm_model::replica_matches(&current.snapshot().unwrap()));
}
