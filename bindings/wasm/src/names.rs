//! Name resolvers and their model type scopes. An empty scope means the resource lives in
//! Image.docState or the session library, not in an addressable model object. under() holds
//! the stored resource and its immutable identity when the format can prove it.

use std::collections::HashMap;
use std::sync::OnceLock;

use kition_kit::KitCommand;

type Typed = (&'static str, &'static [&'static str]);
#[derive(Default)]
struct Declaration {
    names: Vec<(&'static str, &'static str)>,
    types: Vec<Typed>,
}

fn resolved(id: &str) -> Vec<Typed> {
    let mut names = Vec::new();
    if crate::acts::reads_of(id).active || id == "layer.select" {
        names.push(("layer", &["Layer"][..]));
    }
    if crate::acts::reads_of(id).region {
        names.push(("region.channel", &["Channel"][..]));
    }
    let stored: &'static [&'static str] = &[];
    if id.starts_with("layer.newAdjustmentLayer.") {
        names.push(("path", stored));
    }
    match id {
        "image.measure" => names.extend([("layer", &["Layer"][..]), ("with", &["Layer"][..])]),
        "file.render" => names.extend([("target.layer", &["Layer"][..]), ("target.layers", &["Layer"][..]), ("target.objectId", &["Layer"][..])]),
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
        | "edit.smartFill" => names.push(("channel", &["Channel"][..])),
        "edit.smartScale" => names.push(("protect", &["Channel"][..])),
        "image.applyImage" => names.extend(["source.channel", "mask.channel", "sourceChannel", "maskChannel"].map(|p| (p, &["Channel"][..]))),
        "image.calculations" => names
            .extend(["source1.channel", "source2.channel", "mask.channel", "source1Channel", "source2Channel", "maskChannel"].map(|p| (p, &["Channel"][..]))),
        "layerComp.apply"
        | "layerComp.delete"
        | "layerComp.duplicate"
        | "layerComp.rename"
        | "layerComp.setComment"
        | "layerComp.setOptions"
        | "layerComp.update" => names.push(("comp", stored)),
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
        | "layer.vectorMask.currentPath" => names.push(("name", stored)),
        "layer.newFillLayer.solidColor" | "layer.newFillLayer.gradient" | "edit.defineCustomShape" | "filter.render.flame" | "shape.presets.new" => {
            names.push(("path", stored))
        }
        "paint.symmetryFromPath" => names.push(("name", stored)),
        "file.export.pathsToIllustrator" => names.push(("paths", stored)),
        _ => {}
    }
    match id {
        "brush.texturePattern"
        | "edit.fill"
        | "layer.layerStyle.patternOverlay"
        | "paint.bucket"
        | "paint.patternStamp"
        | "pattern.delete"
        | "pattern.rename"
        | "pattern.presets.apply"
        | "pattern.presets.select" => names.push(("pattern", stored)),
        "layer.newFillLayer.pattern" => names.extend([("pattern", stored), ("path", stored)]),
        "layer.layerStyle.bevelEmboss" => names.push(("texture", stored)),
        "layer.layerStyle.replace" => names.extend([
            ("effects[].params.pattern", stored),
            ("effects[].params.texture", stored),
            ("effects[].fx.PatternOverlay.name", stored),
            ("effects[].fx.PatternOverlay.id", stored),
        ]),
        "shape.create" | "shape.edit" => names.extend([("fill.pattern", stored), ("stroke.fill.pattern", stored)]),
        "pattern.export" => names.push(("patterns", stored)),
        "style.presets.apply" => names.extend([("preset", stored), ("group", stored)]),
        "style.presets.edit" | "pattern.presets.edit" => names.extend([("preset", stored), ("group", stored), ("to", stored)]),
        _ => {}
    }
    names
}

pub fn declare(commands: &mut [KitCommand]) {
    static ALL: OnceLock<HashMap<String, Declaration>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        kition_photo_engine::command_specs()
            .iter()
            .map(|s| s.id)
            .chain(["file.render", "image.measure"])
            .map(|id| {
                let types = resolved(id);
                (id.to_owned(), Declaration { names: types.iter().map(|(p, _)| (*p, "name")).collect(), types })
            })
            .collect()
    });
    for command in commands {
        if let Some(declared) = all.get(&command.id) {
            command.name_params = &declared.names;
            if !declared.types.is_empty() {
                command.object_types = &declared.types;
            }
            if let Some(schema) = command.schema.as_mut().and_then(serde_json::Value::as_object_mut) {
                for (path, _) in &declared.names {
                    property(schema, path);
                }
            }
        }
    }
}

// Preserve the parameter's accepted JSON kinds while exposing nested name annotations.
fn property(properties: &mut serde_json::Map<String, serde_json::Value>, path: &str) {
    use serde_json::{Value, json};
    let (segment, rest) = path.split_once('.').map_or((path, None), |(key, rest)| (key, Some(rest)));
    let key = segment.trim_end_matches("[]");
    let node = properties.entry(key).or_insert_with(|| json!({"type":"json"}));
    let Some(mut node) = node.as_object_mut() else { return };
    if segment.ends_with("[]") {
        let items = node.entry("items").or_insert_with(|| json!({"type":"json"}));
        let Some(items) = items.as_object_mut() else { return };
        node = items;
    }
    if let Some(rest) = rest {
        let children = node.entry("properties").or_insert_with(|| json!({}));
        if let Value::Object(children) = children {
            property(children, rest);
        }
    }
}

/// Stored premises for name resolvers whose values have no independent model identity.
pub fn stored_fields(id: &str, params: &serde_json::Value) -> Vec<&'static str> {
    let mut fields = Vec::new();
    for (param, types) in resolved(id) {
        let mut value = Some(params);
        for key in param.split('.') {
            value = value.and_then(|v| v.get(key.trim_end_matches("[]")));
            if key.ends_with("[]") {
                break;
            }
        }
        if value.is_none_or(serde_json::Value::is_null) {
            continue;
        }
        if types == ["Channel"] {
            fields.extend(["channels", "~channelContents"]);
        } else if types.is_empty() {
            let field = match param {
                "comp" => "docState.layer_comps",
                "path" | "paths" | "name"
                    if id.starts_with("path.")
                        || id.starts_with("layer.vectorMask.")
                        || id == "paint.symmetryFromPath"
                        || id == "file.export.pathsToIllustrator"
                        || param == "path" =>
                {
                    fields.push("docState.work_path");
                    "docState.paths"
                }
                _ if id.starts_with("style.") => continue,
                _ => "docState.patterns",
            };
            fields.push(field);
        }
    }
    fields.sort_unstable();
    fields.dedup();
    fields
}
