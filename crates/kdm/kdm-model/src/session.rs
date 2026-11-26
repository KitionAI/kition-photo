//! [`PhotoSession`]: one document's replica, its [`PhotoModel`] view and its blobs, for the
//! engine's command bridge.
//!
//! It is `kition_kit::session::KitSession<PhotoModel>`'s flow (transactions, per-person undo,
//! change exchange, a draft's merge and a restore with the repair ([`KitModel::validate`] and the
//! layer numbers) inside their one undo step, the view patched through [`KitModel::apply`]) on
//! concrete, `Send` parts: `KitSession` keeps its blobs as a
//! `Box<dyn BlobSink>`, which is not `Send`, and the engine's session must be (the headless kit
//! runs commands on a worker thread). Edits are diff-bridged: the engine changes
//! its own `Document`, and [`PhotoSession::record`] writes what changed as one transaction.
//!
//! **Layer numbers.** Layers that store no number (made through the object model, or written
//! before numbers were stored) get the one every read gives them from a `system` repair, on open
//! and after every import, undo and redo, and inside the step of a draft's merge or a restore; a
//! layer that lost a number another layer also stores gets its new one stored with the replica's
//! next edit.

use std::collections::BTreeSet;
use std::sync::Arc;

use kition_kit::kdm::{ActorId, DocId, KdmError, ObjectId, Origin, Replica, Tx, UndoOutcome, UndoScope, Version};
use kition_kit::session::{BlobSource, Damage, KitError, KitModel, MemoryBlobs};
use kition_photo_doc::Document;

use crate::blob_gc::{BlobIndex, Moved};
use crate::{Nodes, PhotoModel, Result, Sink, TileCache, kit_err, read, write, write_changes_with, write_document_with};

/// Why [`PhotoSession::import_changes`] refused changes, and what the refusal left.
#[derive(Debug)]
pub struct ImportError {
    pub error: KitError,
    /// The replica has no undo or redo steps left: steps taken before the import undo from the
    /// caller's own snapshots.
    pub steps_cleared: bool,
    /// The session could not be put back and holds part of what it refused: leave it.
    pub damaged: bool,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

/// One open document as a KDM replica and the engine's view of it. See the module docs.
pub struct PhotoSession {
    replica: Replica,
    model: PhotoModel,
    blobs: MemoryBlobs,
    /// An undo group is open, and whether a transaction in it already made it an undo step.
    group: Option<bool>,
    /// Tiles already encoded, so a later write does not compress them again.
    tiles: TileCache,
    /// Which blobs are still needed ([`crate::blob_gc`]); the rest are dropped after each change.
    index: BlobIndex,
    /// The hashes [`PhotoSession::record`], [`PhotoSession::import_changes`],
    /// [`PhotoSession::undo`] and [`PhotoSession::redo`] added, each with the version right
    /// after it, for [`PhotoSession::blobs_since`]. Entries are chronological: a later entry's
    /// version always includes every earlier one's (same replica, only growing), so `since`
    /// cleanly splits the log into "already has" and "needs".
    ///
    /// This log and the blob store's garbage collector ([`crate::blob_gc`]) are kept separate on
    /// purpose, rather than making GC keep every blob a log entry still names: a blob a log
    /// entry listed can still be GC'd once no undo or redo step needs it, same as today.
    /// [`PhotoSession::blobs_since`] simply skips a hash no longer in the store, and
    /// [`PhotoSession::retain_steps`] drops an entry left with no hash still in the store, so the
    /// log never grows past what the live store can still answer for.
    blob_log: Vec<(Version, Vec<String>)>,
}

impl std::fmt::Debug for PhotoSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhotoSession").field("doc", &self.replica.doc_id()).field("actor", &self.replica.actor()).finish_non_exhaustive()
    }
}

impl PhotoSession {
    /// A new replica holding `document`, written in its creating transaction (not an undo step).
    /// The view is `document` itself, with its own local layer ids.
    pub fn create(document: &Document) -> Result<Self> {
        let mut blobs = MemoryBlobs::default();
        let mut tiles = TileCache::default();
        let mut ids = kition_kit::session::IdMap::default();
        let mut nodes = Nodes::default();
        let (mut replica, ()) = Replica::create_with(PhotoModel::schema(), DocId::new(), ActorId::random(), &Origin::system("create"), |tx| {
            write_document_with(tx, document, &mut ids, &mut nodes, &mut Sink::new(&mut blobs, &mut tiles))?;
            Ok(())
        })
        .map_err(kit_err)?;
        replica.take_changed();
        let index = BlobIndex::build(&replica);
        Ok(Self { replica, model: PhotoModel::from_parts(document.clone(), ids, nodes), blobs, group: None, tiles, index, blob_log: Vec::new() })
    }

    /// Opens a snapshot (or a full change export) of another replica as a new replica of a fresh
    /// actor. `blobs` must hold the blobs it references. A replica of another schema version is
    /// refused ([`crate::same_schema`]).
    pub fn open_snapshot(bytes: &[u8], blobs: MemoryBlobs) -> Result<Self> {
        let mut replica = Replica::load(PhotoModel::schema(), bytes, ActorId::random()).map_err(kit_err)?;
        if !crate::same_schema(&replica) {
            return Err(KitError::new(format!(
                "the replica was written with another schema version ({}); this build reads version {}",
                replica.min_schema_version(),
                replica.schema().schema_version
            )));
        }
        store_missing_numbers(&mut replica)?;
        replica.take_changed();
        let model = PhotoModel::load(&replica, &blobs)?;
        let index = BlobIndex::build(&replica);
        let mut session = Self { replica, model, blobs, group: None, tiles: TileCache::default(), index, blob_log: Vec::new() };
        session.index.collect_garbage(&mut session.blobs);
        Ok(session)
    }

    /// Opens the replica a host stored for a document (`stored`, its history) with the document's
    /// file (`file`, the content snapshot a `.kphoto` holds) taken into it
    /// (`kition_kit::session::take_file`): nothing when the stored replica has the file's version;
    /// the changes the file holds beyond it when the file's history reaches it; otherwise (the file
    /// was saved elsewhere) the file's state, written by object id as one `system` transaction.
    /// Refuses a file of another document. `blobs` must hold what both reference.
    pub fn open_stored_with_file(stored: &[u8], file: &[u8], blobs: MemoryBlobs) -> Result<Self> {
        let mut replica = Replica::load(PhotoModel::schema(), stored, ActorId::random()).map_err(kit_err)?;
        let from_file = Replica::load(PhotoModel::schema(), file, ActorId::random()).map_err(kit_err)?;
        if !crate::same_schema(&replica) || !crate::same_schema(&from_file) {
            return Err(KitError::new("the replica was written with another schema version"));
        }
        kition_kit::session::take_file(&mut replica, &from_file)?;
        store_missing_numbers(&mut replica)?;
        replica.take_changed();
        let model = PhotoModel::load(&replica, &blobs)?;
        let index = BlobIndex::build(&replica);
        let mut session = Self { replica, model, blobs, group: None, tiles: TileCache::default(), index, blob_log: Vec::new() };
        session.index.collect_garbage(&mut session.blobs);
        Ok(session)
    }

    /// What a `.kphoto` holds of the replica: its content (`Replica::content_snapshot`, kition-sdk
    /// data model 7.4: the state alone, no history, no version, no author), and the blobs that
    /// state references. The same state gives the same bytes, whoever saves it.
    pub fn content(&self) -> Result<Content> {
        let blobs = crate::state_blobs(&self.replica).into_iter().filter_map(|hash| self.blobs.0.get(&hash).cloned().map(|bytes| (hash, bytes))).collect();
        Ok(Content { content: self.replica.content_snapshot().map_err(kit_err)?, blobs })
    }

    /// The view: the document as of the last record, undo, redo or import.
    pub fn document(&self) -> &Document {
        &self.model.document
    }

    pub fn model(&self) -> &PhotoModel {
        &self.model
    }

    /// Replaces the view's document without writing anything: for the engine to keep the view
    /// equal to its own document after it put per-person state (the selection) back on it. The
    /// replicated content must already be what the view holds.
    pub fn set_view(&mut self, document: Document) {
        self.model.document = document;
    }

    /// Writes what differs between the view and `after` as one transaction of `origin`, then
    /// makes `after` the view. Answers how many undo steps that made (0 when nothing replicated
    /// changed, or when the transaction joined an undo group that is already a step). On failure
    /// nothing is written and the view is unchanged, so the next record writes the change again.
    pub fn record(&mut self, origin: &Origin, after: &Document) -> Result<u32> {
        let mut ids = self.model.layers.clone();
        let mut nodes = self.model.nodes.clone();
        let before = self.replica.version();
        let mut sink = Sink::new(&mut self.blobs, &mut self.tiles);
        let model = &self.model;
        let result = self.replica.transact(origin, |tx| {
            let wrote = write_changes_with(tx, &model.document, after, &mut ids, &mut nodes, &mut sink)?;
            // An edit stores the numbers the last read gave layers that do not store them, so
            // they last.
            Ok(wrote && write::store_numbers(tx, model.unstored())?)
        });
        let changed = self.replica.take_changed();
        let stored = match result {
            Ok(stored) => stored,
            Err(error) => {
                self.index.collect_garbage(&mut self.blobs);
                return Err(kit_err(error));
            }
        };
        let moved = self.index.update(&self.replica, &changed);
        self.model.adopt(after.clone(), ids, nodes);
        if stored {
            self.model.stored();
        }
        let wrote = self.replica.version() != before;
        let scope = UndoScope::of(origin.kind).filter(|_| wrote);
        let steps = match &mut self.group {
            _ if scope.is_none() => 0,
            None => 1,
            Some(counted) if !*counted => {
                *counted = true;
                1
            }
            Some(_) => 0,
        };
        self.log_added(moved.added.iter().cloned());
        if let Some(scope) = scope {
            if steps == 1 {
                self.index.push_step(scope, moved);
            } else {
                self.index.join_step(scope, moved);
            }
        }
        self.index.collect_garbage(&mut self.blobs);
        Ok(steps)
    }

    /// Starts an undo group (ending any open one first): the transactions until
    /// [`PhotoSession::end_undo_group`] undo as one step.
    pub fn begin_undo_group(&mut self) -> Result<()> {
        self.end_undo_group();
        self.replica.begin_undo_group().map_err(kit_err)?;
        self.group = Some(false);
        Ok(())
    }

    pub fn end_undo_group(&mut self) {
        self.replica.end_undo_group();
        self.group = None;
    }

    /// Undoes this replica's last undo step in `scope` (never imports or repairs, and never the
    /// other scope's steps, KitModel 1.1 M12a), repairs the result and patches the view. Answers
    /// whether there was one, and the damage.
    pub fn undo_in(&mut self, scope: UndoScope) -> Result<(bool, Damage)> {
        self.step_in(scope, true).map(|(outcome, damage)| (outcome.is_done(), damage))
    }

    pub fn redo_in(&mut self, scope: UndoScope) -> Result<(bool, Damage)> {
        self.step_in(scope, false).map(|(outcome, damage)| (outcome.is_done(), damage))
    }

    /// Undoes (`undo`) or redoes this replica's last step of `scope`, repairs the result and
    /// patches the view. Answers which steps the replica took off its stack ([`UndoOutcome`]: the
    /// steps it passed over, with nothing left to change, and the one it took back), so a history
    /// kept beside it (the engine's) stays in step, and the damage.
    pub fn step_in(&mut self, scope: UndoScope, undo: bool) -> Result<(UndoOutcome, Damage)> {
        if !(if undo { self.can_undo_in(scope) } else { self.can_redo_in(scope) }) {
            return Ok((UndoOutcome::default(), Damage::None));
        }
        self.group = None;
        let outcome = if undo { self.replica.undo_in(scope) } else { self.replica.redo_in(scope) }.map_err(kit_err)?;
        let (damage, moved) = self.settle();
        if outcome.steps() > 0 {
            self.log_added(moved.added.iter().cloned());
            if undo {
                self.index.undone(scope, outcome.steps(), outcome.is_done(), moved);
            } else {
                self.index.redone(scope, outcome.steps(), outcome.is_done(), moved);
            }
        }
        self.index.collect_garbage(&mut self.blobs);
        Ok((outcome, damage?))
    }

    /// Takes a draft's changes (made on a fork of this replica, `Replica::fork` of
    /// [`PhotoSession::snapshot`]) in as one change of `origin` with the repair of the merged
    /// state in it ([`in_step`], `Replica::merge_changes_repaired`): one undo step of the person
    /// (KitModel M12a, also for an agent's `origin`; none for a `system` or `import` one), whose
    /// one undo gives back the exact state before the merge and whose redo the merged one. Edits
    /// made here since the draft was made are kept; two sides that painted different tiles of one
    /// layer both stay (each tile is its own key of the layer's `tiles` map, rule D4), the same
    /// tile painted on both is refused. Patches the view as an import does; add the draft's blobs
    /// first ([`PhotoSession::add_blobs`]). A refused merge changes nothing. A merge the repair
    /// takes back entirely changes nothing either. Answers the undo scope the step was recorded in
    /// (`None`: no step) and the damage.
    pub fn merge_changes(&mut self, bytes: &[u8], origin: &Origin) -> Result<(Option<UndoScope>, Damage)> {
        self.end_undo_group();
        let before = self.replica.version();
        let merged = self.replica.merge_changes_repaired(bytes, origin, in_step).map_err(kit_err)?;
        if self.replica.version() == before {
            return Ok((None, Damage::None));
        }
        let changed = self.replica.take_changed();
        Ok((merged.scope, self.took(merged.scope, &changed)?))
    }

    /// What merging a draft's changes would change in the document now, the repair included as
    /// [`PhotoSession::merge_changes`] runs it (`Replica::compare_changes_repaired`), so a draft's
    /// preview is what accepting it gives. Nothing changes; refuses what the merge refuses.
    pub fn compare_changes(&self, bytes: &[u8]) -> Result<kition_kit::kdm::diff::StateDiff> {
        self.replica.compare_changes_repaired(bytes, in_step).map_err(kit_err)
    }

    /// The canonical state merging a draft's changes would give, the repair included
    /// (`Replica::merged_state_repaired`); nothing changes.
    pub fn merged_state(&self, bytes: &[u8]) -> Result<serde_json::Value> {
        self.replica.merged_state_repaired(bytes, in_step).map_err(kit_err)
    }

    /// Writes the state `snapshot` holds (a snapshot, or the content of this document, as a host's
    /// checkpoint keeps it) over the document as one change of `origin`, keyed by object id
    /// (`kition_kit::session::write_state`), the repair ([`in_step`]) in it: one undo step of
    /// `origin`'s scope whose undo gives back the exact state before. Nothing changes (no change,
    /// no step, the version stays) when the states are equal or the repair takes back all the
    /// restore wrote. Add the snapshot's blobs first. Refuses another document's snapshot and one of
    /// another schema version. Answers the undo scope of the step (`None`: no step) and the damage.
    pub fn restore_state(&mut self, snapshot: &[u8], origin: &Origin) -> Result<(Option<UndoScope>, Damage)> {
        let schema = <PhotoModel as KitModel>::schema();
        let loaded = Replica::load(Arc::clone(&schema), snapshot, self.replica.actor()).map_err(kit_err)?;
        if loaded.doc_id() != self.replica.doc_id() {
            return Err(kit_err(KdmError::WrongDocument { expected: self.replica.doc_id().to_string(), found: loaded.doc_id().to_string() }));
        }
        if !crate::same_schema(&loaded) {
            return Err(KitError::new("the saved state was written with another schema version"));
        }
        self.end_undo_group();
        let (now, then) = (kition_kit::session::state(&self.replica)?, kition_kit::session::state(&loaded)?);
        if now == then {
            return Ok((None, Damage::None));
        }
        let changed: BTreeSet<ObjectId> = now.keys().chain(then.keys()).filter(|id| now.get(*id) != then.get(*id)).map(ObjectId::from_string).collect();
        let mut kit_error = None;
        let result = self.replica.transact_unless_unchanged(origin, |tx| {
            kition_kit::session::write_state(tx, &schema, &now, &then).map_err(|error| {
                let message = error.to_string();
                kit_error = Some(error);
                KdmError::Engine(message)
            })?;
            in_step(tx, &changed)
        });
        let ((), wrote) = match (result, kit_error) {
            (_, Some(error)) => return Err(error),
            (result, None) => result.map_err(kit_err)?,
        };
        if !wrote {
            return Ok((None, Damage::None));
        }
        let scope = UndoScope::of(origin.kind);
        let changed = self.replica.take_changed();
        Ok((scope, self.took(scope, &changed)?))
    }

    /// After a merge or a restore that wrote `changed` as one step of `scope` (`None`: no step):
    /// the blob index and log follow, the blobs added for it are kept by the step, and the view
    /// is patched.
    fn took(&mut self, scope: Option<UndoScope>, changed: &BTreeSet<ObjectId>) -> Result<Damage> {
        let moved = self.index.update(&self.replica, changed);
        self.log_added(moved.added.iter().cloned());
        if let Some(scope) = scope {
            self.index.push_step(scope, moved);
        }
        self.index.imported();
        self.index.collect_garbage(&mut self.blobs);
        self.refresh(changed)
    }

    /// Whether `scope` has an undo step to take: one of this replica's, not trimmed by
    /// [`PhotoSession::retain_steps`].
    pub fn can_undo_in(&self, scope: UndoScope) -> bool {
        self.replica.can_undo_in(scope) && self.index.undo_steps(scope) > 0
    }

    pub fn can_redo_in(&self, scope: UndoScope) -> bool {
        self.replica.can_redo_in(scope) && self.index.redo_steps(scope) > 0
    }

    /// Keeps only the newest `undo` undo steps and the next `redo` redo steps of `scope` reachable
    /// (the history's own limit) and drops the blobs only older steps needed. The KDM replica has no
    /// limit of its own on its undo stack; past this one the steps are refused here, and what
    /// the replica still holds of them is a few small references, not pixels.
    pub fn retain_steps(&mut self, scope: UndoScope, undo: usize, redo: usize) {
        self.index.retain(scope, undo, redo);
        self.index.collect_garbage(&mut self.blobs);
        let blobs = &self.blobs;
        self.blob_log.retain(|(_, hashes)| hashes.iter().any(|hash| blobs.0.contains_key(hash)));
    }

    /// Undo and redo steps of `scope` reachable, as [`PhotoSession::retain_steps`] left them.
    pub fn steps(&self, scope: UndoScope) -> (usize, usize) {
        (self.index.undo_steps(scope), self.index.redo_steps(scope))
    }

    /// Imports remote or stored changes (ending any open undo group first, so it stays one
    /// step), repairs the merged state and patches the view.
    ///
    /// Refused changes change nothing (KitAgent A18: a failed call leaves the document as it
    /// was): ones the replica refuses itself (it then changes nothing), and ones the view cannot
    /// read or the repair cannot mend (a field set to a value the engine has no meaning for). For
    /// those, the replica is put back as it was before the import (`Replica::fork_at`, inside the
    /// session's own log); that clears its undo and redo steps ([`ImportError::steps_cleared`]).
    pub fn import_changes(&mut self, bytes: &[u8]) -> std::result::Result<Damage, ImportError> {
        self.end_undo_group();
        let before = self.replica.version();
        let view = (self.model.document.clone(), self.model.layers.clone(), self.model.nodes.clone());
        if let Err(error) = self.replica.import_changes(bytes) {
            return Err(ImportError { error: kit_err(error), steps_cleared: false, damaged: false });
        }
        let (damage, moved) = self.settle();
        let damage = match damage {
            Ok(damage) => damage,
            Err(error) => {
                let damaged = !self.restore(&before, view);
                return Err(ImportError { error, steps_cleared: true, damaged });
            }
        };
        self.log_added(moved.added.iter().cloned());
        self.index.imported();
        self.index.collect_garbage(&mut self.blobs);
        Ok(damage)
    }

    /// Puts the session back as it was at `before` with the view `view`, after an import it
    /// refused: the replica holds the state at `before` under the same actor and on the same line
    /// of changes, without undo or redo steps. Answers whether it could (`before` is in the
    /// session's log).
    fn restore(&mut self, before: &Version, view: (Document, kition_kit::session::IdMap<kition_photo_doc::LayerId>, Nodes)) -> bool {
        let Ok(replica) = self.replica.fork_at(before, self.replica.actor()) else { return false };
        self.replica = replica;
        self.group = None;
        let (document, layers, nodes) = view;
        self.model.adopt(document, layers, nodes);
        self.index = BlobIndex::build(&self.replica);
        self.index.collect_garbage(&mut self.blobs);
        true
    }

    /// The blobs a peer at `since` needs to reach this session's current state: the
    /// deduplicated hashes [`PhotoSession::blob_log`] logged for a version `since` does not
    /// already include ([`Version::includes`]), filtered to blobs still in the store. An empty
    /// (zero) `since` instead answers every blob the *current* state references: the session's
    /// creating transaction (and a snapshot's) is never logged, so a peer starting from nothing
    /// needs this, not the log.
    pub fn blobs_since(&self, since: &Version) -> Vec<(String, Arc<[u8]>)> {
        if *since == Version::empty() {
            return self.index.referenced().into_iter().filter_map(|hash| self.blobs.0.get(&hash).cloned().map(|bytes| (hash, bytes))).collect();
        }
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for (version, hashes) in &self.blob_log {
            if since.includes(version) {
                continue;
            }
            for hash in hashes {
                if seen.insert(hash.clone())
                    && let Some(bytes) = self.blobs.0.get(hash)
                {
                    out.push((hash.clone(), bytes.clone()));
                }
            }
        }
        out
    }

    /// Appends a log entry for the hashes an operation added, at the version right after it
    /// (see [`PhotoSession::blob_log`]); does nothing if `added` is empty.
    fn log_added(&mut self, added: impl IntoIterator<Item = String>) {
        let added: Vec<String> = added.into_iter().collect();
        if !added.is_empty() {
            self.blob_log.push((self.replica.version(), added));
        }
    }

    pub fn export_changes(&self, since: &Version) -> Result<Vec<u8>> {
        self.replica.export_changes(since).map_err(kit_err)
    }

    pub fn version(&self) -> Version {
        self.replica.version()
    }

    pub fn snapshot(&self) -> Result<Vec<u8>> {
        self.replica.snapshot().map_err(kit_err)
    }

    pub fn state_hash(&self) -> Result<String> {
        self.replica.state_hash().map_err(kit_err)
    }

    pub fn replica(&self) -> &Replica {
        &self.replica
    }

    /// The blobs the replica's tiles and embedded data reference.
    pub fn blobs(&self) -> &MemoryBlobs {
        &self.blobs
    }

    /// Adds blobs (from a peer, before importing its changes): kept until the next import.
    pub fn add_blobs(&mut self, blobs: &MemoryBlobs) {
        for (hash, bytes) in &blobs.0 {
            self.blobs.0.entry(hash.clone()).or_insert_with(|| bytes.clone());
        }
        self.index.protect(blobs.0.keys().cloned());
    }

    /// As `KitSession`'s: repairs what changed in a `system` transaction (not an undo step), then
    /// patches the view with both; the view follows the replica even if the repair fails. Also
    /// answers which blobs that moved (the blob index is updated either way).
    fn settle(&mut self) -> (Result<Damage>, Moved) {
        let mut changed = self.replica.take_changed();
        let repaired = if changed.is_empty() {
            Ok(())
        } else {
            let mut kit_error = None;
            let result = self.replica.transact(&Origin::system("repair"), |tx: &mut Tx<'_>| {
                PhotoModel::validate(tx, &changed).map(|_| ()).map_err(|error| {
                    let message = error.to_string();
                    kit_error = Some(error);
                    KdmError::Engine(message)
                })
            });
            match (result, kit_error) {
                (_, Some(error)) => Err(error),
                (result, None) => result.map_err(kit_err),
            }
            .and_then(|()| store_missing_numbers(&mut self.replica))
        };
        changed.extend(self.replica.take_changed());
        let moved = self.index.update(&self.replica, &changed);
        let damage = self.refresh(&changed).and_then(|damage| repaired.map(|()| damage));
        (damage, moved)
    }

    fn refresh(&mut self, changed: &BTreeSet<ObjectId>) -> Result<Damage> {
        if changed.is_empty() {
            return Ok(Damage::None);
        }
        let blobs: &dyn BlobSource = &self.blobs;
        match self.model.apply(&self.replica, changed, blobs) {
            Ok(damage) => Ok(damage),
            Err(_) => {
                self.model.reload(&self.replica, blobs)?;
                Ok(Damage::Full)
            }
        }
    }
}

/// What a save writes of the replica ([`PhotoSession::content`]).
pub struct Content {
    /// The content (`content.json`).
    pub content: Vec<u8>,
    /// The blobs its state references.
    pub blobs: Vec<(String, Arc<[u8]>)>,
}

/// The repair as a draft's merge or a restore runs it inside its own change and undo step
/// (`Replica::merge_changes_repaired`, `compare_changes_repaired`, `merged_state_repaired`, the
/// restore's transaction): [`KitModel::validate`] (a mask or channel naming a surface gone since)
/// and the number every read gives each layer that stores none (one the draft made through the
/// object model), so one undo gives back the exact state before and the preview of a draft is
/// what accepting it gives.
fn in_step(tx: &mut Tx<'_>, changed: &BTreeSet<ObjectId>) -> std::result::Result<(), KdmError> {
    PhotoModel::validate(tx, changed).map_err(|error| KdmError::Engine(error.to_string()))?;
    let numbers = read::number_repairs(&*tx)?;
    write::store_numbers(tx, &numbers)?;
    Ok(())
}

/// Stores, in a `system` transaction labelled `repair` (no undo step), the number every read gives
/// each layer that stores none ([`read::number_repairs`]); writes nothing when every layer stores
/// one. A function of the replica's state, as every repair, so every replica writes the same.
fn store_missing_numbers(replica: &mut Replica) -> Result<()> {
    let numbers = read::number_repairs(replica).map_err(kit_err)?;
    if !numbers.is_empty() {
        replica.transact(&Origin::system("repair"), |tx| write::store_numbers(tx, &numbers).map(|_| ())).map_err(kit_err)?;
    }
    Ok(())
}
