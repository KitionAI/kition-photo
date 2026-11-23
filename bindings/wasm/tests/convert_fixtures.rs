//! Temporary (schema-freeze, history separation): rewrites the `.kphoto` conformance fixtures as
//! content files (their replica's content snapshot and only the blobs its state references,
//! document and object ids kept) into the CI log, base64. Removed in the next commit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[test]
fn print_converted_fixtures() {
    use base64::Engine;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["check-empty-text.kphoto", "check-font-substituted.kphoto", "check-missing-object-source.kphoto"] {
        let bytes = std::fs::read(root.join("conformance").join(name)).unwrap();
        let snapshot = kition_photo_format::read_kdm_snapshot(&bytes).unwrap().unwrap();
        let converted = kition_photo_engine::kition_photo_bridge::replace_replica(&bytes, &snapshot).unwrap();
        println!("FIXTURE {name} {}", base64::engine::general_purpose::STANDARD.encode(&converted));
    }
    panic!("printed the fixtures");
}
