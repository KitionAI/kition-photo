//! TEMPORARY (schema-freeze round 4, item D4): rewrites the `.kphoto` conformance fixtures in the
//! new layout (one content-addressed copy of every tile, no view, no previews) from their replica
//! (`kdm.bin` and the old `blobs/kdm/` entries), document and object ids kept, into the CI log as
//! base64, then fails so the log shows. Removed in the next commit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Read;
use std::sync::Arc;

use base64::Engine;

#[test]
fn print_converted_fixtures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["check-empty-text.kphoto", "check-font-substituted.kphoto", "check-missing-object-source.kphoto"] {
        let old = std::fs::read(root.join("conformance").join(name)).unwrap();
        let zip = kition_photo_format::zip::ZipReader::new(&old).unwrap();
        let snapshot = zip.read_by_name("kdm.bin", usize::MAX).unwrap();
        let mut blobs = kition_photo_engine::MemoryBlobs::default();
        for entry in &zip.entries {
            let Some(hash) = entry.name.strip_prefix("blobs/kdm/").and_then(|rest| rest.strip_suffix(".zst")) else { continue };
            let stored = zip.read(entry, usize::MAX).unwrap();
            let mut data = Vec::new();
            ruzstd::decoding::StreamingDecoder::new(&stored[..]).unwrap().read_to_end(&mut data).unwrap();
            assert_eq!(kition_photo_format::content_hash(&data), hash);
            blobs.0.insert(hash.to_owned(), Arc::from(data));
        }
        let mut session = kition_photo_engine::Session::new();
        let index = session.open_snapshot(&snapshot, blobs, name).unwrap();
        assert!(session.set_active(index));
        session.active_mut().unwrap().seal().unwrap();
        let converted = session.active().unwrap().save_kphoto().unwrap();
        let same = kition_photo_format::read_kdm_snapshot(&converted).unwrap().unwrap() == snapshot;
        println!("SAME_SNAPSHOT {name} {same} doc {:?}", session.doc_id());
        println!("FIXTURE {name} {}", base64::engine::general_purpose::STANDARD.encode(&converted));
    }
    panic!("printed the fixtures");
}
