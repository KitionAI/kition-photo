//! The command catalogue reads right: a layer style command that does
//! not set one effect is not described as "Sets the … layer effect", and the unknown-parameter
//! error names only parameters, never an enum's values or defaults.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

/// The commands that set one layer effect (`layer.layerStyle.<kind>`).
const EFFECTS: &[&str] =
    &["dropShadow", "innerShadow", "outerGlow", "innerGlow", "stroke", "colorOverlay", "gradientOverlay", "patternOverlay", "bevelEmboss", "satin"];

#[test]
fn only_effect_commands_are_described_as_setting_an_effect() {
    let image = PhotoDocument::create("styles.psd", 8, 8).unwrap();
    let mut wrong = Vec::new();
    for command in image.commands() {
        let Some(kind) = command.id.strip_prefix("layer.layerStyle.") else { continue };
        let says_effect = command.description.contains("layer effect on the active layer");
        if says_effect != EFFECTS.contains(&kind) {
            wrong.push(format!("{}: {}", command.id, command.description));
        }
        // The description names the command's own subject, not "Sets the <Title> layer effect".
        assert!(
            !command.description.contains(&format!("Sets the {} layer effect", command.title.trim_end_matches('…'))) || EFFECTS.contains(&kind),
            "{}",
            command.id
        );
    }
    assert!(wrong.is_empty(), "described wrongly:\n{}", wrong.join("\n"));
}

#[test]
fn the_unknown_parameter_error_lists_parameters_only() {
    let mut image = PhotoDocument::create("type.psd", 100, 100).unwrap();
    let error = image.execute("text.add", json!({ "x": 1, "y": 20, "text": "a", "bogusKey": 1 })).unwrap_err();
    assert!(error.contains("`bogusKey`") && error.contains("`orientation`") && error.contains("`antialias`"), "{error}");
    for value in ["horizontal", "vertical", "smooth", "sharp", "left", "justify"] {
        assert!(!error.contains(&format!("`{value}`")), "lists the value {value} as a parameter: {error}");
    }
}

#[test]
fn no_command_takes_a_value_of_its_own_notation_as_a_parameter() {
    // An enum default ("…"="value",) or alternative ("a"|"b",) is a value, never a key.
    let mut image = PhotoDocument::create("all.psd", 8, 8).unwrap();
    let mut wrong = Vec::new();
    for command in image.commands() {
        let notation = command.params.clone().unwrap_or_default();
        let schema = command.schema.clone().unwrap_or_default();
        let error = match image.execute(&command.id, json!({ "zzNotAParameter": 1 })) {
            Err(error) if error.contains("has no parameter `zzNotAParameter`") => error,
            _ => continue,
        };
        for (at, _) in notation.match_indices("=\"") {
            let rest = &notation[at + 2..];
            let Some(end) = rest.find('"') else { continue };
            let value = &rest[..end];
            if !value.is_empty() && schema.get(value).is_none() && error.contains(&format!("`{value}`,")) {
                wrong.push(format!("{}: `{value}`", command.id));
            }
        }
    }
    assert!(wrong.is_empty(), "values listed as parameters:\n{}", wrong.join("\n"));
}
