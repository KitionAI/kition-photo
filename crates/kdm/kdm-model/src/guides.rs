//! `Image.guides`: a map from each guide's key to `{ axis, position, order }`, so guides added,
//! moved and deleted by different people all survive a merge. A guide without a key (one made
//! where no random key could be drawn) is keyed by its place, `#<axis><index>`, and reads back
//! without one.
//!
//! `order` places a guide among its axis's guides: a write keeps the orders already stored where
//! they still sort the guides as they are, so a delete or an insert rewrites no other guide.

use std::collections::HashMap;

use kition_kit::kdm::{KdmError, ObjectId, Replica, Tx};
use kition_photo_doc::{Guide, Guides};
use serde_json::{Value, json};

type KdmResult<T> = Result<T, KdmError>;

const FIELD: &str = "guides";
const AXES: [&str; 2] = ["horizontal", "vertical"];

fn key_of(guide: &Guide, axis: &str, index: usize) -> String {
    match guide.key.as_deref().filter(|key| !key.is_empty() && !key.starts_with('#')) {
        Some(key) => key.to_owned(),
        None => format!("#{}{index}", &axis[..1]),
    }
}

fn axis_list<'a>(guides: &'a Guides, axis: &str) -> &'a [Guide] {
    if axis == "horizontal" { &guides.horizontal } else { &guides.vertical }
}

fn value(axis: &str, position: f32, order: f64) -> Value {
    json!({ "axis": axis, "position": position, "order": order })
}

/// Orders for `keys` (one axis, in order): a stored order is kept while it is above the last one
/// kept; the others are spread between their kept neighbours.
fn orders(keys: &[String], stored: &HashMap<String, f64>) -> Vec<f64> {
    let mut kept: Vec<Option<f64>> = Vec::with_capacity(keys.len());
    let mut last = f64::NEG_INFINITY;
    for key in keys {
        match stored.get(key).copied().filter(|order| order.is_finite() && *order > last) {
            Some(order) => {
                kept.push(Some(order));
                last = order;
            }
            None => kept.push(None),
        }
    }
    let mut out = Vec::with_capacity(keys.len());
    let mut index = 0usize;
    while index < kept.len() {
        if let Some(order) = kept.get(index).copied().flatten() {
            out.push(order);
            index += 1;
            continue;
        }
        let start = index;
        while kept.get(index).is_some_and(Option::is_none) {
            index += 1;
        }
        let low = out.last().copied();
        let high = kept.get(index).copied().flatten();
        let count = (index - start) as f64;
        let (from, step) = match (low, high) {
            (Some(low), Some(high)) => (low, (high - low) / (count + 1.0)),
            (Some(low), None) => (low, 1.0),
            (None, Some(high)) => (high - count - 1.0, 1.0),
            (None, None) => (-1.0, 1.0),
        };
        for n in 1..=(index - start) {
            out.push(from + step * n as f64);
        }
    }
    out
}

/// Whether two stored values say the same (numbers compared as numbers: a stored `0.0` may read
/// back as `0`).
fn same(a: &Value, b: &Value) -> bool {
    a.get("axis") == b.get("axis") && ["position", "order"].iter().all(|field| a.get(*field).and_then(Value::as_f64) == b.get(*field).and_then(Value::as_f64))
}

/// Writes every guide (a new document).
pub(crate) fn write_all(tx: &mut Tx<'_>, image: &ObjectId, guides: &Guides) -> KdmResult<()> {
    for axis in AXES {
        for (index, guide) in axis_list(guides, axis).iter().enumerate() {
            tx.map_set(image, FIELD, &key_of(guide, axis, index), value(axis, guide.position, index as f64))?;
        }
    }
    Ok(())
}

/// Makes the stored guides `after`. Answers whether it wrote.
pub(crate) fn write_diff(tx: &mut Tx<'_>, image: &ObjectId, after: &Guides) -> KdmResult<bool> {
    let stored: HashMap<String, Value> = tx.map_entries(image, FIELD)?.into_iter().collect();
    let mut wanted: HashMap<String, Value> = HashMap::new();
    for axis in AXES {
        let list = axis_list(after, axis);
        let keys: Vec<String> = list.iter().enumerate().map(|(index, guide)| key_of(guide, axis, index)).collect();
        let stored_orders: HashMap<String, f64> = keys
            .iter()
            .filter_map(|key| {
                let entry = stored.get(key)?;
                (entry.get("axis").and_then(Value::as_str) == Some(axis))
                    .then(|| entry.get("order").and_then(Value::as_f64))
                    .flatten()
                    .map(|o| (key.clone(), o))
            })
            .collect();
        for ((key, guide), order) in keys.iter().zip(list).zip(orders(&keys, &stored_orders)) {
            wanted.insert(key.clone(), value(axis, guide.position, order));
        }
    }
    let mut wrote = false;
    let mut gone: Vec<&String> = stored.keys().filter(|key| !wanted.contains_key(*key)).collect();
    gone.sort();
    for key in gone {
        tx.map_delete(image, FIELD, key)?;
        wrote = true;
    }
    let mut keys: Vec<&String> = wanted.keys().collect();
    keys.sort();
    for key in keys {
        let Some(want) = wanted.get(key) else { continue };
        if !stored.get(key).is_some_and(|have| same(have, want)) {
            tx.map_set(image, FIELD, key, want.clone())?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// The guides stored, each axis in order.
pub(crate) fn read(replica: &Replica, image: &ObjectId) -> KdmResult<Guides> {
    let mut guides = Guides::default();
    let mut entries: Vec<(String, f64, String, f32)> = Vec::new();
    for (key, entry) in replica.map_entries(image, FIELD)? {
        let Some(axis) = entry.get("axis").and_then(Value::as_str) else { continue };
        if !AXES.contains(&axis) {
            continue;
        }
        let Some(position) = entry.get("position").and_then(Value::as_f64) else { continue };
        let order = entry.get("order").and_then(Value::as_f64).unwrap_or(0.0);
        entries.push((axis.to_owned(), order, key, position as f32));
    }
    entries.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.2.cmp(&b.2)));
    for (axis, _, key, position) in entries {
        let guide = Guide { key: (!key.starts_with('#')).then_some(key), position };
        if axis == "horizontal" {
            guides.horizontal.push(guide);
        } else {
            guides.vertical.push(guide);
        }
    }
    Ok(guides)
}
