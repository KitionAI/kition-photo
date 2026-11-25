//! What one call depends on (task card C-photo; `src/place.rs`): `under` names the layer a call
//! names by id, by `"#n"` (the top level from the top) or by a point, with the premises of the
//! families whose code was read; `covered` says what a point named before others changed the
//! image; `derived` answers each layer's non-transparent bounds.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::{CoveredAsk, KitDocument, Premise, Within};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

/// White 600 x 400: A, a red block at (10, 10, 100, 100); B, a blue block at (300, 50, 200, 200)
/// above it. B is active.
fn image() -> (PhotoDocument, u64, u64, u64) {
    let mut doc = PhotoDocument::create("art.psd", 600, 400).unwrap();
    let background = doc.inspect().unwrap()["layers"][0]["id"].as_u64().unwrap();
    let layer = |doc: &mut PhotoDocument, name: &str, rect: [i32; 4], color: &str| {
        let id = doc.execute("layer.new.layer", json!({ "name": name })).unwrap()["layer"].as_u64().unwrap();
        doc.execute("select.rect", json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] })).unwrap();
        doc.execute("edit.fill", json!({ "color": color })).unwrap();
        doc.execute("edit.deselectAll", json!({})).unwrap();
        id
    };
    let a = layer(&mut doc, "A", [10, 10, 100, 100], "#ff0000");
    let b = layer(&mut doc, "B", [300, 50, 200, 200], "#0000ff");
    (doc, background, a, b)
}

fn premise_of(under: &kition_kit::Under, id: impl Into<Value>) -> Premise {
    let id = id.into();
    under.premises.iter().find(|premise| premise.id == id).cloned().unwrap_or_else(|| panic!("no premise on {id}: {under:?}"))
}

#[test]
fn a_property_write_by_id_holds_the_fields_it_sets_and_no_position() {
    let (doc, _, a, _) = image();
    let under = doc.under("layer.setProps", &json!({ "layer": a, "opacity": 0.5, "name": "x" })).unwrap().unwrap();
    assert!(under.objects.is_empty() && under.within.is_empty() && under.covers.is_none(), "{under:?}");
    let premise = premise_of(&under, a);
    assert_eq!(premise.fields, Some(vec!["name".to_owned(), "opacity".to_owned()]));
    assert_eq!(premise.positions, Some(vec![]));
}

#[test]
fn a_fill_in_a_region_holds_destination_content_and_effective_canvas_clipping() {
    let (doc, _, a, _) = image();
    let under = doc.under("edit.fill", &json!({ "layer": a, "color": "#000000", "region": { "rect": [250, 10, 20, 300] } })).unwrap().unwrap();
    let premise = premise_of(&under, a);
    assert_eq!(premise.fields, Some(["kind", "locks", "data", "text"].map(str::to_owned).to_vec()));
    assert_eq!(premise_of(&under, "image").fields, Some(["mode", "depth", "~inheritedLocks"].map(str::to_owned).to_vec()));
    assert_eq!(
        under.content,
        vec![
            kition_kit::ContentPremise { id: json!(a), field: "tiles".to_owned(), region: json!([250, 10, 270, 310]) },
            kition_kit::ContentPremise { id: json!("image"), field: "~canvasCoordinates".to_owned(), region: json!(a) },
            kition_kit::ContentPremise { id: json!("image"), field: "width".to_owned(), region: json!([250, 10, 270, 310]) },
            kition_kit::ContentPremise { id: json!("image"), field: "height".to_owned(), region: json!([250, 10, 270, 310]) },
        ]
    );
    // A stroke: its points widened by the brush.
    let under = doc.under("paint.stroke", &json!({ "layer": a, "points": [[2, 2, 1], [20, 20, 1]], "size": 8 })).unwrap().unwrap();
    assert_eq!(premise_of(&under, a).fields, Some(["kind", "locks", "data", "text"].map(str::to_owned).to_vec()));
    assert_eq!(under.content[0], kition_kit::ContentPremise { id: json!(a), field: "tiles".to_owned(), region: json!([-3, -3, 25, 25]) });
    // A filter on the whole layer.
    let under = doc.under("filter.blur.gaussianBlur", &json!({ "layer": a, "radius": 2 })).unwrap().unwrap();
    assert_eq!(premise_of(&under, a).fields, Some(["kind", "locks", "tiles", "data", "text"].map(str::to_owned).to_vec()));
    // A command that reads all of its layer (C8-photo-read: rasterizing reads its content, its
    // effects and its masks) holds every field of it.
    let under = doc.under("layer.rasterize.layer", &json!({ "layer": a })).unwrap().unwrap();
    assert_eq!(premise_of(&under, a).fields, None, "{under:?}");
}

/// `"#n"` counts the image's top level from the top, as inspect and the Layers panel list it
/// (`index_reversed`): `"#1"` is the top layer. `under` answers the window in stored order (the
/// replica's tree lists the stack bottom first) and the image, where the index counts, as a premise
/// of existence alone.
#[test]
fn an_index_names_the_top_level_from_the_top() {
    let (doc, background, a, b) = image();
    let under = doc.under("layer.setProps", &json!({ "layer": "#2", "visible": false })).unwrap().unwrap();
    assert_eq!(under.objects, vec![json!(a)]);
    assert_eq!(under.within, vec![Within { parent: json!("image"), field: "layers".to_owned(), from: 1, to: 2, ..Default::default() }]);
    assert_eq!(premise_of(&under, a).fields, Some(vec!["visible".to_owned()]));
    let image = premise_of(&under, "image");
    assert_eq!((image.fields, image.positions), (Some(vec![]), Some(vec![])), "the image: a premise of existence alone");
    for (index, id, stored) in [("#1", b, 2), ("#3", background, 0)] {
        let under = doc.under("layer.setProps", &json!({ "layer": index, "visible": false })).unwrap().unwrap();
        assert_eq!(under.objects, vec![json!(id)], "{index}");
        assert_eq!((under.within[0].from, under.within[0].to), (stored, stored + 1), "{index}: the window in stored order");
    }
    assert!(doc.under("layer.setProps", &json!({ "layer": "#4" })).is_err());
    let command = doc.commands().into_iter().find(|command| command.id == "layer.setProps").unwrap();
    assert_eq!((command.index, command.index_reversed), (Some("#"), true));
}

#[test]
fn a_point_names_the_topmost_layer_with_a_pixel_there_and_is_told_by_covers() {
    let (doc, background, a, b) = image();
    let at = |x: i32, y: i32| doc.under("layer.setProps", &json!({ "at": [x, y], "opacity": 0.5 })).unwrap().unwrap();
    assert_eq!(at(50, 50).objects, vec![json!(a)]);
    assert_eq!(at(350, 100).objects, vec![json!(b)]);
    assert_eq!(at(200, 300).objects, vec![json!(background)]);
    assert!(at(50, 50).covers.is_some_and(|covers| covers.fields.contains(&"visible".to_owned())));
}

#[test]
fn covered_says_what_a_point_named_then() {
    let (mut doc, background, a, b) = image();
    // A was hidden by a colleague since: at basis, A (visible then) was under (50, 50).
    doc.execute("layer.setProps", json!({ "layer": a, "visible": false })).unwrap();
    let ask = |id: u64, before: Value| CoveredAsk { id: json!(id), before: before.as_object().cloned().unwrap_or_default(), ..Default::default() };
    let then = doc.covered("layer.setProps", &json!({ "at": [50, 50] }), &[ask(a, json!({ "visible": true }))]).unwrap().unwrap();
    assert_eq!(then.under, vec![json!(a)]);
    assert_eq!(then.covered[0].covered, Some(true));
    // B painted far from the point, its bounds then not holding it: B was not there.
    let then = doc.covered("layer.setProps", &json!({ "at": [200, 300] }), &[ask(b, json!({ "tiles": {}, "~bounds": [300, 50, 200, 200] }))]).unwrap().unwrap();
    assert_eq!((then.under.clone(), then.covered[0].covered), (vec![json!(background)], Some(false)));
    // Pixels changed where the point is: not known.
    let then = doc.covered("layer.setProps", &json!({ "at": [350, 100] }), &[ask(b, json!({ "tiles": {} }))]).unwrap().unwrap();
    assert_eq!(then.covered[0].covered, None);
    // A layer created since was not there.
    let made = CoveredAsk { id: json!(b), created: true, ..Default::default() };
    let then = doc.covered("layer.setProps", &json!({ "at": [350, 100] }), &[made]).unwrap().unwrap();
    assert_eq!((then.under, then.covered[0].covered), (vec![json!(background)], Some(false)));
}

#[test]
fn derived_bounds_are_each_layers_non_transparent_extent() {
    let (doc, background, a, b) = image();
    let values = doc.derived(&[json!(background), json!(a), json!(b), json!(999)], &["bounds".to_owned()]).unwrap();
    let bounds: Vec<Value> = values.iter().map(|value| value.values["bounds"].clone()).collect();
    assert_eq!(bounds, vec![json!([0, 0, 600, 400]), json!([10, 10, 100, 100]), json!([300, 50, 200, 200])]);
}

#[test]
fn a_name_names_one_layer_and_holds_that_it_still_does() {
    let (mut doc, _, a, _) = image();
    let under = doc.under("layer.setProps", &json!({ "layer": "A", "opacity": 0.5 })).unwrap().unwrap();
    assert_eq!(under.objects, vec![json!(a)]);
    assert_eq!(premise_of(&under, a).fields, Some(vec!["opacity".to_owned(), "name".to_owned()]));
    doc.execute("layer.new.layer", json!({ "name": "A" })).unwrap();
    let error = doc.under("layer.setProps", &json!({ "layer": "A", "opacity": 0.5 })).unwrap_err();
    assert!(error.contains("2 layers are named"), "{error}");
}

#[test]
fn review_a_region_loaded_from_a_layer_holds_its_source_pixels() {
    let (doc, _, a, b) = image();
    let under = doc.under("edit.fill", &json!({ "layer": a, "region": { "layer": b }, "color": "#000000" })).unwrap().unwrap();
    assert_eq!(premise_of(&under, b).fields, Some(vec!["kind".to_owned(), "tiles".to_owned(), "data".to_owned(), "text".to_owned()]));
}

#[test]
fn review_filters_hold_the_canvas_even_without_an_explicit_region() {
    let (doc, _, a, _) = image();
    let under = doc.under("filter.distort.twirl", &json!({ "layer": a, "angle": 90 })).unwrap().unwrap();
    assert!(premise_of(&under, "image").fields.unwrap().contains(&"width".to_owned()));
}

#[test]
fn review_editable_filters_hold_the_source_and_filter_stack() {
    let (mut doc, _, a, _) = image();
    let made = doc.execute("layer.objectLayers.convertToObjectLayer", json!({ "layer": a })).unwrap();
    let smart = made["layer"].as_u64().unwrap();
    let under = doc.under("filter.blur.gaussianBlur", &json!({ "layer": smart, "radius": 2 })).unwrap().unwrap();
    assert!(premise_of(&under, smart).fields.unwrap().contains(&"data".to_owned()));
}

#[test]
fn review_inherited_lock_inputs_belong_to_the_image_that_owns_the_ancestors() {
    let (mut doc, _, a, _) = image();
    let group = doc.execute("layer.new.group", json!({ "name": "Locked" })).unwrap()["layer"].clone();
    doc.execute("layer.setProps", json!({ "layer": group, "locks": { "transparency": true } })).unwrap();
    doc.execute("layer.moveTo", json!({ "layer": a, "target": group, "position": "into" })).unwrap();
    let values = doc.derived(&[json!("image")], &["inheritedLocks".to_owned()]).unwrap();
    assert!(values.iter().any(|v| v.id == "image" && !v.values["inheritedLocks"].as_array().unwrap().is_empty()));
    let under = doc.under("edit.fill", &json!({ "layer": a, "color": "#000000" })).unwrap().unwrap();
    assert!(premise_of(&under, "image").fields.unwrap().contains(&"~inheritedLocks".to_owned()));
}

#[test]
fn review_smart_fill_holds_its_surroundings_for_every_accepted_spelling() {
    let (doc, _, a, _) = image();
    for spelling in ["smart", "SMART"] {
        let under = doc.under("edit.fill", &json!({ "layer": a, "contents": spelling, "region": { "rect": [20, 20, 4, 4] } })).unwrap().unwrap();
        assert!(premise_of(&under, a).fields.unwrap().contains(&"tiles".to_owned()), "{spelling}: inpainting reads outside the filled area");
    }
}

#[test]
fn review_bucket_pattern_source_holds_the_document_pattern() {
    let (doc, _, a, _) = image();
    let under = doc.under("paint.bucket", &json!({ "layer": a, "source": "pattern", "pattern": "Checkerboard" })).unwrap().unwrap();
    // The pattern the name named at basis, told through the image's `~patternContents`.
    assert!(under.objects.contains(&json!("image")), "{under:?}");
    assert!(under.covers.as_ref().is_some_and(|covers| covers.fields.contains(&"~patternContents".to_owned())), "{under:?}");
}

#[test]
fn review_channel_targets_are_held_for_tools_outside_the_pixel_family() {
    let (mut doc, _, a, _) = image();
    doc.execute("select.rect", json!({ "x": 10, "y": 10, "width": 20, "height": 20 })).unwrap();
    doc.execute("select.saveSelection", json!({ "name": "Alpha" })).unwrap();
    let under = doc.under("paint.bucket", &json!({ "layer": a, "target": { "channel": 0 }, "x": 10, "y": 10 })).unwrap().unwrap();
    assert!(premise_of(&under, "image").fields.unwrap().contains(&"~channelContents".to_owned()));
}

#[test]
fn review_null_bounds_do_not_prove_a_deleted_fill_missed_the_point() {
    let (doc, _, _, _) = image();
    let then = doc
        .covered(
            "layer.setProps",
            &json!({ "at": [200, 300], "opacity": 0.5 }),
            &[CoveredAsk {
                id: json!(999),
                deleted: true,
                before: serde_json::from_value(json!({ "~bounds": null, "kind": "fill", "visible": true })).unwrap(),
                ..Default::default()
            }],
        )
        .unwrap()
        .unwrap();
    assert_eq!(then.covered[0].covered, None, "a fill covers the point although its derived bounds are null");
}

#[test]
fn review_command_object_roles_follow_the_implementation() {
    let (doc, _, _, _) = image();
    let commands = doc.commands();
    for (id, paths) in [
        ("image.applyImage", vec!["source.layer", "mask.layer", "sourceLayer", "maskLayer"]),
        ("image.calculations", vec!["source1.layer", "source2.layer", "mask.layer", "source1Layer", "source2Layer", "maskLayer"]),
    ] {
        let command = commands.iter().find(|c| c.id == id).unwrap();
        for path in paths {
            assert!(command.objects.contains(&path), "{id}: missing {path}");
        }
    }
    for id in ["cloneSource.select", "color.profileMismatch", "file.activate", "image.adjustments.hdrToning"] {
        let command = commands.iter().find(|c| c.id == id).unwrap();
        assert!(!command.reads.contains(&kition_kit::EditorState::Active), "{id} does not read the active layer");
        assert_eq!(command.targets, None, "{id} has no layer target");
    }
    let measure = commands.iter().find(|c| c.id == "image.measure").unwrap();
    assert!(measure.objects.contains(&"with"), "declaration must preserve the measurement's second layer");
    assert_eq!(commands.iter().find(|c| c.id == "layer.setProps").unwrap().name_field, Some("name"));
}

#[test]
fn review_an_unnarrowed_command_by_name_keeps_all_target_fields() {
    let (doc, _, a, _) = image();
    // A rename replaces the name alone (C8-photo-read), which the name premise holds too.
    let under = doc.under("layer.renameLayer", &json!({ "layer": "A", "name": "Renamed" })).unwrap().unwrap();
    assert_eq!(premise_of(&under, a).fields, Some(vec!["name".to_owned()]), "{under:?}");
    // A command that reads all of its layer, by name: every field.
    let under = doc.under("layer.rasterize.layer", &json!({ "layer": "A" })).unwrap().unwrap();
    assert!(under.premises.iter().find(|p| p.id == json!(a)).is_none_or(|p| p.fields.is_none()), "{under:?}");
}

#[test]
fn review_doc_state_inputs_have_member_premises_even_without_an_active_target() {
    let (doc, _, a, _) = image();
    for (command, params, field) in [
        ("edit.fill", json!({ "layer": a, "contents": "pattern", "pattern": "Bubbles" }), "~patternContents"),
        ("layer.newFillLayer.pattern", json!({ "pattern": "Bubbles" }), "~patternContents"),
        ("layerComp.apply", json!({ "comp": 1 }), "comps"),
        ("text.characterStyle.apply", json!({ "layer": a, "id": 1 }), "docState.text_styles"),
        ("text.paragraphStyle.set", json!({ "id": 1, "attrs": { "size": 12 } }), "docState.text_styles"),
    ] {
        let under = doc.under(command, &params).unwrap().unwrap_or_else(|| panic!("{command}: no premises"));
        // A stored pattern is held through the covers, the rest as fields of the image.
        let covered = under.covers.as_ref().is_some_and(|covers| covers.fields.contains(&field.to_owned()));
        assert!(covered || premise_of(&under, "image").fields.unwrap().contains(&field.to_owned()), "{command}: {under:?}");
    }
}

#[test]
fn review_stroke_footprint_uses_the_resolved_brush_size() {
    let (doc, _, a, _) = image();
    let under = doc.under("paint.stroke", &json!({ "layer": a, "points": [[50, 50, 1]], "brush": { "size": 800 } })).unwrap().unwrap();
    assert_eq!(under.content[0], kition_kit::ContentPremise { id: json!(a), field: "tiles".to_owned(), region: json!([-351, -351, 452, 452]) });
}

#[test]
fn review_pixel_filter_reads_follow_the_shared_implementation() {
    let (doc, _, _, _) = image();
    let mut missing = Vec::new();
    for command in doc.commands() {
        if kition_photo_engine::filters::params_for(&command.id, &json!({})).is_some()
            || matches!(command.id.as_str(), "filter.lensCorrection" | "filter.adaptiveWideAngle" | "filter.rawDevelop")
        {
            if !command.reads.contains(&kition_kit::EditorState::Active) {
                missing.push(format!("{} active", command.id));
            }
            if !command.reads.contains(&kition_kit::EditorState::Area) {
                missing.push(format!("{} area", command.id));
            }
        }
    }
    let measure = doc.commands().into_iter().find(|c| c.id == "image.measure").unwrap();
    if !measure.reads.is_empty() || !measure.places.is_empty() {
        missing.push("image.measure reads no editor state and takes no point".to_owned());
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn review_absolute_selection_and_symmetry_settings_do_not_read_the_active_layer() {
    let (doc, _, _, _) = image();
    for id in ["paint.setSymmetry", "paint.symmetryDisable", "layer.selectTop", "layer.selectBottom"] {
        let command = doc.commands().into_iter().find(|c| c.id == id).unwrap();
        assert!(!command.reads.contains(&kition_kit::EditorState::Active), "{id}");
    }
}

#[test]
fn review_implicit_channel_target_holds_channel_contents() {
    let (mut doc, _, a, _) = image();
    doc.execute("select.rect", json!({ "x": 0, "y": 0, "width": 10, "height": 10 })).unwrap();
    doc.execute("select.saveSelection", json!({ "name": "Alpha" })).unwrap();
    doc.execute("channel.target", json!({ "channel": 0 })).unwrap();
    let under = doc.under("image.adjustments.invert", &json!({ "layer": a })).unwrap().unwrap();
    assert!(premise_of(&under, "image").fields.unwrap().contains(&"~channelContents".to_owned()));
}

#[test]
fn review_every_region_layer_is_marked_beside_other_object_parameters() {
    let (doc, _, _, _) = image();
    let missed: Vec<String> =
        doc.commands().into_iter().filter(|c| c.reads.contains(&kition_kit::EditorState::Area) && !c.objects.contains(&"region.layer")).map(|c| c.id).collect();
    assert!(missed.is_empty(), "{}", missed.join("\n"));
}
