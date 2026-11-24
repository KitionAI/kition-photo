//! doc → .kphoto → doc equality (the file's content and the view state's view), incremental
//! saves, ids, no previews, no one's view in the file.

mod common;
use common::*;
use kition_photo_color::{ColorMode, SampleType};
use kition_photo_doc::text::{CharStyle, ParagraphStyle, TextRun};
use kition_photo_doc::text_styles::{CharacterStyleDef, ParagraphStyleDef};
use kition_photo_doc::{Document, Layer, LayerContent, LayerId, Slice, TextLayer};
use kition_photo_format::view_state::{BYTES_LIMIT, put_view_state, view_state, without_view};
use kition_photo_format::*;

const MODES: [ColorMode; 4] = [ColorMode::Rgb, ColorMode::Grayscale, ColorMode::Cmyk, ColorMode::Lab];

fn check_zip(mode: ColorMode, depth: SampleType) {
    let doc = rich_doc(mode, depth);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    assert!(is_kphoto(&bytes));
    let back = load_from_bytes(&bytes).unwrap();
    assert_eq!(viewed(back.clone(), &doc), doc, "{mode:?} {depth:?}");
}

macro_rules! zip_cases {
    ($($name:ident: $m:ident, $d:ident;)*) => {$(
        #[test]
        fn $name() {
            check_zip(ColorMode::$m, SampleType::$d);
        }
    )*};
}

zip_cases! {
    rgb_u8: Rgb, U8; rgb_u16: Rgb, U16; rgb_f32: Rgb, F32;
    gray_u8: Grayscale, U8; gray_u16: Grayscale, U16; gray_f32: Grayscale, F32;
    cmyk_u8: Cmyk, U8; cmyk_u16: Cmyk, U16; cmyk_f32: Cmyk, F32;
    lab_u8: Lab, U8; lab_u16: Lab, U16; lab_f32: Lab, F32;
}

#[test]
fn directory_bundle_roundtrip_all_modes() {
    for mode in MODES {
        for depth in SampleType::ALL {
            let doc = rich_doc(mode, depth);
            let dir = temp_dir("dir");
            KitionPhotoWriter::new().save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
            assert!(dir.join("manifest.json").is_file());
            assert_eq!(viewed(load_path(&dir).unwrap(), &doc), doc, "{mode:?} {depth:?}");
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

#[test]
fn save_path_zip_file() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let dir = temp_dir("file");
    let path = dir.join("a.kphoto");
    KitionPhotoWriter::new().save_path(&doc, &path, &SaveOptions::default()).unwrap();
    assert!(path.is_file());
    assert_eq!(viewed(load_path(&path).unwrap(), &doc), doc);
    std::fs::remove_dir_all(dir).unwrap();
}

/// Deepest group nesting in `layers` (0 when there are no groups).
fn group_depth(layers: &[Layer]) -> usize {
    layers.iter().map(|l| if let kition_photo_doc::LayerContent::Group(g) = &l.content { 1 + group_depth(&g.children) } else { 0 }).max().unwrap_or(0)
}

/// [`rich_doc`] with its layers wrapped in groups until they are nested `levels` deep.
fn nested_doc(levels: usize, depth: SampleType) -> Document {
    let mut doc = rich_doc(ColorMode::Rgb, depth);
    let mut layers = std::mem::take(&mut doc.layers);
    for i in group_depth(&layers)..levels {
        layers = vec![Layer::group(format!("Level {i}"), layers)];
    }
    doc.layers = layers;
    assert_eq!(group_depth(&doc.layers), levels);
    doc
}

/// serde_json's default limit (128 levels) used to make bundles with 40+ nested groups unreadable.
/// Runs on a 1 MiB stack, the smallest main-thread stack we ship on (Windows, wasm).
#[test]
fn deeply_nested_groups_roundtrip() {
    let run = || {
        for (levels, depth) in [(40, SampleType::U8), (MAX_GROUP_DEPTH, SampleType::U16), (MAX_GROUP_DEPTH, SampleType::F32)] {
            let doc = nested_doc(levels, depth);
            let back = load_from_bytes(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap();
            assert_eq!(viewed(back.clone(), &doc), doc, "{levels} levels at {depth:?}");
        }
    };
    std::thread::Builder::new().stack_size(1 << 20).spawn(run).unwrap().join().unwrap();
}

/// A save never writes a bundle the loader would reject for its nesting.
#[test]
fn nesting_beyond_the_load_limit_is_refused_at_save() {
    let e = save_to_bytes(&nested_doc(MAX_GROUP_DEPTH + 1, SampleType::U8), &SaveOptions::default()).unwrap_err();
    assert!(matches!(e, FormatError::LimitExceeded(_)), "{e}");
}

#[test]
fn empty_document_roundtrips() {
    let doc = Document::new("empty", kition_photo_doc::Size::new(1, 1), ColorMode::Rgb, SampleType::U8);
    assert_eq!(viewed(load_from_bytes(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap(), &doc), doc);
}

#[test]
fn exhausted_document_ids_and_maximal_text_runs_roundtrip() {
    let mut doc = Document::new("exhausted", kition_photo_doc::Size::new(2, 1), ColorMode::Rgb, SampleType::U8);
    doc.slices.list.push(Slice { id: u32::MAX, ..Default::default() });
    doc.text_styles.character.push(CharacterStyleDef { id: u32::MAX, ..Default::default() });
    doc.text_styles.paragraph.push(ParagraphStyleDef { id: u32::MAX, ..Default::default() });
    doc.layers.push(Layer::new(
        "Text",
        LayerContent::Text(TextLayer {
            text: "ab".into(),
            runs: vec![TextRun { len: 1, style: CharStyle::default() }, TextRun { len: usize::MAX, style: CharStyle { size_pt: 24.0, ..Default::default() } }],
            paragraphs: vec![
                kition_photo_doc::text::ParagraphRun { len: 1, style: ParagraphStyle::default() },
                kition_photo_doc::text::ParagraphRun { len: usize::MAX, style: ParagraphStyle::default() },
            ],
            ..Default::default()
        }),
    ));

    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let loaded = load_from_bytes(&bytes).unwrap();
    assert_eq!(loaded.slices.next_id(), None);
    assert_eq!(loaded.text_styles.next_char_id(), None);
    assert_eq!(loaded.text_styles.next_para_id(), None);
    let LayerContent::Text(text) = &loaded.layers.last().unwrap().content else { panic!("text layer must roundtrip") };
    assert_eq!(text.char_runs().iter().map(|r| r.len).sum::<usize>(), text.text.len());
    assert_eq!(text.char_runs()[1].len, 1);
    assert_eq!(text.char_runs()[1].style.size_pt, 24.0);
    assert_eq!(text.paragraph_runs().iter().map(|r| r.len).sum::<usize>(), text.text.len());
}

#[test]
fn zip_incremental_only_new_tiles() {
    let mut doc = rich_doc(ColorMode::Rgb, SampleType::U16);
    let mut w = KitionPhotoWriter::new();
    let (_, s1) = w.save_zip(&doc, &SaveOptions::default()).unwrap();
    assert!(s1.tiles_total > 5);
    assert_eq!(s1.tiles_written, s1.tiles_total);
    let (_, s2) = w.save_zip(&doc, &SaveOptions::default()).unwrap();
    assert_eq!(s2.tiles_written, 0);
    assert_eq!(s2.tiles_reused, s2.tiles_total);
    assert_eq!(s2.blobs_written, 0);
    // Touch one pixel of one layer: exactly one tile is new.
    let id = doc.layers[1].id;
    doc.layer_mut(id).unwrap().surface_mut().unwrap().write_pixel(3, 3, &[0.5, 0.5, 0.5, 1.0]);
    let (bytes, s3) = w.save_zip(&doc, &SaveOptions::default()).unwrap();
    assert_eq!(s3.tiles_written, 1);
    assert_eq!(viewed(load_from_bytes(&bytes).unwrap(), &doc), doc);
}

#[test]
fn directory_incremental_and_gc() {
    let mut doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let dir = temp_dir("gc");
    let mut w = KitionPhotoWriter::new();
    let s1 = w.save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
    assert_eq!(s1.tiles_written, s1.tiles_total);
    let s2_same_writer = w.save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
    assert_eq!(s2_same_writer.tiles_written, 0);
    assert_eq!(s2_same_writer.tiles_reused, s2_same_writer.tiles_total);
    // A fresh writer still skips files already on disk.
    let s2 = KitionPhotoWriter::new().save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
    assert_eq!(s2.tiles_written, 0);
    assert_eq!(s2.objects_removed, 0);
    // Delete a layer: its unique tiles get garbage-collected.
    let paint = doc.layers[1].id;
    doc.remove(paint);
    let s3 = w.save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
    assert!(s3.objects_removed > 0, "{s3:?}");
    assert_eq!(viewed(load_path(&dir).unwrap(), &doc), doc);
    let files = std::fs::read_dir(dir.join("tiles")).unwrap().count();
    assert_eq!(files, s3.tiles_total);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn identical_tiles_are_stored_once() {
    let mut doc = Document::new("d", kition_photo_doc::Size::new(10, 10), ColorMode::Rgb, SampleType::U8);
    let mut a = Layer::raster("a", doc.pixel_format());
    scribble(a.surface_mut().unwrap(), 5, false);
    let b = a.duplicate();
    doc.layers.push(a);
    doc.layers.push(b);
    let (_, s) = KitionPhotoWriter::new().save_zip(&doc, &SaveOptions::default()).unwrap();
    let per_layer = doc.layers[0].surface().unwrap().tile_count();
    assert_eq!(s.tiles_total, per_layer);
}

#[test]
fn ids_preserved_and_counter_advanced() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    // Simulate a file written with much larger ids.
    let mut m = read_manifest(&bytes).unwrap();
    m.document.layers[0].id = LayerId::fresh().0 + 5000;
    let big = m.document.layers[0].id;
    let text = serde_json::to_vec(&m).unwrap();
    let mut z = zip::ZipWriter::new();
    z.add("manifest.json", &text).unwrap();
    let r = zip::ZipReader::new(&bytes).unwrap();
    for e in r.entries.iter().filter(|e| e.name != "manifest.json") {
        z.add(&e.name, &r.read(e, usize::MAX).unwrap()).unwrap();
    }
    let back = load_from_bytes(&z.finish().unwrap()).unwrap();
    assert_eq!(back.layers[0].id.0, big);
    assert!(LayerId::fresh().0 > big, "fresh ids must not collide with loaded ids");
}

#[test]
fn fresh_ids_option() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let back = load_from_bytes_with(&bytes, &LoadOptions { preserve_ids: false, ..Default::default() }).unwrap();
    assert_ne!(back.id, doc.id);
    assert_ne!(back.layers[0].id, doc.layers[0].id);
    assert_eq!(back.layers.len(), doc.layers.len());
    assert_eq!(back.layers[1].surface(), doc.layers[1].surface());
}

/// Owner decision: no thumbnail or preview image in the content file, which is a layered bundle
/// no other program opens as a picture. A bundle holds its manifest, its tiles and blobs, and
/// (with a replica) `kdm.bin`, nothing else.
#[test]
fn the_bundle_holds_no_preview() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let r = zip::ZipReader::new(&bytes).unwrap();
    for e in &r.entries {
        assert!(e.name == "manifest.json" || e.name.starts_with("tiles/") || e.name.starts_with("blobs/"), "{}", e.name);
    }
    let m = serde_json::to_value(read_manifest(&bytes).unwrap()).unwrap();
    assert_eq!(m["format_version"], FORMAT_VERSION);
    assert!(m.get("thumbnail").is_none() && m.get("composite").is_none(), "{m}");
}

/// A rich image with every part of one person's view set: a collapsed group, a selection and a
/// quick mask ([`rich_doc`]), an open note, the timeline at frame 4, and a layer comp applied
/// with its Last Document State.
fn viewed_doc(depth: SampleType) -> Document {
    use kition_photo_doc::{CompLayerState, LayerComp, Note, Timeline};
    let mut doc = rich_doc(ColorMode::Rgb, depth);
    doc.notes.push(Note { key: Some("n1".into()), text: "open".into(), open: true, ..Default::default() });
    doc.notes.push(Note { text: "closed".into(), ..Default::default() });
    doc.notes.push(Note { text: "open, no key".into(), open: true, ..Default::default() });
    let mut timeline = Timeline::new(10, 24.0);
    timeline.current = 4;
    doc.timeline = Some(timeline);
    let layer = doc.layers[0].id;
    let comp = LayerComp {
        id: 7,
        name: "Comp".into(),
        comment: String::new(),
        apply_visibility: true,
        apply_position: false,
        apply_appearance: false,
        states: vec![CompLayerState { layer, visible: Some(false), position: None, appearance: None }],
    };
    doc.layer_comps.push(comp.clone());
    doc.last_applied_comp = Some(7);
    doc.last_document_state = Some(LayerComp { id: 0, name: "Last Document State".into(), ..comp });
    doc
}

/// The file holds the image's content and nobody's view, nor the session's handle or title: the
/// same image saved with another view, handle and title is the same bytes. Put back from the view
/// state, the view makes the whole document again.
#[test]
fn the_file_keeps_no_view_and_the_view_state_keeps_it() {
    for depth in SampleType::ALL {
        let doc = viewed_doc(depth);
        let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
        let mut other = without_view(&doc);
        other.id = kition_photo_doc::DocId::fresh();
        other.name = "Elsewhere.kphoto".into();
        assert_ne!(other, doc);
        assert_eq!(save_to_bytes(&other, &SaveOptions::default()).unwrap(), bytes, "{depth:?}: the same file");
        let text = String::from_utf8(zip::ZipReader::new(&bytes).unwrap().read_by_name("manifest.json", usize::MAX).unwrap()).unwrap();
        for key in ["\"selection\"", "\"quick_mask\"", "\"expanded\"", "\"last_applied_comp\"", "\"last_document_state\"", "\"open\": true", "\"Rich\""] {
            assert!(!text.contains(key), "{depth:?}: the manifest holds {key}");
        }

        let mut back = load_from_bytes(&bytes).unwrap();
        assert_eq!(back, without_view(&back), "{depth:?}: opened with no view");
        let view = view_state(&doc);
        let state = view.state.as_ref().expect("a view to keep");
        assert!(serde_json::to_vec(state).unwrap().len() <= kition_photo_format::view_state::JSON_LIMIT);
        put_view_state(&mut back, state, &view.bytes).unwrap();
        back.id = doc.id;
        back.name.clone_from(&doc.name);
        assert_eq!(back, doc, "{depth:?}: the file and the view state make the whole document");
    }
}

/// A view with nothing in it is not kept (the host deletes what it kept); a value of another
/// version or another kit changes nothing; a damaged one is an error and changes nothing.
#[test]
fn an_empty_view_is_not_kept_and_a_foreign_one_changes_nothing() {
    let doc = viewed_doc(SampleType::U8);
    assert_eq!(view_state(&without_view(&doc)), Default::default());
    let view = view_state(&doc);
    let state = view.state.clone().unwrap();
    let mut back = without_view(&doc);
    let before = back.clone();
    let mut other = state.clone();
    other["v"] = 2.into();
    put_view_state(&mut back, &other, &view.bytes).unwrap();
    put_view_state(&mut back, &serde_json::json!({ "zoom": 2 }), &[]).unwrap();
    assert_eq!(back, before, "another version and another payload are ignored");
    assert!(put_view_state(&mut back, &state, &view.bytes[..view.bytes.len() / 2]).is_err(), "cut bytes");
    let mut flipped = view.bytes.clone();
    flipped[0] ^= 1;
    assert!(put_view_state(&mut back, &state, &flipped).is_err(), "a changed byte");
    assert_eq!(back, before, "a damaged value changes nothing");
}

/// A selection whose compressed pixels pass the host's 16 MB is not kept (nor the quick mask);
/// the rest of the view is.
#[test]
fn a_view_too_large_for_the_host_keeps_all_but_its_pixels() {
    let mut doc = viewed_doc(SampleType::U8);
    let mut selection = kition_photo_raster::Surface::new(kition_photo_color::PixelFormat::GRAY8);
    let mut rng = Rng::new(3);
    for i in 0..300 {
        for byte in selection.tile_mut(kition_photo_geom::TileCoord::new(i % 20, i / 20)).bytes_mut() {
            *byte = rng.next() as u8;
        }
    }
    doc.selection = Some(selection);
    let view = view_state(&doc);
    assert!(view.bytes.len() <= BYTES_LIMIT, "{}", view.bytes.len());
    let state = view.state.unwrap();
    assert!(state.get("selection").is_none() && state.get("quickMask").is_none(), "{state}");
    assert_eq!(state["frame"], 4);
    assert_eq!(state["lastAppliedComp"], 7);
    assert!(state.get("lastDocumentState").is_some());
}

#[test]
fn manifest_is_human_readable_json() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let r = zip::ZipReader::new(&bytes).unwrap();
    assert_eq!(r.entries[0].name, "manifest.json");
    let text = String::from_utf8(r.read_by_name("manifest.json", usize::MAX).unwrap()).unwrap();
    assert!(text.contains("\"format_version\": 1"));
    assert!(text.contains("\"Hue/Sat\""));
    assert!(text.contains("filter.blur.gaussian"));
    assert!(r.entries.iter().any(|e| e.name.strip_prefix("tiles/").is_some_and(|hash| hash.len() == 64)));
    assert!(r.entries.iter().any(|e| e.name.starts_with("blobs/")));
}

#[test]
fn save_is_deterministic() {
    let doc = rich_doc(ColorMode::Cmyk, SampleType::U16);
    let a = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let b = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn not_kphoto() {
    assert!(!is_kphoto(b""));
    assert!(!is_kphoto(b"8BPS...."));
    let mut z = zip::ZipWriter::new();
    z.add("other.txt", b"x").unwrap();
    assert!(!is_kphoto(&z.finish().unwrap()));
}

#[test]
fn vector_fields_roundtrip_and_default_when_absent() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let back = load_from_bytes(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap();
    assert_eq!(back.paths, doc.paths);
    assert_eq!(back.work_path, doc.work_path);
    assert_eq!(back.clipping_path, doc.clipping_path);
    let vm = |d: &Document| d.walk().into_iter().find_map(|(_, _, l)| l.vector_mask.clone());
    assert!(vm(&back).is_some());
    assert_eq!(vm(&back), vm(&doc));
    // Manifests written before the vector fields existed still load (serde defaults).
    let m = read_manifest(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap();
    let mut v = serde_json::to_value(&m.document).unwrap();
    for k in ["paths", "work_path", "clipping_path"] {
        v.as_object_mut().unwrap().remove(k);
    }
    fn strip(layers: &mut serde_json::Value) {
        for l in layers.as_array_mut().unwrap() {
            l.as_object_mut().unwrap().remove("vector_mask");
            let c = l["content"].as_object_mut().unwrap();
            for k in ["path", "stroke", "live"] {
                c.remove(k);
            }
            if let Some(ch) = c.get_mut("children") {
                strip(ch);
            }
        }
    }
    strip(&mut v["layers"]);
    let old: kition_photo_format::manifest::DocM = serde_json::from_value(v).unwrap();
    assert!(old.paths.is_empty() && old.work_path.is_none());
    assert!(old.layers.iter().all(|l| l.vector_mask.is_none()));
}

/// Type kerning (#206): the auto mode and manual kerning survive .kphoto; manifests written
/// before `kern` existed load with no manual kerning.
#[test]
fn text_kerning_roundtrips_and_defaults() {
    use kition_photo_doc::LayerContent;
    use kition_photo_doc::text::Kerning;
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let back = load_from_bytes(&bytes).unwrap();
    let runs = |d: &Document| {
        d.walk()
            .into_iter()
            .find_map(|(_, _, l)| match &l.content {
                LayerContent::Text(t) => Some(t.runs.iter().map(|r| (r.len, r.style.kerning, r.style.kern)).collect::<Vec<_>>()),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(runs(&back), vec![(1, Kerning::Off, 120.0), (5, Kerning::Metrics, 0.0), (9, Kerning::Optical, 0.0)]);
    assert_eq!(runs(&back), runs(&doc));
    let m = read_manifest(&bytes).unwrap();
    let mut v = serde_json::to_value(&m.document).unwrap();
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(o) => {
                o.remove("kern");
                o.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut v);
    assert!(!v.to_string().contains("\"kern\""));
    let old: kition_photo_format::manifest::DocM = serde_json::from_value(v).unwrap();
    let s = serde_json::to_string(&old).unwrap();
    assert!(s.contains("\"kern\":0.0"), "kern defaults to 0");
}

#[test]
fn channel_restrictions_and_bevel_elements_roundtrip() {
    use kition_photo_doc::{Bevel, BevelContour, BevelTexture, Contour, Effect};
    let mut doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    doc.layers[0].excluded_channels = 0b101;
    let mut b = Bevel {
        enabled: true,
        style: kition_photo_doc::BevelStyle::Emboss,
        technique: kition_photo_doc::BevelTechnique::ChiselHard,
        depth: 1.2,
        up: false,
        size: 9.0,
        soften: 1.0,
        angle: 30.0,
        altitude: 40.0,
        use_global_light: false,
        gloss_contour: Contour::Linear,
        highlight: kition_photo_doc::FxCommon::new(kition_photo_color::BlendMode::Screen, 0.7),
        highlight_color: kition_photo_color::Color::WHITE,
        shadow: kition_photo_doc::FxCommon::new(kition_photo_color::BlendMode::Multiply, 0.6),
        shadow_color: kition_photo_color::Color::BLACK,
        contour: None,
        texture: None,
    };
    b.contour = Some(BevelContour { contour: Contour::Linear, range: 0.3, anti_alias: true });
    b.texture = Some(BevelTexture { name: "p".into(), id: "i".into(), scale: 0.5, depth: -1.5, invert: true, link: false, phase: (1.0, 2.0) });
    doc.layers[0].effects.items.push(Effect::BevelEmboss(Bevel { ..b }));
    let back = load_from_bytes(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap();
    assert_eq!(viewed(back.clone(), &doc), doc);
    assert_eq!(back.layers[0].excluded_channels, 0b101);
}

#[test]
fn blend_if_roundtrips() {
    use kition_photo_doc::{BlendIf, BlendRange};
    let mut doc = rich_doc(ColorMode::Rgb, SampleType::U16);
    let mut bi = BlendIf::default();
    bi.set(0, [BlendRange { black: [20, 60], white: [255, 255] }, BlendRange::FULL]);
    bi.set(3, [BlendRange::FULL, BlendRange { black: [0, 0], white: [180, 220] }]);
    doc.layers[0].blend_if = bi.clone();
    let back = load_from_bytes(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap();
    assert_eq!(viewed(back.clone(), &doc), doc);
    assert_eq!(back.layers[0].blend_if, bi);
    // Layers without Blend If keep the default (the field is omitted from the manifest).
    assert!(back.layers.iter().skip(1).all(|l| l.blend_if.is_default()));
}

/// Advanced Blending (knockout, as-group switches, transparency shapes, masks hide effects)
/// survives .kphoto at every depth; manifests written before the field existed load with
/// Photoshop's defaults, and default layers don't write it.
#[test]
fn advanced_blending_roundtrips_and_defaults_when_absent() {
    use kition_photo_doc::{AdvancedBlending, Knockout};
    for depth in SampleType::ALL {
        let mut doc = rich_doc(ColorMode::Rgb, depth);
        let a = AdvancedBlending {
            knockout: Knockout::Deep,
            blend_interior: true,
            blend_clipped: false,
            transparency_shapes: false,
            layer_mask_hides_effects: true,
            vector_mask_hides_effects: true,
        };
        doc.layers[0].advanced = a;
        if let Some(l) = doc.layers.get_mut(1) {
            l.advanced.knockout = Knockout::Shallow;
        }
        let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
        let back = load_from_bytes(&bytes).unwrap();
        assert_eq!(viewed(back.clone(), &doc), doc, "{depth:?}");
        assert_eq!(back.layers[0].advanced, a);
        // Only the non-default layers carry the field.
        let m = read_manifest(&bytes).unwrap();
        let v = serde_json::to_value(&m.document).unwrap();
        let with = v["layers"].as_array().unwrap().iter().filter(|l| l.get("advanced").is_some()).count();
        assert_eq!(with, if doc.layers.len() > 1 { 2 } else { 1 });
        // An older manifest (no `advanced`) loads with the defaults.
        let mut old = v.clone();
        for l in old["layers"].as_array_mut().unwrap() {
            l.as_object_mut().unwrap().remove("advanced");
        }
        let old: kition_photo_format::manifest::DocM = serde_json::from_value(old).unwrap();
        assert!(old.layers.iter().all(|l| l.advanced.is_default()));
    }
}

#[test]
fn video_layer_frames_survive_roundtrip() {
    use kition_photo_doc::{Timeline, VideoData, VideoSource};
    use kition_photo_geom::Rect;
    let mut doc = Document::new("Vid", kition_photo_geom::Size { width: 8, height: 6 }, ColorMode::Rgb, SampleType::U8);
    doc.timeline = Some(Timeline::new(3, 24.0));
    let fmt = doc.pixel_format();
    let mut l = Layer::raster("Clip", fmt);
    let mut frames = Vec::new();
    for i in 0..3u8 {
        let mut s = kition_photo_raster::Surface::new(fmt);
        let v = i as f32 / 2.0;
        s.fill_rect(Rect::new(0, 0, 8, 6), &kition_photo_raster::from_rgba(&fmt, [v, 0.0, 1.0 - v, 1.0]));
        frames.push(s);
    }
    let mut vd = VideoData::new(frames, 24.0);
    vd.source = VideoSource::File { path: "/tmp/seq".into() };
    vd.show_altered = false;
    l.video = Some(vd);
    doc.layers.push(l);

    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    let back = load_from_bytes(&bytes).unwrap();
    let v = back.layers.last().unwrap().video.as_ref().expect("video survived");
    assert_eq!(v.frames.len(), 3);
    assert_eq!(v.fps, 24.0);
    assert!(!v.show_altered);
    assert_eq!(v.source, VideoSource::File { path: "/tmp/seq".into() });
    assert_eq!(back.timeline.as_ref().unwrap().duration, 3);
    // A middle frame's pixel survived.
    let mut px = [[0.0f32; 4]; 1];
    v.frames[2].read_rgba_into(Rect::from_xywh(1, 1, 1, 1), &mut px);
    assert!(px[0][0] > 0.9 && px[0][2] < 0.1, "frame 2 is reddish: {:?}", px[0]);
}

/// Issue #1101: serde_json writes NaN and the infinities as `null`, which the manifest's float
/// fields refuse on load, so such a document saved "successfully" and never opened again. The
/// save is refused instead, naming the value; finite documents are unaffected.
#[test]
fn a_non_finite_float_is_refused_at_save_rather_than_breaking_every_later_load() {
    use kition_photo_doc::Adjustment;
    type Mutate = Box<dyn Fn(&mut Document)>;
    let base = rich_doc(ColorMode::Rgb, SampleType::U8);
    let cases: Vec<(&str, Mutate)> = vec![
        (
            "exposure",
            Box::new(|d| {
                d.insert_above(None, Layer::new("adj", LayerContent::Adjustment(Adjustment::Exposure { exposure: f32::NAN, offset: 0.0, gamma: 1.0 })));
            }),
        ),
        (
            "gamma",
            Box::new(|d| {
                d.insert_above(
                    None,
                    Layer::new("adj", LayerContent::Adjustment(Adjustment::Exposure { exposure: 0.0, offset: 0.0, gamma: f32::NEG_INFINITY })),
                );
            }),
        ),
        ("resolution_dpi", Box::new(|d| d.resolution_dpi = f32::INFINITY)),
    ];
    for (field, mutate) in cases {
        let mut doc = base.clone();
        mutate(&mut doc);
        let err = save_to_bytes(&doc, &SaveOptions::default()).expect_err(field);
        assert!(matches!(err, FormatError::NonFinite { .. }), "{field}: {err:?}");
        let msg = err.to_string();
        assert!(msg.contains(field) && msg.contains("NaN or infinite"), "{field}: {msg}");
        // The incremental writer shares the check, for autosave and directory bundles.
        let mut w = KitionPhotoWriter::new();
        assert!(matches!(w.save_zip(&doc, &SaveOptions::default()), Err(FormatError::NonFinite { .. })), "{field}: KitionPhotoWriter");
    }
    // The same document with finite values round-trips as before.
    let mut doc = base.clone();
    doc.insert_above(None, Layer::new("adj", LayerContent::Adjustment(Adjustment::Exposure { exposure: 0.5, offset: 0.0, gamma: 1.0 })));
    let back = load_from_bytes(&save_to_bytes(&doc, &SaveOptions::default()).unwrap()).unwrap();
    assert_eq!(viewed(back.clone(), &doc), doc);
}
