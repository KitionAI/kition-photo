//! `.kphoto`'s KDM replica payload: `content.json` beside the manifest (DEFLATE in the ZIP form,
//! plain in the folder form), and the replica's blobs among
//! the bundle's own content-addressed entries (`tiles/<sha256>`, `blobs/<sha256>.zst`), so a tile
//! the manifest and the replica both name is stored once; a bundle without a replica loads as a
//! plain document; an incremental save never rewrites an entry whose bytes are unchanged.

mod common;
use std::sync::Arc;

use common::*;
use kition_photo_color::{ColorMode, SampleType};
use kition_photo_format::*;

fn kdm_save(content: &[u8], blobs: &[&[u8]]) -> KdmSave {
    KdmSave { content: content.to_vec(), blobs: blobs.iter().map(|b| (content_hash(b), Arc::from(*b))).collect() }
}

fn names(zip_bytes: &[u8]) -> Vec<String> {
    kition_photo_format::zip::ZipReader::new(zip_bytes).unwrap().entries.iter().map(|e| e.name.clone()).collect()
}

#[test]
fn a_bundle_without_a_replica_has_no_kdm() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let bytes = save_to_bytes(&doc, &SaveOptions::default()).unwrap();
    assert!(read_kdm(&bytes).unwrap().is_none());
    // It loads as a plain document.
    assert_eq!(viewed(load_from_bytes(&bytes).unwrap(), &doc), doc);
}

#[test]
fn a_bundle_with_a_replica_round_trips_its_content_and_its_blobs() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let opts = SaveOptions { kdm: Some(kdm_save(b"a fake replica snapshot", &[b"first blob", b"second blob"])) };
    let bytes = save_to_bytes(&doc, &opts).unwrap();
    // The document itself is unaffected: a reader that knows nothing about KDM still loads it.
    assert_eq!(viewed(load_from_bytes(&bytes).unwrap(), &doc), doc);

    let bundle = read_kdm(&bytes).unwrap().unwrap();
    assert_eq!(bundle.content, b"a fake replica snapshot");
    assert_eq!(read_kdm_content(&bytes).unwrap().as_deref(), Some(b"a fake replica snapshot".as_slice()));
    assert_eq!(bundle.blobs.get(&content_hash(b"first blob")).map(Vec::as_slice), Some(b"first blob".as_slice()));
    assert_eq!(bundle.blobs.get(&content_hash(b"second blob")).map(Vec::as_slice), Some(b"second blob".as_slice()));
    // Every entry the bundle holds is named by the hash of what it holds.
    for (hash, data) in &bundle.blobs {
        assert_eq!(&content_hash(data), hash);
    }
}

/// The replica names a tile by the SHA-256 of its encoded bytes, as the manifest does: the bundle
/// holds that tile once, as the manifest's `tiles/<sha256>`, and the replica reads it from there.
#[test]
fn a_tile_the_manifest_and_the_replica_both_name_is_stored_once() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U16);
    let layer = doc.layers.iter().find(|l| l.surface().is_some_and(|s| s.tile_count() > 0)).unwrap();
    let surface = layer.surface().unwrap();
    let (_, tile) = surface.tiles().next().unwrap();
    let encoded = encode_tile(surface.format(), tile);
    let hash = content_hash(&encoded);
    let opts = SaveOptions { kdm: Some(kdm_save(b"snapshot", &[&encoded])) };
    let (bytes, stats) = KitionPhotoWriter::new().save_zip(&doc, &opts).unwrap();
    let names = names(&bytes);
    let holding: Vec<&String> = names.iter().filter(|n| n.contains(&hash)).collect();
    assert_eq!(holding, vec![&format!("tiles/{hash}")], "one entry holds the tile");
    assert_eq!(names.iter().filter(|n| n.starts_with("tiles/")).count(), stats.tiles_total, "one entry per distinct tile");
    let bundle = read_kdm(&bytes).unwrap().unwrap();
    assert_eq!(bundle.blobs.get(&hash), Some(&encoded), "the replica reads the manifest's entry");
    assert_eq!(viewed(load_from_bytes(&bytes).unwrap(), &doc), doc);
}

#[test]
fn an_invalid_kdm_blob_hash_is_refused_at_save() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let opts = SaveOptions { kdm: Some(KdmSave { content: b"snap".to_vec(), blobs: vec![("not-a-valid-hash".to_owned(), Arc::from(&b"x"[..]))] }) };
    assert!(matches!(save_to_bytes(&doc, &opts), Err(FormatError::Corrupt(_))));
}

/// An entry whose bytes do not match the hash it is named by is refused when the replica is read.
#[test]
fn a_replica_blob_that_does_not_match_its_hash_is_refused() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let wrong = content_hash(b"something else");
    let opts = SaveOptions { kdm: Some(KdmSave { content: b"snap".to_vec(), blobs: vec![(wrong, Arc::from(&b"a blob"[..]))] }) };
    let bytes = save_to_bytes(&doc, &opts).unwrap();
    assert!(matches!(read_kdm(&bytes), Err(FormatError::Corrupt(_))));
}

#[test]
fn zip_incremental_save_never_rewrites_an_unchanged_kdm_blob() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let h = content_hash(b"stable blob");
    let mut w = KitionPhotoWriter::new();
    let opts1 = SaveOptions { kdm: Some(kdm_save(b"snapshot v1", &[b"stable blob"])) };
    let (bytes1, _) = w.save_zip(&doc, &opts1).unwrap();
    let blob1 = find_entry(&bytes1, &format!("blobs/{h}.zst"));

    // A later save with a new snapshot (as any edit makes) but the same blob content: the blob
    // entry's compressed bytes are identical (content-addressed, reused from the writer's cache).
    let opts2 = SaveOptions { kdm: Some(kdm_save(b"snapshot v2, after an edit", &[b"stable blob"])) };
    let (bytes2, _) = w.save_zip(&doc, &opts2).unwrap();
    let blob2 = find_entry(&bytes2, &format!("blobs/{h}.zst"));
    assert_eq!(blob1, blob2, "the unchanged blob's bytes in the bundle did not change");

    let bundle2 = read_kdm(&bytes2).unwrap().unwrap();
    assert_eq!(bundle2.content, b"snapshot v2, after an edit");
    assert_eq!(bundle2.blobs.get(&h).map(Vec::as_slice), Some(b"stable blob".as_slice()));
}

#[test]
fn directory_save_writes_and_then_garbage_collects_kdm_blobs() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let dir = temp_dir("kdm-dir");
    let mut w = KitionPhotoWriter::new();
    let (h1, h2) = (content_hash(b"one"), content_hash(b"two"));
    let opts1 = SaveOptions { kdm: Some(kdm_save(b"v1", &[b"one", b"two"])) };
    w.save_dir(&doc, &dir, &opts1).unwrap();
    assert_eq!(std::fs::read(dir.join("content.json")).unwrap(), b"v1", "the folder form holds the content as it is");
    assert!(dir.join(format!("blobs/{h1}.zst")).is_file());
    assert!(dir.join(format!("blobs/{h2}.zst")).is_file());

    // A later save that only references one of the two blobs drops the other.
    let opts2 = SaveOptions { kdm: Some(kdm_save(b"v2", &[b"one"])) };
    w.save_dir(&doc, &dir, &opts2).unwrap();
    assert!(dir.join(format!("blobs/{h1}.zst")).is_file());
    assert!(!dir.join(format!("blobs/{h2}.zst")).is_file(), "the no-longer-referenced blob was removed");

    // Saving without a replica at all removes `content.json` too.
    w.save_dir(&doc, &dir, &SaveOptions::default()).unwrap();
    assert!(!dir.join("content.json").exists());
    assert!(!dir.join(format!("blobs/{h1}.zst")).is_file());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// In the ZIP form `content.json` is a DEFLATE entry the manifest names, and the same content
/// saves the same bytes.
#[test]
fn the_zip_form_deflates_its_content_entry_at_a_fixed_level() {
    let doc = rich_doc(ColorMode::Rgb, SampleType::U8);
    let content = br#"{"kdm":1,"kit":"kit.photo","objects":{}}"#.repeat(64);
    let opts = SaveOptions { kdm: Some(kdm_save(&content, &[])) };
    let bytes = save_to_bytes(&doc, &opts).unwrap();
    assert_eq!(bytes, save_to_bytes(&doc, &opts).unwrap(), "the same content, the same bytes");
    let manifest = read_manifest(&bytes).unwrap();
    assert_eq!(manifest.kdm.map(|kdm| kdm.content).as_deref(), Some("content.json"));
    assert!(names(&bytes).contains(&"content.json".to_owned()));
    assert!(!names(&bytes).iter().any(|name| name == "kdm.bin"));
    assert_eq!(read_kdm(&bytes).unwrap().unwrap().content, content);
    let stored = bytes.len();
    let plain = save_to_bytes(&doc, &SaveOptions { kdm: Some(kdm_save(b"", &[])) }).unwrap().len();
    assert!(stored - plain < content.len() / 4, "the entry is compressed: {} bytes for {}", stored - plain, content.len());
}

/// The raw stored bytes of one ZIP entry, for comparing that an unchanged content-addressed
/// object's bytes in the bundle really did not change between two saves.
fn find_entry(zip_bytes: &[u8], name: &str) -> Vec<u8> {
    let archive = kition_photo_format::zip::ZipReader::new(zip_bytes).unwrap();
    let entry = archive.find(name).unwrap_or_else(|| panic!("missing `{name}`"));
    archive.read(entry, usize::MAX).unwrap()
}
