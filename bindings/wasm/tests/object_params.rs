//! Object parameters (task card C-photo): kition-sdk reads a whole number in a parameter other
//! than a command's targets as an object's id only where the command marks the parameter
//! (`KitCommand::objects`, `"x-khp-role": "object"`). A parameter that takes a layer by id and is
//! not marked silently loses its premise: an agent's write with `basis` would not be refused after
//! others changed the layer it names. This test walks every command's top-level parameters and
//! fails for one that takes an integer, a list of them or a JSON value and whose name or
//! description says it names a layer or an id, unless it is the command's targets, marked, or
//! listed below with why it names no layer of the image.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::Value;

/// `(command, parameter, why it names no layer of this image)`.
const NOT_LAYERS: &[(&str, &str, &str)] = &[
    ("layer.fill.solidColor.set", "document", "an open document's index, to reject a stale dialog"),
    ("file.export.layerCompsToFiles", "comps", "layer comp ids (not offered to agents: it writes files)"),
    ("file.export.artboardsToFiles", "artboards", "not offered to agents: it writes files"),
    ("file.export.artboardsToPdf", "artboards", "not offered to agents: it writes files"),
    ("file.export.colorLookupTables", "layers", "not offered to agents: it writes files"),
    ("layer.layerStyle.bevelEmboss", "texture", "a pattern, by id or name"),
    ("edit.checkSpelling", "suggestions", "a count of suggestions"),
    ("slice.set", "slice", "a web slice's id, not a layer"),
    ("slice.promote", "slice", "a web slice's id, not a layer"),
    ("slice.delete", "slice", "a web slice's id, not a layer"),
    ("slice.divide", "slice", "a web slice's id, not a layer"),
];

fn takes_id(node: &Value) -> bool {
    matches!(node.get("type").and_then(Value::as_str), Some("integer" | "json"))
        || node.get("items").is_some_and(takes_id)
        || node.get("oneOf").and_then(Value::as_array).is_some_and(|branches| branches.iter().any(takes_id))
}

fn reads_like_a_layer(name: &str, node: &Value) -> bool {
    let description = node.to_string().to_lowercase();
    name.to_lowercase().ends_with("layer")
        || name.to_lowercase().ends_with("layers")
        || description.contains("an id")
        || description.contains("layer id")
        || description.contains("layerid")
}

#[test]
fn every_parameter_that_takes_a_layer_id_is_marked_as_naming_objects() {
    let doc = PhotoDocument::create("art.psd", 64, 48).unwrap();
    let mut missed = Vec::new();
    let mut marked = 0;
    for command in doc.commands() {
        let Some(Value::Object(schema)) = command.schema.as_ref() else { continue };
        for (name, node) in schema {
            if Some(name.as_str()) == command.targets || !takes_id(node) || !reads_like_a_layer(name, node) {
                continue;
            }
            if command.objects.contains(&name.as_str()) {
                marked += 1;
            } else if !NOT_LAYERS.iter().any(|(id, param, _)| *id == command.id && param == name) {
                missed.push(format!("{} `{name}`: {node}", command.id));
            }
        }
    }
    assert!(missed.is_empty(), "parameters that name layers and are not marked (acts::OBJECTS):\n{}", missed.join("\n"));
    assert!(marked >= 20, "{marked} marked parameters");
}

/// Parameters that take the id of something the file keeps in the `Image`'s one `docState`
/// register (patterns, layer comps, character and paragraph styles): no KDM object with a tool id,
/// so nothing the premise guard could hold by that id. Consumers hold the relevant docState
/// member in under; pattern deletion changes only the session library, not the document.
const IN_DOC_STATE: &[(&str, &[&str])] = &[
    (
        "pattern",
        &[
            "brush.texturePattern",
            "edit.fill",
            "layer.layerStyle.patternOverlay",
            "layer.newFillLayer.pattern",
            "paint.bucket",
            "paint.patternStamp",
            "pattern.delete",
            "pattern.presets.apply",
            "pattern.presets.select",
            "pattern.rename",
        ],
    ),
    (
        "comp",
        &["layerComp.apply", "layerComp.delete", "layerComp.duplicate", "layerComp.rename", "layerComp.setComment", "layerComp.setOptions", "layerComp.update"],
    ),
    (
        "id",
        &[
            "type.characterStyle.delete",
            "type.characterStyle.duplicate",
            "type.characterStyle.redefine",
            "type.characterStyle.rename",
            "type.characterStyle.set",
            "type.paragraphStyle.delete",
            "type.paragraphStyle.duplicate",
            "type.paragraphStyle.redefine",
            "type.paragraphStyle.rename",
            "type.paragraphStyle.set",
        ],
    ),
];

/// kition-sdk's own check (`testing::check_id_params`, C8-fix-9): every integer parameter whose
/// name or description says id is the command's targets, marked in `objects`, or a place; the
/// allow-list is the parameters above that name no layer of the image and those in `docState`.
#[test]
fn the_sdk_finds_every_integer_id_parameter_marked() {
    let doc = PhotoDocument::create("art.psd", 64, 48).unwrap();
    let mut allow: Vec<(&str, &str)> = NOT_LAYERS.iter().map(|(command, param, _)| (*command, *param)).collect();
    for (param, commands) in IN_DOC_STATE {
        allow.extend(commands.iter().map(|command| (*command, *param)));
    }
    if let Err(problems) = kition_kit::testing::check_id_params(&doc.commands(), &allow) {
        panic!("{} unmarked id parameters:\n{}", problems.len(), problems.join("\n"));
    }
}
