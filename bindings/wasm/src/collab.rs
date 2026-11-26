//! Kition's collaboration members (engine contract 2, Kition phase 3a Task 4): the document as
//! a KDM replica.
//!
//! The engine keeps its own `Session` (lazy replica creation, blob GC, per-person undo on the
//! replica), not a `kition_kit::session::KitSession`, so `kition_kit::export_kit_session!` does
//! not apply here; these members are added by hand with the exact JS names and shapes
//! `kition-sdk`'s `session::web` gives (`crates/kition-kit/src/session/web.rs`), so Kition's
//! host code is the same for every kit. The wasm bindings live in [`crate::web`].

use std::collections::HashMap;
use std::sync::Arc;

use kition_kit::kdm::Schema;
use kition_kit::kdm::tool_ids::ToolIds;
use kition_kit::session::KitModel;
use kition_kit::session::model::{Exchange, ReplicaPort};
use kition_kit::{KitCommand, KitDocument};
use kition_photo_engine::kdm_model::PhotoModel;
use kition_photo_engine::{Damage, MemoryBlobs, Origin, UndoScope, Version};
use serde_json::{Value, json};

use crate::PhotoDocument;

impl PhotoDocument {
    /// Opens a file with the replica a host stored for it (its history; the Kit UI protocol's
    /// `replica` capability): the document is the replica's, so it keeps its KDM identity across
    /// saves. For a format that cannot carry a replica (PSD) the host vouches that the replica
    /// was saved with these bytes (their stamps match). A `.kphoto` holds the document's content
    /// (its content snapshot, no history): when it is the same document, the file is taken into
    /// the stored replica (`kition_kit::session::take_file`: nothing, the changes it holds beyond
    /// the replica, or its state by object id when it was saved elsewhere). A replica that does
    /// not open (damaged, another schema) is ignored and the file opens as [`KitDocument::open`]
    /// opens it.
    pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        let restored = decode_blobs(blobs).and_then(|mut blobs| {
            let mut session = crate::new_session();
            // The new document is the active one, as after `open`, and is saved to `name`.
            let file = kition_photo_engine::kition_photo_bridge::embedded_bundle(bytes).ok().flatten();
            match file {
                Some((content, file_blobs)) => {
                    for (hash, data) in file_blobs.0 {
                        blobs.0.entry(hash).or_insert(data);
                    }
                    session.open_stored_with_file(snapshot, &content, blobs, name).map_err(|error| error.to_string())?;
                }
                None => {
                    session.open_snapshot(snapshot, blobs, name).map_err(|error| error.to_string())?;
                }
            }
            if let Some(state) = session.active_mut() {
                state.path = Some(name.to_owned());
            }
            Ok(session)
        });
        match restored {
            Ok(session) => Ok(Self::saved_as(session, name)),
            Err(_) => Self::open(name, bytes),
        }
    }

    /// Whether the document has made its replica yet: it makes one lazily, on the first shared
    /// edit or when asked for it ([`PhotoDocument::version`] and the rest).
    pub fn has_replica(&self) -> bool {
        self.state().is_ok_and(|state| state.kdm().is_some())
    }

    /// The active document's replica version (empty before it has made one).
    pub fn version(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.session.version().map_err(|error| error.to_string())?.encode())
    }

    /// The active document's changes since `since` (empty bytes: every change).
    pub fn export_changes(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        let since = if since.is_empty() { Version::empty() } else { Version::decode(since).map_err(|error| error.to_string())? };
        self.session.export_changes(&since).map_err(|error| error.to_string())
    }

    /// Imports a peer's (or stored) changes; add their blobs first with
    /// [`PhotoDocument::add_blobs`]. Answers what went stale on screen.
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<Damage, String> {
        self.session.import_changes(bytes).map_err(|error| error.to_string())
    }

    /// The active document's replica as a snapshot, for a peer to open as a new replica of the
    /// same document ([`PhotoDocument::open`\]'ing the saved bytes does the same for a `.kphoto`).
    pub fn snapshot(&mut self) -> Result<Vec<u8>, String> {
        self.session.snapshot().map_err(|error| error.to_string())
    }

    /// Undoes the active document's last local step on its replica. `done` is `false`, not an
    /// error, when there is nothing to undo.
    pub fn undo(&mut self) -> Result<(bool, Damage), String> {
        let done = self.session.undo();
        Ok((done, if done { self.step_damage()? } else { Damage::None }))
    }

    pub fn redo(&mut self) -> Result<(bool, Damage), String> {
        let done = self.session.redo();
        Ok((done, if done { self.step_damage()? } else { Damage::None }))
    }

    /// The damage of the step [`PhotoDocument::undo`] or [`PhotoDocument::redo`] just took:
    /// `last_damage`'s rect, or everything changed when it did not track one exactly.
    fn step_damage(&self) -> Result<Damage, String> {
        let state = self.state()?;
        Ok(match state.last_damage {
            None => Damage::Full,
            Some(rect) => Damage::Rects(vec![[rect.x0, rect.y0, rect.x1, rect.y1]]),
        })
    }

    pub fn can_undo(&self) -> bool {
        self.session.is_enabled("edit.undo")
    }

    pub fn can_redo(&self) -> bool {
        self.session.is_enabled("edit.redo")
    }

    /// Runs a command as `origin` (a person, an agent): the replica records it as theirs, and it
    /// undoes as that replica's own step. Contract 1's [`KitDocument::execute`](kition_kit::KitDocument::execute)
    /// is this with the local person.
    pub fn execute_as(&mut self, command: &str, params: Value, origin: &Origin) -> Result<Value, String> {
        self.run(command, params, Some(origin))
    }

    /// Takes an agent's draft (its changes since it was made from [`PhotoDocument::snapshot`]) in
    /// as one undo step of the person who accepts it (`mergeChanges`, as `export_kit_session!`
    /// answers it; KitModel M12a: the person's step even for an agent's `origin`, which is what the
    /// host passes): the image follows, the person's one Undo takes the whole draft back and keeps
    /// what they did meanwhile, and the History panel lists the step as "Merge Draft". Edits on
    /// different tiles of one layer both stay; one tile painted on both sides is refused, naming
    /// the layer and the tile. Add the draft's blobs first ([`PhotoDocument::add_blobs`]). Answers
    /// `{"damage", "summary", "scope"}`: `summary` what the merge and its repair changed
    /// (`StateDiff`, read only at the objects they wrote, `Replica::diff_since`), `scope` the undo
    /// scope of its step (`"person"`, `null` when it changed nothing). A refused merge changes
    /// nothing and fails with the KDM's message.
    pub fn merge_changes(&mut self, bytes: &[u8], origin: &Origin) -> Result<Value, String> {
        let since = self.session.version().map_err(|error| error.to_string())?;
        let scope = self.session.merge_changes(bytes, origin).map_err(|error| error.to_string())?;
        let replica = self.state()?.kdm().ok_or("the document left its replica")?.replica();
        let summary = replica.diff_since(&since).map_err(|error| error.to_string())?;
        let damage = if replica.version() == since { Damage::None } else { Damage::Full };
        Ok(json!({ "damage": damage, "summary": summary.to_json(), "scope": scope.map(UndoScope::as_str) }))
    }

    /// What merging a draft's changes would change in the image now, as `StateDiff` JSON, the
    /// repair included as [`PhotoDocument::merge_changes`] runs it (the draft's preview is what
    /// accepting it gives); nothing changes. Refuses what the merge refuses.
    pub fn compare_changes(&self, bytes: &[u8]) -> Result<Value, String> {
        let kdm = self.state()?.kdm().ok_or("the image has no replica, so no draft of it")?;
        Ok(kdm.compare_changes(bytes).map_err(|error| error.to_string())?.to_json())
    }

    /// Writes the state a saved version holds (`snapshot`: a KDM snapshot or this document's
    /// content, as a host's checkpoint keeps it; the Kit UI protocol's `replica/restore`) over the
    /// image as one undo step of `origin`'s scope, labelled "Restore Version", whose Undo gives back
    /// the exact image before. Nothing changes when the image already holds it. Add the snapshot's
    /// blobs first. Another document's snapshot is refused. Answers the damage.
    pub fn restore_state(&mut self, snapshot: &[u8], origin: &Origin) -> Result<Damage, String> {
        let since = self.session.version().map_err(|error| error.to_string())?;
        self.session.restore_state(snapshot, origin).map_err(|error| error.to_string())?;
        let changed = self.session.version().map_err(|error| error.to_string())? != since;
        Ok(if changed { Damage::Full } else { Damage::None })
    }

    /// What changed since the version `since` (empty bytes: since the replica began), a receipt as
    /// `export_kit_session!`'s `changesSince` answers it: `{"changes": StateDiff}`, or `{"changes":
    /// null, "unavailable": "<why>"}` when the replica's log does not hold `since` (or the image has
    /// no replica yet), never an empty change set in its place.
    pub fn changes_since(&self, since: &[u8]) -> Value {
        let Some(kdm) = self.state().ok().and_then(|state| state.kdm()) else {
            return json!({ "changes": null, "unavailable": "the image has no replica yet" });
        };
        let replica = kdm.replica();
        let since = if since.is_empty() { Ok(Version::empty()) } else { Version::decode(since) };
        match since.and_then(|since| replica.changes_since(&since)) {
            Ok(changes) => json!({ "changes": changes.to_recorded_json(replica.schema()) }),
            Err(error) => json!({ "changes": null, "unavailable": error.to_string() }),
        }
    }

    /// An agent's reply (JSON text) with every object id it holds as a value or a key named as
    /// the tool interface names it (a layer's number, KitObjectModel OM9); unchanged without a
    /// replica or when it names no object. Only the objects the reply names are read
    /// (`ToolIds::show_reply`), never the whole image's tool ids: the editor sends every
    /// `ai/inspect`, `document/check` and `ai/run` reply through it.
    pub fn model_reply(&self, value: &str) -> Result<String, String> {
        let value: Value = serde_json::from_str(value).map_err(|error| format!("the reply is not JSON: {error}"))?;
        let Some(kdm) = self.state().ok().and_then(|state| state.kdm()) else { return Ok(value.to_string()) };
        Ok(ToolIds::show_reply(kdm.replica(), &value).to_string())
    }

    /// `model/describe` (KitObjectModel 1.0) as JSON, through the change exchange the headless kit
    /// serves it with. A failure's message is the error's JSON (`{"code", "index", "message"}`).
    pub fn model_describe(&mut self) -> Result<Value, String> {
        Exchange::new().describe(self).map_err(|failure| failure.to_json().to_string())
    }

    /// `model/batch`: the request and the answer as JSON; a failure's message is the error's JSON
    /// (`{"code", "index", "message"}`).
    pub fn model_batch(&mut self, request: &str) -> Result<Value, String> {
        let request: Value = serde_json::from_str(request)
            .map_err(|failure| json!({ "code": "invalid-op", "index": null, "message": format!("the batch is not JSON: {failure}") }).to_string())?;
        Exchange::new().batch(self, &request, false).map_err(|failure| failure.to_json().to_string())
    }

    /// Every blob the active document's replica references or can still bring back, as one
    /// buffer a peer feeds straight into [`PhotoDocument::add_blobs`].
    pub fn blobs(&mut self) -> Result<Vec<u8>, String> {
        let blobs = self.session.blobs().map_err(|error| error.to_string())?;
        Ok(encode_blobs(blobs))
    }

    /// Adds a peer's blobs (from their [`PhotoDocument::blobs`]): kept until the next
    /// [`PhotoDocument::import_changes`], which a host calls right after.
    pub fn add_blobs(&mut self, bytes: &[u8]) -> Result<(), String> {
        let blobs = decode_blobs(bytes)?;
        self.session.add_blobs(&blobs).map_err(|error| error.to_string())
    }

    /// The blobs the active document's replica has newly needed since `since` (empty bytes:
    /// every blob the current state references), encoded the same way as
    /// [`PhotoDocument::blobs`]: for live sync, so one change ships only the blobs it needs
    /// rather than the whole store.
    pub fn blobs_since(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        let since = if since.is_empty() { Version::empty() } else { Version::decode(since).map_err(|error| error.to_string())? };
        let blobs = self.session.blobs_since(&since).map_err(|error| error.to_string())?;
        Ok(encode_blobs(&blobs))
    }

    /// Each layer's KDM object id by its local id (as `inspect()` lists layers), as JSON
    /// `{ "<layer>": "<objectId>" }`: the same object id on every replica of the document, so a
    /// lease on a layer (`layer:<objectId>`, Kit UI protocol section 7b) means the same layer to
    /// everyone. Empty before the document has made a replica.
    pub fn layer_object_ids(&self) -> Value {
        let mut out = serde_json::Map::new();
        let Ok(state) = self.state() else { return Value::Object(out) };
        let Some(kdm) = state.kdm() else { return Value::Object(out) };
        let ids = kdm.model().ids();
        for (_, _, layer) in state.doc.walk() {
            if let Some(object) = ids.object(layer.id) {
                out.insert(layer.id.0.to_string(), Value::String(object.to_string()));
            }
        }
        Value::Object(out)
    }

    /// The active document's KDM identity, `None` before it has made a replica (it makes one
    /// lazily, on the first shared edit or when asked for one of its other collaboration
    /// members; this one never forces that).
    pub fn doc_id(&self) -> Option<String> {
        self.session.doc_id()
    }

    /// How many times the document left its replica because it refused a write or an undo (a
    /// document it cannot hold, or a bug). The document keeps working, undoing from its own
    /// history; the next shared edit makes a fresh replica, which collaborators must resync
    /// (its full [`PhotoDocument::snapshot`]). Hosts watch this.
    pub fn kdm_resets(&self) -> u32 {
        self.state().map_or(0, |state| u32::try_from(state.kdm_resets()).unwrap_or(u32::MAX))
    }
}

/// The document's replica for a headless kit (`kition_headless::Headless::with_replica_port`):
/// the object model over its change exchange, the replica methods with the blobs their changes
/// reference, and commands as the caller runs them (KHP A11).
impl ReplicaPort for PhotoDocument {
    fn model_schema(&self) -> Arc<Schema> {
        <PhotoModel as KitModel>::schema()
    }

    fn model_export(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        self.export_changes(since)
    }

    fn model_import(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.import_changes(bytes).map(|_| ())
    }

    fn model_add_blobs(&mut self, blobs: &[u8]) -> Result<(), String> {
        self.add_blobs(blobs)
    }

    fn model_blobs_since(&mut self, since: &[u8]) -> Result<Vec<u8>, String> {
        self.blobs_since(since)
    }

    fn model_command(&mut self, command: &str, params: Value, origin: &Origin) -> Result<Value, String> {
        self.execute_as(command, params, origin)
    }

    fn model_commands(&self) -> Vec<KitCommand> {
        self.commands()
    }

    /// A sync room's document from its snapshot and blobs: strictly, so a replica that does not
    /// open is an error, never an empty image (a server checks changes against what this opens).
    fn model_open_replica(name: &str, snapshot: &[u8], blobs: &[u8]) -> Result<Self, String> {
        let blobs = decode_blobs(blobs)?;
        let mut session = crate::new_session();
        session.open_snapshot(snapshot, blobs, name).map_err(|error| error.to_string())?;
        if let Some(state) = session.active_mut() {
            state.path = Some(name.to_owned());
        }
        Ok(Self::saved_as(session, name))
    }
}

/// Encodes blobs for one wasm call's worth of blob transfer
/// ([`PhotoDocument::blobs`]/[`PhotoDocument::add_blobs`]): `(64-byte ASCII-hex hash, u64 LE
/// length, bytes) * N`. Every hash this crate produces is a 64-char hex SHA-256, so the hash
/// field needs no length prefix of its own.
fn encode_blobs(blobs: &MemoryBlobs) -> Vec<u8> {
    let mut out = Vec::new();
    for (hash, data) in &blobs.0 {
        out.extend_from_slice(hash.as_bytes());
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

/// Reverses [`encode_blobs`]; truncated or malformed input is an error, never a panic.
fn decode_blobs(bytes: &[u8]) -> Result<MemoryBlobs, String> {
    let mut map = HashMap::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let hash_end = i.checked_add(64).ok_or("blob bundle: overflow")?;
        let hash = bytes.get(i..hash_end).ok_or("truncated blob bundle: hash")?;
        let hash = std::str::from_utf8(hash).map_err(|_| "blob bundle: hash is not ASCII".to_owned())?.to_owned();
        let len_end = hash_end.checked_add(8).ok_or("blob bundle: overflow")?;
        let len_bytes: [u8; 8] =
            bytes.get(hash_end..len_end).ok_or("truncated blob bundle: length")?.try_into().map_err(|_| "blob bundle: length".to_owned())?;
        let len = usize::try_from(u64::from_le_bytes(len_bytes)).map_err(|_| "blob bundle: length too large".to_owned())?;
        let data_end = len_end.checked_add(len).ok_or("blob bundle: overflow")?;
        let data = bytes.get(len_end..data_end).ok_or("truncated blob bundle: data")?;
        map.insert(hash, Arc::from(data));
        i = data_end;
    }
    Ok(MemoryBlobs(map))
}

#[cfg(test)]
mod tests {
    use kition_kit::KitDocument;
    use serde_json::json;

    use super::*;

    fn fixture() -> Vec<u8> {
        let mut doc = PhotoDocument::create("new.psd", 48, 32).unwrap();
        doc.execute("layer.new.layer", json!({"name": "Ink"})).unwrap();
        kition_photo_io::export(&doc.state().unwrap().doc, "fixture.psd", &kition_photo_io::ExportOptions::default()).unwrap().bytes
    }

    fn stroke(doc: &mut PhotoDocument) {
        doc.execute("paint.stroke", json!({"points": [[4, 4, 1], [20, 10, 1]], "size": 4, "color": "#ff0000"})).unwrap();
    }

    #[test]
    fn version_and_snapshot_force_a_replica_lazily() {
        let mut doc = PhotoDocument::open("fixture.psd", &fixture()).unwrap();
        assert!(!doc.has_replica(), "opening a PSD makes no replica");
        let version = doc.version().unwrap();
        assert!(doc.has_replica());
        assert_eq!(doc.version().unwrap(), version, "asking twice makes no further replica");
        let snap = doc.snapshot().unwrap();
        assert!(!snap.is_empty());
    }

    #[test]
    fn undo_redo_report_done_and_damage() {
        let mut doc = PhotoDocument::open("fixture.psd", &fixture()).unwrap();
        assert!(!doc.can_undo());
        stroke(&mut doc);
        assert!(doc.can_undo());
        let (done, damage) = doc.undo().unwrap();
        assert!(done);
        assert_ne!(damage, Damage::None);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        let (done, _) = doc.redo().unwrap();
        assert!(done);
        // Undoing with nothing to undo is a normal `false`, not an error.
        doc.undo().unwrap();
        let (done, damage) = doc.undo().unwrap();
        assert!(!done);
        assert_eq!(damage, Damage::None);
    }

    #[test]
    fn execute_as_records_the_given_origin() {
        let mut doc = PhotoDocument::open("fixture.psd", &fixture()).unwrap();
        let origin = Origin::person("user:agent-1");
        doc.execute_as("paint.stroke", json!({"points": [[1, 1, 1], [10, 10, 1]], "size": 3, "color": "#00ff00"}), &origin).unwrap();
        assert!(doc.can_undo());
    }

    /// Every replica of a document names each layer by the same object id, whatever local id its
    /// own view gave the layer: what a layer lease's scope is built from.
    #[test]
    fn layer_object_ids_are_the_same_on_every_replica() {
        let mut a = PhotoDocument::open("fixture.psd", &fixture()).unwrap();
        assert_eq!(a.layer_object_ids(), json!({}), "no replica, no object ids");
        a.version().unwrap();
        let ids_a = a.layer_object_ids();
        let ids_a = ids_a.as_object().unwrap();
        assert_eq!(ids_a.len(), 2, "the Background and Ink: {ids_a:?}");
        let mut b = PhotoDocument::open_with_replica("fixture.psd", &fixture(), &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
        let ids_b = b.layer_object_ids();
        let objects = |ids: &serde_json::Map<String, Value>| {
            let mut found: Vec<String> = ids.values().map(|value| value.as_str().unwrap().to_owned()).collect();
            found.sort();
            found
        };
        assert_eq!(objects(ids_a), objects(ids_b.as_object().unwrap()));
        // The active layer (Ink) has the same object id on both.
        let active = |doc: &PhotoDocument| doc.inspect().unwrap()["activeLayer"].as_u64().unwrap().to_string();
        assert_eq!(ids_a[&active(&a)], b.layer_object_ids()[&active(&b)]);
        stroke(&mut b);
        assert_eq!(objects(ids_a), objects(b.layer_object_ids().as_object().unwrap()), "painting keeps the layer's object");
    }

    #[test]
    fn blobs_round_trip_through_encode_and_decode() {
        let mut a = MemoryBlobs::default();
        a.0.insert("a".repeat(64), Arc::from(b"hello".as_slice()));
        a.0.insert("b".repeat(64), Arc::from(b"".as_slice()));
        let bytes = encode_blobs(&a);
        let b = decode_blobs(&bytes).unwrap();
        assert_eq!(b.0.len(), 2);
        assert_eq!(b.0.get(&"a".repeat(64)).map(|v| v.as_ref()), Some(b"hello".as_ref()));
        assert_eq!(b.0.get(&"b".repeat(64)).map(|v| v.as_ref()), Some(b"".as_ref()));
    }

    #[test]
    fn decode_blobs_rejects_truncated_input_without_panicking() {
        assert!(decode_blobs(&[1, 2, 3]).is_err());
        assert!(decode_blobs(&"a".repeat(64).into_bytes()).is_err(), "hash with no length");
        let mut malformed = "a".repeat(64).into_bytes();
        malformed.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_blobs(&malformed).is_err(), "a length claiming more data than exists");
    }

    /// A PSD saved with its replica reopens as that replica: same version, same state, the same
    /// pixels, not dirty, and it takes new edits (which undo) at once.
    #[test]
    fn a_psd_reopens_with_its_stored_replica() {
        let mut a = PhotoDocument::open("art.psd", &fixture()).unwrap();
        stroke(&mut a);
        let bytes = a.save(None).unwrap();
        let (snapshot, blobs) = (a.snapshot().unwrap(), a.blobs().unwrap());
        let version = a.version().unwrap();

        let mut b = PhotoDocument::open_with_replica("art.psd", &bytes, &snapshot, &blobs).unwrap();
        assert!(b.has_replica());
        assert_eq!(b.version().unwrap(), version);
        assert_eq!(b.session.state_hash().unwrap(), a.session.state_hash().unwrap());
        assert_eq!(b.render(0.0, 0.0, 1.0, 48, 32).unwrap(), a.render(0.0, 0.0, 1.0, 48, 32).unwrap());
        assert!(!b.dirty());
        assert!(!b.can_undo());
        stroke(&mut b);
        assert!(b.dirty());
        assert_ne!(b.version().unwrap(), version);
        assert!(b.undo().unwrap().0);
        assert_eq!(b.save(None).unwrap()[..4], *b"8BPS", "it still saves as PSD");
    }

    /// A damaged replica, or blobs that are damaged or missing, never stop the file from opening:
    /// it opens plainly, never as a blank image.
    #[test]
    fn a_damaged_stored_replica_opens_the_file_plainly() {
        let bytes = fixture();
        let mut a = PhotoDocument::open("art.psd", &bytes).unwrap();
        let snapshot = a.snapshot().unwrap();
        for (snapshot, blobs) in [(&b"not a snapshot"[..], &[][..]), (&snapshot[..], &[1u8, 2, 3][..]), (&snapshot[..], &[][..])] {
            let doc = PhotoDocument::open_with_replica("art.psd", &bytes, snapshot, blobs).unwrap();
            assert_eq!((doc.width(), doc.height()), (48, 32));
            assert!(!doc.has_replica(), "the file opened plainly");
        }
        assert!(PhotoDocument::open_with_replica("art.psd", b"not a psd", b"not a snapshot", &[]).is_err());
    }

    /// B opens A's `.kphoto` with the replica A's host stored for it (`openWithReplica`), so both
    /// are on one line of changes. A paints some more; `B.importChanges(A.exportChanges(B.version()))`,
    /// with the blobs the change needs transferred through [`PhotoDocument::blobs`] /
    /// [`PhotoDocument::add_blobs`], brings B to A's state.
    #[test]
    fn a_document_takes_changes_through_export_and_import_changes() {
        let mut a = PhotoDocument::create("shared.kphoto", 48, 32).unwrap();
        a.execute("layer.new.layer", json!({"name": "Ink"})).unwrap();
        stroke(&mut a);
        let bytes = a.save(None).unwrap();
        assert!(!a.version().unwrap().is_empty(), "painting made a replica");

        let mut b = PhotoDocument::open_with_replica("shared.kphoto", &bytes, &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
        assert_eq!(b.version().unwrap(), a.version().unwrap(), "b starts from a's own replica");

        a.execute("paint.stroke", json!({"points": [[30, 5, 1], [40, 25, 1]], "size": 5, "color": "#0000ff"})).unwrap();

        let since = b.version().unwrap();
        let changes = a.export_changes(&since).unwrap();
        b.add_blobs(&a.blobs().unwrap()).unwrap();
        b.import_changes(&changes).unwrap();

        assert_eq!(a.session.state_hash().unwrap(), b.session.state_hash().unwrap());
        assert_eq!(a.version().unwrap(), b.version().unwrap());
    }

    /// `blobsSince` ships only a change's own blobs (fewer than the whole `blobs()` store, which
    /// also holds undo history), and that alone is enough for the document behind to take them.
    #[test]
    fn blobs_since_ships_only_what_the_changes_need() {
        let mut a = PhotoDocument::create("shared.kphoto", 48, 32).unwrap();
        a.execute("layer.new.layer", json!({"name": "Ink"})).unwrap();
        stroke(&mut a);
        let bytes = a.save(None).unwrap();

        let mut b = PhotoDocument::open_with_replica("shared.kphoto", &bytes, &a.snapshot().unwrap(), &a.blobs_since(&[]).unwrap()).unwrap();
        assert_eq!(b.version().unwrap(), a.version().unwrap());

        for (x, color) in [(5, "#ff0000"), (10, "#00ff00"), (15, "#0000ff")] {
            a.execute("paint.stroke", json!({"points": [[x, 5, 1], [x + 2, 8, 1]], "size": 3, "color": color})).unwrap();
        }

        let since = b.version().unwrap();
        let changes = a.export_changes(&since).unwrap();
        let new_blobs = a.blobs_since(&since).unwrap();
        assert!(!new_blobs.is_empty(), "the changes need at least their own tiles");
        assert!(new_blobs.len() < a.blobs().unwrap().len(), "fewer bytes than the whole blob store");
        b.add_blobs(&new_blobs).unwrap();
        b.import_changes(&changes).unwrap();

        assert_eq!(a.session.state_hash().unwrap(), b.session.state_hash().unwrap());
        assert_eq!(a.version().unwrap(), b.version().unwrap());
    }

    /// `blobsSince` with an empty `since` (every blob the current state references) is on its
    /// own enough to open a peer from a snapshot, without `exportChanges` first.
    #[test]
    fn blobs_since_empty_opens_a_fresh_peer_from_a_snapshot() {
        let mut a = PhotoDocument::create("shared.kphoto", 48, 32).unwrap();
        a.execute("layer.new.layer", json!({"name": "Ink"})).unwrap();
        stroke(&mut a);
        let snapshot = a.snapshot().unwrap();
        let blobs = a.blobs_since(&[]).unwrap();

        let b = PhotoDocument::open_with_replica("peer.kphoto", &a.save(None).unwrap(), &snapshot, &blobs).unwrap();
        assert!(b.has_replica());
        assert_eq!(a.render(0.0, 0.0, 1.0, 48, 32).unwrap(), b.render(0.0, 0.0, 1.0, 48, 32).unwrap());
    }

    /// `docId` is `None` before a replica exists (and asking does not make one), then the
    /// replica's own identity once there is one, shared by a peer opened from its snapshot.
    #[test]
    fn doc_id_is_none_until_a_replica_exists_then_matches_a_peer() {
        let mut a = PhotoDocument::open("fixture.psd", &fixture()).unwrap();
        assert!(a.doc_id().is_none(), "no replica yet");
        assert!(!a.has_replica(), "doc_id did not force one");

        stroke(&mut a);
        let id = a.doc_id();
        assert!(id.is_some());
        assert_eq!(a.doc_id(), id, "stable across further calls");

        let b = PhotoDocument::open_with_replica("fixture.psd", &fixture(), &a.snapshot().unwrap(), &a.blobs().unwrap()).unwrap();
        assert_eq!(b.doc_id(), id, "a peer of the same KDM document shares its identity");
    }

    // The draft and restore members (engine contract K3a): `mergeChanges`, `compareChanges`,
    // `restoreState`, `changesSince`, `modelReply`, `modelDescribe`, `modelBatch`.

    use kition_kit::kdm::{ActorId, ObjectId, Replica};

    fn agent() -> Origin {
        Origin::agent("agent:test")
    }

    fn hash(doc: &mut PhotoDocument) -> String {
        doc.session.state_hash().unwrap()
    }

    fn history(doc: &PhotoDocument) -> Vec<String> {
        doc.inspect().unwrap()["history"].as_array().unwrap().iter().map(|label| label.as_str().unwrap().to_owned()).collect()
    }

    /// The colour the image shows at `(x, y)`: `'r'`, `'g'` or `'b'` where one channel leads,
    /// `'w'` for white, else `'?'`.
    fn pixel(doc: &PhotoDocument, x: i32, y: i32) -> char {
        let p = doc.render_area(x, y, x + 1, y + 1, 1, 1).unwrap();
        match (p[0] > 200, p[1] > 200, p[2] > 200) {
            (true, true, true) => 'w',
            (true, false, false) => 'r',
            (false, true, false) => 'g',
            (false, false, true) => 'b',
            _ => '?',
        }
    }

    /// An image wider than one 256-pixel tile with a layer "Ink" on top of a white background.
    fn wide() -> PhotoDocument {
        let mut doc = PhotoDocument::create("art.kphoto", 600, 300).unwrap();
        doc.execute("layer.new.layer", json!({"name": "Ink"})).unwrap();
        doc
    }

    /// An agent's draft of `doc` as it is now (a document opened from its snapshot, as the editor
    /// makes one), and the version it was made at.
    fn draft_of(doc: &mut PhotoDocument) -> (PhotoDocument, Vec<u8>) {
        let since = doc.version().unwrap();
        let draft = <PhotoDocument as ReplicaPort>::model_open_replica("art.kphoto", &doc.snapshot().unwrap(), &doc.blobs().unwrap()).unwrap();
        (draft, since)
    }

    /// The draft's changes since `since`, its blobs added to `doc` first.
    fn changes_of(doc: &mut PhotoDocument, draft: &mut PhotoDocument, since: &[u8]) -> Vec<u8> {
        doc.add_blobs(&draft.blobs_since(since).unwrap()).unwrap();
        draft.export_changes(since).unwrap()
    }

    fn paint(doc: &mut PhotoDocument, origin: &Origin, from: [i32; 2], color: &str) {
        let [x, y] = from;
        doc.execute_as("paint.stroke", json!({"points": [[x, y, 1], [x + 40, y + 40, 1]], "size": 8, "color": color}), origin).unwrap();
    }

    /// The person paints one tile of Ink while the agent's draft paints another tile of the same
    /// layer; the person accepts the draft: both strokes show, the merge is one step of the
    /// person's ("Merge Draft"), the agent's undo does not take it, and one Undo gives back the
    /// exact image before (the person's stroke stays), one Redo the merge.
    #[test]
    fn merge_changes_keeps_two_tiles_of_one_layer_as_one_step_of_the_person() {
        let mut doc = wide();
        let (mut draft, since) = draft_of(&mut doc);
        paint(&mut draft, &agent(), [400, 200], "#0000ff");
        paint(&mut doc, &kition_photo_engine::kdm_bridge::local_person(), [20, 20], "#ff0000");
        let (before, before_pixels, before_history) = (hash(&mut doc), doc.render(0.0, 0.0, 1.0, 600, 300).unwrap(), history(&doc));
        let changes = changes_of(&mut doc, &mut draft, &since);
        let preview = doc.compare_changes(&changes).unwrap();

        let answer = doc.merge_changes(&changes, &agent()).unwrap();
        assert_eq!(answer["scope"], "person", "{answer}");
        assert_eq!(answer["summary"], preview, "the preview is what the merge changed");
        assert_eq!(pixel(&doc, 420, 220), 'b', "the draft's stroke");
        assert_eq!(pixel(&doc, 40, 40), 'r', "the person's stroke");
        assert_eq!(history(&doc).last().map(String::as_str), Some("Merge Draft"));
        assert_eq!(history(&doc).len(), before_history.len() + 1);
        assert!(!doc.session.undo_in(Some(UndoScope::Agent)), "the agent's undo does not take the person's merge");
        let after = hash(&mut doc);

        assert!(doc.undo().unwrap().0);
        assert_eq!(hash(&mut doc), before, "one Undo gives back the exact state before the merge");
        assert_eq!(doc.render(0.0, 0.0, 1.0, 600, 300).unwrap(), before_pixels, "and the image");
        assert_eq!(history(&doc), before_history);
        assert!(doc.redo().unwrap().0);
        assert_eq!(hash(&mut doc), after, "one Redo the merge");
        assert_eq!(pixel(&doc, 420, 220), 'b');
    }

    /// One tile painted in the draft and by the person with different pixels: the merge is refused,
    /// naming the layer and the tile, and changes nothing.
    #[test]
    fn merge_changes_refuses_one_tile_painted_on_both_sides_naming_the_layer_and_tile() {
        let mut doc = wide();
        let (mut draft, since) = draft_of(&mut doc);
        paint(&mut draft, &agent(), [20, 20], "#0000ff");
        paint(&mut doc, &kition_photo_engine::kdm_bridge::local_person(), [30, 30], "#ff0000");
        let (before, version, labels) = (hash(&mut doc), doc.version().unwrap(), history(&doc));
        let changes = changes_of(&mut doc, &mut draft, &since);
        let ink = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap().to_string();
        let object = doc.layer_object_ids()[&ink].as_str().unwrap().to_owned();

        let refused = doc.merge_changes(&changes, &agent()).unwrap_err();
        assert!(refused.contains(&format!("{object}.tiles[\"0,0\"]")), "the refusal names the layer and the tile: {refused}");
        assert!(doc.compare_changes(&changes).is_err(), "the preview refuses what the merge refuses");
        assert_eq!(hash(&mut doc), before, "a refused merge changes nothing");
        assert_eq!(doc.version().unwrap(), version);
        assert_eq!(history(&doc), labels);
        assert_eq!(pixel(&doc, 50, 50), 'r', "the person's stroke stays");
    }

    /// The object id of the mask of the layer whose engine id is `layer`.
    fn mask_of(doc: &PhotoDocument, layer: u64) -> Option<String> {
        let object = doc.layer_object_ids()[&layer.to_string()].as_str()?.to_owned();
        let replica = doc.state().unwrap().kdm().unwrap().replica();
        replica.get(&ObjectId::from_string(object), "mask").unwrap().as_str().map(str::to_owned)
    }

    /// A draft written straight to a replica of `doc` (as the object model writes one), by the agent.
    fn raw_draft(doc: &mut PhotoDocument, write: impl FnOnce(&mut kition_kit::kdm::Tx<'_>) -> Result<(), kition_kit::kdm::KdmError>) -> Vec<u8> {
        let mut replica = Replica::load(PhotoModel::schema(), &doc.snapshot().unwrap(), ActorId::random()).unwrap();
        let since = replica.version();
        replica.transact(&agent(), write).unwrap();
        replica.export_changes(&since).unwrap()
    }

    /// The draft deletes a mask's surface and leaves the layer naming it: the repair inside the
    /// merge clears the layer's mask; the preview (`compareChanges`, and the merged state) is what
    /// the merge gives, the image reads a layer without a mask, and one Undo gives back the mask.
    #[test]
    fn the_preview_and_the_merge_run_the_repair_in_one_step() {
        let mut doc = wide();
        doc.execute("layer.layerMask.revealAll", json!({})).unwrap();
        let ink = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap();
        let surface = mask_of(&doc, ink).expect("Ink has a mask");
        let changes = raw_draft(&mut doc, |tx| tx.delete(&ObjectId::from_string(surface.clone())));
        let before = hash(&mut doc);
        let preview = doc.compare_changes(&changes).unwrap();
        let merged = doc.session.merged_state(&changes).unwrap();

        let answer = doc.merge_changes(&changes, &agent()).unwrap();
        assert_eq!(answer["scope"], "person");
        assert_eq!(mask_of(&doc, ink), None, "the repair cleared the layer's mask inside the merge");
        assert_eq!(merged, doc.state().unwrap().kdm().unwrap().replica().state_json().unwrap(), "the preview's state is the merged one");
        assert_eq!(answer["summary"], preview);
        assert!(doc.inspect().unwrap()["layers"].to_string().contains("Ink"));
        let after = hash(&mut doc);
        assert!(doc.undo().unwrap().0);
        assert_eq!(hash(&mut doc), before, "one Undo gives back the exact state, mask and all");
        assert_eq!(mask_of(&doc, ink).as_deref(), Some(surface.as_str()));
        assert!(doc.redo().unwrap().0);
        assert_eq!(hash(&mut doc), after);
    }

    /// The person deletes a layer's mask (a named step); the agent's draft gave another layer that
    /// mask meanwhile: merged, the repair clears the reference again, so the merge changes nothing.
    /// Nothing is recorded: the state, the version, the replica's undo depth and the history stay,
    /// and the next Undo takes back the person's delete, exactly.
    #[test]
    fn a_merge_the_repair_cancels_records_nothing() {
        let mut doc = wide();
        let ink = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap();
        doc.execute("layer.layerMask.revealAll", json!({})).unwrap();
        doc.execute("layer.new.layer", json!({"name": "Other"})).unwrap();
        let other = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap();
        let surface = mask_of(&doc, ink).expect("Ink has a mask");
        let other_object = doc.layer_object_ids()[&other.to_string()].as_str().unwrap().to_owned();
        let changes = raw_draft(&mut doc, |tx| tx.set(&ObjectId::from_string(other_object.clone()), "mask", json!(surface)));
        let before_delete = hash(&mut doc);
        doc.execute("layer.layerMask.delete", json!({"layer": ink})).unwrap();
        assert_eq!(mask_of(&doc, ink), None);
        let (before, version, labels) = (hash(&mut doc), doc.version().unwrap(), history(&doc));
        let depth = doc.state().unwrap().kdm().unwrap().replica().undo_depth(UndoScope::Person);

        let answer = doc.merge_changes(&changes, &agent()).unwrap();
        assert_eq!(answer["scope"], Value::Null, "no step: {answer}");
        assert_eq!(answer["damage"], json!(Damage::None));
        assert_eq!(hash(&mut doc), before);
        assert_eq!(doc.version().unwrap(), version, "the version stays");
        assert_eq!(doc.state().unwrap().kdm().unwrap().replica().undo_depth(UndoScope::Person), depth);
        assert_eq!(history(&doc), labels, "the history has no step for it");
        assert_eq!(mask_of(&doc, other), None);
        assert!(doc.undo().unwrap().0);
        assert_eq!(hash(&mut doc), before_delete, "the next Undo takes back the person's delete");
        assert_eq!(mask_of(&doc, ink).as_deref(), Some(surface.as_str()));
    }

    /// A version restored over a later edit is one step ("Restore Version") whose Undo gives back
    /// the exact state before and whose Redo the restored one; the file's content restores the same
    /// way; restoring what the image already holds records nothing.
    #[test]
    fn restore_state_is_one_exact_undo_step() {
        let mut doc = wide();
        paint(&mut doc, &kition_photo_engine::kdm_bridge::local_person(), [20, 20], "#ff0000");
        let (saved, saved_hash) = (doc.snapshot().unwrap(), hash(&mut doc));
        let content = kition_photo_format::read_kdm_content(&doc.save(None).unwrap()).unwrap().unwrap();
        paint(&mut doc, &kition_photo_engine::kdm_bridge::local_person(), [400, 200], "#0000ff");
        let before = hash(&mut doc);

        let damage = doc.restore_state(&saved, &kition_photo_engine::kdm_bridge::local_person()).unwrap();
        assert_ne!(damage, Damage::None);
        assert_eq!(hash(&mut doc), saved_hash, "the saved version's state");
        assert_eq!(pixel(&doc, 420, 220), 'w', "the later stroke is gone");
        assert_eq!(history(&doc).last().map(String::as_str), Some("Restore Version"));
        assert!(doc.undo().unwrap().0);
        assert_eq!(hash(&mut doc), before, "one Undo gives back the exact state before the restore");
        assert_eq!(pixel(&doc, 420, 220), 'b');
        assert!(doc.redo().unwrap().0);
        assert_eq!(hash(&mut doc), saved_hash);

        let labels = history(&doc);
        assert_eq!(doc.restore_state(&content, &kition_photo_engine::kdm_bridge::local_person()).unwrap(), Damage::None, "already there");
        assert_eq!(history(&doc), labels, "nothing recorded");
        assert!(doc.undo().unwrap().0);
        assert_eq!(doc.restore_state(&content, &kition_photo_engine::kdm_bridge::local_person()).unwrap(), Damage::Full, "the file's content restores too");
        assert_eq!(hash(&mut doc), saved_hash);

        let other = PhotoDocument::create("other.kphoto", 8, 8).unwrap().snapshot().unwrap();
        assert!(doc.restore_state(&other, &kition_photo_engine::kdm_bridge::local_person()).is_err(), "another document's state is refused");
    }

    #[test]
    fn changes_since_answers_a_receipt_or_why_not() {
        let fresh = PhotoDocument::create("art.kphoto", 600, 300).unwrap();
        assert_eq!(fresh.changes_since(&[])["changes"], Value::Null, "no replica yet");
        let mut doc = wide();
        let since = doc.version().unwrap();
        paint(&mut doc, &kition_photo_engine::kdm_bridge::local_person(), [20, 20], "#ff0000");
        let receipt = doc.changes_since(&since);
        assert!(receipt["changes"].is_object(), "{receipt}");
        assert!(receipt["changes"].to_string().contains("tiles"), "the stroke's tiles: {receipt}");
        let now = doc.version().unwrap();
        let unchanged = doc.changes_since(&now);
        assert!(unchanged["changes"].is_object(), "{unchanged}");
        let elsewhere = PhotoDocument::create("other.kphoto", 8, 8).unwrap().version().unwrap();
        let refused = doc.changes_since(&elsewhere);
        assert_eq!(refused["changes"], Value::Null, "{refused}");
        assert!(refused["unavailable"].is_string());
    }

    /// `modelReply` names the one layer a reply holds by its tool id (its number), as the whole
    /// document's tool ids name it, and leaves a reply naming no object as it was.
    #[test]
    fn model_reply_names_only_the_objects_the_reply_holds() {
        let mut doc = wide();
        for n in 0..40 {
            doc.execute("layer.new.layer", json!({"name": format!("L{n}")})).unwrap();
        }
        doc.version().unwrap();
        let layer = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap();
        let object = doc.layer_object_ids()[&layer.to_string()].as_str().unwrap().to_owned();
        let replica = doc.state().unwrap().kdm().unwrap().replica();
        let full = ToolIds::of_replica(replica);
        let want = full.show(&ObjectId::from_string(object.clone()));
        let reply = doc.model_reply(&json!({"target": object, "note": "kept"}).to_string()).unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["target"], json!(want), "the same id as the full read");
        assert_eq!(reply["target"], json!(layer.to_string()));
        assert_eq!(reply["note"], "kept");
        let plain = json!({"answer": 42, "text": "no ids here"});
        assert_eq!(serde_json::from_str::<Value>(&doc.model_reply(&plain.to_string()).unwrap()).unwrap(), plain);
        assert!(doc.model_reply("not json").is_err());
    }

    #[test]
    fn model_describe_and_model_batch_go_through_the_object_model() {
        let mut doc = wide();
        let described = doc.model_describe().unwrap();
        assert!(described.to_string().contains("Layer"), "{described}");
        let layer = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap();
        let answer = doc.model_batch(&json!({"ops": [{"op": "set", "target": layer.to_string(), "field": "name", "value": "Batched"}]}).to_string()).unwrap();
        assert!(answer.is_object(), "{answer}");
        assert!(doc.inspect().unwrap()["layers"].to_string().contains("Batched"), "the batch renamed the layer");
        let refused = doc.model_batch("not json").unwrap_err();
        assert!(refused.contains("invalid-op"), "{refused}");
    }
}
