//! Name-taking parameters, checked against the resolvers rather than parameter spelling.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::Value;

fn name_hint(node: &Value) -> bool {
    let description = node.get("description").and_then(Value::as_str).unwrap_or_default().to_lowercase();
    description == "name"
        || ["id|name", "index|name", "id or name", "layer name", "whole name", "saved path name"].iter().any(|hint| description.contains(hint))
        || node.get("oneOf").and_then(Value::as_array).is_some_and(|branches| branches.iter().any(name_hint))
        || node.get("items").is_some_and(name_hint)
}

fn scan(properties: &Value, prefix: &str, out: &mut Vec<String>) {
    for (key, node) in properties.as_object().into_iter().flatten() {
        let path = format!("{prefix}{key}");
        if name_hint(node) {
            out.push(path.clone());
        }
        if let Some(children) = node.get("properties") {
            scan(children, &format!("{path}."), out);
        }
    }
}

#[test]
fn every_resolved_name_parameter_is_declared() {
    let doc = PhotoDocument::create("art.psd", 32, 24).unwrap();
    let mut missed = Vec::new();
    for command in doc.commands() {
        let mut names = Vec::new();
        // Binding resolvers: place::layer_value, render::layer_named, context::region.
        if command.targets == Some("layer") {
            names.push("layer");
        }
        if command.id == "image.measure" {
            names.extend(["layer", "with"]);
        }
        if command.id == "file.render" {
            names.extend(["target.layer", "target.layers", "target.objectId"]);
        }
        if command.reads.contains(&kition_kit::EditorState::Area) {
            names.push("region.channel");
        }
        if command.id.starts_with("layer.newAdjustmentLayer.") {
            names.push("path");
        }
        // Engine resolvers: channel_cmds::{parse_ref,source_spec}, comps_cmds::comp_param,
        // vector_cmds::{resolve_path,path_mut,mask_new_layer}, pattern_cmds::resolve_param.
        match command.id.as_str() {
            "channel.duplicate"
            | "channel.delete"
            | "channel.rename"
            | "channel.move"
            | "channel.target"
            | "channel.setVisible"
            | "channel.options"
            | "channel.mergeSpot"
            | "select.saveSelection"
            | "select.loadSelection"
            | "edit.smartFill" => names.push("channel"),
            "edit.smartScale" => names.push("protect"),
            "image.applyImage" => names.extend(["source.channel", "mask.channel", "sourceChannel", "maskChannel"]),
            "image.calculations" => names.extend(["source1.channel", "source2.channel", "mask.channel", "source1Channel", "source2Channel", "maskChannel"]),
            "layerComp.apply"
            | "layerComp.delete"
            | "layerComp.duplicate"
            | "layerComp.rename"
            | "layerComp.setComment"
            | "layerComp.setOptions"
            | "layerComp.update" => names.push("comp"),
            "path.info"
            | "path.set"
            | "path.delete"
            | "path.transform"
            | "path.rename"
            | "path.toSelection"
            | "path.fill"
            | "path.stroke"
            | "path.clippingPath.set"
            | "path.moveAnchors"
            | "path.moveHandle"
            | "path.bendSegment"
            | "path.convertPoint"
            | "layer.vectorMask.add"
            | "layer.vectorMask.fromPath"
            | "layer.vectorMask.currentPath" => names.push("name"),
            "layer.newFillLayer.solidColor" | "layer.newFillLayer.gradient" | "edit.defineCustomShape" | "filter.render.flame" | "shape.presets.new" => {
                names.push("path")
            }
            "paint.symmetryFromPath" => names.push("name"),
            "file.export.pathsToIllustrator" => names.push("paths"),
            _ => {}
        }
        let mut hinted = Vec::new();
        if let Some(schema) = &command.schema {
            scan(schema, "", &mut hinted);
        }
        for path in hinted {
            if !command.name_params.iter().any(|(param, _)| *param == path) {
                missed.push(format!("{} `{path}` advertises a name but declares none", command.id));
            }
        }
        match command.id.as_str() {
            "brush.texturePattern"
            | "edit.fill"
            | "layer.layerStyle.patternOverlay"
            | "paint.bucket"
            | "paint.patternStamp"
            | "pattern.delete"
            | "pattern.rename"
            | "pattern.presets.apply"
            | "pattern.presets.select" => names.push("pattern"),
            "layer.newFillLayer.pattern" => names.extend(["pattern", "path"]),
            "layer.layerStyle.bevelEmboss" => names.push("texture"),
            "layer.layerStyle.replace" => {
                names.extend(["effects[].params.pattern", "effects[].params.texture", "effects[].fx.PatternOverlay.name", "effects[].fx.PatternOverlay.id"])
            }
            "shape.create" | "shape.edit" => names.extend(["fill.pattern", "stroke.fill.pattern"]),
            "pattern.export" => names.push("patterns"),
            "style.presets.apply" => names.extend(["preset", "group"]),
            "style.presets.edit" | "pattern.presets.edit" => names.extend(["preset", "group", "to"]),
            _ => {}
        }
        for name in names {
            if !command.name_params.iter().any(|(param, field)| *param == name && *field == "name") {
                missed.push(format!("{}: {name}", command.id));
            }
            assert!(
                command.name_params.iter().all(|(param, _)| command.object_types.iter().any(|(typed, _)| typed == param)),
                "unscoped name in {}",
                command.id
            );
        }
        if command.id == "style.presets.edit" {
            assert!(!command.name_params.iter().any(|(param, _)| *param == "name"), "a new name does not resolve an existing style");
        }
    }
    assert!(missed.is_empty(), "undeclared name resolvers:\n{}", missed.join("\n"));
}
