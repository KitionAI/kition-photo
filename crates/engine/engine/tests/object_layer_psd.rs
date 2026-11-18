//! Object layers and editable filters made in Kition Photo survive a PSD save and open (#215): the
//! layer comes back as a live object layer (not pixels) with the same nested document (embedded
//! in `lnk2` as a PSB), transform, filter list and filter mask, and re-renders like before.
//!
//! Set `KITION_PHOTO_SMART_PSD_OUT=<dir>` to keep the exported files (for opening in Photoshop).

use kition_photo_doc::{Document, LayerContent, ObjectLayer, SmartSource};
use kition_photo_engine::Session;
use kition_photo_engine::object_layer_cmds::{decode_source, refresh_layer, source_bytes};
use kition_photo_geom::Rect;
use serde_json::json;

const W: i32 = 96;
const H: i32 = 64;

fn session(depth: u64) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": W, "height": H, "depth": depth})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.edit("paint", |doc, active| {
        let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
        let fmt = surf.format();
        let r = Rect::new(8, 6, 80, 58);
        let mut data = Vec::new();
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let v = ((x * 7 + y * 13) % 23) as f32 / 22.0;
                data.extend(kition_photo_raster::from_rgba(&fmt, [v, 1.0 - v, ((x * y) % 7) as f32 / 6.0, 1.0]));
            }
        }
        surf.write_region(r, &data);
        Ok(())
    })
    .unwrap();
    s
}

fn smart(doc: &Document) -> (u64, ObjectLayer) {
    doc.walk()
        .into_iter()
        .find_map(|(_, _, l)| match &l.content {
            LayerContent::Smart(sm) => Some((l.id.0, sm.clone())),
            _ => None,
        })
        .expect("an object layer")
}

/// `doc` with its editable filter params in canonical KDM JSON (whole-number doubles as integers).
fn canonical_filters(doc: &Document) -> Document {
    fn canonical(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Number(n) if !n.is_i64() && !n.is_u64() => match n.as_f64() {
                Some(f) if f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_992.0 => json!(f as i64),
                _ => serde_json::Value::Number(n),
            },
            serde_json::Value::Array(items) => serde_json::Value::Array(items.into_iter().map(canonical).collect()),
            serde_json::Value::Object(map) => serde_json::Value::Object(map.into_iter().map(|(k, v)| (k, canonical(v))).collect()),
            other => other,
        }
    }
    let mut doc = doc.clone();
    for (_, _, layer) in doc.clone().walk() {
        if let Some(LayerContent::Smart(sm)) = doc.layer_mut(layer.id).map(|l| &mut l.content) {
            for filter in &mut sm.editable_filters {
                filter.params = canonical(filter.params.clone());
            }
        }
    }
    doc
}

fn max_diff(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).flat_map(|(p, q)| (0..4).map(move |c| (p[c] * p[3].max(0.0) - q[c] * q[3].max(0.0)).abs())).fold(0.0, f32::max)
}

/// Every block with inner structure re-parses strictly (`TaggedBlock::check_structure`).
fn structure_errors(bytes: &[u8]) -> Vec<String> {
    let f = kition_photo_psd::PsdFile::from_bytes(bytes).unwrap();
    let mut errs = Vec::new();
    for b in &f.global_blocks {
        if let Err(e) = b.check_structure() {
            errs.push(format!("global {}: {e}", b.key_str()));
        }
    }
    for l in f.layers() {
        for b in &l.blocks {
            if let Err(e) = b.check_structure() {
                errs.push(format!("layer {}: {e}", b.key_str()));
            }
        }
    }
    errs
}

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("KITION_PHOTO_SMART_PSD_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

#[test]
fn object_layers_and_filters_survive_psd() {
    for depth in [8, 16, 32] {
        let mut s = session(depth);
        s.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap();
        s.execute("filter.blur.gaussianBlur", json!({"radius": 2})).unwrap();
        s.execute("select.rect", json!({"x": 0, "y": 0, "width": 40, "height": 64})).unwrap();
        s.execute("filter.noise.addNoise", json!({"amount": 20, "seed": 3, "distribution": "gaussian"})).unwrap();
        s.execute("select.deselect", json!({})).unwrap();
        s.execute("layer.editableFilter.blendingOptions", json!({"opacity": 0.5, "blend": "screen"})).unwrap();
        s.execute("filter.stylize.emboss", json!({"angle": 120, "height": 2, "amount": 80})).unwrap();
        s.execute("layer.editableFilter.setVisible", json!({"index": 2, "visible": false})).unwrap();
        let id = s.active().unwrap().active_layer.unwrap().0;
        s.execute("edit.transform", json!({"layer": id, "matrix": [0.75, 0, 0, 0.75, 10, 4]})).unwrap();
        let doc = s.active().unwrap().doc.clone();
        let (_, before) = smart(&doc);
        let SmartSource::Embedded { bytes: kphoto, .. } = &before.source else { panic!("embedded") };
        let nested = decode_source("x.kphoto", kphoto).unwrap();

        let out = kition_photo_io::export(&doc, "psd", &Default::default()).unwrap();
        assert!(!out.warnings.iter().any(|w| w.contains("pixels")), "{:?}", out.warnings);
        assert_eq!(structure_errors(&out.bytes), Vec::<String>::new());
        keep(&format!("smart-{depth}.psd"), &out.bytes);

        let back = kition_photo_io::import("x.psd", &out.bytes).unwrap().document;
        let (bid, after) = smart(&back);
        assert!(matches!(after.source, SmartSource::Linked { .. }), "depth {depth}: source in lnk2");
        // Same filters, blending and settings (numbers may come back as floats, defaults spelled out).
        assert_eq!(after.editable_filters.len(), before.editable_filters.len());
        for (a, b) in after.editable_filters.iter().zip(&before.editable_filters) {
            assert_eq!((&a.command, a.blend, a.opacity, a.visible), (&b.command, b.blend, b.opacity, b.visible), "depth {depth}");
            for (k, v) in b.params.as_object().unwrap() {
                let w = &a.params[k];
                assert!(w == v || w.as_f64().zip(v.as_f64()).is_some_and(|(x, y)| x == y), "{}: {k} {v} came back as {w}", b.command);
            }
        }
        assert_eq!(after.filters_enabled, before.filters_enabled);
        // The filter mask covers the same pixels (PSD extends it with white past its bounds).
        let (ma, mb) = (after.filter_mask.as_ref().unwrap(), before.filter_mask.as_ref().unwrap());
        assert_eq!((ma.enabled, ma.linked), (mb.enabled, mb.linked));
        let area = doc.bounds().union(&before.cache.as_ref().unwrap().content_bounds());
        let (mut va, mut vb) = (Vec::new(), Vec::new());
        ma.values_into(area, &mut va);
        mb.values_into(area, &mut vb);
        assert_eq!(va, vb, "depth {depth}: filter mask");
        for (a, b) in after.transform.m.iter().zip(before.transform.m) {
            assert!((a - b).abs() < 1e-9, "depth {depth}: transform {:?} vs {:?}", after.transform, before.transform);
        }
        // The nested document came back (as the embedded PSB), layer for layer.
        let (name, src) = source_bytes(&back.metadata, &after.source).unwrap();
        assert!(name.ends_with(".psb"), "{name}");
        let inner = decode_source(&name, &src).unwrap();
        assert_eq!((inner.size, inner.layers.len(), inner.depth), (nested.size, nested.layers.len(), nested.depth));
        assert!(max_diff(&kition_photo_compose::flatten(&inner).px, &kition_photo_compose::flatten(&nested).px) <= 1.0 / 255.0 + 1e-6);
        // And it re-renders from that source like the original did.
        let mut l = back.layer(kition_photo_doc::LayerId(bid)).unwrap().clone();
        assert!(refresh_layer(&back, &mut l).unwrap());
        let mut rerendered = back.clone();
        *rerendered.layer_mut(kition_photo_doc::LayerId(bid)).unwrap() = l;
        let d = max_diff(&kition_photo_compose::flatten(&rerendered).px, &kition_photo_compose::flatten(&doc).px);
        assert!(d <= 2.0 / 255.0, "depth {depth}: re-render differs by {d}");

        // Saving the re-imported file again keeps the same embedded file (no growth).
        let again = kition_photo_io::export(&back, "psd", &Default::default()).unwrap();
        assert_eq!(structure_errors(&again.bytes), Vec::<String>::new());
        assert!(again.bytes.len() <= out.bytes.len() + 64, "{} vs {}", again.bytes.len(), out.bytes.len());
    }
}

#[test]
fn raw_develop_psd_filters_survive_re_editing_native_save_and_undo() {
    for depth in [8, 16, 32] {
        let mut s = session(depth);
        s.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap();
        s.execute("filter.blur.gaussianBlur", json!({"radius": 1})).unwrap();
        s.execute("select.rect", json!({"x": 10, "y": 8, "width": 50, "height": 40})).unwrap();
        s.execute(
            "filter.rawDevelop",
            json!({
                "exposure": 1.15, "contrast": -38, "highlights": 35, "shadows": -50, "whites": 49, "blacks": -34,
                "curveDarks": -21, "curveLights": 38, "hslHue": [-50, 0, 29, 1, 0, 0, 0, 0],
                "temperature": -26, "tint": -25, "texture": -31, "clarity": 30, "dehaze": -34, "vibrance": 33, "saturation": 33,
                "sharpenAmount": 43, "sharpenRadius": 1.2, "sharpenDetail": 36, "sharpenMasking": 30,
                "noiseLuminance": 47, "noiseLuminanceDetail": 50, "noiseColor": 25, "noiseColorDetail": 50,
                "gradeShadows": {"hue": 117, "sat": 52, "lum": -24}, "gradeMidtones": {"hue": 51, "sat": 51, "lum": 41},
                "gradeHighlights": {"hue": 316, "sat": 70, "lum": 28}, "gradeGlobal": {"hue": 20, "sat": 61, "lum": 45},
                "gradeBlending": 36, "gradeBalance": 35, "grainAmount": 45, "grainSize": 25, "grainRoughness": 50,
                "pointCurve": [[0, 0], [146, 102], [255, 255]], "pointCurveRed": [[0, 0], [90, 174], [255, 255]],
                "pointCurveGreen": [[0, 0], [162, 122], [255, 255]], "pointCurveBlue": [[0, 0], [129, 202], [255, 255]]
            }),
        )
        .unwrap();
        s.execute("select.deselect", json!({})).unwrap();
        s.execute("filter.blur.gaussianBlur", json!({"radius": 2})).unwrap();
        let before = s.active().unwrap().doc.clone();
        let out = kition_photo_io::export(&before, "psd", &Default::default()).unwrap();
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(structure_errors(&out.bytes).is_empty());
        let back = kition_photo_io::import("raw-develop.psd", &out.bytes).unwrap().document;
        let (layer, imported) = smart(&back);
        let (_, original) = smart(&before);
        let mask_values = |sm: &ObjectLayer| {
            let mut values = Vec::new();
            sm.filter_mask.as_ref().unwrap().values_into(before.bounds(), &mut values);
            values
        };
        assert_eq!(imported.editable_filters.len(), 3);
        assert_eq!(imported.editable_filters[1].command, "filter.rawDevelop");
        assert_eq!(mask_values(&imported), mask_values(&original));
        let expected = kition_photo_engine::lens_cmds::raw_params("filter.rawDevelop", &original.editable_filters[1].params).unwrap();
        let actual = kition_photo_engine::lens_cmds::raw_params("filter.rawDevelop", &imported.editable_filters[1].params).unwrap();
        assert_eq!(actual, expected, "{depth}-bit");
        let template = imported.editable_filters[1].params["__rawDevelopPsd"].clone();
        let mut edited = Session::new();
        edited.add_document(back, None);
        edited.execute("layer.editableFilter.setParams", json!({"layer": layer, "index": 1, "params": {"exposure": 0, "curveLights": 0}})).unwrap();
        assert_eq!(smart(&edited.active().unwrap().doc).1.editable_filters[1].params["__rawDevelopPsd"], template);
        let changed = edited.active().unwrap().doc.clone();
        edited.undo();
        assert_eq!(smart(&edited.active().unwrap().doc).1.editable_filters[1].params["exposure"], imported.editable_filters[1].params["exposure"]);
        edited.redo();
        // Redo comes from the document's KDM replica, whose JSON writes whole-number doubles as
        // integers (canonical KDM JSON): compare filter params in that form.
        assert_eq!(canonical_filters(&edited.active().unwrap().doc), canonical_filters(&changed));
        let native = kition_photo_format::save_to_bytes(&changed, &Default::default()).unwrap();
        let native_back = kition_photo_format::load_from_bytes(&native).unwrap();
        assert_eq!(smart(&native_back).1.editable_filters, smart(&changed).1.editable_filters);
        let out = kition_photo_io::export(&native_back, "psd", &Default::default()).unwrap();
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
        assert!(structure_errors(&out.bytes).is_empty());
        let back = kition_photo_io::import("edited.psd", &out.bytes).unwrap().document;
        let (_, after) = smart(&back);
        assert_eq!(after.editable_filters.len(), 3);
        let params = kition_photo_engine::lens_cmds::raw_params("filter.rawDevelop", &after.editable_filters[1].params).unwrap();
        assert_eq!((params.exposure, params.curve_lights), (0.0, 0.0));
        assert_eq!(params.hsl_hue, expected.hsl_hue);
        assert_eq!(mask_values(&after), mask_values(&original));
        let mut unfiltered = after.clone();
        unfiltered.editable_filters.clear();
        assert!(kition_photo_engine::object_layer_cmds::render(&back, &unfiltered).unwrap().is_some());
    }
}

/// Private, user-supplied files remain outside the repository. Set KITION_PHOTO_RAW_DEVELOP_PSD
/// to check a Raw Develop fixture and KITION_PHOTO_SMART_PSD_OUT to retain exports for manual checks.
#[test]
#[ignore = "requires a user-supplied Camera Raw PSD"]
fn raw_develop_psd_fixture() {
    let path = std::env::var("KITION_PHOTO_RAW_DEVELOP_PSD").expect("set KITION_PHOTO_RAW_DEVELOP_PSD");
    let bytes = std::fs::read(&path).unwrap();
    let imported = kition_photo_io::import("fixture.psd", &bytes).unwrap();
    let (_, sm) = smart(&imported.document);
    let index = sm
        .editable_filters
        .iter()
        .position(|f| {
            kition_photo_io::object_layer_map::item_for_filter(f).ok().is_some_and(
                |item| matches!(item.get("Fltr"), Some(kition_photo_psd::descriptor::Value::Descriptor(d)) if d.class_id.is("Adobe Camera Raw Filter")),
            )
        })
        .expect("a Raw Develop filter");
    let original_item = kition_photo_io::object_layer_map::item_for_filter(&sm.editable_filters[index]).unwrap();
    let out = kition_photo_io::export(&imported.document, "psd", &Default::default()).unwrap();
    assert!(!out.warnings.iter().any(|w| w.contains("editable filter")), "{:?}", out.warnings);
    assert!(structure_errors(&out.bytes).is_empty());
    keep("raw-develop-roundtrip.psd", &out.bytes);
    let back = kition_photo_io::import("roundtrip.psd", &out.bytes).unwrap().document;
    let (layer, _) = smart(&back);
    assert_eq!(kition_photo_io::object_layer_map::item_for_filter(&smart(&back).1.editable_filters[index]).unwrap(), original_item);
    let native = kition_photo_format::save_to_bytes(&back, &Default::default()).unwrap();
    assert_eq!(smart(&kition_photo_format::load_from_bytes(&native).unwrap()).1.editable_filters, smart(&back).1.editable_filters);
    if sm.editable_filters[index].command == kition_photo_io::object_layer_map::UNSUPPORTED_FILTER {
        assert_eq!(smart(&back).1.editable_filters[index].command, kition_photo_io::object_layer_map::UNSUPPORTED_FILTER);
        println!("Raw Develop settings survive PSD and native saves; active unmapped controls keep this filter opaque");
        return;
    }
    let mut s = Session::new();
    s.add_document(back, None);
    s.execute("layer.editableFilter.setParams", json!({"layer": layer, "index": index, "params": {"exposure": 0.5}})).unwrap();
    let out = kition_photo_io::export(&s.active().unwrap().doc, "psd", &Default::default()).unwrap();
    assert!(!out.warnings.iter().any(|w| w.contains("editable filter")), "{:?}", out.warnings);
    assert!(structure_errors(&out.bytes).is_empty());
    keep("raw-develop-edited.psd", &out.bytes);
    let back = kition_photo_io::import("edited.psd", &out.bytes).unwrap().document;
    let after = &smart(&back).1.editable_filters[index];
    assert_eq!(after.params["exposure"], 0.5);
    let mut expected = original_item;
    let mut fltr = match expected.get("Fltr").unwrap() {
        kition_photo_psd::descriptor::Value::Descriptor(d) => d.clone(),
        _ => panic!("Fltr must be a descriptor"),
    };
    fltr.items.iter_mut().find(|(key, _)| key.is("Ex12")).unwrap().1 = kition_photo_psd::descriptor::Value::Double(0.5);
    expected.items.iter_mut().find(|(key, _)| key.is("Fltr")).unwrap().1 = kition_photo_psd::descriptor::Value::Descriptor(fltr);
    assert_eq!(kition_photo_io::object_layer_map::item_for_filter(after).unwrap(), expected, "only Exposure changes");
}

/// Every colour model: the object layer, its filters and the filter cache's unfiltered pixels
/// (in the document's model, CMYK inverted and 16-bit Lab scaled as in layer channels) survive.
#[test]
fn editable_filters_survive_psd_in_every_colour_model() {
    for (mode, depth) in [("cmyk", 8), ("cmyk", 16), ("grayscale", 8), ("lab", 8), ("lab", 16)] {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 32, "height": 24, "mode": mode, "depth": depth})).unwrap();
        s.execute("layer.new.layer", json!({})).unwrap();
        s.execute("select.rect", json!({"x": 4, "y": 4, "width": 20, "height": 12})).unwrap();
        s.execute("edit.fill", json!({"color": "#c83c28"})).unwrap();
        s.execute("select.deselect", json!({})).unwrap();
        s.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap();
        s.execute("filter.blur.gaussianBlur", json!({"radius": 1.5})).unwrap();
        let doc = s.active().unwrap().doc.clone();
        let out = kition_photo_io::export(&doc, "psd", &Default::default()).unwrap();
        assert!(!out.warnings.iter().any(|w| w.contains("smart")), "{mode} {depth}: {:?}", out.warnings);
        assert_eq!(structure_errors(&out.bytes), Vec::<String>::new());
        let back = kition_photo_io::import("x.psd", &out.bytes).unwrap().document;
        let ((_, a), (_, b)) = (smart(&doc), smart(&back));
        assert_eq!(b.editable_filters.len(), 1, "{mode} {depth}");
        assert_eq!(b.editable_filters[0].command, a.editable_filters[0].command);
        // The cache's unfiltered planes decode back to the unfiltered rendering.
        let f = kition_photo_psd::PsdFile::from_bytes(&out.bytes).unwrap();
        let fx = f.global_blocks.iter().find(|g| &g.key == b"FEid").map(|g| kition_photo_psd::filter_effects::FilterEffects::parse(&g.data).unwrap()).unwrap();
        let item = &fx.items[0];
        let (w, h) = item.rect.size().unwrap();
        let bits = u16::try_from(item.depth).unwrap();
        let first = item.slots[0].as_ref().unwrap().decode(w, h, bits).unwrap();
        let mut unfiltered = a.clone();
        unfiltered.editable_filters.clear();
        let px = kition_photo_engine::object_layer_cmds::render(&doc, &unfiltered).unwrap().unwrap();
        let (x, y) = (10 - item.rect.left, 8 - item.rect.top);
        let at = (y as usize * w + x as usize) * usize::from(bits / 8);
        let stored = if bits == 8 { f32::from(first[at]) / 255.0 } else { f32::from(u16::from_be_bytes([first[at], first[at + 1]])) / 65535.0 };
        let v = px.sample_channel(10, 8, 0);
        let expect = if mode == "cmyk" { 1.0 - v } else { v };
        assert!((stored - expect).abs() < 0.01, "{mode} {depth}: slot 0 holds {stored}, expected {expect}");
    }
}

/// Writes sample files for checking in Photoshop (`KITION_PHOTO_SMART_PSD_OUT`): per depth, a
/// plain layer, an object layer without filters, and one with filters.
#[test]
#[ignore = "writes files for a manual Photoshop check"]
fn photoshop_samples() {
    for (mode, depth) in [("rgb", 8), ("rgb", 16), ("rgb", 32), ("cmyk", 8), ("lab", 16), ("grayscale", 8)] {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": W, "height": H, "mode": mode, "depth": depth})).unwrap();
        s.execute("layer.new.layer", json!({})).unwrap();
        s.execute("select.rect", json!({"x": 10, "y": 8, "width": 50, "height": 40})).unwrap();
        s.execute("edit.fill", json!({"color": "#c83c28"})).unwrap();
        s.execute("select.deselect", json!({})).unwrap();
        let tag = format!("{mode}{depth}");
        keep(&format!("plain-{tag}.psd"), &kition_photo_io::export(&s.active().unwrap().doc, "psd", &Default::default()).unwrap().bytes);
        s.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap();
        keep(&format!("nofilter-{tag}.psd"), &kition_photo_io::export(&s.active().unwrap().doc, "psd", &Default::default()).unwrap().bytes);
        s.execute("filter.blur.gaussianBlur", json!({"radius": 3})).unwrap();
        keep(&format!("gauss-{tag}.psd"), &kition_photo_io::export(&s.active().unwrap().doc, "psd", &Default::default()).unwrap().bytes);
    }
}

/// PSD export of a 24 MP object layer versus the same layer as pixels (release:
/// `cargo test --release -p kition-photo-engine --test object_layer_psd -- --ignored --nocapture perf`).
#[test]
#[ignore = "benchmark"]
fn perf_24mp_object_layer_export() {
    let (w, h) = (6000, 4000);
    let mut s = Session::new();
    s.execute("file.new", json!({"width": w, "height": h, "depth": 8})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.edit("paint", |doc, active| {
        let surf = doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap();
        let fmt = surf.format();
        let r = Rect::new(0, 0, w, h);
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                // Photo-like: smooth gradients plus a little texture (RLE can't collapse it).
                let v = (x as f32 / w as f32) * 0.7 + (((x * 31 + y * 17) % 29) as f32 / 29.0) * 0.1;
                data.extend(kition_photo_raster::from_rgba(&fmt, [v, (y as f32 / h as f32) * 0.8, 1.0 - v, 1.0]));
            }
        }
        surf.write_region(r, &data);
        Ok(())
    })
    .unwrap();
    let time = |doc: &Document| {
        let t = std::time::Instant::now();
        let out = kition_photo_io::export(doc, "psd", &Default::default()).unwrap();
        (t.elapsed().as_secs_f64(), out.bytes.len(), out.warnings)
    };
    let (tf, sf, _) = time(&s.active().unwrap().doc);
    s.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap();
    let (ts, ss, ws) = time(&s.active().unwrap().doc);
    assert!(ws.is_empty(), "{ws:?}");
    s.execute("filter.blur.gaussianBlur", json!({"radius": 3})).unwrap();
    let (tg, sg, wg) = time(&s.active().unwrap().doc);
    assert!(wg.is_empty(), "{wg:?}");
    let mb = |b: usize| b as f64 / 1e6;
    eprintln!(
        "24 MP PSD export: pixels {tf:.2} s {:.1} MB | object layer {ts:.2} s {:.1} MB | + Gaussian Blur editable filter {tg:.2} s {:.1} MB",
        mb(sf),
        mb(ss),
        mb(sg)
    );
    // The embedded file is about one more copy of the layer; the filter cache one more again.
    assert!(ss < sf * 5 / 2, "object layer {ss} vs pixels {sf}");
    assert!(sg < sf * 4, "with filters {sg} vs pixels {sf}");
}

#[test]
fn edited_psd_object_layers_drop_their_old_file() {
    let mut s = session(8);
    s.execute("layer.objectLayers.convertToObjectLayer", json!({})).unwrap();
    let doc = s.active().unwrap().doc.clone();
    let first = kition_photo_io::export(&doc, "psd", &Default::default()).unwrap().bytes;
    // Re-open, edit the contents (a new embedded file), save: one embedded file, the new one.
    let mut s2 = Session::new();
    s2.add_document(kition_photo_io::import("a.psd", &first).unwrap().document, None);
    let (id, _) = smart(&s2.active().unwrap().doc);
    s2.select_layer(kition_photo_doc::LayerId(id)).unwrap();
    s2.execute("layer.objectLayers.editContents", json!({})).unwrap();
    s2.execute("image.adjustments.invert", json!({})).unwrap();
    s2.execute("layer.objectLayers.saveContents", json!({})).unwrap();
    let parent = s2.documents().iter().position(|d| d.doc.layer(kition_photo_doc::LayerId(id)).is_some()).unwrap();
    let doc2 = s2.documents()[parent].doc.clone();
    let second = kition_photo_io::export(&doc2, "psd", &Default::default()).unwrap().bytes;
    let f = kition_photo_psd::PsdFile::from_bytes(&second).unwrap();
    let uuids: Vec<String> = f.global_blocks.iter().filter(|b| &b.key == b"lnk2").flat_map(|b| kition_photo_io::linked::block_uuids(&b.data)).collect();
    let back = kition_photo_io::import("b.psd", &second).unwrap().document;
    let (_, sm) = smart(&back);
    let SmartSource::Linked { path } = &sm.source else { panic!() };
    assert_eq!(uuids, vec![path.clone()]);
}
