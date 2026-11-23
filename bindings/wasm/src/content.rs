//! Historical pixel bytes and canvas clipping, scoped to a call's bounded destination.

use kition_kit::{ContentAsk, HistoricalBlobs};
use kition_photo_geom::{Rect, TILE_SIZE, TileCoord};
use kition_photo_raster::Surface;
use serde_json::Value;

use crate::PhotoDocument;

pub(crate) fn rectangle(value: &Value) -> Option<Rect> {
    let a = value.as_array()?;
    if a.len() != 4 {
        return None;
    }
    Some(Rect::new(
        i32::try_from(a.first()?.as_i64()?).ok()?,
        i32::try_from(a.get(1)?.as_i64()?).ok()?,
        i32::try_from(a.get(2)?.as_i64()?).ok()?,
        i32::try_from(a.get(3)?.as_i64()?).ok()?,
    ))
}

pub(crate) fn read_blob(value: &Value, history: &dyn HistoricalBlobs) -> Option<Vec<u8>> {
    let hash = value.get("hash")?.as_str()?;
    let size = usize::try_from(value.get("size")?.as_u64()?).ok()?;
    if size > 16 * 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::with_capacity(size);
    while bytes.len() < size {
        let length = (size - bytes.len()).min(1024 * 1024);
        let chunk = history.read(hash, bytes.len(), length).ok()?;
        if chunk.len() != length {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    Some(bytes)
}

fn pixels_changed(surface: &Surface, before: &Value, area: Rect, history: &dyn HistoricalBlobs) -> Option<bool> {
    let before = before.as_object()?;
    if area.is_empty() {
        return Some(false);
    }
    let from = TileCoord::containing(area.x0, area.y0);
    let to = TileCoord::containing(area.x1.checked_sub(1)?, area.y1.checked_sub(1)?);
    let count = (i64::from(to.tx) - i64::from(from.tx) + 1).checked_mul(i64::from(to.ty) - i64::from(from.ty) + 1)?;
    if count > 16_384 {
        return None;
    }
    let bpp = surface.format().bytes_per_pixel();
    for ty in from.ty..=to.ty {
        for tx in from.tx..=to.tx {
            let coord = TileCoord { tx, ty };
            let old = match before.get(&format!("{tx},{ty}")) {
                Some(blob) if !blob.is_null() => Some(kition_photo_format::decode_tile(surface.format(), &read_blob(blob, history)?).ok()?),
                _ => None,
            };
            let now = surface.tile(coord).map(|tile| tile.bytes());
            let x0 = tx.checked_mul(TILE_SIZE)?;
            let y0 = ty.checked_mul(TILE_SIZE)?;
            let clip = area.intersect(&Rect::from_xywh(x0, y0, TILE_SIZE as u32, TILE_SIZE as u32));
            for y in clip.y0..clip.y1 {
                for x in clip.x0..clip.x1 {
                    let offset =
                        (usize::try_from(y.checked_sub(y0)?).ok()?.checked_mul(TILE_SIZE as usize)?.checked_add(usize::try_from(x.checked_sub(x0)?).ok()?)?)
                            .checked_mul(bpp)?;
                    let range = offset..offset.checked_add(bpp)?;
                    let old = match old.as_ref() {
                        Some(bytes) => bytes.get(range.clone())?,
                        None => surface.default_bytes(),
                    };
                    let now = match now {
                        Some(bytes) => bytes.get(range)?,
                        None => surface.default_bytes(),
                    };
                    if old != now {
                        return Some(true);
                    }
                }
            }
        }
    }
    Some(false)
}

pub(crate) fn changed(doc: &PhotoDocument, ask: &ContentAsk, history: &dyn HistoricalBlobs) -> Result<Option<bool>, String> {
    let state = doc.state()?;
    if ask.premise.id == "image" && ask.premise.field == "~canvasCoordinates" {
        let Some(id) = crate::context::layer_of(&ask.premise.region) else { return Ok(None) };
        let Some(now) = crate::point::state(doc, id) else { return Ok(None) };
        let Some(before) = crate::point::read_state(doc, &ask.before, history) else { return Ok(None) };
        let (Some(before_canvas), Some(before_pixels)) = (before.get("canvas"), before.get("pixels").and_then(|pixels| pixels.get(id.0.to_string()))) else {
            return Ok(None);
        };
        return Ok(Some(Some(before_canvas) != now.get("canvas") && Some(before_pixels) != now.get("pixels")));
    }
    let Some(area) = rectangle(&ask.premise.region) else { return Ok(None) };
    if ask.premise.id == "image" {
        let (lo, hi, size) = match ask.premise.field.as_str() {
            "width" => (area.x0, area.x1, state.doc.size.width),
            "height" => (area.y0, area.y1, state.doc.size.height),
            _ => return Ok(None),
        };
        let Some(old) = ask.before.as_i64() else { return Ok(None) };
        // Both the empty intersection and the clipped endpoint matter.
        let clip = |size: i64| {
            let lo = i64::from(lo).max(0);
            let hi = i64::from(hi).min(size);
            (lo < hi).then_some((lo, hi))
        };
        return Ok(Some(clip(old) != clip(i64::from(size))));
    }
    let Some(id) = crate::context::layer_of(&ask.premise.id) else { return Ok(None) };
    let Some(surface) = state.doc.layer(id).and_then(|layer| layer.surface()) else { return Ok(None) };
    Ok(if ask.premise.field == "tiles" { pixels_changed(surface, &ask.before, area, history) } else { None })
}
