//! Schema freeze (task schema-freeze): every collection and every text of the `kit.photo` replica
//! is a container, so two people editing different parts of it at once both keep their edit.
//! Each test makes two replicas of one document, edits each its own way at the same time,
//! exchanges the changes, and checks that both edits survive and the replicas are identical.
//! What is each person's own view (a group's expanded state, an open note, the timeline's
//! playhead) is not replicated at all.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::sync::Arc;

use kdm_model::PhotoSession;
use kdm_model::kition_kit::kdm::{ActorId, DocId, Origin, Replica};
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

/// `a` holds `doc`; `b` is a second replica of it.
fn pair(doc: &Document) -> (PhotoSession, PhotoSession) {
    let a = PhotoSession::create(doc).unwrap();
    let b = PhotoSession::open_snapshot(&a.snapshot().unwrap(), a.blobs().clone()).unwrap();
    (a, b)
}

/// Each replica records its own edit of its own view, then they exchange changes: both converge.
fn concurrently(a: &mut PhotoSession, b: &mut PhotoSession, edit_a: impl FnOnce(&mut Document), edit_b: impl FnOnce(&mut Document)) {
    let mut doc_a = a.document().clone();
    edit_a(&mut doc_a);
    assert_eq!(a.record(&me(), &doc_a).unwrap(), 1, "a's edit is one step");
    let mut doc_b = b.document().clone();
    edit_b(&mut doc_b);
    assert_eq!(b.record(&me(), &doc_b).unwrap(), 1, "b's edit is one step");
    let (va, vb) = (a.version(), b.version());
    let (from_a, from_b) = (a.export_changes(&vb).unwrap(), b.export_changes(&va).unwrap());
    let (blobs_a, blobs_b) = (a.blobs().clone(), b.blobs().clone());
    a.add_blobs(&blobs_b);
    b.add_blobs(&blobs_a);
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap(), "the replicas converged");
    assert_eq!(a.model(), b.model(), "both views read the same document");
}

#[test]
fn two_people_typing_in_one_note_both_keep_their_words() {
    let mut doc = base();
    doc.notes.push(Note { author: "Ann".into(), text: "Check the horizon".into(), ..Default::default() });
    let (mut a, mut b) = pair(&doc);
    concurrently(&mut a, &mut b, |d| d.notes[0].text = "Please check the horizon".into(), |d| d.notes[0].text = "Check the horizon line".into());
    assert_eq!(a.document().notes[0].text, "Please check the horizon line");
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

fn retype(d: &mut Document, f: impl FnOnce(&mut TextLayer)) {
    if let LayerContent::Text(text) = &mut d.layers[1].content {
        f(text);
    }
}

#[test]
fn two_people_typing_in_one_type_layer_both_keep_their_words() {
    let mut doc = base();
    doc.layers.push(type_layer("Hello world"));
    let (mut a, mut b) = pair(&doc);
    concurrently(
        &mut a,
        &mut b,
        |d| {
            retype(d, |t| {
                t.text = "Big Hello world".into();
                t.runs[0].len = t.text.len();
            })
        },
        |d| {
            retype(d, |t| {
                t.text = "Hello world!".into();
                t.runs[0].len = t.text.len();
            })
        },
    );
    let text = typed(a.document());
    assert_eq!(text.text, "Big Hello world!");
    assert_eq!(text.runs.len(), 1, "the typed characters joined the run: {:?}", text.runs);
    assert_eq!(text.runs[0].len, "Big Hello world!".len());
    assert_eq!(text.runs[0].style.size_pt, 18.0);
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
fn a_run_restyled_while_text_is_typed_elsewhere_keeps_both() {
    let mut doc = base();
    let mut layer = type_layer("one two");
    if let LayerContent::Text(text) = &mut layer.content {
        text.runs = vec![TextRun { len: 4, style: CharStyle::default() }, TextRun { len: 3, style: CharStyle::default() }];
    }
    doc.layers.push(layer);
    let (mut a, mut b) = pair(&doc);
    concurrently(
        &mut a,
        &mut b,
        |d| retype(d, |t| t.runs[1].style.italic = true),
        |d| {
            retype(d, |t| {
                t.text = "zero one two".into();
                t.runs[0].len += 5;
            })
        },
    );
    let text = typed(a.document());
    assert_eq!(text.text, "zero one two");
    assert!(text.runs.last().unwrap().style.italic, "{:?}", text.runs);
    assert_eq!(text.runs.last().unwrap().len, 3, "{:?}", text.runs);
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
    // A collaborator's edit arrives: the view keeps its own state.
    let mut other = b.document().clone();
    other.layers[0].name = "Renamed".into();
    b.record(&me(), &other).unwrap();
    a.import_changes(&b.export_changes(&a.version()).unwrap()).unwrap();
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
