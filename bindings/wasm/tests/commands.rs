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
