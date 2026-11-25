//! The parameters a command takes, and refusing one it does not (platform rule E7a, kit
//! requirement AX15): an agent that misspells a parameter is told, not left with the command
//! running without it.
//!
//! A command takes the keys its schema names and the keys its parameter notation names (the
//! notation is the engine's own description; the kit-schema converter does not carry every key
//! of a long one into the schema), plus `coalesce`, which the session reads for every command.

use std::collections::{BTreeSet, HashMap};

use kition_kit::KitCommand;
use serde_json::{Map, Value, json};

/// Commands whose top level takes keys no schema can list, with why.
pub const OPEN: &[(&str, &str)] = &[
    ("layer.setAdjustment", "takes the parameters of the adjustment layer's own kind (Levels, Curves, …), which differ per kind"),
    ("tool.setBrush", "takes any BrushSettings field (camelCase, deep-merged), as the brush settings panel writes them"),
];

/// A key a command reads: its name, its type, and what it does.
type Read = (&'static str, &'static str, &'static str);

/// Keys a command reads that its notation does not name: each was found in the command's code.
const READS: &[(&str, &[Read])] = &[
    (
        "edit.deleteAndFillSelection",
        &[
            ("output", "string", "Where the fill goes: \"current\" (default) or a new layer."),
            ("sampling", "string", "The sampling area; \"auto\" by default."),
        ],
    ),
    ("edit.transform.splitWarpCrosswise", &[("style", "string", "The warp style to split.")]),
    ("edit.transform.warpGrid", &[("grid", "json", "The warp grid size or its points.")]),
    ("file.export.renderVideo", &[("format", "string", "The frames' image format; \"png\" by default.")]),
    ("file.printOneCopy", &[("dryRun", "boolean", "Lay the page out without printing.")]),
    ("filter.other.offset", &[("layer", "integer", "The layer to offset; the active layer by default.")]),
    ("image.analysis.recordMeasurements", &[("dateTime", "string", "The measurement's time (ISO 8601); now by default.")]),
    ("layer.new.artboard", &[("x", "integer", "The artboard's left edge, in pixels.")]),
    ("text.add", &[("font", "string", "The font family."), ("kerning", "json", "\"metrics\", \"optical\" or a number (1/1000 em).")]),
];

/// The layer style commands that set one effect on a layer (`layer`, the active layer by default).
const EFFECTS: &[&str] = &[
    "layer.layerStyle.innerShadow",
    "layer.layerStyle.outerGlow",
    "layer.layerStyle.innerGlow",
    "layer.layerStyle.stroke",
    "layer.layerStyle.colorOverlay",
    "layer.layerStyle.gradientOverlay",
    "layer.layerStyle.patternOverlay",
    "layer.layerStyle.bevelEmboss",
    "layer.layerStyle.satin",
];

fn typed(kind: &str, description: &str) -> Value {
    if kind == "json" { json!({ "type": "json", "description": description }) } else { json!({ "type": kind, "description": description }) }
}

/// Adds to every schema the keys the session and the command's code read beyond its notation.
pub fn declare(commands: &mut [KitCommand]) {
    // A new type layer takes every character and paragraph key Type › Set Style takes
    // (`apply_char_props`, `apply_para_props`): the Type tool's options go in with the text.
    let styles: Vec<(String, Value)> = commands
        .iter()
        .find(|command| command.id == "text.setStyle")
        .and_then(|command| command.schema.as_ref()?.as_object().cloned())
        .map(|schema| schema.into_iter().filter(|(key, _)| !matches!(key.as_str(), "layer" | "range")).collect())
        .unwrap_or_default();
    for command in commands {
        let id = command.id.clone();
        let Some(Value::Object(schema)) = command.schema.as_mut() else { continue };
        schema
            .entry("coalesce")
            .or_insert_with(|| typed("string", "Merges consecutive edits with the same key into one undo step, as a slider drag or a typing session is one."));
        if kition_photo_engine::takes_target(&id) {
            schema.entry("target").or_insert_with(|| {
                json!({
                    "description": "Where the pixels go: \"pixels\" (the layer, by default), \"mask\", \"quickMask\", or {\"channel\": i}.",
                    "oneOf": [{ "type": "string" }, { "type": "object", "additionalProperties": true }],
                })
            });
        }
        if id == "text.add" {
            for (key, spec) in &styles {
                schema.entry(key.clone()).or_insert_with(|| spec.clone());
            }
        }
        if EFFECTS.contains(&id.as_str()) {
            schema.entry("layer").or_insert_with(|| typed("integer", "The layer; the active layer by default."));
        }
        if let Some((_, reads)) = READS.iter().find(|(command, _)| *command == id) {
            for (key, kind, description) in *reads {
                schema.entry(*key).or_insert_with(|| typed(kind, description));
            }
        }
        numeric_limits(&id, schema);
        if matches!(id.as_str(), "image.imageSize" | "image.canvasSize") {
            for key in ["width", "height"] {
                if let Some(node) = schema.get_mut(key).and_then(Value::as_object_mut) {
                    node.insert(
                        "description".to_owned(),
                        json!("Pixels; the resulting canvas side is rounded and clamped to 1..300000. With relative canvas sizing this parameter is a delta."),
                    );
                }
            }
            if id == "image.imageSize"
                && let Some(node) = schema.get_mut("resolution").and_then(Value::as_object_mut)
            {
                node.insert("description".to_owned(), json!("Pixels per inch, clamped to 1..10000."));
            }
        }
    }
}

fn numeric_limits(id: &str, schema: &mut Map<String, Value>) {
    let mut bound = |key: &str, minimum: f64, maximum: Option<f64>| {
        let Some(node) = schema.get_mut(key).and_then(Value::as_object_mut) else { return };
        if !matches!(node.get("type").and_then(Value::as_str), Some("number" | "integer")) {
            node.insert("type".to_owned(), json!("number"));
        }
        node.insert("minimum".to_owned(), json!(minimum));
        if let Some(maximum) = maximum {
            node.insert("maximum".to_owned(), json!(maximum));
        }
    };
    match id {
        "file.new" => {
            bound("width", 1.0, Some(300_000.0));
            bound("height", 1.0, Some(300_000.0));
            bound("resolution", 1.0, Some(30_000.0));
        }
        "image.imageSize" => {
            bound("width", 1.0, Some(300_000.0));
            bound("height", 1.0, Some(300_000.0));
            bound("resolution", 1.0, Some(10_000.0));
        }
        "layer.vectorMask.edit" | "layer.layerMask.density" => {
            bound("density", 0.0, Some(100.0));
            bound("feather", 0.0, None);
        }
        "layer.layerMask.feather" => bound("feather", 0.0, None),
        _ => {}
    }
    if id.starts_with("layer.layerStyle.") {
        for key in ["opacity", "spread", "choke", "noise"] {
            bound(key, 0.0, Some(100.0));
        }
        for key in ["range", "contourRange"] {
            bound(key, 1.0, Some(100.0));
        }
        for key in ["scale", "textureScale"] {
            bound(key, 1.0, Some(1000.0));
        }
        bound("textureDepth", -1000.0, Some(1000.0));
    }
}

/// The top-level keys a parameter notation names: `"key":`, `"key",`, `"key")`, and
/// `key:` after `{` or `,`. Alternatives and defaults of a value (`"left"|"right"`,
/// `"mode":"a|b"="a",`) and the keys of a value's own object (`{"channel":i}`) are not.
fn notation_keys(notation: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    let bytes = notation.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if bytes[i] == b'"' {
            // A quoted word right after `=`, `|` or `:` is a value (a default, an alternative), not
            // a key; ` | ` with spaces separates alternative forms of the parameters.
            let value = i > 0 && matches!(bytes[i - 1], b'=' | b'|' | b':');
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && word(bytes[end]) {
                end += 1;
            }
            if end > start && end < bytes.len() && bytes[end] == b'"' && !bytes[start].is_ascii_digit() {
                let mut next = end + 1;
                while next < bytes.len() && bytes[next] == b' ' {
                    next += 1;
                }
                if !value && depth <= 1 && next < bytes.len() && matches!(bytes[next], b':' | b',' | b')') {
                    keys.insert(notation[start..end].to_owned());
                }
                i = end + 1;
                continue;
            }
        } else if matches!(bytes[i], b'{' | b',') {
            let mut start = i + 1;
            while start < bytes.len() && bytes[start] == b' ' {
                start += 1;
            }
            let mut end = start;
            while end < bytes.len() && word(bytes[end]) {
                end += 1;
            }
            let mut next = end;
            if next < bytes.len() && bytes[next] == b'?' {
                next += 1;
            }
            if depth <= 1 && end > start && next < bytes.len() && bytes[next] == b':' && !bytes[start].is_ascii_digit() {
                keys.insert(notation[start..end].to_owned());
            }
        }
        i += 1;
    }
    keys
}

/// Each command's parameters, as the object [`kition_kit::schema::refuse_unknown`] checks.
pub fn taken(commands: &[KitCommand]) -> HashMap<String, Value> {
    commands
        .iter()
        .filter(|command| !OPEN.iter().any(|(id, _)| *id == command.id))
        .filter_map(|command| {
            let Some(Value::Object(schema)) = &command.schema else { return None };
            let mut names: Map<String, Value> = schema.clone();
            for key in notation_keys(command.params.as_deref().unwrap_or_default()) {
                names.entry(key).or_insert(Value::Null);
            }
            Some((command.id.clone(), Value::Object(names)))
        })
        .collect()
}

/// Refuses a key `command` does not take, naming it, the key it resembles, and the ones it takes.
pub fn check(command: &str, params: &Value) -> Result<(), String> {
    static TAKEN: std::sync::OnceLock<HashMap<String, Value>> = std::sync::OnceLock::new();
    let taken = TAKEN.get_or_init(|| {
        use kition_kit::KitDocument;
        crate::PhotoDocument::create("Untitled.psd", 1, 1).map(|document| taken(&document.commands())).unwrap_or_default()
    });
    match taken.get(command) {
        Some(names) => kition_kit::schema::refuse_unknown(command, names, params),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::notation_keys;

    #[test]
    fn a_notation_names_its_keys_and_not_its_alternatives() {
        let keys = notation_keys(
            r#"{"rect":[x,y,w,h] | "x","y", plus ("name","kind":"image|noImage"="image","alt")?, "target":"pixels"|{"channel":i}|"quickMask", "mode":"a|b"="b", "side": "left"|"right", size?: px}"#,
        );
        let keys: Vec<_> = keys.iter().map(String::as_str).collect();
        assert_eq!(keys, ["alt", "kind", "mode", "name", "rect", "side", "size", "target", "x", "y"]);
    }
}
