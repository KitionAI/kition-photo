//! Every committed `.kphoto` sample (`conformance/`, `cases/`, `web/public/`) loads and saves to its
//! own bytes (kition-sdk data model 7.4; feature `photo.file.content-only`): its `content.json` is
//! the canonical content the kit writes and its manifest the one the kit writes for it, so the
//! samples are the kit's own files. Each keeps its document id.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use base64::Engine;
use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed samples, by their path from the repository root, sorted.
fn samples() -> Vec<String> {
    let root = repo();
    let mut found = Vec::new();
    let mut stack: Vec<PathBuf> = ["conformance", "cases", "web/public"].iter().map(|dir| root.join(dir)).filter(|dir| dir.is_dir()).collect();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "kphoto") {
                found.push(path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"));
            } else if path.is_dir() {
                stack.push(path);
            }
        }
    }
    found.sort();
    found
}

fn name_of(sample: &str) -> &str {
    sample.rsplit('/').next().unwrap()
}

#[test]
fn every_committed_sample_loads_and_saves_to_its_own_bytes() {
    let samples = samples();
    assert!(samples.len() >= 3, "the samples: {samples:?}");
    for sample in samples {
        let bytes = std::fs::read(repo().join(&sample)).unwrap();
        let content = kition_photo_format::read_kdm_content(&bytes).unwrap().unwrap_or_else(|| panic!("{sample} holds its content"));
        let entry: Value = serde_json::from_slice(&content).unwrap_or_else(|error| panic!("{sample}: content.json is JSON: {error}"));
        let doc = PhotoDocument::open(name_of(&sample), &bytes).unwrap_or_else(|error| panic!("{sample}: {error}"));
        assert!(doc.has_replica(), "{sample} opens from its replica");
        assert_eq!(doc.doc_id().as_deref(), entry["docId"].as_str(), "{sample} keeps its document id");
        assert_eq!(doc.save(Some(name_of(&sample))).unwrap(), bytes, "{sample} saves to its own bytes");
    }
}

/// TEMPORARY (plain model sample conversion, deleted in the next commit): each sample with its
/// old replica entry replaced by the converted `content.json` (`tests/regen/<path with / as
/// __>.json`, from kition-sdk's `tools/convert-content`), opened and saved by the kit, printed as
/// base64; then fails so CI shows the output.
#[test]
fn regenerate_samples() {
    use kition_photo_format::zip::{ZipReader, ZipWriter};
    let regen = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regen");
    let mut printed = 0;
    for sample in samples() {
        let old = std::fs::read(repo().join(&sample)).unwrap();
        let content = std::fs::read(regen.join(format!("{}.json", sample.replace('/', "__")))).unwrap();
        let zip = ZipReader::new(&old).unwrap();
        let mut out = ZipWriter::new();
        for entry in &zip.entries {
            let data = zip.read(entry, usize::MAX).unwrap();
            match entry.name.as_str() {
                "manifest.json" => {
                    let mut manifest: Value = serde_json::from_slice(&data).unwrap();
                    manifest["kdm"] = serde_json::json!({ "content": "content.json" });
                    out.add("manifest.json", &serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
                }
                "kdm.bin" => out.add("content.json", &content).unwrap(),
                name => out.add(name, &data).unwrap(),
            }
        }
        let patched = out.finish().unwrap();
        let doc = PhotoDocument::open(name_of(&sample), &patched).unwrap_or_else(|error| panic!("{sample}: {error}"));
        assert!(doc.has_replica(), "{sample} opens from the converted content");
        let bytes = doc.save(Some(name_of(&sample))).unwrap();
        let again = PhotoDocument::open(name_of(&sample), &bytes).unwrap();
        let stable = again.save(Some(name_of(&sample))).unwrap() == bytes;
        println!("REGEN {sample} {} stable={stable} doc={:?}", base64::engine::general_purpose::STANDARD.encode(&bytes), doc.doc_id());
        printed += 1;
    }
    panic!("printed {printed} converted samples");
}
