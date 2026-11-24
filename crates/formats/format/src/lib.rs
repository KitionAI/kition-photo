//! # kition-photo-format
//!
//! The native, lossless `.kphoto` document bundle (architecture §9).
//!
//! ```text
//! manifest.json            versioned description of the image's content (see [`manifest`])
//! kdm.bin                  optional: the document's KDM replica as its content snapshot
//! tiles/<sha256>           content-addressed 256×256 tiles (zstd of little-endian samples)
//! blobs/<sha256>.zst       content-addressed binary data (ICC, EXIF, PSD blocks, object layers)
//! ```
//!
//! **One copy of every tile and blob.** Each entry is named by the SHA-256 of the bytes it
//! holds (a tile's encoded bytes, [`encode_tile`]; a blob's own bytes), which is how the KDM
//! replica names its blobs too: the manifest and `kdm.bin` refer to the same entries, and a save
//! writes each once.
//!
//! `kdm.bin` holds the document's content, not its history (kition-sdk data model 7.3, rule
//! M16b): the replica's state and, of its history, only the seal; the engine writes it
//! (`DocState::save_kphoto`). The undo history stays in the session and the host's store.
//! The manifest stays beside it: it is what a reader without the replica model opens (the PSD
//! exporter placing a `.kphoto`, the importer, `kition-photo-io`'s export of a document that has
//! made no replica yet), and it is what opens when this build refuses the replica (another schema
//! version).
//!
//! **Content only.** Neither the manifest nor the replica holds one person's view (the
//! selection, the quick mask, expanded groups, open notes, the timeline's playhead, the last
//! applied layer comp), the document's handle or its title, so two people who open the same file
//! and save it write the same bytes; [`view_state`] keeps a person's view apart, for the host's
//! `view-state` store. Nor does the file hold a thumbnail or a flattened preview: it is not a
//! composite another program opens, and anything that needs a picture renders one.
//!
//! A bundle is either a ZIP archive (STORE entries) or a directory with the
//! same layout. Saving is incremental: [`KitionPhotoWriter`] remembers what it
//! already compressed/wrote, so only new tiles are encoded. Directory saves
//! verify existing content-addressed objects once per writer and folder,
//! rechecking files whose size or modification time changes; they replace
//! damaged or missing objects and garbage-collect unreferenced ones.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod atomic;
pub mod autosave;
mod convert;
mod finite;
pub mod host_files;
pub mod manifest;
mod migrate;
pub mod read;
mod store;
pub mod view_state;
pub mod zip;

use std::path::Path;
use std::sync::Arc;

use kition_photo_doc::Document;

pub use atomic::atomic_write;
#[cfg(not(target_arch = "wasm32"))]
pub use autosave::RecoveryStore;
pub use autosave::{Autosaver, RecoveryEntry, discard_recovery, list_recovery, recover};
pub use convert::MAX_GROUP_DEPTH;
pub use manifest::{FORMAT_VERSION, KdmM, Manifest};
pub use read::read_file;
pub use store::{DEFAULT_REVERIFY_BUDGET, KdmBundle, KitionPhotoWriter, SaveStats, decode_tile, encode_tile, hash_bytes as content_hash};

/// File extension of the native format.
pub const EXTENSION: &str = "kphoto";

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("corrupt .kphoto bundle: {0}")]
    Corrupt(String),
    #[error("unsupported .kphoto feature: {0}")]
    Unsupported(String),
    #[error("bundle written by a newer version (format {found}, this build reads up to {supported})")]
    TooNew { found: u32, supported: u32 },
    #[error("limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("`{path}` is NaN or infinite, which a .kphoto manifest cannot hold; reset that value and save again")]
    NonFinite { path: String },
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl FormatError {
    pub(crate) fn corrupt(msg: impl Into<String>) -> Self {
        FormatError::Corrupt(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, FormatError>;

/// What a save writes beside the document's manifest.
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    /// The document's KDM replica to embed, if it has made one yet.
    pub kdm: Option<KdmSave>,
}

/// A KDM replica to embed in a `.kphoto` save: the snapshot bytes (written as `kdm.bin`) and the
/// blobs it references, keyed by their SHA-256 hex hash. A blob the manifest names too (every
/// tile of the image) is stored once, as the manifest's entry; the others as `blobs/<sha256>.zst`.
#[derive(Debug, Clone, Default)]
pub struct KdmSave {
    pub snapshot: Vec<u8>,
    pub blobs: Vec<(String, Arc<[u8]>)>,
}

/// Limits applied while loading untrusted bundles.
#[derive(Debug, Clone, Copy)]
pub struct LoadOptions {
    /// Maximum size of `manifest.json`.
    pub max_manifest_bytes: usize,
    /// Maximum size of one decompressed blob.
    pub max_blob_bytes: usize,
    /// Maximum total decompressed tile + blob bytes.
    pub max_total_bytes: u64,
    /// Keep document and layer ids from the file (the id counter is advanced
    /// past them). When `false`, fresh ids are assigned.
    pub preserve_ids: bool,
}

impl Default for LoadOptions {
    fn default() -> Self {
        LoadOptions { max_manifest_bytes: 256 << 20, max_blob_bytes: 1 << 30, max_total_bytes: 16 << 30, preserve_ids: true }
    }
}

/// `true` when ZIP bytes contain a `manifest.json` entry.
///
/// Bundles written here place the manifest first; re-zipped bundles may reorder entries.
pub fn is_kphoto(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK\x03\x04") {
        return false;
    }
    if bytes.len() >= 30 {
        let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
        if bytes.get(30..30 + name_len) == Some(b"manifest.json") {
            return true;
        }
    }
    zip::ZipReader::new(bytes).is_ok_and(|archive| archive.find("manifest.json").is_some())
}

/// Save to an in-memory ZIP bundle (one-shot; use [`KitionPhotoWriter`] for
/// incremental saves).
pub fn save_to_bytes(doc: &Document, opts: &SaveOptions) -> Result<Vec<u8>> {
    KitionPhotoWriter::new().save_zip(doc, opts).map(|(b, _)| b)
}

/// Load a ZIP bundle from memory.
pub fn load_from_bytes(bytes: &[u8]) -> Result<Document> {
    load_from_bytes_with(bytes, &LoadOptions::default())
}

pub fn load_from_bytes_with(bytes: &[u8], opts: &LoadOptions) -> Result<Document> {
    let src = store::ZipSource::new(bytes)?;
    store::load(&src, opts)
}

/// Load a bundle from a path: a directory bundle or a ZIP file.
pub fn load_path(path: &Path) -> Result<Document> {
    load_path_with(path, &LoadOptions::default())
}

pub fn load_path_with(path: &Path, opts: &LoadOptions) -> Result<Document> {
    host_files::refuse(path)?;
    if path.is_dir() {
        store::load(&store::DirSource { root: path.to_path_buf() }, opts)
    } else {
        let bytes = read::read_file(path)?;
        load_from_bytes_with(&bytes, opts)
    }
}

/// Read and migrate only the manifest (fast: no tiles are decoded).
pub fn read_manifest(bytes: &[u8]) -> Result<Manifest> {
    let src = store::ZipSource::new(bytes)?;
    store::read_manifest(&src, &LoadOptions::default())
}

/// The bundle's KDM replica, if it has one: the snapshot bytes and the bundle's tiles and blobs,
/// keyed by their SHA-256 hex hash. `None` for a bundle saved without a replica; such a document
/// still loads through [`load_from_bytes`], as a new replica.
pub fn read_kdm(bytes: &[u8]) -> Result<Option<KdmBundle>> {
    store::read_kdm(bytes, &LoadOptions::default())
}

/// The bundle's KDM replica snapshot (`kdm.bin`) without its blobs, or `None` for a bundle saved
/// without a replica: what a `.kphoto` carries as its document's identity (`document.identity`).
pub fn read_kdm_snapshot(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    store::read_kdm_snapshot(bytes, &LoadOptions::default())
}

/// `bytes` (a bundle with a replica) holding the replica `snapshot` instead of its own
/// (`document.fork`): `kdm.bin` is `snapshot`, and of the tiles and blobs only those the manifest
/// or `keep` names stay; the manifest and entries this build does not know are kept as they were.
/// A bundle without a replica is an error.
pub fn replace_kdm(bytes: &[u8], snapshot: &[u8], keep: &dyn Fn(&str) -> bool) -> Result<Vec<u8>> {
    store::replace_kdm(bytes, snapshot, keep, &LoadOptions::default())
}
