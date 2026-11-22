//! Every command says what it does, and read commands declare what they answer
//! (`kition_kit::testing::check_commands`, kition-sdk W1-46 P2-1).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

fn sample() -> PhotoDocument {
    PhotoDocument::open("basic.psd", include_bytes!("../../../conformance/basic.psd")).unwrap()
}

#[test]
fn every_command_says_what_it_does() {
    if let Err(problems) = kition_kit::testing::check_commands(&sample().commands(), kition_photo_kit::described::NO_OUTPUT) {
        panic!("{}", problems.join("\n"));
    }
}

/// regressions/issue-unreadable-param-notes: a notation entry with no key the parser reads
/// (`"width"|"height"|"percent"?`, `"replace"; changeAll: "word"`, a default with commas in it)
/// gives the schema no parameter. `check_commands` does not see it here, since every schema also
/// takes `coalesce` and so differs from the notation's, so each notation is read on its own.
#[test]
fn every_notation_entry_names_a_key() {
    let problems: Vec<String> = sample()
        .commands()
        .iter()
        .filter_map(|command| {
            let converted = kition_kit::schema::from_kit_params(command.params.as_deref()?).ok()?;
            (!converted.unreadable.is_empty()).then(|| format!("{}: {:?}", command.id, converted.unreadable))
        })
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The keys those entries meant are parameters of their commands.
#[test]
fn keys_of_once_unreadable_entries_are_parameters() {
    let commands = sample().commands();
    for (id, keys) in [
        ("select.transformSelection", &["style", "bend", "hDistort", "vDistort", "vertical", "mesh", "grid", "warp"][..]),
        ("file.export.dataSetsAsFiles", &["dataSets", "naming"]),
        ("brush.presets.move", &["folder", "before", "index"]),
        ("file.export.saveForWebLegacy", &["width", "height", "percent", "resample"]),
        ("edit.checkSpelling", &["start", "end", "word", "replace"]),
        ("slice.new", &["rect", "x", "y", "width", "height", "name", "kind", "horizontalAlign", "verticalAlign", "outsets"]),
        ("swatches.export", &["format", "path"]),
        ("image.applyImage", &["sourceDocument", "sourceLayer", "sourceChannel", "sourceInvert", "maskDocument", "maskLayer", "maskChannel", "maskInvert"]),
        ("type.setStyle", &["align", "firstLineIndent"]),
    ] {
        let command = commands.iter().find(|command| command.id == id).unwrap_or_else(|| panic!("no command {id}"));
        let schema = command.schema.as_ref().and_then(|schema| schema.as_object()).unwrap_or_else(|| panic!("{id}: no schema"));
        for key in keys {
            assert!(schema.contains_key(*key), "{id}: the schema has no `{key}`");
        }
    }
}

#[test]
fn every_schema_is_valid() {
    for command in sample().commands() {
        if let Some(schema) = &command.schema {
            kition_kit::schema::validate(schema).unwrap_or_else(|error| panic!("{}: {error}", command.id));
        }
        if let Some(schema) = &command.output_schema {
            kition_kit::schema::validate_output(schema).unwrap_or_else(|error| panic!("{}: {error}", command.id));
        }
    }
}

/// Read commands answer what they declare on `basic.psd`; ones whose precondition it lacks (a
/// selection, a type or shape layer, a path) are left out.
#[test]
fn read_commands_answer_what_they_declare() {
    let mut checked = 0;
    for command in sample().commands() {
        let Some(output) = command.output_schema.clone() else { continue };
        let Ok(answer) = sample().execute(&command.id, json!({})) else { continue };
        let schema = json!({ "answer": output });
        let answer = serde_json::Map::from_iter([("answer".to_owned(), answer)]);
        if let Err(error) = kition_kit::schema::check_params(&schema, &answer) {
            panic!("{}: {error}", command.id);
        }
        checked += 1;
    }
    assert!(checked >= 60, "only {checked} read commands could be checked");
}
