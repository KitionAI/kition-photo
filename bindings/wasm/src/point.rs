//! Basis-time picking from private holder facts and immutable raster blobs.
use crate::PhotoDocument;
use kition_kit::{Covered, CoveredAsk, CoveredAt, HistoricalBlobs, KitDocument};
use kition_photo_engine::doc::{LayerContent, LayerId, PixelFormat};
use kition_photo_geom::{TILE_SIZE, TileCoord};
use kition_photo_raster::Surface;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

/// Immutable private hit-test inputs. Compact references fit the SDK's receipt values;
/// eviction makes a historical comparison unavailable rather than guessing a hit.
#[derive(Clone, Default)]
pub(crate) struct Snapshots {
    values: BTreeMap<String, Arc<[u8]>>,
    order: VecDeque<String>,
    bytes: usize,
}

impl Snapshots {
    pub(crate) fn get(&self, hash: &str) -> Option<Arc<[u8]>> {
        self.values.get(hash).cloned()
    }

    fn put(&mut self, bytes: Vec<u8>) -> Option<Value> {
        const LIMIT: usize = 16 * 1024 * 1024;
        if bytes.len() > LIMIT {
            return None;
        }
        let hash = kition_kit::session::sha256_hex(&bytes);
        let size = bytes.len();
        if !self.values.contains_key(&hash) {
            while self.values.len() >= 2048 || self.bytes.saturating_add(size) > LIMIT {
                let oldest = self.order.pop_front()?;
                if let Some(old) = self.values.remove(&oldest) {
                    self.bytes = self.bytes.saturating_sub(old.len());
                }
            }
            self.bytes = self.bytes.saturating_add(size);
            self.order.push_back(hash.clone());
            self.values.insert(hash.clone(), bytes.into());
        }
        Some(json!({"hash":hash,"size":size}))
    }
}

pub(crate) fn fact(doc: &PhotoDocument, id: LayerId) -> Option<Value> {
    let bytes = serde_json::to_vec(&state(doc, id)?).ok()?;
    doc.pick_snapshots.lock().unwrap_or_else(std::sync::PoisonError::into_inner).put(bytes)
}

pub(crate) fn canvas_fact(doc: &PhotoDocument) -> Option<Value> {
    let st = doc.state().ok()?;
    let pixels: BTreeMap<String, Value> =
        st.doc.walk().into_iter().filter_map(|(_, _, layer)| Some((layer.id.0.to_string(), state(doc, layer.id)?.get("pixels")?.clone()))).collect();
    let bytes = serde_json::to_vec(&json!({"canvas":[st.doc.size.width,st.doc.size.height],"pixels":pixels})).ok()?;
    doc.pick_snapshots.lock().unwrap_or_else(std::sync::PoisonError::into_inner).put(bytes)
}

pub(crate) fn read_state(doc: &PhotoDocument, reference: &Value, history: &dyn HistoricalBlobs) -> Option<Value> {
    let hash = reference.get("hash")?.as_str()?;
    let bytes = doc.blob_source()?.get(hash).map(|bytes| bytes.to_vec()).or_else(|| crate::content::read_blob(reference, history))?;
    if Some(bytes.len() as u64) != reference.get("size").and_then(Value::as_u64) {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn surface(surface: &Surface, tiles: Value) -> Value {
    json!({"format":surface.format(),"default":surface.default_pixel(),"tiles":tiles})
}

pub(crate) fn state(doc: &PhotoDocument, id: LayerId) -> Option<Value> {
    let st = doc.state().ok()?;
    let (path, _, layer) = st.doc.walk().into_iter().find(|(_, _, layer)| layer.id == id)?;
    let kdm = st.kdm()?;
    let object = kdm.model().ids().object(id)?;
    let tiles = |object| kdm.replica().map_entries(object, "tiles").ok().map(|entries| Value::Object(entries.into_iter().collect()));
    let pixels = match &layer.content {
        LayerContent::Raster(s) => surface(s, tiles(object)?),
        LayerContent::Fill(_) => json!("fill"),
        LayerContent::Group(_) | LayerContent::Adjustment(_) => Value::Null,
        _ => json!("unavailable"),
    };
    let mask = if let Some(mask) = layer.mask.as_ref().filter(|mask| mask.enabled) {
        let raw = kdm.replica().get(object, "mask").ok()?;
        let object = kition_kit::kdm::ObjectId::from_string(raw.as_str()?.to_owned());
        surface(&mask.surface, tiles(&object)?)
    } else {
        Value::Null
    };
    let visible = (1..=path.len()).all(|n| path.get(..n).and_then(|p| st.doc.layer_at(p)).is_some_and(|l| l.visible));
    let board = path.first().and_then(|i| st.doc.layers.get(*i)).and_then(|l| l.artboard()).map(|a| a.rect);
    Some(json!({"path":path,"visible":visible,"locked":st.doc.locks_at(&path).all,"pixels":pixels,"mask":mask,
        "canvas":[st.doc.size.width,st.doc.size.height],
        "board":board.map(|r| [r.x0,r.y0,r.x1,r.y1]),"artboard":layer.artboard().is_some()}))
}

fn sample(doc: &PhotoDocument, value: &Value, x: i32, y: i32, alpha: bool, history: &dyn HistoricalBlobs) -> Option<f32> {
    let format: PixelFormat = serde_json::from_value(value.get("format")?.clone()).ok()?;
    let index = if alpha && format.alpha {
        format.channels().checked_sub(1)?
    } else if alpha {
        return Some(1.0);
    } else {
        0
    };
    let coord = TileCoord::containing(x, y);
    let key = format!("{},{}", coord.tx, coord.ty);
    let pixel = if let Some(blob) = value.get("tiles")?.get(&key) {
        let bytes = blob
            .get("hash")
            .and_then(Value::as_str)
            .and_then(|hash| doc.blob_source()?.get(hash))
            .filter(|bytes| bytes.len() <= 16 * 1024 * 1024 && Some(bytes.len() as u64) == blob.get("size").and_then(Value::as_u64))
            .map(|bytes| bytes.to_vec())
            .or_else(|| crate::content::read_blob(blob, history))?;
        let pixels = kition_photo_format::decode_tile(format, &bytes).ok()?;
        let offset =
            (usize::try_from(y.rem_euclid(TILE_SIZE)).ok()?.checked_mul(TILE_SIZE as usize)?.checked_add(usize::try_from(x.rem_euclid(TILE_SIZE)).ok()?)?)
                .checked_mul(format.bytes_per_pixel())?;
        kition_photo_raster::decode_pixel(&format, pixels.get(offset..offset.checked_add(format.bytes_per_pixel())?)?)
    } else {
        serde_json::from_value(value.get("default")?.clone()).ok()?
    };
    pixel.get(index).copied()
}

fn hit(doc: &PhotoDocument, value: &Value, x: i32, y: i32, history: &dyn HistoricalBlobs) -> Option<bool> {
    if !value.get("visible")?.as_bool()? || value.get("locked")?.as_bool()? {
        return Some(false);
    }
    if let Some(board) = value.get("board").filter(|v| !v.is_null())
        && !crate::content::rectangle(board)?.contains(x, y)
    {
        return Some(false);
    }
    if value.get("artboard")?.as_bool()? {
        return Some(true);
    }
    let pixels = value.get("pixels")?;
    let alpha = if pixels == "fill" {
        1.0
    } else if pixels.is_null() {
        return Some(false);
    } else {
        sample(doc, pixels, x, y, true, history)?
    };
    let mask = value.get("mask")?;
    Some(alpha * if mask.is_null() { 1.0 } else { sample(doc, mask, x, y, false, history)? } > 0.0)
}

pub(crate) fn covered(
    doc: &PhotoDocument,
    command: &str,
    params: &Value,
    objects: &[CoveredAsk],
    history: &dyn HistoricalBlobs,
) -> Result<Option<CoveredAt>, String> {
    if !crate::acts::picks_by_point(command) {
        return Ok(None);
    }
    let Some(at) = params.get("at").filter(|at| !at.is_null()) else { return Ok(None) };
    let (x, y) = crate::place::point(at)?;
    let st = doc.state()?;
    let mut states: BTreeMap<u64, Value> = st.doc.walk().into_iter().filter_map(|(_, _, l)| state(doc, l.id).map(|v| (l.id.0, v))).collect();
    for ask in objects {
        // Raw surfaces, channels and the image are not pick candidates. Their effects on
        // a layer's pixels, mask and inherited state are in that layer's pickState.
        let Some(id) = crate::context::layer_of(&ask.id) else { continue };
        if ask.created {
            states.remove(&id.0);
            continue;
        }
        if ask.unknown.iter().any(|f| f == "~pickState" || f == "$was") {
            return Ok(None);
        }
        if let Some(before) = ask.before.get("~pickState") {
            let Some(before) = read_state(doc, before, history) else { return Ok(None) };
            states.insert(id.0, before);
        } else if ask.deleted {
            return Ok(None);
        }
    }
    let mut ordered: Vec<_> = states
        .iter()
        .filter_map(|(id, value)| {
            let path: Vec<usize> = serde_json::from_value(value.get("path")?.clone()).ok()?;
            Some((path, *id, value))
        })
        .collect();
    ordered.sort_by(|a, b| b.0.cmp(&a.0));
    let current_hits = kition_photo_engine::pick_cmds::layers_at(&st.doc, x, y);
    let mut picked = None;
    for (path, id, value) in &ordered {
        let matches = if value.get("pixels") == Some(&json!("unavailable")) {
            if objects.iter().any(|o| crate::context::layer_of(&o.id) == Some(LayerId(*id)) && (!o.before.is_empty() || o.deleted || !o.unknown.is_empty())) {
                return Ok(None);
            }
            current_hits.contains(&LayerId(*id)) && !st.doc.effective_locks(LayerId(*id)).all
        } else {
            let Some(hit) = hit(doc, value, x, y, history) else { return Ok(None) };
            hit
        };
        if matches {
            picked = Some((path, *id));
            break;
        }
    }
    let covered = objects
        .iter()
        .map(|ask| {
            let id = crate::context::layer_of(&ask.id).map(|id| id.0);
            let path = id.and_then(|id| ordered.iter().find(|(_, candidate, _)| *candidate == id).map(|(path, _, _)| path));
            let matched = picked.as_ref().is_some_and(|(picked, _)| path.is_some_and(|path| picked.starts_with(path)));
            Covered { id: ask.id.clone(), covered: Some(matched), gone: ask.deleted && matched, ..Default::default() }
        })
        .collect();
    Ok(Some(CoveredAt { under: picked.map(|(_, id)| vec![json!(id)]).unwrap_or_default(), covered }))
}
