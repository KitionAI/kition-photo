//! Temporary (schema-freeze): writes the `.kphoto` conformance fixtures with the current replica
//! schema into the CI log, base64, so they can be committed. Removed in the next commit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

#[path = "../examples/checks.rs"]
mod checks;

#[test]
fn print_kphoto_fixtures() {
    use base64::Engine;
    for (name, session) in checks::fixtures().unwrap() {
        if !name.ends_with(".kphoto") {
            continue;
        }
        let bytes = session.active().unwrap().save_kphoto().unwrap();
        println!("FIXTURE {name} {}", base64::engine::general_purpose::STANDARD.encode(&bytes));
    }
    panic!("printed the fixtures");
}
