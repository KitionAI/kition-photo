//! A person's view of an image, kept apart from the `.kphoto` (Kit UI protocol section 7n, the
//! host's `view-state` capability): the selection, the quick mask, which groups are collapsed,
//! which notes are open, the timeline's playhead, and the last applied layer comp with the Last
//! Document State it saved. None of it is in the file or the replica, so two people who open the
//! same file and save it write the same bytes; each person's view travels through their host.
//!
//! [`view_state`] answers it as the host keeps it: `state`, a JSON object (versioned by its `v`,
//! at most [`JSON_LIMIT`] bytes), and `bytes` (at most [`BYTES_LIMIT`]): the selection's and the
//! quick mask's tiles as [`crate::encode_tile`] writes them, and the blobs of the Last Document
//! State, each once, named by its SHA-256 as in the file, with `state.objects` telling where each
//! lies. Pixels stored this way are compressed, and a selection is mostly uniform tiles, so it
//! fits; one that does not (more than 16 MB of compressed pixels) is not kept, nor the quick mask,
//! and the rest is. [`put_view_state`] puts one back on a document opened from its file; a value
//! of another version, or a damaged one, changes nothing.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use kition_photo_color::PixelFormat;
use kition_photo_doc::{Document, Layer, LayerContent, LayerId, MAX_GROUP_DEPTH};
use kition_photo_raster::Tile;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::convert::{self, Fetch, Loader, Sink};
use crate::manifest::{ChannelM, Hash, LayerCompM, SurfaceM};
use crate::store::{decompress, hash_bytes};
use crate::{FormatError, Result};

/// The version of the payload [`view_state`] writes; another is ignored (UI-S1).
pub const VERSION: u32 = 1;
/// The most JSON a host keeps (`VIEW_STATE_JSON_LIMIT`, UTF-8 bytes).
pub const JSON_LIMIT: usize = 256 * 1024;
/// The most bytes a host keeps (`VIEW_STATE_BYTES_LIMIT`).
pub const BYTES_LIMIT: usize = 16 * 1024 * 1024;

/// A person's view of an image as a host keeps it: `state` `None` when there is nothing to keep
/// (the host then deletes what it kept).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewState {
    pub state: Option<Value>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewM {
    v: u32,
    /// The groups shown collapsed in the Layers panel, by layer id (the others are expanded).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    collapsed: Vec<u64>,
    /// The notes whose popup is open: by key, or by place for a note without one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    open_notes: Vec<NoteM>,
    /// The timeline's current frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_applied_comp: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_document_state: Option<LayerCompM>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selection: Option<SurfaceM>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quick_mask: Option<ChannelM>,
    /// Where each tile and blob named above lies in the bytes: `[offset, length]`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    objects: BTreeMap<Hash, [u64; 2]>,
}

impl ViewM {
    fn is_empty(&self) -> bool {
        self.collapsed.is_empty()
            && self.open_notes.is_empty()
            && self.frame.is_none()
            && self.last_applied_comp.is_none()
            && self.last_document_state.is_none()
            && self.selection.is_none()
            && self.quick_mask.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum NoteM {
    Key(String),
    Index(usize),
}

/// The tiles (encoded) and blobs the view names, by hash.
#[derive(Default)]
struct Objects(BTreeMap<Hash, Arc<Vec<u8>>>);

impl Sink for Objects {
    fn tile(&mut self, format: PixelFormat, tile: &Arc<Tile>) -> Hash {
        let encoded = crate::encode_tile(format, tile);
        let hash = hash_bytes(&encoded);
        self.0.entry(hash.clone()).or_insert_with(|| Arc::new(encoded));
        hash
    }

    fn blob(&mut self, data: &Arc<Vec<u8>>) -> Hash {
        let hash = hash_bytes(data);
        self.0.entry(hash.clone()).or_insert_with(|| data.clone());
        hash
    }
}

fn collapsed(layers: &[Layer], out: &mut Vec<u64>, depth: usize) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for layer in layers {
        if let LayerContent::Group(group) = &layer.content {
            if !group.expanded {
                out.push(layer.id.0);
            }
            collapsed(&group.children, out, depth + 1);
        }
    }
}

fn set_expanded(layers: &mut [Layer], collapsed: &BTreeSet<u64>, depth: usize) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for layer in layers {
        let id = layer.id.0;
        if let LayerContent::Group(group) = &mut layer.content {
            group.expanded = !collapsed.contains(&id);
            set_expanded(&mut group.children, collapsed, depth + 1);
        }
    }
}

/// What to keep of the view, from the most to the least.
#[derive(Clone, Copy, PartialEq)]
enum Keep {
    All,
    NoPixels,
    Small,
}

fn build(doc: &Document, keep: Keep) -> (ViewM, Vec<u8>) {
    let mut m = ViewM { v: VERSION, ..Default::default() };
    collapsed(&doc.layers, &mut m.collapsed, 0);
    for (index, note) in doc.notes.iter().enumerate() {
        if note.open {
            m.open_notes.push(match &note.key {
                Some(key) => NoteM::Key(key.clone()),
                None => NoteM::Index(index),
            });
        }
    }
    m.frame = doc.timeline.as_ref().map(|timeline| timeline.current).filter(|frame| *frame > 0);
    m.last_applied_comp = doc.last_applied_comp;
    let mut objects = Objects::default();
    if keep != Keep::Small {
        m.last_document_state = doc.last_document_state.as_ref().map(|comp| convert::comp_m(comp, &mut objects));
    }
    if keep == Keep::All {
        m.selection = doc.selection.as_ref().map(|surface| convert::surface_m(surface, &mut objects));
        m.quick_mask = doc.quick_mask.as_ref().map(|channel| convert::channel_m(channel, &mut objects));
    }
    let mut bytes = Vec::new();
    for (hash, data) in objects.0 {
        m.objects.insert(hash, [bytes.len() as u64, data.len() as u64]);
        bytes.extend_from_slice(&data);
    }
    (m, bytes)
}

/// `doc`'s view as a host keeps it (see the module docs). The pixels are dropped when they do
/// not fit [`BYTES_LIMIT`], then the Last Document State, when the JSON does not fit
/// [`JSON_LIMIT`] or holds a value JSON cannot spell (NaN); nothing is kept when even the rest
/// does not fit.
pub fn view_state(doc: &Document) -> ViewState {
    for keep in [Keep::All, Keep::NoPixels, Keep::Small] {
        let (m, bytes) = build(doc, keep);
        if m.is_empty() {
            return ViewState::default();
        }
        if bytes.len() > BYTES_LIMIT || crate::finite::check(&m).is_err() {
            continue;
        }
        let Ok(state) = serde_json::to_value(&m) else { continue };
        let Ok(json) = serde_json::to_vec(&state) else { continue };
        if json.len() > JSON_LIMIT {
            continue;
        }
        return ViewState { state: Some(state), bytes };
    }
    ViewState::default()
}

/// Reads the view's tiles and blobs from its bytes, each checked against its hash.
struct Stored<'a> {
    objects: &'a BTreeMap<Hash, [u64; 2]>,
    bytes: &'a [u8],
}

impl Stored<'_> {
    fn get(&self, hash: &str) -> Result<&[u8]> {
        let [offset, len] = *self.objects.get(hash).ok_or_else(|| FormatError::corrupt(format!("the view state names `{hash}` but holds none")))?;
        let start = usize::try_from(offset).map_err(|_| FormatError::corrupt("view state offset out of range"))?;
        let end = usize::try_from(len).ok().and_then(|len| start.checked_add(len)).ok_or_else(|| FormatError::corrupt("view state length out of range"))?;
        let data = self.bytes.get(start..end).ok_or_else(|| FormatError::corrupt(format!("the view state's `{hash}` lies past its bytes")))?;
        if hash_bytes(data) != hash {
            return Err(FormatError::corrupt(format!("the view state's `{hash}` does not match its hash")));
        }
        Ok(data)
    }
}

impl Fetch for Stored<'_> {
    fn tile(&mut self, hash: &str, len: usize) -> Result<Vec<u8>> {
        decompress(self.get(hash)?, len, "view state tile")
    }

    fn blob(&mut self, hash: &str) -> Result<Arc<Vec<u8>>> {
        Ok(Arc::new(self.get(hash)?.to_vec()))
    }
}

/// Puts a view a host kept ([`view_state`]'s) on `doc`, an image opened from its file: every part
/// of the view is set (what the value does not name is the default: groups expanded, notes
/// closed, the first frame, no selection). A value of another version, or one that is not this
/// kit's, is ignored; a damaged one is an error and changes nothing. Layers and notes the value
/// names that the image no longer has are skipped.
pub fn put_view_state(doc: &mut Document, state: &Value, bytes: &[u8]) -> Result<()> {
    let Ok(m) = serde_json::from_value::<ViewM>(state.clone()) else { return Ok(()) };
    if m.v != VERSION {
        return Ok(());
    }
    let mut stored = Stored { objects: &m.objects, bytes };
    let ids: HashMap<u64, LayerId> = doc.walk().into_iter().map(|(_, _, layer)| (layer.id.0, layer.id)).collect();
    let mut loader = Loader { fetch: &mut stored, preserve_ids: true, max_id: 0, id_map: ids };
    // Everything is read before anything is set, so a damaged value changes nothing.
    let selection = m.selection.as_ref().map(|surface| loader.surface(surface)).transpose()?;
    let quick_mask = m.quick_mask.as_ref().map(|channel| loader.channel(channel)).transpose()?;
    let last_document_state = m.last_document_state.as_ref().map(|comp| loader.comp(comp)).transpose()?;
    let collapsed: BTreeSet<u64> = m.collapsed.iter().copied().collect();
    set_expanded(&mut doc.layers, &collapsed, 0);
    let mut keys = BTreeSet::new();
    let mut places = BTreeSet::new();
    for note in &m.open_notes {
        match note {
            NoteM::Key(key) => keys.insert(key.as_str()),
            NoteM::Index(index) => places.insert(*index),
        };
    }
    for (index, note) in doc.notes.iter_mut().enumerate() {
        note.open = match &note.key {
            Some(key) => keys.contains(key.as_str()),
            None => places.contains(&index),
        };
    }
    if let Some(timeline) = doc.timeline.as_mut() {
        timeline.current = m.frame.unwrap_or(0).min(timeline.duration.saturating_sub(1));
    }
    doc.last_applied_comp = m.last_applied_comp.filter(|id| doc.layer_comps.iter().any(|comp| comp.id == *id));
    doc.last_document_state = last_document_state;
    doc.selection = selection;
    doc.quick_mask = quick_mask;
    Ok(())
}

/// `doc` as its file keeps it: its view taken away (as [`put_view_state`] would set an empty one).
/// The handle and title are the session's; they stay.
pub fn without_view(doc: &Document) -> Document {
    let mut doc = doc.clone();
    set_expanded(&mut doc.layers, &BTreeSet::new(), 0);
    for note in &mut doc.notes {
        note.open = false;
    }
    if let Some(timeline) = doc.timeline.as_mut() {
        timeline.current = 0;
    }
    doc.last_applied_comp = None;
    doc.last_document_state = None;
    doc.selection = None;
    doc.quick_mask = None;
    doc
}
