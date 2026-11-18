//! `.kphoto`'s KDM replica payload (Kition phase 3a, Task 4): `kdm.bin` and `blobs/kdm/<sha256>`
//! round-trip, a bundle without one loads as before, and an incremental save never rewrites a
//! KDM blob whose bytes are unchanged.

mod common;
use std::sync::Arc;

use common::*;
use kition_photo_color::{ColorMode, SampleType};
use kition_photo_format::*;

fn kdm_save(snapshot: &[u8], blobs: &[(&str, &[u8])]) -> KdmSave {
    KdmSave { snapshot: snapshot.to_vec(), blobs: blobs.iter().map(|(h, b)| ((*h).to_owned(), Arc::from(*b))).collect() }
}

/// A plausible-looking 64-hex-char SHA-256 key, distinct per `seed`.
fn hash(seed: u8) -> String {
    (0..64).map(|i| char::from_digit(u32::from((seed.wrapping_add(i)) % 16), 16).unwrap_or('0')).collect()
}

#[test]
fn a_bundle_without_a_replica_has_no_kdm() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    assert!(read_kdm(&bytes).unwrap().is_none());
    // It still loads as a plain document, exactly as before this feature existed.
    assert_eq!(load_from_bytes(&bytes).unwrap(), doc);
}

#[test]
fn a_bundle_with_a_replica_round_trips_the_snapshot_and_its_blobs() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let (h1, h2) = (hash(1), hash(2));
    let opts = SaveOptions { kdm: Some(kdm_save(b"a fake replica snapshot", &[(&h1, b"first blob"), (&h2, b"second blob")])), ..Default::default() };
    let bytes = save_to_bytes(&doc, &opts).unwrap();
    // The document itself is unaffected: a reader that knows nothing about KDM still loads it.
    assert_eq!(load_from_bytes(&bytes).unwrap(), doc);

    let bundle = read_kdm(&bytes).unwrap().unwrap();
    assert_eq!(bundle.snapshot, b"a fake replica snapshot");
    assert_eq!(bundle.blobs.len(), 2);
    assert_eq!(bundle.blobs.get(&h1).map(Vec::as_slice), Some(b"first blob".as_slice()));
    assert_eq!(bundle.blobs.get(&h2).map(Vec::as_slice), Some(b"second blob".as_slice()));
}

#[test]
fn an_invalid_kdm_blob_hash_is_refused_at_save() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let opts = SaveOptions { kdm: Some(kdm_save(b"snap", &[("not-a-valid-hash", b"x")])), ..Default::default() };
    assert!(matches!(save_to_bytes(&doc, &opts), Err(FormatError::Corrupt(_))));
}

#[test]
fn zip_incremental_save_never_rewrites_an_unchanged_kdm_blob() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let h = hash(3);
    let mut w = KitionPhotoWriter::new();
    let opts1 = SaveOptions { kdm: Some(kdm_save(b"snapshot v1", &[(&h, b"stable blob")])), ..Default::default() };
    let (bytes1, _) = w.save_zip(&doc, &opts1).unwrap();
    let blob1 = find_entry(&bytes1, &format!("blobs/kdm/{h}.zst"));

    // A later save with a new snapshot (as any edit makes) but the same blob content: the blob
    // entry's compressed bytes are identical (content-addressed, reused from the writer's cache).
    let opts2 = SaveOptions { kdm: Some(kdm_save(b"snapshot v2, after an edit", &[(&h, b"stable blob")])), ..Default::default() };
    let (bytes2, _) = w.save_zip(&doc, &opts2).unwrap();
    let blob2 = find_entry(&bytes2, &format!("blobs/kdm/{h}.zst"));
    assert_eq!(blob1, blob2, "the unchanged blob's bytes in the bundle did not change");

    let bundle2 = read_kdm(&bytes2).unwrap().unwrap();
    assert_eq!(bundle2.snapshot, b"snapshot v2, after an edit");
    assert_eq!(bundle2.blobs.get(&h).map(Vec::as_slice), Some(b"stable blob".as_slice()));
}

#[test]
fn directory_save_writes_and_then_garbage_collects_kdm_blobs() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let dir = temp_dir("kdm-dir");
    let mut w = KitionPhotoWriter::new();
    let (h1, h2) = (hash(4), hash(5));
    let opts1 = SaveOptions { kdm: Some(kdm_save(b"v1", &[(&h1, b"one"), (&h2, b"two")])), ..Default::default() };
    w.save_dir(&doc, &dir, &opts1).unwrap();
    assert!(dir.join("kdm.bin").is_file());
    assert!(dir.join(format!("blobs/kdm/{h1}.zst")).is_file());
    assert!(dir.join(format!("blobs/kdm/{h2}.zst")).is_file());

    // A later save that only references one of the two blobs drops the other.
    let opts2 = SaveOptions { kdm: Some(kdm_save(b"v2", &[(&h1, b"one")])), ..Default::default() };
    w.save_dir(&doc, &dir, &opts2).unwrap();
    assert!(dir.join(format!("blobs/kdm/{h1}.zst")).is_file());
    assert!(!dir.join(format!("blobs/kdm/{h2}.zst")).is_file(), "the no-longer-referenced blob was removed");

    // Saving without a replica at all removes `kdm.bin` too.
    w.save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
    assert!(!dir.join("kdm.bin").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The raw stored bytes of one ZIP entry, for comparing that an unchanged content-addressed
/// object's bytes in the bundle really did not change between two saves.
fn find_entry(zip_bytes: &[u8], name: &str) -> Vec<u8> {
    let archive = kition_photo_format::zip::ZipReader::new(zip_bytes).unwrap();
    let entry = archive.find(name).unwrap_or_else(|| panic!("missing `{name}`"));
    archive.read(entry, usize::MAX).unwrap()
}
