//! The image's stored resources as object lists: notes (`Image.notes`, the body a text),
//! layer comps (`Image.comps`), saved paths (`Image.paths`) and patterns (`Image.patterns`).
//! Each item is an object of its own (see [`crate::lists`]): two people editing two notes, or
//! adding a path each, both keep their edit.
//!
//! A comp names layers by their KDM object ids, resolved back through the [`IdMap`]
//! [`crate::read_document`] builds, so it still names the right layer after a reload mints that
//! layer a new local id.

use kition_kit::kdm::{KdmError, ObjectId, Replica, Tx};
use kition_kit::session::{BlobRef, BlobSource, IdMap};
use kition_photo_doc::{Color, CompAppearance, CompLayerState, Document, LayerComp, LayerId, NamedPath, Note, Path, Pattern};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::doc_state::{EmbeddedSurface, embed_surface, extract_surface};
use crate::lists::{self, Items, identity, set_if, splice_text, unique};
use crate::props::{self, EffectsData};
use crate::tiles::Sink;
use crate::{image_id, kdm_err};

type KdmResult<T> = Result<T, KdmError>;

fn fields(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn string(replica: &Replica, object: &ObjectId, field: &str) -> KdmResult<String> {
    Ok(replica.get(object, field)?.as_str().unwrap_or_default().to_owned())
}

fn parsed<T: serde::de::DeserializeOwned + Default>(replica: &Replica, object: &ObjectId, field: &str) -> KdmResult<T> {
    let value = replica.get(object, field)?;
    if value.is_null() { Ok(T::default()) } else { serde_json::from_value(value).map_err(kdm_err) }
}

// ---------- notes ----------

/// A note's register fields (its body is the `text` container).
fn note_fields(note: &Note) -> KdmResult<Map<String, Value>> {
    Ok(fields(json!({
        "key": note.key,
        "author": note.author,
        "color": serde_json::to_value(note.color).map_err(kdm_err)?,
        "position": note.position,
        "popup": note.popup,
        "modified": note.modified,
    })))
}

struct Notes;

impl Items for Notes {
    type Item = Note;

    fn create(&mut self, tx: &mut Tx<'_>, note: &Note) -> KdmResult<ObjectId> {
        let object = tx.create("Note", note_fields(note)?)?;
        if !note.text.is_empty() {
            tx.text_insert(&object, "text", 0, &note.text)?;
        }
        Ok(object)
    }

    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &Note, after: &Note) -> KdmResult<bool> {
        let (old, new) = (note_fields(before)?, note_fields(after)?);
        let mut wrote = false;
        for (field, value) in new {
            if field != "key" {
                wrote |= set_if(tx, object, &field, old.get(&field).unwrap_or(&Value::Null), value)?;
            }
        }
        wrote |= splice_text(tx, object, "text", &before.text, &after.text)?;
        Ok(wrote)
    }
}

fn read_note(replica: &Replica, object: &ObjectId) -> KdmResult<Note> {
    let mut note = Note { key: replica.get(object, "key")?.as_str().map(str::to_owned), ..Note::default() };
    note.author = string(replica, object, "author")?;
    let color = replica.get(object, "color")?;
    if !color.is_null() {
        note.color = serde_json::from_value::<Color>(color).map_err(kdm_err)?;
    }
    note.text = replica.text(object, "text")?;
    note.position = parsed(replica, object, "position")?;
    note.popup = parsed(replica, object, "popup")?;
    note.modified = string(replica, object, "modified")?;
    // Whether its popup is open is each person's own view.
    note.open = false;
    Ok(note)
}

// ---------- layer comps ----------

#[derive(Serialize, Deserialize)]
struct StoredAppearance {
    blend: kition_photo_doc::BlendMode,
    opacity: f32,
    fill_opacity: f32,
    effects: EffectsData,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct StoredLayerState {
    /// The layer's KDM object id, or `""` if it could not be resolved when this was written.
    layer: String,
    visible: Option<bool>,
    position: Option<(i32, i32)>,
    appearance: Option<StoredAppearance>,
}

fn comp_fields(comp: &LayerComp, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<Map<String, Value>> {
    let mut states = Vec::with_capacity(comp.states.len());
    for state in &comp.states {
        let appearance = state.appearance.as_ref().map(|appearance| StoredAppearance {
            blend: appearance.blend,
            opacity: appearance.opacity,
            fill_opacity: appearance.fill_opacity,
            effects: EffectsData::from_effects(&appearance.effects, blobs),
        });
        states.push(StoredLayerState {
            layer: ids.object(state.layer).map(ObjectId::as_str).unwrap_or_default().to_owned(),
            visible: state.visible,
            position: state.position,
            appearance,
        });
    }
    Ok(fields(json!({
        "number": comp.id,
        "name": comp.name,
        "comment": comp.comment,
        "applyVisibility": comp.apply_visibility,
        "applyPosition": comp.apply_position,
        "applyAppearance": comp.apply_appearance,
        "states": serde_json::to_value(states).map_err(kdm_err)?,
    })))
}

struct Comps<'a, 'b> {
    ids: &'a IdMap<LayerId>,
    blobs: &'a mut Sink<'b>,
}

impl Items for Comps<'_, '_> {
    type Item = LayerComp;

    fn create(&mut self, tx: &mut Tx<'_>, comp: &LayerComp) -> KdmResult<ObjectId> {
        tx.create("Comp", comp_fields(comp, self.ids, self.blobs)?)
    }

    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &LayerComp, after: &LayerComp) -> KdmResult<bool> {
        let (old, new) = (comp_fields(before, self.ids, self.blobs)?, comp_fields(after, self.ids, self.blobs)?);
        let mut wrote = false;
        for (field, value) in new {
            if field != "number" {
                wrote |= set_if(tx, object, &field, old.get(&field).unwrap_or(&Value::Null), value)?;
            }
        }
        Ok(wrote)
    }
}

fn read_comp(replica: &Replica, object: &ObjectId, ids: &IdMap<LayerId>, blobs: &dyn BlobSource) -> KdmResult<LayerComp> {
    let stored: Vec<StoredLayerState> = parsed(replica, object, "states")?;
    let mut states = Vec::with_capacity(stored.len());
    for state in stored {
        // A layer this comp recorded that no longer exists (deleted since): drop it, like
        // `LayerComp::missing_layers` already treats a comp whose layer disappeared.
        let Some(layer) = ids.local(&ObjectId::from_string(state.layer)) else { continue };
        let appearance = match state.appearance {
            Some(appearance) => Some(CompAppearance {
                blend: appearance.blend,
                opacity: appearance.opacity,
                fill_opacity: appearance.fill_opacity,
                effects: appearance.effects.into_effects(blobs)?,
            }),
            None => None,
        };
        states.push(CompLayerState { layer, visible: state.visible, position: state.position, appearance });
    }
    let flag = |field: &str| -> KdmResult<bool> { Ok(replica.get(object, field)?.as_bool().unwrap_or(false)) };
    Ok(LayerComp {
        id: replica.get(object, "number")?.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or_default(),
        name: string(replica, object, "name")?,
        comment: string(replica, object, "comment")?,
        apply_visibility: flag("applyVisibility")?,
        apply_position: flag("applyPosition")?,
        apply_appearance: flag("applyAppearance")?,
        states,
    })
}

// ---------- saved paths ----------

fn path_fields(path: &NamedPath, blobs: &mut Sink<'_>) -> KdmResult<Map<String, Value>> {
    Ok(fields(json!({
        "key": path.key,
        "name": path.name,
        "path": serde_json::to_value(&path.path).map_err(kdm_err)?,
        "psdRaw": path.psd_raw.as_ref().map(|data| props::embed(blobs, data)),
    })))
}

struct Paths<'a, 'b> {
    blobs: &'a mut Sink<'b>,
}

impl Items for Paths<'_, '_> {
    type Item = NamedPath;

    fn create(&mut self, tx: &mut Tx<'_>, path: &NamedPath) -> KdmResult<ObjectId> {
        tx.create("Path", path_fields(path, self.blobs)?)
    }

    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &NamedPath, after: &NamedPath) -> KdmResult<bool> {
        if before == after {
            return Ok(false);
        }
        let (old, new) = (path_fields(before, self.blobs)?, path_fields(after, self.blobs)?);
        let mut wrote = false;
        for (field, value) in new {
            if field != "key" {
                wrote |= set_if(tx, object, &field, old.get(&field).unwrap_or(&Value::Null), value)?;
            }
        }
        Ok(wrote)
    }
}

fn read_path(replica: &Replica, object: &ObjectId, blobs: &dyn BlobSource) -> KdmResult<NamedPath> {
    let psd_raw = replica.get(object, "psdRaw")?;
    Ok(NamedPath {
        key: replica.get(object, "key")?.as_str().map(str::to_owned),
        name: string(replica, object, "name")?,
        path: parsed::<Path>(replica, object, "path")?,
        psd_raw: if psd_raw.is_null() {
            None
        } else {
            let blob: BlobRef = serde_json::from_value(psd_raw).map_err(kdm_err)?;
            Some(std::sync::Arc::new(props::fetch(blobs, &blob)?.to_vec()))
        },
    })
}

// ---------- patterns ----------

fn pattern_fields(pattern: &Pattern, blobs: &mut Sink<'_>) -> KdmResult<Map<String, Value>> {
    Ok(fields(json!({
        "key": pattern.id,
        "name": pattern.name,
        "width": pattern.width,
        "height": pattern.height,
        "pixels": serde_json::to_value(embed_surface(&pattern.surface, blobs)?).map_err(kdm_err)?,
    })))
}

struct Patterns<'a, 'b> {
    blobs: &'a mut Sink<'b>,
}

impl Items for Patterns<'_, '_> {
    type Item = Pattern;

    fn create(&mut self, tx: &mut Tx<'_>, pattern: &Pattern) -> KdmResult<ObjectId> {
        tx.create("Pattern", pattern_fields(pattern, self.blobs)?)
    }

    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &Pattern, after: &Pattern) -> KdmResult<bool> {
        if before == after {
            return Ok(false);
        }
        let (old, new) = (pattern_fields(before, self.blobs)?, pattern_fields(after, self.blobs)?);
        let mut wrote = false;
        for (field, value) in new {
            if field != "key" {
                wrote |= set_if(tx, object, &field, old.get(&field).unwrap_or(&Value::Null), value)?;
            }
        }
        Ok(wrote)
    }
}

fn read_pattern(replica: &Replica, object: &ObjectId, blobs: &dyn BlobSource) -> KdmResult<Pattern> {
    let pixels: EmbeddedSurface = serde_json::from_value(replica.get(object, "pixels")?).map_err(kdm_err)?;
    let side = |field: &str| -> KdmResult<u32> { Ok(replica.get(object, field)?.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(1)) };
    Ok(Pattern {
        id: string(replica, object, "key")?,
        name: string(replica, object, "name")?,
        width: side("width")?,
        height: side("height")?,
        surface: extract_surface(&pixels, blobs)?,
    })
}

// ---------- the four lists ----------

fn note_ids(notes: &[Note]) -> Vec<String> {
    unique(notes.iter().enumerate().map(|(index, note)| identity(note.key.clone(), index)).collect())
}

fn comp_ids(comps: &[LayerComp]) -> Vec<String> {
    unique(comps.iter().map(|comp| comp.id.to_string()).collect())
}

fn path_ids(paths: &[NamedPath]) -> Vec<String> {
    unique(paths.iter().enumerate().map(|(index, path)| identity(path.key.clone(), index)).collect())
}

fn pattern_ids(patterns: &[Pattern]) -> Vec<String> {
    unique(patterns.iter().enumerate().map(|(index, pattern)| identity(Some(pattern.id.clone()), index)).collect())
}

/// The four lists of one document.
struct Lists<'a> {
    notes: &'a [Note],
    comps: &'a [LayerComp],
    paths: &'a [NamedPath],
    patterns: &'a [Pattern],
}

impl<'a> Lists<'a> {
    fn of(document: &'a Document) -> Self {
        Self { notes: &document.notes, comps: &document.layer_comps, paths: &document.paths, patterns: &document.patterns }
    }
}

/// Writes every resource of a new document. Needs `ids` fully built: comps name layers.
pub(crate) fn write_all(tx: &mut Tx<'_>, document: &Document, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<()> {
    let none = Lists { notes: &[], comps: &[], paths: &[], patterns: &[] };
    sync_all(tx, &none, &Lists::of(document), ids, blobs).map(|_| ())
}

/// Writes what differs between `before`'s resources and `after`'s. Answers whether it wrote.
pub(crate) fn write_changes(tx: &mut Tx<'_>, before: &Document, after: &Document, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<bool> {
    sync_all(tx, &Lists::of(before), &Lists::of(after), ids, blobs)
}

fn sync_all(tx: &mut Tx<'_>, before: &Lists<'_>, after: &Lists<'_>, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<bool> {
    let image = image_id();
    let mut wrote = false;
    if before.notes != after.notes {
        wrote |= lists::sync(tx, &image, "notes", Some("key"), before.notes, &note_ids(before.notes), after.notes, &note_ids(after.notes), &mut Notes)?;
    }
    // Comps name layers through `ids`, which the same edit can change (a new layer, a delete):
    // they always take the full comparison when there are any.
    if !before.comps.is_empty() || !after.comps.is_empty() {
        wrote |= lists::sync(
            tx,
            &image,
            "comps",
            Some("number"),
            before.comps,
            &comp_ids(before.comps),
            after.comps,
            &comp_ids(after.comps),
            &mut Comps { ids, blobs: &mut *blobs },
        )?;
    }
    if before.paths != after.paths {
        wrote |= lists::sync(
            tx,
            &image,
            "paths",
            Some("key"),
            before.paths,
            &path_ids(before.paths),
            after.paths,
            &path_ids(after.paths),
            &mut Paths { blobs: &mut *blobs },
        )?;
    }
    if before.patterns != after.patterns {
        wrote |= lists::sync(
            tx,
            &image,
            "patterns",
            Some("key"),
            before.patterns,
            &pattern_ids(before.patterns),
            after.patterns,
            &pattern_ids(after.patterns),
            &mut Patterns { blobs: &mut *blobs },
        )?;
    }
    Ok(wrote)
}

/// Reads the four lists onto `document`. After `ids`: a comp names layers by object id.
pub(crate) fn read(replica: &Replica, document: &mut Document, ids: &IdMap<LayerId>, blobs: &dyn BlobSource) -> KdmResult<()> {
    let image = image_id();
    let live = |field: &str| -> KdmResult<Vec<ObjectId>> { Ok(replica.list(&image, field)?.into_iter().filter(|object| replica.exists(object)).collect()) };
    document.notes = live("notes")?.iter().map(|object| read_note(replica, object)).collect::<KdmResult<_>>()?;
    document.layer_comps = live("comps")?.iter().map(|object| read_comp(replica, object, ids, blobs)).collect::<KdmResult<_>>()?;
    document.paths = live("paths")?.iter().map(|object| read_path(replica, object, blobs)).collect::<KdmResult<_>>()?;
    document.patterns = live("patterns")?.iter().map(|object| read_pattern(replica, object, blobs)).collect::<KdmResult<_>>()?;
    Ok(())
}
