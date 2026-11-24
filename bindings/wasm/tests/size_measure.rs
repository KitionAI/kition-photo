//! TEMPORARY (schema-freeze round 4, item D2): prints what a large image's `.kphoto` holds, entry
//! by entry, into the CI log (a deliberate failure, so the output shows). Removed before landing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

fn report(label: &str, bytes: &[u8]) {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let mut groups: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for entry in &zip.entries {
        let data = zip.read(entry, 1 << 30).unwrap();
        let group = match entry.name.rsplit_once('/') {
            Some((dir, _)) => dir.to_owned(),
            None => entry.name.clone(),
        };
        let slot = groups.entry(group).or_default();
        slot.0 += 1;
        slot.1 += data.len();
    }
    println!("SIZE {label}: {} bytes; {groups:?}", bytes.len());
}

#[test]
fn print_sizes() {
    let mut doc = PhotoDocument::create("Large.kphoto", 4000, 3000).unwrap();
    doc.execute("filter.render.clouds", json!({ "seed": 7 })).unwrap();
    let large = doc.save_sealed(None).unwrap();
    report("large 4000x3000 clouds", &large);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["check-empty-text.kphoto", "check-font-substituted.kphoto", "check-missing-object-source.kphoto"] {
        let bytes = std::fs::read(root.join("conformance").join(name)).unwrap();
        report(name, &bytes);
    }
    panic!("printed the sizes");
}
