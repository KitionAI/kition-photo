//! A surface's pixels as `tiles` map entries: one content-addressed blob per 256×256 tile, keyed
//! `"x,y"`. Reusing `.kphoto`'s tile codec ([`kition_photo_format::encode_tile`]) means the bytes a
//! KDM replica hashes are the same bytes `.kphoto` would hash, so the two storage paths agree.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use kition_kit::kdm::{KdmError, ObjectId, Replica, Tx};
use kition_kit::session::{BlobRef, BlobSink, BlobSource};
use kition_photo_color::PixelFormat;
use kition_photo_geom::TileCoord;
use kition_photo_raster::{Surface, Tile};
use serde_json::Value;

use crate::kdm_err;

type KdmResult<T> = Result<T, KdmError>;

/// The mime recorded on a tile blob; informative only (the bytes are `.kphoto`'s own tile codec).
const TILE_MIME: &str = "application/x-kition-photo-tile+zstd";

fn tile_key(coord: TileCoord) -> String {
    format!("{},{}", coord.tx, coord.ty)
}

fn parse_key(key: &str) -> KdmResult<TileCoord> {
    let (tx, ty) = key.split_once(',').ok_or_else(|| KdmError::Engine(format!("bad tile key `{key}`")))?;
    Ok(TileCoord { tx: tx.parse().map_err(kdm_err)?, ty: ty.parse().map_err(kdm_err)? })
}

/// Encodes each distinct tile once, across writes: a solid fill shares one `Arc<Tile>` across
/// every tile it covers (#705), a duplicated layer shares its original's tiles, and an unchanged
/// video frame keeps its tiles, so none of them is compressed again. Keyed by the tile's address,
/// checked against a weak handle (a freed tile's address can be reused), its format, and the
/// blob still being in the store.
#[derive(Default)]
pub struct TileCache {
    map: HashMap<usize, (Weak<Tile>, PixelFormat, BlobRef)>,
    inserts: usize,
}

impl TileCache {
    pub(crate) fn blob(&mut self, format: PixelFormat, tile: &Arc<Tile>, blobs: &mut dyn BlobSink) -> BlobRef {
        let key = Arc::as_ptr(tile) as usize;
        if let Some((weak, cached_format, blob)) = self.map.get(&key)
            && *cached_format == format
            && weak.upgrade().is_some_and(|alive| Arc::ptr_eq(&alive, tile))
            && blobs.get(&blob.hash).is_some()
        {
            return blob.clone();
        }
        let encoded = kition_photo_format::encode_tile(format, tile);
        let blob = blobs.put(&encoded, TILE_MIME);
        self.map.insert(key, (Arc::downgrade(tile), format, blob.clone()));
        self.inserts += 1;
        if self.inserts >= 4096 {
            self.inserts = 0;
            self.map.retain(|_, (weak, _, _)| weak.strong_count() > 0);
        }
        blob
    }
}

/// Where a write puts blobs: the store, and the tile cache that saves re-encoding.
pub struct Sink<'a> {
    pub(crate) blobs: &'a mut dyn BlobSink,
    pub(crate) cache: &'a mut TileCache,
}

impl<'a> Sink<'a> {
    pub fn new(blobs: &'a mut dyn BlobSink, cache: &'a mut TileCache) -> Self {
        Self { blobs, cache }
    }

    pub(crate) fn tile(&mut self, format: PixelFormat, tile: &Arc<Tile>) -> KdmResult<Value> {
        serde_json::to_value(self.cache.blob(format, tile, self.blobs)).map_err(kdm_err)
    }
}

impl BlobSource for Sink<'_> {
    fn get(&self, hash: &str) -> Option<Arc<[u8]>> {
        self.blobs.get(hash)
    }
}

impl BlobSink for Sink<'_> {
    fn put(&mut self, bytes: &[u8], mime: &str) -> BlobRef {
        self.blobs.put(bytes, mime)
    }
}

/// Writes every tile of `surface` onto `owner`'s `tiles` map (full write: used by
/// [`crate::write_document`] and whenever a surface is created fresh).
pub(crate) fn write_all(tx: &mut Tx<'_>, owner: &ObjectId, surface: &Surface, blobs: &mut Sink<'_>) -> KdmResult<()> {
    for (coord, tile) in surface.tiles() {
        let blob = blobs.tile(surface.format(), tile)?;
        tx.map_set(owner, "tiles", &tile_key(*coord), blob)?;
    }
    Ok(())
}

/// Writes only the tiles that differ between `before` and `after` (pointer identity first, a
/// byte compare only when the pointers differ), plus deletes for tiles `after` dropped. Answers
/// whether it wrote anything.
pub(crate) fn write_diff(tx: &mut Tx<'_>, owner: &ObjectId, before: &Surface, after: &Surface, blobs: &mut Sink<'_>) -> KdmResult<bool> {
    let mut wrote = false;
    let format = after.format();
    for (coord, tile) in after.tiles() {
        let unchanged = before.tile(*coord).is_some_and(|prior| Arc::ptr_eq(prior, tile) || prior == tile);
        if unchanged {
            continue;
        }
        let blob = blobs.tile(format, tile)?;
        tx.map_set(owner, "tiles", &tile_key(*coord), blob)?;
        wrote = true;
    }
    for (coord, _) in before.tiles() {
        if after.tile(*coord).is_none() {
            tx.map_delete(owner, "tiles", &tile_key(*coord))?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Rebuilds a surface from `owner`'s `tiles` map (a committed replica).
/// Tiles with the same blob decode once and share one `Arc<Tile>`, as they did when written (a
/// solid fill's tiles), so a reload does not multiply their memory.
pub(crate) fn read_all(replica: &Replica, owner: &ObjectId, format: PixelFormat, default_pixel: &[f32], blobs: &dyn BlobSource) -> KdmResult<Surface> {
    let mut surface = Surface::with_default(format, default_pixel);
    let mut decoded: HashMap<String, Arc<Tile>> = HashMap::new();
    let mut placed = Vec::new();
    for (key, value) in replica.map_entries(owner, "tiles")? {
        let coord = parse_key(&key)?;
        let blob: BlobRef = serde_json::from_value(value).map_err(kdm_err)?;
        let tile = match decoded.get(&blob.hash) {
            Some(tile) => tile.clone(),
            None => {
                let bytes = blobs.get(&blob.hash).ok_or_else(|| KdmError::Engine(format!("missing tile blob {}", blob.hash)))?;
                let pixels = kition_photo_format::decode_tile(format, &bytes).map_err(kdm_err)?;
                let mut scratch = Surface::new(format);
                let origin = TileCoord { tx: 0, ty: 0 };
                let target = scratch.tile_mut(origin).bytes_mut();
                if target.len() != pixels.len() {
                    return Err(KdmError::Engine(format!("tile blob {} has the wrong size", blob.hash)));
                }
                target.copy_from_slice(&pixels);
                let tile = scratch.tile(origin).cloned().ok_or_else(|| KdmError::Engine("tile not created".into()))?;
                decoded.insert(blob.hash, tile.clone());
                tile
            }
        };
        placed.push((coord, tile));
    }
    surface.put_tiles(placed);
    Ok(surface)
}
