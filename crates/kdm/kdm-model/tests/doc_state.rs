//! `Image.docState`: document-level state the named schema fields do not cover (patterns, layer
//! comps, notes, paths, …) survives a round trip through the replica.

use kdm_model::{Nodes, write_document};
use kition_kit::kdm::{ActorId, DocId, Origin, Replica};
use kition_kit::session::{IdMap, KitModel, MemoryBlobs};
use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{Color, CompAppearance, CompLayerState, Document, Layer, LayerComp, NamedPath, Note, Path, Pattern, Size, Subpath};
use kition_photo_geom::Rect;

fn sample_document() -> Document {
    let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
    doc.layers.push(Layer::raster("Layer 1", PixelFormat::RGBA8));

    let mut pattern_surface = kition_photo_raster::Surface::new(PixelFormat::RGBA8);
    pattern_surface.fill_rect(Rect::new(0, 0, 4, 4), &[1.0, 0.0, 0.0, 1.0]);
    doc.patterns.push(Pattern::new("Red", pattern_surface, 4, 4));

    doc.notes.push(Note { author: "Agent".into(), color: Color::rgb(1.0, 1.0, 0.0), text: "Check the horizon".into(), ..Default::default() });

    doc.paths.push(NamedPath {
        name: "Outline".into(),
        path: Path::new(vec![Subpath::polygon(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)])]),
        ..Default::default()
    });

    let layer_id = doc.layers[0].id;
    doc.layer_comps.push(LayerComp {
        id: 1,
        name: "Comp 1".into(),
        comment: "".into(),
        apply_visibility: true,
        apply_position: false,
        apply_appearance: true,
        states: vec![CompLayerState {
            layer: layer_id,
            visible: Some(true),
            position: None,
            appearance: Some(CompAppearance { blend: doc.layers[0].blend, opacity: 0.75, fill_opacity: 1.0, effects: doc.layers[0].effects.clone() }),
        }],
    });

    doc
}

#[test]
fn doc_state_round_trips_patterns_comps_notes_and_paths() {
    let document = sample_document();
    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    let mut blobs = MemoryBlobs::default();
    let (replica, ()) = Replica::create_with(kdm_model::PhotoModel::schema(), DocId::new(), ActorId::from_u64(1).unwrap(), &Origin::system("import"), |tx| {
        write_document(tx, &document, &mut ids, &mut nodes, &mut blobs).map(|_| ())
    })
    .unwrap();

    let model = kdm_model::PhotoModel::load(&replica, &blobs).unwrap();
    let loaded = &model.document;

    assert_eq!(loaded.patterns.len(), 1);
    assert_eq!(loaded.patterns[0].name, "Red");
    assert_eq!(loaded.patterns[0].width, 4);
    assert_eq!(loaded.patterns[0].surface.read_region(Rect::new(0, 0, 4, 4)), document.patterns[0].surface.read_region(Rect::new(0, 0, 4, 4)));

    assert_eq!(loaded.notes.len(), 1);
    assert_eq!(loaded.notes[0].author, "Agent");
    assert_eq!(loaded.notes[0].text, "Check the horizon");

    assert_eq!(loaded.paths.len(), 1);
    assert_eq!(loaded.paths[0].name, "Outline");
    assert_eq!(loaded.paths[0].path, document.paths[0].path);

    assert_eq!(loaded.layer_comps.len(), 1);
    let comp = &loaded.layer_comps[0];
    assert_eq!(comp.name, "Comp 1");
    assert!(comp.apply_visibility);
    assert!(comp.apply_appearance);
    assert_eq!(comp.states.len(), 1, "the comp's layer reference resolved");
    // The comp names the layer by its KDM object id, translated back through the freshly built
    // id map: it must resolve to the SAME layer (not just any layer with a fresh, unrelated id).
    let named_layer = model.document.layers.iter().find(|l| l.id == comp.states[0].layer).unwrap();
    assert_eq!(named_layer.name, "Layer 1");
    assert_eq!(comp.states[0].appearance.as_ref().unwrap().opacity, 0.75);
}
