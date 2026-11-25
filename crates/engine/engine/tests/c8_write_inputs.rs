//! C8-photo-3: commands use the addressed layer and the document's rendering inputs.
use kition_photo_engine::Session;
use kition_photo_engine::doc::LayerId;
use serde_json::json;

fn image(depth: u32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width":64,"height":48,"depth":depth})).unwrap();
    s.execute("layer.new.layer", json!({"name":"Lower"})).unwrap();
    s.execute("edit.fill", json!({"color":"#ffffff"})).unwrap();
    s.execute("layer.new.layer", json!({"name":"Upper"})).unwrap();
    s.execute("select.rect", json!({"x":20,"y":16,"width":16,"height":16})).unwrap();
    s.execute("edit.fill", json!({"color":"#cc3366"})).unwrap();
    s.execute("edit.deselectAll", json!({})).unwrap();
    s
}

fn pixels(s: &Session, id: LayerId) -> Vec<f32> {
    let d = &s.active().unwrap().doc;
    d.layer(id).unwrap().surface().unwrap().read_region(d.bounds())
}

fn composite(s: &Session) -> Vec<[f32; 4]> {
    let d = &s.active().unwrap().doc;
    kition_photo_compose::render(d, d.bounds()).px
}

#[test]
fn explicit_pixel_target_is_enabled_even_when_the_active_layer_is_a_group() {
    let mut s = image(8);
    let id = s.active().unwrap().active_layer.unwrap();
    s.execute("layer.new.group", json!({})).unwrap();
    let active = s.active().unwrap().active_layer;
    let before = pixels(&s, id);
    assert!(s.is_enabled_with("filter.blur.gaussianBlur", &json!({"layer":id.0,"radius":2})));
    s.execute("filter.blur.gaussianBlur", json!({"layer":id.0,"radius":2})).unwrap();
    assert_ne!(pixels(&s, id), before);
    assert_eq!(s.active().unwrap().active_layer, active, "checking a target does not select it");
}

#[test]
fn single_colour_channel_restriction_follows_the_layer_written() {
    for depth in [8, 16, 32] {
        let mut s = image(depth);
        let id = s.active().unwrap().active_layer.unwrap();
        s.execute("layer.new.layer", json!({"name":"Active"})).unwrap();
        let active = s.active().unwrap().active_layer.unwrap();
        let other = pixels(&s, active);
        let before = pixels(&s, id);
        s.execute("channel.target", json!({"channel":"red"})).unwrap();
        s.execute("image.adjustments.invert", json!({"layer":id.0})).unwrap();
        let after = pixels(&s, id);
        for (old, new) in before.as_chunks::<4>().0.iter().zip(after.as_chunks::<4>().0.iter()) {
            assert_eq!(&new[1..], &old[1..], "depth {depth}: invert changed channels other than red");
        }
        assert!(before != after, "red was edited");
        assert_eq!(pixels(&s, active), other, "the active layer was not edited");
    }
}

#[test]
fn clear_and_cut_keep_transparency_locked_alpha() {
    for command in ["edit.delete", "edit.cut"] {
        for depth in [8, 16, 32] {
            let mut s = image(depth);
            let id = s.active().unwrap().active_layer.unwrap();
            s.execute("layer.lockLayers", json!({"transparency":true})).unwrap();
            let before = pixels(&s, id);
            s.execute(command, json!({"layer":id.0})).unwrap();
            let after = pixels(&s, id);
            for (old, new) in before.as_chunks::<4>().0.iter().zip(after.as_chunks::<4>().0.iter()) {
                assert_eq!(new[3], old[3], "{command}, depth {depth}: locked alpha changed");
            }
            assert_eq!(s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().rgba(24, 20), [1.0; 4]);
        }
    }
}

fn shadow(s: &mut Session) {
    s.execute("layer.layerStyle.globalLight", json!({"angle":0,"altitude":60})).unwrap();
    s.execute("layer.layerStyle.dropShadow", json!({"useGlobalLight":true,"distance":8,"size":0,"opacity":100})).unwrap();
}

fn assert_appearance(before: &[[f32; 4]], after: &[[f32; 4]], what: &str) {
    let error = before.iter().zip(after).flat_map(|(a, b)| a.iter().zip(b)).map(|(a, b)| (a - b).abs()).fold(0.0_f32, f32::max);
    assert!(error <= 1.0 / 255.0 + 1e-6, "{what}: max channel error {error}");
}

#[test]
fn merge_down_keeps_the_documents_global_light() {
    for depth in [8, 16, 32] {
        let mut s = image(depth);
        shadow(&mut s);
        let before = composite(&s);
        s.execute("layer.mergeDown", json!({})).unwrap();
        assert_appearance(&before, &composite(&s), "Merge Down");
    }
}

#[test]
fn flatten_effects_keeps_the_documents_global_light() {
    for depth in [8, 16, 32] {
        let mut s = image(depth);
        shadow(&mut s);
        let before = composite(&s);
        s.execute("file.scripts.flattenAllLayerEffects", json!({})).unwrap();
        assert_appearance(&before, &composite(&s), "Flatten All Layer Effects");
    }
}

#[test]
fn flatten_effects_keeps_document_patterns() {
    let mut s = image(8);
    let id = s.active().unwrap().active_layer.unwrap();
    s.execute("layer.newFillLayer.pattern", json!({"pattern":"Checkerboard"})).unwrap();
    let fill = s.active().unwrap().active_layer.unwrap();
    s.execute("layer.delete", json!({"layer":fill.0})).unwrap();
    s.execute("layer.select", json!({"layer":id.0})).unwrap();
    s.execute("layer.layerStyle.patternOverlay", json!({"pattern":"Checkerboard","opacity":100})).unwrap();
    let before = composite(&s);
    s.execute("file.scripts.flattenAllLayerEffects", json!({})).unwrap();
    assert_appearance(&before, &composite(&s), "Pattern Overlay");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn snapshot_jobs_discard_their_result_after_a_colleague_writes() {
    use kition_photo_engine::jobs::Started;
    for (command, params) in [("edit.smartFill", json!({})), ("paint.smartMove", json!({"offset":[8,0]})), ("layer.removeBackground", json!({"refine":false}))]
    {
        let mut s = image(8);
        s.execute("select.rect", json!({"x":20,"y":16,"width":8,"height":8})).unwrap();
        let Started::Job(job) = s.start(command, params).unwrap() else { panic!("{command} did not start a job") };
        // A separate engine edits the snapshot while the worker computes. Importing its new
        // document bypasses the local command lock, as replica delivery does.
        let mut colleague = Session::new();
        colleague.add_document((*s.active().unwrap().doc).clone(), None);
        colleague.execute("select.rect", json!({"x":2,"y":2,"width":4,"height":4})).unwrap();
        colleague.execute("edit.fill", json!({"color":"#0000ff"})).unwrap();
        let changed = colleague.active().unwrap().doc.clone();
        s.active_mut().unwrap().doc = changed.clone();
        let error = s.wait_job(job).expect_err("a stale job must not write its snapshot back");
        assert!(error.to_string().contains("changed while"), "{command}: {error}");
        assert!(std::sync::Arc::ptr_eq(&s.active().unwrap().doc, &changed), "{command}: collaborator's document was replaced");
    }
}
