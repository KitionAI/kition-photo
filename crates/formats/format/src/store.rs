//! Object storage (ZIP or directory), incremental writer and loader.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use kition_photo_color::{PixelFormat, SampleType};
use kition_photo_doc::Document;
use kition_photo_raster::Tile;
use sha2::Digest;

use crate::convert::{self, Fetch, Loader, Sink, is_valid_hash, swap_to_le};
use crate::manifest::{ContentM, FORMAT_VERSION, Hash, KdmM, LayerM, Manifest};
use crate::zip::{ZipReader, ZipWriter};
use crate::{FormatError, LoadOptions, Result, SaveOptions};

pub(crate) const MANIFEST: &str = "manifest.json";
/// The document's KDM content (`Replica::content_snapshot`), stored next to the manifest rather
/// than content-addressed: it is the one entry that changes on nearly every save. DEFLATE at a
/// fixed level in the ZIP form ([`crate::zip::DEFLATE_LEVEL`]), plain in the folder form.
pub(crate) const KDM: &str = "content.json";

/// A tile: its encoded bytes ([`encode_tile`]), stored as they are (they are zstd already).
fn tile_path(h: &str) -> String {
    format!("tiles/{h}")
}
/// Any other binary data, zstd-compressed.
fn blob_path(h: &str) -> String {
    format!("blobs/{h}.zst")
}

/// The hash a tile or blob is named by: the SHA-256 of its bytes (a tile's encoded bytes), as
/// the KDM replica names its blobs, so the manifest and the replica name one stored copy.
pub fn hash_bytes(b: &[u8]) -> Hash {
    convert::hex(&sha2::Sha256::digest(b))
}

fn compress(data: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(data, ruzstd::encoding::CompressionLevel::Fastest)
}

pub(crate) fn decompress(data: &[u8], expected: usize, what: &str) -> Result<Vec<u8>> {
    let mut dec = ruzstd::decoding::StreamingDecoder::new(data).map_err(|e| FormatError::corrupt(format!("{what}: zstd: {e}")))?;
    let mut out = Vec::with_capacity(expected.min(64 << 20));
    (&mut dec).take(expected as u64 + 1).read_to_end(&mut out).map_err(|e| FormatError::corrupt(format!("{what}: zstd: {e}")))?;
    if out.len() != expected {
        return Err(FormatError::corrupt(format!("{what}: expected {expected} bytes, got {}", out.len())));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Sources
// ---------------------------------------------------------------------------

pub(crate) trait Source {
    fn get(&self, path: &str, max: usize) -> Result<Vec<u8>>;
}

pub(crate) struct ZipSource<'a> {
    zip: ZipReader<'a>,
}

impl<'a> ZipSource<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        Ok(ZipSource { zip: ZipReader::new(bytes)? })
    }
}

impl Source for ZipSource<'_> {
    fn get(&self, path: &str, max: usize) -> Result<Vec<u8>> {
        self.zip.read_by_name(path, max)
    }
}

pub(crate) struct DirSource {
    pub root: PathBuf,
}

impl Source for DirSource {
    fn get(&self, path: &str, max: usize) -> Result<Vec<u8>> {
        let p = self.root.join(path);
        let len = std::fs::metadata(&p).map_err(|_| FormatError::corrupt(format!("missing `{path}`")))?.len();
        if len > max as u64 {
            return Err(FormatError::LimitExceeded(format!("`{path}` is {len} bytes (max {max})")));
        }
        Ok(std::fs::read(p)?)
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Deepest JSON nesting a manifest may have: each group level adds three (the layer object, its
/// content object and the `children` array), with headroom for the document and leaf layer fields.
/// The saver checks the same bound, so every bundle it writes loads again.
const MAX_MANIFEST_DEPTH: usize = 3 * (convert::MAX_GROUP_DEPTH + 1) + 64;

/// Rejects JSON nested deeper than [`MAX_MANIFEST_DEPTH`] with a linear scan (no recursion), so
/// parsing with serde_json's recursion limit disabled can't exhaust the stack on hostile input.
fn check_manifest_depth(json: &[u8]) -> Result<()> {
    let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
    for &b in json {
        if in_str {
            match b {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > MAX_MANIFEST_DEPTH {
                    return Err(FormatError::LimitExceeded(format!("manifest JSON nested deeper than {MAX_MANIFEST_DEPTH} levels")));
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn read_manifest(src: &dyn Source, opts: &LoadOptions) -> Result<Manifest> {
    let raw = src.get(MANIFEST, opts.max_manifest_bytes)?;
    // serde_json stops at 128 levels (about 40 nested groups); the bounded check replaces that limit.
    check_manifest_depth(&raw)?;
    let mut de = serde_json::Deserializer::from_slice(&raw);
    de.disable_recursion_limit();
    let mut v = <serde_json::Value as serde::Deserialize>::deserialize(&mut de)?;
    de.end()?;
    crate::migrate::migrate(&mut v)?;
    let layers = take_array(&mut v, "/document/layers");
    let mut m: Manifest = serde_json::from_value(v)?;
    m.document.layers = layers_from_json(layers)?;
    Ok(m)
}

/// Decodes layers one at a time, group children first. Decoding the whole tree in one go costs
/// kilobytes of stack per nesting level; here each level only holds a few pointers.
fn layers_from_json(values: Vec<serde_json::Value>) -> Result<Vec<LayerM>> {
    let mut out = Vec::with_capacity(values.len());
    for mut v in values {
        let children = layers_from_json(take_array(&mut v, "/content/children"))?;
        out.push(layer_from_json(v, children)?);
    }
    Ok(out)
}

/// One layer without its children (kept out of line so its large locals stay off the recursion).
#[inline(never)]
fn layer_from_json(v: serde_json::Value, children: Vec<LayerM>) -> Result<LayerM> {
    let mut layer: LayerM = serde_json::from_value(v)?;
    if let ContentM::Group { children: c, .. } = &mut layer.content {
        *c = children;
    }
    Ok(layer)
}

/// Takes the array at JSON `pointer` out of `v`, leaving an empty one (nothing if absent).
fn take_array(v: &mut serde_json::Value, pointer: &str) -> Vec<serde_json::Value> {
    match v.pointer_mut(pointer) {
        Some(serde_json::Value::Array(a)) => std::mem::take(a),
        _ => Vec::new(),
    }
}

struct LoadFetch<'a> {
    src: &'a dyn Source,
    opts: LoadOptions,
    total: u64,
    blobs: HashMap<String, Arc<Vec<u8>>>,
}

impl LoadFetch<'_> {
    fn account(&mut self, n: usize) -> Result<()> {
        self.total += n as u64;
        if self.total > self.opts.max_total_bytes {
            return Err(FormatError::LimitExceeded(format!("bundle expands beyond {} bytes", self.opts.max_total_bytes)));
        }
        Ok(())
    }
}

impl Fetch for LoadFetch<'_> {
    fn tile(&mut self, hash: &str, len: usize) -> Result<Vec<u8>> {
        if !is_valid_hash(hash) {
            return Err(FormatError::corrupt(format!("invalid tile hash `{hash}`")));
        }
        self.account(len)?;
        let path = tile_path(hash);
        let encoded = self.src.get(&path, len + len / 8 + 4096)?;
        if hash_bytes(&encoded) != hash {
            return Err(FormatError::corrupt(format!("{path}: content does not match its hash")));
        }
        decompress(&encoded, len, &path)
    }

    fn blob(&mut self, hash: &str) -> Result<Arc<Vec<u8>>> {
        if let Some(b) = self.blobs.get(hash) {
            return Ok(b.clone());
        }
        if !is_valid_hash(hash) {
            return Err(FormatError::corrupt(format!("invalid blob hash `{hash}`")));
        }
        let path = blob_path(hash);
        let z = self.src.get(&path, self.opts.max_blob_bytes)?;
        // Blob length is not in the manifest; the frame header bounds it and
        // we cap at max_blob_bytes.
        let data = decompress_capped(&z, self.opts.max_blob_bytes, &path)?;
        self.account(data.len())?;
        if hash_bytes(&data) != hash {
            return Err(FormatError::corrupt(format!("{path}: content does not match its hash")));
        }
        let data = Arc::new(data);
        self.blobs.insert(hash.to_owned(), data.clone());
        Ok(data)
    }
}

/// Decompresses zstd data whose decompressed length is not recorded ahead of time, capped at
/// `max` bytes.
fn decompress_capped(data: &[u8], max: usize, what: &str) -> Result<Vec<u8>> {
    let mut dec = ruzstd::decoding::StreamingDecoder::new(data).map_err(|e| FormatError::corrupt(format!("{what}: zstd: {e}")))?;
    let mut out = Vec::new();
    (&mut dec).take(max as u64 + 1).read_to_end(&mut out).map_err(|e| FormatError::corrupt(format!("{what}: zstd: {e}")))?;
    if out.len() > max {
        return Err(FormatError::LimitExceeded(format!("{what} exceeds max_blob_bytes")));
    }
    Ok(out)
}

/// The document's KDM replica, if the bundle has one: its content (`content.json`) and the bundle's tiles
/// and blobs, keyed by the SHA-256 hex hash the replica names them by (a tile as its encoded
/// bytes, a blob decompressed). They are the manifest's too: the file holds each once. Every
/// entry is checked against its hash.
pub struct KdmBundle {
    pub content: Vec<u8>,
    pub blobs: HashMap<String, Vec<u8>>,
}

pub(crate) fn read_kdm(bytes: &[u8], opts: &LoadOptions) -> Result<Option<KdmBundle>> {
    let src = ZipSource::new(bytes)?;
    let m = read_manifest(&src, opts)?;
    let Some(kdm) = m.kdm else { return Ok(None) };
    let mut total = 0u64;
    let content = src.zip.read_by_name(&kdm.content, opts.max_blob_bytes)?;
    total += content.len() as u64;
    if total > opts.max_total_bytes {
        return Err(FormatError::LimitExceeded(format!("bundle expands beyond {} bytes", opts.max_total_bytes)));
    }
    let max_compressed = opts.max_blob_bytes.saturating_add(opts.max_blob_bytes / 8).saturating_add(4096);
    let mut blobs = HashMap::new();
    for entry in &src.zip.entries {
        let Some((hash, data)) = object(&src, entry, max_compressed, opts)? else { continue };
        total += data.len() as u64;
        if total > opts.max_total_bytes {
            return Err(FormatError::LimitExceeded(format!("bundle expands beyond {} bytes", opts.max_total_bytes)));
        }
        blobs.insert(hash, data);
    }
    Ok(Some(KdmBundle { content, blobs }))
}

/// A content-addressed entry's hash and the bytes it names (a tile's stored bytes, a blob
/// decompressed), checked against the hash; `None` for an entry that is not one.
fn object(src: &ZipSource<'_>, entry: &crate::zip::Entry, max_compressed: usize, opts: &LoadOptions) -> Result<Option<(String, Vec<u8>)>> {
    let (hash, compressed) = match (entry.name.strip_prefix("tiles/"), entry.name.strip_prefix("blobs/").and_then(|rest| rest.strip_suffix(".zst"))) {
        (Some(hash), _) => (hash, false),
        (None, Some(hash)) => (hash, true),
        (None, None) => return Ok(None),
    };
    if !is_valid_hash(hash) {
        return Err(FormatError::corrupt(format!("invalid object hash in `{}`", entry.name)));
    }
    let stored = src.zip.read(entry, max_compressed)?;
    let data = if compressed { decompress_capped(&stored, opts.max_blob_bytes, &entry.name)? } else { stored };
    if hash_bytes(&data) != hash {
        return Err(FormatError::corrupt(format!("{}: content does not match its hash", entry.name)));
    }
    Ok(Some((hash.to_owned(), data)))
}

/// The bundle's KDM content alone (`content.json`), without reading its blobs, or `None` for
/// a bundle saved without one.
pub(crate) fn read_kdm_content(bytes: &[u8], opts: &LoadOptions) -> Result<Option<Vec<u8>>> {
    let src = ZipSource::new(bytes)?;
    let Some(kdm) = read_manifest(&src, opts)?.kdm else { return Ok(None) };
    src.zip.read_by_name(&kdm.content, opts.max_blob_bytes).map(Some)
}

/// Every hash `value` (a manifest's JSON) names: any string of 64 lowercase hex digits. One that
/// only looks like a hash keeps an entry it need not, never drops one it needs.
fn named_hashes(value: &serde_json::Value, out: &mut HashSet<String>) {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::String(text) if is_valid_hash(text) => {
                out.insert(text.clone());
            }
            serde_json::Value::Array(items) => stack.extend(items),
            serde_json::Value::Object(members) => stack.extend(members.values()),
            _ => {}
        }
    }
}

/// `bytes` (a bundle with a replica) with `content` as its replica content, keeping the tiles
/// and blobs the manifest names or `keep` does and dropping the others; every other entry (the
/// manifest, entries this build does not know) is kept as it was, in the same order.
pub(crate) fn replace_kdm(bytes: &[u8], content: &[u8], keep: &dyn Fn(&str) -> bool, opts: &LoadOptions) -> Result<Vec<u8>> {
    let src = ZipSource::new(bytes)?;
    let Some(kdm) = read_manifest(&src, opts)?.kdm else { return Err(FormatError::corrupt("the bundle holds no replica to replace")) };
    let raw = src.get(MANIFEST, opts.max_manifest_bytes)?;
    check_manifest_depth(&raw)?;
    let mut de = serde_json::Deserializer::from_slice(&raw);
    de.disable_recursion_limit();
    let manifest = <serde_json::Value as serde::Deserialize>::deserialize(&mut de)?;
    let mut named = HashSet::new();
    named_hashes(&manifest, &mut named);
    let max = opts.max_blob_bytes.saturating_add(opts.max_blob_bytes / 8).saturating_add(4096);
    let mut out = ZipWriter::new();
    let mut total = 0u64;
    for entry in &src.zip.entries {
        if entry.name == kdm.content {
            out.add_deflated(&entry.name, content)?;
            continue;
        }
        let hash = entry.name.strip_prefix("tiles/").or_else(|| entry.name.strip_prefix("blobs/").and_then(|rest| rest.strip_suffix(".zst")));
        if let Some(hash) = hash
            && !named.contains(hash)
            && !keep(hash)
        {
            continue;
        }
        let data = src.zip.read(entry, max)?;
        total = total.saturating_add(data.len() as u64);
        if total > opts.max_total_bytes {
            return Err(FormatError::LimitExceeded(format!("bundle expands beyond {} bytes", opts.max_total_bytes)));
        }
        out.add(&entry.name, &data)?;
    }
    out.finish()
}

pub(crate) fn load(src: &dyn Source, opts: &LoadOptions) -> Result<Document> {
    // Ids the bundle does not keep are not the numbers of whatever document is being edited.
    let _detached = kition_photo_doc::detached();
    let m = read_manifest(src, opts)?;
    let mut fetch = LoadFetch { src, opts: *opts, total: 0, blobs: HashMap::new() };
    let mut loader = Loader { fetch: &mut fetch, preserve_ids: opts.preserve_ids, max_id: 0, id_map: HashMap::new() };
    let doc = loader.document(&m.document)?;
    if opts.preserve_ids && !convert::reserve_ids_through(loader.max_id) {
        // Ids far beyond our counter: remap instead of risking collisions.
        let fresh = LoadOptions { preserve_ids: false, ..*opts };
        return load(src, &fresh);
    }
    Ok(doc)
}

// ---------------------------------------------------------------------------
// Saving
// ---------------------------------------------------------------------------

/// What a save did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaveStats {
    /// Distinct tiles referenced by the document.
    pub tiles_total: usize,
    /// Tiles that had to be compressed (ZIP) or written (directory).
    pub tiles_written: usize,
    /// Tiles reused from the previous save.
    pub tiles_reused: usize,
    pub blobs_total: usize,
    pub blobs_written: usize,
    /// Unreferenced objects removed (directory bundles).
    pub objects_removed: usize,
    /// Bytes of the manifest.
    pub manifest_bytes: usize,
}

/// A tile's hash and its encoded bytes, remembered by the tile's address (checked against a
/// weak handle: a freed tile's address can be reused).
type TileCache = HashMap<usize, (Weak<Tile>, Hash, Arc<Vec<u8>>)>;

/// Collects what a manifest names: every distinct tile (encoded) and blob, by hash.
#[derive(Default)]
struct Collect<'c> {
    tiles: BTreeMap<Hash, Arc<Vec<u8>>>,
    blobs: BTreeMap<Hash, Arc<Vec<u8>>>,
    tile_cache: Option<&'c mut TileCache>,
}

impl Sink for Collect<'_> {
    fn tile(&mut self, format: PixelFormat, tile: &Arc<Tile>) -> Hash {
        let key = Arc::as_ptr(tile) as usize;
        if let Some(cache) = self.tile_cache.as_deref_mut()
            && let Some((w, h, encoded)) = cache.get(&key)
            && w.upgrade().is_some_and(|t| Arc::ptr_eq(&t, tile))
        {
            let h = h.clone();
            self.tiles.entry(h.clone()).or_insert_with(|| encoded.clone());
            return h;
        }
        let encoded = Arc::new(encode_tile(format, tile));
        let h = hash_bytes(&encoded);
        if let Some(cache) = self.tile_cache.as_deref_mut() {
            cache.insert(key, (Arc::downgrade(tile), h.clone(), encoded.clone()));
        }
        self.tiles.entry(h.clone()).or_insert(encoded);
        h
    }

    fn blob(&mut self, data: &Arc<Vec<u8>>) -> Hash {
        let h = hash_bytes(data);
        self.blobs.entry(h.clone()).or_insert_with(|| data.clone());
        h
    }
}

/// Gathers the tiles a manifest names that `cache` has not encoded yet (by address), so
/// [`KitionPhotoWriter::prepare`] encodes them on every core before it writes the manifest.
struct Uncached<'c> {
    cache: &'c TileCache,
    seen: HashSet<usize>,
    tiles: Vec<(PixelFormat, Arc<Tile>)>,
}

impl Sink for Uncached<'_> {
    fn tile(&mut self, format: PixelFormat, tile: &Arc<Tile>) -> Hash {
        let key = Arc::as_ptr(tile) as usize;
        let cached = self.cache.get(&key).is_some_and(|(w, _, _)| w.upgrade().is_some_and(|t| Arc::ptr_eq(&t, tile)));
        if !cached && self.seen.insert(key) {
            self.tiles.push((format, tile.clone()));
        }
        Hash::new()
    }

    fn blob(&mut self, _data: &Arc<Vec<u8>>) -> Hash {
        Hash::new()
    }
}

fn tile_le(t: &Tile, sample: SampleType) -> std::borrow::Cow<'_, [u8]> {
    if cfg!(target_endian = "big") {
        let mut b = t.bytes().to_vec();
        swap_to_le(&mut b, sample);
        std::borrow::Cow::Owned(b)
    } else {
        std::borrow::Cow::Borrowed(t.bytes())
    }
}

/// Encodes one tile exactly as `.kphoto` stores it and names it: canonical little-endian sample
/// bytes, zstd-compressed; the file's `tiles/<sha256>` entry holds these bytes, and the KDM
/// replica (`crates/kdm/kdm-model`) keeps the same bytes as the tile's blob, under the same hash.
pub fn encode_tile(format: PixelFormat, tile: &Tile) -> Vec<u8> {
    compress(&tile_le(tile, format.sample))
}

/// Reverses [`encode_tile`]: zstd-decompresses and swaps back to native-endian sample bytes
/// (`tile.bytes_mut()`'s length, `TILE_SIZE² × format.bytes_per_pixel()`).
pub fn decode_tile(format: PixelFormat, bytes: &[u8]) -> Result<Vec<u8>> {
    let mut data = decompress(bytes, convert::tile_len(&format), "tile")?;
    swap_to_le(&mut data, format.sample);
    Ok(data)
}

/// Default for [`KitionPhotoWriter::set_reverify_budget`]: compressed bytes of already-verified
/// objects re-read per directory save.
pub const DEFAULT_REVERIFY_BUDGET: u64 = 16 * 1024 * 1024;

/// Incremental `.kphoto` writer. Keep one per open document: it remembers
/// tile hashes (by `Arc` identity), compressed objects, and objects it has
/// verified in the current directory, so repeated saves stay cheap.
///
/// **What the directory cache trusts.** A directory save reuses an existing object file only
/// after decompressing it and checking its content hash. After that, the writer remembers the
/// file's signature (length and modification time, plus inode, device and change time on Unix)
/// and skips the full check on later saves while the signature is unchanged. A file damaged in
/// place without changing its signature (bit rot, or a tool that restores timestamps where
/// there is no change time) would be trusted, so each save also re-verifies a rolling share of
/// the cached objects, oldest-checked first, up to [`DEFAULT_REVERIFY_BUDGET`] bytes (see
/// [`Self::set_reverify_budget`]). Over enough saves every object is read again, and a damaged
/// one is rewritten.
pub struct KitionPhotoWriter {
    tile_cache: TileCache,
    /// Compressed objects by bundle path (ZIP mode).
    compressed: HashMap<String, Arc<Vec<u8>>>,
    verified_directory: Option<DirectoryVerificationCache>,
    reverify_budget: u64,
}

impl Default for KitionPhotoWriter {
    fn default() -> Self {
        Self { tile_cache: HashMap::new(), compressed: HashMap::new(), verified_directory: None, reverify_budget: DEFAULT_REVERIFY_BUDGET }
    }
}

#[derive(Default)]
struct DirectoryVerificationCache {
    directory: PathBuf,
    /// Directory saves so far; an object's `checked_at` is the save that last read it fully.
    generation: u64,
    objects: HashMap<String, Verified>,
}

#[derive(Clone, Copy)]
struct Verified {
    signature: FileSignature,
    checked_at: u64,
}

/// What a file looked like when it was verified. Any change means "verify again".
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileSignature {
    len: u64,
    modified: SystemTime,
    /// Inode, device and status-change time (s, ns): an in-place rewrite changes the change
    /// time even when a tool restores the modification time.
    #[cfg(unix)]
    unix: (u64, u64, i64, i64),
}

struct Prepared {
    manifest: Vec<u8>,
    objects: BTreeMap<String, Object>,
    /// The KDM content (`content.json`), if the save has one; its blobs are among `objects`.
    kdm: Option<Vec<u8>>,
    stats: SaveStats,
}

/// A content-addressed entry: a tile's encoded bytes (stored as they are) or a blob (stored
/// zstd-compressed). Either way the entry's name is the SHA-256 of these bytes.
enum Object {
    Tile(Arc<Vec<u8>>),
    Blob(Arc<Vec<u8>>),
}

impl Object {
    /// The bytes the bundle stores.
    fn stored(&self) -> Arc<Vec<u8>> {
        match self {
            Object::Tile(encoded) => encoded.clone(),
            Object::Blob(b) => Arc::new(compress(b)),
        }
    }

    /// The length of the bytes the entry names.
    fn len(&self) -> usize {
        match self {
            Object::Tile(b) | Object::Blob(b) => b.len(),
        }
    }

    fn matches_content(&self, stored: &[u8], what: &str) -> bool {
        match self {
            Object::Tile(encoded) => stored == encoded.as_slice(),
            Object::Blob(blob) => decompress(stored, blob.len(), what).is_ok_and(|actual| actual == **blob),
        }
    }
}

fn file_signature(metadata: &std::fs::Metadata) -> Option<FileSignature> {
    #[cfg(unix)]
    let unix = {
        use std::os::unix::fs::MetadataExt;
        (metadata.ino(), metadata.dev(), metadata.ctime(), metadata.ctime_nsec())
    };
    Some(FileSignature {
        len: metadata.len(),
        modified: metadata.modified().ok()?,
        #[cfg(unix)]
        unix,
    })
}

fn path_signature(path: &Path) -> Result<Option<FileSignature>> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(file_signature(&metadata)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn existing_object_is_valid(path: &Path, object: &Object, what: &str) -> Result<(bool, Option<FileSignature>)> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((false, None)),
        Err(e) => return Err(e.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Ok((false, None));
    }
    let signature = file_signature(&metadata);

    let expected_len = object.len();
    // This is the same bound used by the tile loader. It comfortably includes
    // zstd framing overhead while keeping damaged files bounded when inspected.
    let max_compressed_len = expected_len.saturating_add(expected_len / 8).saturating_add(4096);
    let max_compressed_len_u64 = u64::try_from(max_compressed_len).unwrap_or(u64::MAX);
    if metadata.len() > max_compressed_len_u64 {
        return Ok((false, None));
    }

    let mut compressed = Vec::new();
    (&mut file).take(max_compressed_len_u64.saturating_add(1)).read_to_end(&mut compressed)?;
    if compressed.len() > max_compressed_len {
        return Ok((false, None));
    }
    Ok((object.matches_content(&compressed, what), signature))
}

/// A bundle path and its compressed bytes.
type Compressed<'a> = (&'a String, Arc<Vec<u8>>);

/// Compresses `objects` on scoped worker threads (sequentially on wasm).
fn par_compress<'a>(objects: &[(&'a String, &'a Object)]) -> Result<Vec<Compressed<'a>>> {
    par_map(objects, |(p, o)| (*p, o.stored()))
}

/// `f` of every item, on scoped worker threads (sequentially on wasm), in no particular order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Result<Vec<R>> {
    let threads = if cfg!(target_arch = "wasm32") { 1 } else { std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 32) };
    if threads < 2 || items.len() < 2 {
        return Ok(items.iter().map(&f).collect());
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads.min(items.len()))
            .map(|_| {
                sc.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        out.push(f(item));
                    }
                    out
                })
            })
            .collect();
        // Join every worker before looking at the results, so one failure can't leave
        // another worker unjoined (the scope would then panic).
        let joined: Vec<_> = hs.into_iter().map(|h| h.join()).collect();
        let mut out = Vec::with_capacity(items.len());
        for r in joined {
            out.extend(r.map_err(|_| FormatError::Io(std::io::Error::other("compression worker panicked")))?);
        }
        Ok(out)
    })
}

impl KitionPhotoWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many compressed bytes of already-verified objects each directory save re-reads and
    /// re-checks (oldest-checked first; at least one object when the budget is non-zero). `0`
    /// trusts unchanged signatures entirely; `u64::MAX` re-verifies everything on every save.
    pub fn set_reverify_budget(&mut self, bytes: u64) {
        self.reverify_budget = bytes;
    }

    fn prepare(&mut self, doc: &Document, opts: &SaveOptions) -> Result<Prepared> {
        self.tile_cache.retain(|_, (w, _, _)| w.strong_count() > 0);
        convert::check_nesting(&doc.layers)?;
        // The tiles not encoded yet, encoded on every core first (zstd dominates a full save);
        // the manifest is then written from the cache.
        let mut uncached = Uncached { cache: &self.tile_cache, seen: HashSet::new(), tiles: Vec::new() };
        convert::doc_m(doc, &mut uncached);
        let fresh = par_map(&uncached.tiles, |(format, tile)| {
            let encoded = Arc::new(encode_tile(*format, tile));
            (Arc::as_ptr(tile) as usize, (Arc::downgrade(tile), hash_bytes(&encoded), encoded))
        })?;
        self.tile_cache.extend(fresh);
        let mut c = Collect { tile_cache: Some(&mut self.tile_cache), ..Default::default() };
        let document = convert::doc_m(doc, &mut c);
        let manifest = Manifest {
            format_version: FORMAT_VERSION,
            generator: format!("kition-photo-format {}", env!("CARGO_PKG_VERSION")),
            document,
            kdm: opts.kdm.as_ref().map(|_| KdmM { content: KDM.to_owned() }),
        };
        // serde_json writes NaN and the infinities as `null`, which the manifest's float fields
        // refuse on load (issue #1101): refuse the save instead, naming the value.
        crate::finite::check(&manifest).map_err(|e| match e {
            crate::finite::Problem::NonFinite { path } => FormatError::NonFinite { path },
            crate::finite::Problem::Other(m) => FormatError::Unsupported(format!("manifest: {m}")),
        })?;
        let manifest = serde_json::to_vec_pretty(&manifest)?;
        check_manifest_depth(&manifest)?;
        let stats = SaveStats { tiles_total: c.tiles.len(), blobs_total: c.blobs.len(), manifest_bytes: manifest.len(), ..Default::default() };
        let mut objects = BTreeMap::new();
        for (h, encoded) in c.tiles {
            objects.insert(tile_path(&h), Object::Tile(encoded));
        }
        for (h, b) in c.blobs {
            objects.insert(blob_path(&h), Object::Blob(b));
        }
        let mut kdm = None;
        if let Some(save) = &opts.kdm {
            // The replica names its blobs by the same hashes: a tile or blob the manifest names
            // is stored once, under that name; only what the replica alone names is added.
            for (h, bytes) in &save.blobs {
                if !is_valid_hash(h) {
                    return Err(FormatError::corrupt(format!("invalid kdm blob hash `{h}`")));
                }
                if objects.contains_key(&tile_path(h)) || objects.contains_key(&blob_path(h)) {
                    continue;
                }
                objects.insert(blob_path(h), Object::Blob(Arc::new(bytes.to_vec())));
            }
            kdm = Some(save.content.clone());
        }
        Ok(Prepared { manifest, objects, kdm, stats })
    }

    /// Save as a ZIP bundle in memory.
    pub fn save_zip(&mut self, doc: &Document, opts: &SaveOptions) -> Result<(Vec<u8>, SaveStats)> {
        let p = self.prepare(doc, opts)?;
        let mut stats = p.stats;
        let mut z = ZipWriter::new();
        z.add(MANIFEST, &p.manifest)?;
        if let Some(kdm) = &p.kdm {
            z.add_deflated(KDM, kdm)?;
        }
        let mut next = HashMap::with_capacity(p.objects.len());
        // New blobs are compressed on all cores first (tiles are encoded already).
        let todo: Vec<(&String, &Object)> = p.objects.iter().filter(|(path, _)| !self.compressed.contains_key(*path)).collect();
        let mut fresh: HashMap<&String, Arc<Vec<u8>>> = par_compress(&todo)?.into_iter().collect();
        for (path, obj) in &p.objects {
            let data = match self.compressed.get(path) {
                Some(d) => {
                    if matches!(obj, Object::Tile(..)) {
                        stats.tiles_reused += 1;
                    }
                    d.clone()
                }
                None => {
                    match obj {
                        Object::Tile(..) => stats.tiles_written += 1,
                        Object::Blob(_) => stats.blobs_written += 1,
                    }
                    fresh.remove(path).unwrap_or_else(|| obj.stored())
                }
            };
            z.add(path, &data)?;
            next.insert(path.clone(), data);
        }
        self.compressed = next;
        Ok((z.finish()?, stats))
    }

    /// Save into a directory bundle: reuses only valid existing objects,
    /// rewrites missing or damaged objects, then writes the manifest
    /// atomically and removes unreferenced objects.
    pub fn save_dir(&mut self, doc: &Document, dir: &Path, opts: &SaveOptions) -> Result<SaveStats> {
        crate::host_files::refuse(dir)?;
        let p = self.prepare(doc, opts)?;
        let mut stats = p.stats;
        for sub in ["tiles", "blobs"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        let canonical_dir = std::fs::canonicalize(dir)?;
        if self.verified_directory.as_ref().is_none_or(|cache| cache.directory != canonical_dir) {
            self.verified_directory = Some(DirectoryVerificationCache { directory: canonical_dir, ..Default::default() });
        }
        let existing = list_objects(dir)?;
        let generation = match &mut self.verified_directory {
            Some(cache) => {
                cache.generation = cache.generation.saturating_add(1);
                cache.generation
            }
            None => 0,
        };
        let recheck = self.rolling_recheck(&p.objects);
        for (path, obj) in &p.objects {
            let object_path = dir.join(path);
            let signature = path_signature(&object_path)?;
            let already_verified = !recheck.contains(path)
                && signature.is_some_and(|signature| {
                    self.verified_directory.as_ref().is_some_and(|cache| cache.objects.get(path).is_some_and(|v| v.signature == signature))
                });
            if existing.contains(path) && already_verified {
                if matches!(obj, Object::Tile(..)) {
                    stats.tiles_reused += 1;
                }
                continue;
            }
            if existing.contains(path) {
                let (valid, signature) = existing_object_is_valid(&object_path, obj, path)?;
                if valid {
                    if let Some(signature) = signature
                        && let Some(cache) = &mut self.verified_directory
                    {
                        cache.objects.insert(path.clone(), Verified { signature, checked_at: generation });
                    }
                    if matches!(obj, Object::Tile(..)) {
                        stats.tiles_reused += 1;
                    }
                    continue;
                }
            }
            match obj {
                Object::Tile(..) => stats.tiles_written += 1,
                Object::Blob(_) => stats.blobs_written += 1,
            }
            write_atomic(&object_path, &obj.stored())?;
            let written = path_signature(&object_path)?;
            if let Some(cache) = &mut self.verified_directory {
                match written {
                    Some(signature) => {
                        cache.objects.insert(path.clone(), Verified { signature, checked_at: generation });
                    }
                    None => {
                        cache.objects.remove(path);
                    }
                }
            }
        }
        match &p.kdm {
            Some(bytes) => write_atomic(&dir.join(KDM), bytes)?,
            None => {
                let _ = std::fs::remove_file(dir.join(KDM));
            }
        }
        write_atomic(&dir.join(MANIFEST), &p.manifest)?;
        for path in existing {
            if !p.objects.contains_key(&path) {
                std::fs::remove_file(dir.join(&path))?;
                stats.objects_removed += 1;
                if let Some(cache) = &mut self.verified_directory {
                    cache.objects.remove(&path);
                }
            }
        }
        Ok(stats)
    }

    /// The cached objects this save re-verifies even though their signature is unchanged:
    /// oldest-checked first, until their recorded sizes reach the re-verify budget (always at
    /// least one, so an object larger than the budget is still reached in turn).
    fn rolling_recheck(&self, objects: &BTreeMap<String, Object>) -> HashSet<String> {
        let mut out = HashSet::new();
        let Some(cache) = &self.verified_directory else { return out };
        if self.reverify_budget == 0 {
            return out;
        }
        let mut candidates: Vec<(&String, &Verified)> = cache.objects.iter().filter(|(path, _)| objects.contains_key(*path)).collect();
        candidates.sort_by(|a, b| a.1.checked_at.cmp(&b.1.checked_at).then_with(|| a.0.cmp(b.0)));
        let mut spent = 0u64;
        for (path, verified) in candidates {
            let next = spent.saturating_add(verified.signature.len);
            if !out.is_empty() && next > self.reverify_budget {
                break;
            }
            spent = next;
            out.insert(path.clone());
        }
        out
    }

    /// Save to `path`: a directory bundle if `path` is an existing directory
    /// or ends with a path separator, otherwise a ZIP file (written atomically).
    pub fn save_path(&mut self, doc: &Document, path: &Path, opts: &SaveOptions) -> Result<SaveStats> {
        crate::host_files::refuse(path)?;
        let s = path.to_string_lossy();
        if path.is_dir() || s.ends_with('/') || s.ends_with('\\') {
            self.save_dir(doc, path, opts)
        } else {
            let (bytes, stats) = self.save_zip(doc, opts)?;
            write_atomic(path, &bytes)?;
            Ok(stats)
        }
    }
}

/// The content-addressed entries of a directory bundle: `tiles/<sha256>` and
/// `blobs/<sha256>.zst` (a temporary file of an interrupted write is neither).
fn list_objects(dir: &Path) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    for (sub, suffix) in [("tiles", ""), ("blobs", ".zst")] {
        let rd = std::fs::read_dir(dir.join(sub))?;
        for e in rd {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.strip_suffix(suffix).is_some_and(is_valid_hash) {
                out.insert(format!("{sub}/{name}"));
            }
        }
    }
    Ok(out)
}

/// Crash-safe replace of one file (see [`crate::atomic`]).
pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    crate::atomic::atomic_write(path, data)?;
    Ok(())
}
