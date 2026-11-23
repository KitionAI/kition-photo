//! Object lists (`Image.notes`, `Image.comps`, `Image.paths`, `Image.patterns`, `Layer.styles`,
//! `Layer.frames`): each item is an object of its own, so concurrent edits of different items,
//! and concurrent inserts, all survive a merge.
//!
//! An item is matched to its object by an identity: its key (a note's, a path's, a pattern's id, a
//! comp's number), or its position when it has none (`#<index>`). [`sync`] makes the list hold
//! exactly the items given, in their order, editing the objects whose items persist, creating
//! the new ones and deleting the ones gone.

use std::collections::{HashMap, HashSet};

use kition_kit::kdm::{KdmError, ObjectId, Tx};
use serde_json::Value;

type KdmResult<T> = Result<T, KdmError>;

/// Identities made unique: a repeated identity gets `@<n>` after it (its `n`th repeat), on both
/// sides alike, so two items with one key still match one object each.
pub(crate) fn unique(identities: Vec<String>) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    identities
        .into_iter()
        .map(|identity| {
            let count = seen.entry(identity.clone()).or_insert(0);
            *count += 1;
            if *count == 1 { identity } else { format!("{identity}@{count}") }
        })
        .collect()
}

/// The identity of an item that has `key` (`None`: matched by its position `index`).
pub(crate) fn identity(key: Option<String>, index: usize) -> String {
    key.filter(|key| !key.is_empty()).unwrap_or_else(|| format!("#{index}"))
}

/// The identity a stored object has: its `key_field`'s value (text, or a number as text), else
/// its position.
fn stored_identity(tx: &Tx<'_>, object: &ObjectId, key_field: Option<&str>, index: usize) -> String {
    let key = key_field.and_then(|field| match tx.get(object, field).ok()? {
        Value::String(text) => Some(text),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    });
    identity(key, index)
}

/// What [`sync`] does with one item.
pub(crate) trait Items {
    type Item;
    /// Creates the object for `item` (its containers filled too), not yet in the list.
    fn create(&mut self, tx: &mut Tx<'_>, item: &Self::Item) -> KdmResult<ObjectId>;
    /// Writes what differs between `before` (the item the object holds now) and `after`.
    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &Self::Item, after: &Self::Item) -> KdmResult<bool>;
    /// Deletes the object and what it owns.
    fn delete(&mut self, tx: &mut Tx<'_>, object: &ObjectId) -> KdmResult<()> {
        tx.delete(object)
    }
}

/// Makes `owner.field` hold `after` (the list held `before`), items matched by `identities`
/// (one per item, see [`unique`]) and stored objects by `key_field`. Answers whether it wrote.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync<I: Items>(
    tx: &mut Tx<'_>,
    owner: &ObjectId,
    field: &str,
    key_field: Option<&str>,
    before: &[I::Item],
    before_ids: &[String],
    after: &[I::Item],
    after_ids: &[String],
    items: &mut I,
) -> KdmResult<bool> {
    let mut wrote = false;
    let stored = tx.list_items(owner, field)?;
    let stored_ids = {
        let reader: &Tx<'_> = tx;
        unique(stored.iter().enumerate().map(|(index, object)| stored_identity(reader, object, key_field, index)).collect())
    };
    let wanted: HashSet<&String> = after_ids.iter().collect();
    // Gone: delete, last first so the indexes stay right.
    for index in (0..stored.len()).rev() {
        let (Some(object), Some(identity)) = (stored.get(index), stored_ids.get(index)) else { continue };
        if !wanted.contains(identity) || !tx.exists(object) {
            if tx.exists(object) {
                items.delete(tx, object)?;
            }
            tx.list_remove(owner, field, index)?;
            wrote = true;
        }
    }
    let mut objects: HashMap<String, ObjectId> = HashMap::new();
    for (object, identity) in stored.iter().zip(&stored_ids) {
        if wanted.contains(identity) && tx.exists(object) {
            objects.insert(identity.clone(), object.clone());
        }
    }
    let previous: HashMap<&String, &I::Item> = before_ids.iter().zip(before).collect();
    let mut order = Vec::with_capacity(after.len());
    for (item, identity) in after.iter().zip(after_ids) {
        let object = match (objects.get(identity), previous.get(identity)) {
            (Some(object), Some(before)) => {
                wrote |= items.update(tx, object, before, item)?;
                object.clone()
            }
            (Some(object), None) => {
                // Stored, but the view did not hold it: write it again whole.
                let object = object.clone();
                let index = tx.list_items(owner, field)?.iter().position(|o| *o == object);
                items.delete(tx, &object)?;
                if let Some(index) = index {
                    tx.list_remove(owner, field, index)?;
                }
                let made = items.create(tx, item)?;
                let len = tx.list_items(owner, field)?.len();
                tx.list_insert(owner, field, len, &made)?;
                wrote = true;
                made
            }
            (None, _) => {
                let made = items.create(tx, item)?;
                let len = tx.list_items(owner, field)?.len();
                tx.list_insert(owner, field, len, &made)?;
                wrote = true;
                made
            }
        };
        order.push(object);
    }
    // Order: move each object to its place, front to back.
    let mut current = tx.list_items(owner, field)?;
    for (target, object) in order.iter().enumerate() {
        let Some(at) = current.iter().position(|o| o == object) else {
            return Err(KdmError::Engine(format!("`{field}` lost item {object}")));
        };
        if at != target {
            tx.list_move(owner, field, at, target)?;
            let moved = current.remove(at);
            current.insert(target, moved);
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Writes `value` to `object.field` when it differs from `before`. Answers whether it wrote.
pub(crate) fn set_if(tx: &mut Tx<'_>, object: &ObjectId, field: &str, before: &Value, after: Value) -> KdmResult<bool> {
    if *before == after {
        return Ok(false);
    }
    tx.set(object, field, after)?;
    Ok(true)
}

/// Replaces the characters of the plain `text` field `object.field` that differ between `before`
/// and `after` (their common start and end are kept): concurrent edits elsewhere in the text
/// survive. Answers whether it wrote.
pub(crate) fn splice_text(tx: &mut Tx<'_>, object: &ObjectId, field: &str, before: &str, after: &str) -> KdmResult<bool> {
    let old: Vec<char> = before.chars().collect();
    let new: Vec<char> = after.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    let removed = old.len() - prefix - suffix;
    let inserted: String = new[prefix..new.len() - suffix].iter().collect();
    if removed == 0 && inserted.is_empty() {
        return Ok(false);
    }
    // The stored text may be shorter than the view thinks (a peer's delete merged in): stay inside it.
    let len = tx.text_len(object, field)?;
    let start = prefix.min(len);
    let count = removed.min(len - start);
    if count > 0 {
        tx.text_delete(object, field, start, count)?;
    }
    if !inserted.is_empty() {
        tx.text_insert(object, field, start, &inserted)?;
    }
    Ok(true)
}
