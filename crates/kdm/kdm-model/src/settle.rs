//! Canonical model values, before a batch commits or produces its receipt.
use kition_kit::kdm::{KdmError, Tx};
use kition_kit::session::MemoryBlobs;
use kition_photo_doc::{Effect, Effects, Locks};
use serde_json::{Value, json};

use crate::props::{LayerExtra, StyleSettings, SurfaceMeta};

pub fn settle(tx: &mut Tx<'_>, psd: bool) -> Result<(), KdmError> {
    let tree = tx.tree(&crate::image_id(), "layers")?;
    for entry in &tree {
        if let Some(parent) = &entry.parent {
            let owner = tree.iter().find(|candidate| candidate.node == *parent).ok_or_else(|| crate::kdm_err("layer parent is missing"))?;
            if tx.get(&owner.object, "kind")? != "group" {
                return Err(crate::kdm_err("only a group can hold child layers"));
            }
        }
    }
    for id in tx.objects_of_type("Layer") {
        if !tree.iter().any(|entry| entry.object == id) {
            return Err(crate::kdm_err("a layer must be placed in image.layers before committing"));
        }
        for (field, default) in [
            ("locks", serde_json::to_value(Locks::default()).map_err(crate::kdm_err)?),
            ("extra", serde_json::to_value(LayerExtra::default()).map_err(crate::kdm_err)?),
            ("styleSettings", serde_json::to_value(StyleSettings::of(&Effects::default(), &mut MemoryBlobs::default())).map_err(crate::kdm_err)?),
        ] {
            if tx.get(&id, field)?.is_null() {
                tx.set(&id, field, default)?;
            }
        }
        let kind = tx.get(&id, "kind")?;
        if kind != "raster" && tx.get(&id, "data")?.is_null() {
            if kind == "group" {
                tx.set(&id, "data", json!({"artboard":null}))?;
            } else {
                return Err(crate::kdm_err(format!("Layer kind {kind} requires data; no layer was created")));
            }
        }
        let extra = tx.get(&id, "extra")?;
        if let Some(mask) = extra.get("vector_mask").filter(|mask| !mask.is_null()) {
            limits(mask, "density", 0.0, 1.0)?;
            limits(mask, "feather", 0.0, f64::from(f32::MAX))?;
        }
        let mut stored: LayerExtra = serde_json::from_value(extra).map_err(crate::kdm_err)?;
        if psd && let Some(mask) = stored.vector_mask.as_mut() {
            mask.density = kition_photo_io::stored::psd_fraction(mask.density);
        }
        set_if_different(tx, &id, "extra", serde_json::to_value(stored).map_err(crate::kdm_err)?)?;
        // Stored effects use fractions, unlike the commands' percentage parameters.
        let styles = tx.list_items(&id, "styles")?;
        let mut settings = tx.get(&id, "styleSettings")?;
        if psd && styles.is_empty() {
            settings["enabled"] = json!(true);
            tx.set(&id, "styleSettings", settings.clone())?;
        }
        let settings: StyleSettings = serde_json::from_value(settings).map_err(crate::kdm_err)?;
        set_if_different(tx, &id, "styleSettings", serde_json::to_value(settings).map_err(crate::kdm_err)?)?;
        for style in styles {
            if !tx.exists(&style) {
                continue;
            }
            let effect = tx.get(&style, "effect")?;
            for value in effect.as_object().into_iter().flat_map(|object| object.values()) {
                effect_limits(value, 0)?;
            }
            let effect: Effect = serde_json::from_value(effect).map_err(crate::kdm_err)?;
            set_if_different(tx, &style, "effect", serde_json::to_value(effect).map_err(crate::kdm_err)?)?;
        }
    }
    for id in tx.objects_of_type("Surface") {
        let props = tx.get(&id, "props")?;
        if props.is_null() {
            continue;
        }
        let mut stored: SurfaceMeta = serde_json::from_value(props).map_err(crate::kdm_err)?;
        if psd {
            stored.density = kition_photo_io::stored::psd_fraction(stored.density);
        }
        set_if_different(tx, &id, "props", serde_json::to_value(stored).map_err(crate::kdm_err)?)?;
    }
    Ok(())
}

fn set_if_different(tx: &mut Tx<'_>, id: &kition_kit::kdm::ObjectId, field: &str, value: Value) -> Result<(), KdmError> {
    if tx.get(id, field)? != value {
        tx.set(id, field, value)?;
    }
    Ok(())
}

fn limits(value: &Value, member: &str, minimum: f64, maximum: f64) -> Result<(), KdmError> {
    if let Some(number) = value.get(member)
        && number.as_f64().is_none_or(|n| !n.is_finite() || n < minimum || n > maximum)
    {
        return Err(crate::kdm_err(format!("{member} must be between {minimum} and {maximum}")));
    }
    Ok(())
}

fn effect_limits(value: &Value, depth: usize) -> Result<(), KdmError> {
    if depth > 16 {
        return Err(crate::kdm_err("effect parameters are nested too deeply"));
    }
    for member in ["opacity", "spread", "noise", "jitter", "highlight_opacity", "shadow_opacity"] {
        limits(value, member, 0.0, 1.0)?;
    }
    limits(value, "range", 0.01, 1.0)?;
    for child in value.as_object().into_iter().flat_map(|object| object.values()) {
        if child.is_object() {
            effect_limits(child, depth + 1)?;
        }
    }
    Ok(())
}
