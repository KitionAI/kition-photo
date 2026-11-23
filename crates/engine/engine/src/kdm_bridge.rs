//! The command bridge to the document's KDM replica (Kition phase 3a).
//!
//! Every open document has a [`kdm_model::PhotoSession`]: the replica is the shared document's
//! source of truth, and [`DocState::doc`] is the engine's view of it. Commands run exactly as
//! before on the in-memory `Document`; [`crate::Session::edit`] then writes what changed as one
//! transaction ([`kdm_model::write_changes`], against the view as last written, so changes a
//! command made outside `edit` are written too). Changes made outside any edit (view-like
//! document state that is not a history step) are written after the command as a `system`
//! transaction, which is not an undo step.
//!
//! Undo is per person: each history step remembers how many replica undo steps it took
//! ([`kition_photo_ops::HistoryState::external`]), and Edit › Undo undoes those on the replica, which
//! keeps edits imported from collaborators. The selection and the quick mask are this person's
//! own state: never written, restored from the history snapshot on undo and redo.

use std::sync::Arc;

use kdm_model::PhotoSession;
use kition_photo_doc::{Document, LayerId};

use kition_photo_ops::SNAPSHOT_STEP;

use crate::{Damage, DocState, EngineError, MemoryBlobs, Origin, Result, Session, UndoScope, Version};

/// The origin of the local person's commands.
pub fn local_person() -> Origin {
    Origin::person("local")
}

pub(crate) fn kdm_error(error: impl std::fmt::Display) -> EngineError {
    EngineError::Other(format!("document replica: {error}"))
}

impl DocState {
    /// The document's replica, if it has one.
    pub fn kdm(&self) -> Option<&PhotoSession> {
        self.kdm.as_ref()
    }

    /// A new history step starts: a coalescing one opens an undo group its later edits join;
    /// any other ends the open group.
    pub(crate) fn begin_kdm_step(&mut self, coalescing: bool) {
        let Some(kdm) = self.kdm.as_mut() else { return };
        if coalescing {
            if kdm.begin_undo_group().is_err() {
                kdm.end_undo_group();
            }
        } else {
            kdm.end_undo_group();
        }
    }

    /// How many times the document left its replica because the replica refused a write or an
    /// undo (a document it cannot hold, or a bug). The document keeps working, undoing from its
    /// history snapshots; the next edit makes a fresh replica, which is a new KDM document:
    /// collaborators need its full [`Session::snapshot`] to join again. Hosts watch this.
    pub fn kdm_resets(&self) -> u64 {
        self.kdm_resets
    }

    /// Writes what an edit changed since the last sync as one transaction of `origin`, making
    /// the replica first if there is none yet. Answers the undo steps that took.
    pub(crate) fn sync_kdm(&mut self, origin: &Origin) -> u32 {
        self.sync_kdm_with(origin, true)
    }

    fn sync_kdm_with(&mut self, origin: &Origin, edit: bool) -> u32 {
        if Arc::ptr_eq(&self.synced, &self.doc) {
            return 0;
        }
        // Every layer the replica stores carries its number in this document.
        self.number_layers();
        if self.kdm.is_none() {
            // Nothing shared changed (a selection): no replica is needed yet.
            if same_shared(&self.synced, &self.doc) {
                self.synced = self.doc.clone();
                return 0;
            }
            // After a reset, only an edit makes the new replica (an undo goes on from snapshots).
            // The first replica holds the document as last synced, and this change is written on
            // top of it as this step; after a reset it holds the document as it is now, and the
            // steps before (taken without a replica) undo from their snapshots.
            let base = if self.kdm_unbacked { self.doc.clone() } else { self.synced.clone() };
            if (self.kdm_unbacked && !edit) || !self.create_kdm(&base) {
                self.kdm_unbacked = true;
                self.synced = self.doc.clone();
                return 0;
            }
        }
        let Some(kdm) = self.kdm.as_mut() else { return 0 };
        match kdm.record(origin, &self.doc) {
            Ok(steps) => {
                self.synced = self.doc.clone();
                self.kdm_steps += u64::from(steps);
                steps
            }
            Err(_) => {
                self.detach_kdm();
                0
            }
        }
    }

    /// A replica holding `base` (the document as last synced: the change being synced is then
    /// written on top of it, as this step). Refused for a layer tree deeper than a replica holds.
    fn create_kdm(&mut self, base: &Arc<Document>) -> bool {
        if self.kdm_blocked || too_deep(base) {
            return false;
        }
        match PhotoSession::create(base) {
            Ok(kdm) => {
                if self.kdm_unbacked {
                    // Steps taken without a replica (after a reset) undo from their snapshots.
                    self.history.mark_snapshot_steps();
                    self.other_history.mark_snapshot_steps();
                    self.kdm_unbacked = false;
                    self.kdm_snapshot_steps = true;
                }
                self.kdm = Some(kdm);
                self.synced = base.clone();
                true
            }
            Err(_) => {
                self.kdm_blocked = true;
                false
            }
        }
    }

    /// The replica, made from the current document if there is none yet (for the change
    /// exchange: a host asks for it before any edit).
    pub(crate) fn ensure_kdm(&mut self) -> Option<&mut PhotoSession> {
        if self.kdm.is_none() {
            let base = self.doc.clone();
            self.create_kdm(&base);
        }
        self.kdm.as_mut()
    }

    /// The replica refused the document: leave it (see [`DocState::kdm_resets`]).
    fn detach_kdm(&mut self) {
        self.kdm = None;
        self.kdm_resets += 1;
        self.kdm_unbacked = true;
        self.synced = self.doc.clone();
    }

    /// Lets the replica forget undo and redo steps the history no longer holds, and the blobs
    /// only they needed.
    pub(crate) fn trim_kdm(&mut self) {
        let (undo, redo) = (self.history.external_undo_steps(), self.history.external_redo_steps());
        let other = (self.other_history.external_undo_steps(), self.other_history.external_redo_steps());
        if let Some(kdm) = self.kdm.as_mut() {
            kdm.retain_steps(self.history_scope, undo, redo);
            kdm.retain_steps(other_scope(self.history_scope), other.0, other.1);
        }
    }

    /// Makes `history` the one of `scope`.
    /// A coalescing gesture never spans two scopes: its open replica undo group ends here.
    pub(crate) fn use_scope(&mut self, scope: UndoScope) {
        if self.history_scope != scope {
            self.coalesce = None;
            if let Some(kdm) = self.kdm.as_mut() {
                kdm.end_undo_group();
            }
            std::mem::swap(&mut self.history, &mut self.other_history);
            self.history_scope = scope;
        }
    }

    /// Writes changes made outside any edit (state a command keeps current without a history
    /// step of its own, such as layer-based slices) as part of the current history step, as the
    /// snapshot history did: undoing the step takes them back too. Right after an undo, they are
    /// a `system` transaction instead, which is no undo step and keeps the redo states.
    pub(crate) fn sync_kdm_loose(&mut self, origin: &Origin) {
        let origin = if self.history.can_redo() { Origin::system("view") } else { origin.clone() };
        let steps = self.sync_kdm_with(&origin, false);
        self.history.add_current_external(steps);
        self.trim_kdm();
    }

    /// Whether the replica can take `steps` undo (or redo) steps for the next history step.
    pub(crate) fn kdm_can_step(&self, steps: u32, undo: bool) -> bool {
        match &self.kdm {
            Some(kdm) if steps > 0 && steps != SNAPSHOT_STEP => {
                if undo {
                    kdm.can_undo_in(self.history_scope)
                } else {
                    kdm.can_redo_in(self.history_scope)
                }
            }
            _ => true,
        }
    }

    /// Undoes (or redoes) `steps` replica steps and answers the document to make current: the
    /// replica's, with the selection and quick mask of `snapshot` (the history state stepped
    /// to). Without a replica, `snapshot` itself; for a snapshot step too, which is then written
    /// to the replica as a change that is no undo step.
    pub(crate) fn kdm_step(&mut self, snapshot: Arc<Document>, steps: u32, undo: bool) -> Arc<Document> {
        let Some(kdm) = self.kdm.as_mut() else { return snapshot };
        if steps == SNAPSHOT_STEP {
            if kdm.record(&Origin::system("snapshot step"), &snapshot).is_err() {
                self.doc = snapshot.clone();
                self.detach_kdm();
            }
            self.synced = snapshot.clone();
            return snapshot;
        }
        for _ in 0..steps {
            let scope = self.history_scope;
            if (if undo { kdm.undo_in(scope) } else { kdm.redo_in(scope) }).is_err() {
                // The view may no longer match the replica: leave it, and step from the
                // snapshots, which are this person's own history.
                self.detach_kdm();
                return snapshot;
            }
        }
        let mut doc = kdm.document().clone();
        doc.selection.clone_from(&snapshot.selection);
        doc.quick_mask.clone_from(&snapshot.quick_mask);
        // With no collaborator's edit in between, the replica comes back to the snapshot: keep
        // the snapshot itself (its tiles are shared with the history, and its rendered caches
        // are the ones the document had).
        let doc = if same_but_caches(&doc, &snapshot) {
            kdm.set_view((*snapshot).clone());
            snapshot
        } else {
            rebuild_caches(&mut doc);
            kdm.set_view(doc.clone());
            Arc::new(doc)
        };
        self.synced = doc.clone();
        doc
    }

    /// After an import: the document follows the replica, and the view state that named a
    /// layer deleted remotely moves to a neighbour.
    fn follow_kdm(&mut self) {
        let Some(kdm) = self.kdm.as_mut() else { return };
        let mut doc = kdm.document().clone();
        if rebuild_caches(&mut doc) {
            kdm.set_view(doc.clone());
        }
        let old = std::mem::replace(&mut self.doc, Arc::new(doc));
        self.synced = self.doc.clone();
        self.coalesce = None;
        if self.kdm_snapshot_steps {
            // Restoring a snapshot now would take the collaborator's change back.
            self.history.clear();
            self.other_history.clear();
            self.kdm_snapshot_steps = false;
            self.trim_kdm();
        }
        if self.active_layer.is_some_and(|id| self.doc.layer(id).is_none()) {
            self.active_layer = self.active_layer.and_then(|id| neighbour(&old, &self.doc, id));
        }
        crate::fix_active(self);
        crate::mask_view_cmds::fix(self);
        self.revision += 1;
        self.last_damage = None;
    }
}

/// Whether a layer tree is deeper than a replica holds.
fn too_deep(doc: &Document) -> bool {
    doc.walk().iter().any(|(path, _, _)| path.len() > kition_photo_doc::MAX_GROUP_DEPTH)
}

/// Whether two documents hold the same shared state: equal apart from rendered caches and this
/// person's own state (the selection, the quick mask, the view's identity and title).
fn same_shared(a: &Document, b: &Document) -> bool {
    let strip = |doc: &Document| {
        let mut doc = doc.clone();
        doc.selection = None;
        doc.quick_mask = None;
        doc.name = String::new();
        doc.id = kition_photo_doc::DocId(0);
        doc
    };
    same_but_caches(&strip(a), &strip(b))
}

/// Whether two documents are equal apart from rendered caches (which the replica does not hold).
fn same_but_caches(a: &Document, b: &Document) -> bool {
    fn strip(doc: &Document) -> Document {
        fn layers(list: &mut [kition_photo_doc::Layer], depth: usize) {
            for layer in list {
                layer.fill_cache = None;
                match &mut layer.content {
                    kition_photo_doc::LayerContent::Text(t) => t.cache = None,
                    kition_photo_doc::LayerContent::Shape(sh) => sh.cache = None,
                    kition_photo_doc::LayerContent::Smart(sm) => sm.cache = None,
                    kition_photo_doc::LayerContent::Group(g) if depth < kition_photo_doc::MAX_GROUP_DEPTH => layers(&mut g.children, depth + 1),
                    _ => {}
                }
            }
        }
        let mut doc = doc.clone();
        layers(&mut doc.layers, 0);
        doc
    }
    a == b || strip(a) == strip(b)
}

/// Renders the type, shape and object layers the replica brought without their rendered
/// appearance (it is not replicated; a layer whose content did not change keeps the view's).
/// Only the caches change. Answers whether any was rendered.
pub(crate) fn rebuild_caches(doc: &mut Document) -> bool {
    let missing: Vec<LayerId> = doc
        .walk()
        .into_iter()
        .filter(|(_, _, l)| match &l.content {
            kition_photo_doc::LayerContent::Text(t) => t.cache.is_none(),
            kition_photo_doc::LayerContent::Shape(sh) => sh.cache.is_none(),
            kition_photo_doc::LayerContent::Smart(sm) => sm.cache.is_none(),
            _ => false,
        })
        .map(|(_, _, l)| l.id)
        .collect();
    let mut rendered = false;
    for id in missing {
        let Some(mut layer) = doc.layer(id).cloned() else { continue };
        let cache = match &mut layer.content {
            kition_photo_doc::LayerContent::Text(t) => {
                crate::type_cmds::refresh(doc, t);
                t.cache.take()
            }
            kition_photo_doc::LayerContent::Shape(sh) => {
                crate::vector_cmds::refresh_shape(doc, sh);
                sh.cache.take()
            }
            kition_photo_doc::LayerContent::Smart(_) => {
                // A missing source (an unavailable linked file) leaves it unrendered.
                let _ = crate::object_layer_cmds::refresh_layer(doc, &mut layer);
                match &mut layer.content {
                    kition_photo_doc::LayerContent::Smart(sm) => sm.cache.take(),
                    _ => None,
                }
            }
            _ => None,
        };
        if cache.is_none() {
            continue;
        }
        match doc.layer_mut(id).map(|l| &mut l.content) {
            Some(kition_photo_doc::LayerContent::Text(t)) => t.cache = cache,
            Some(kition_photo_doc::LayerContent::Shape(sh)) => sh.cache = cache,
            Some(kition_photo_doc::LayerContent::Smart(sm)) => sm.cache = cache,
            _ => continue,
        }
        rendered = true;
    }
    rendered
}

/// The layer nearest `gone` in `old`'s stacking order (below it first) that `now` still has.
fn neighbour(old: &Document, now: &Document, gone: LayerId) -> Option<LayerId> {
    let order: Vec<LayerId> = old.walk().into_iter().map(|(_, _, l)| l.id).collect();
    let at = order.iter().position(|id| *id == gone)?;
    let below = order.get(..at).unwrap_or_default().iter().rev();
    let above = order.get(at + 1..).unwrap_or_default().iter();
    below.chain(above).copied().find(|id| now.layer(*id).is_some())
}

impl Session {
    /// Who the running command acts for: the origin of [`Session::execute_as`], or the local
    /// person.
    pub fn origin(&self) -> Origin {
        self.origin.clone().unwrap_or_else(local_person)
    }

    /// [`Session::execute`] as `origin` (a person, an agent): the replica records its edits as
    /// theirs, and they undo as that replica's steps.
    pub fn execute_as(&mut self, id: &str, params: serde_json::Value, origin: &Origin) -> Result<serde_json::Value> {
        let previous = self.set_origin(Some(origin.clone()));
        let r = self.execute(id, params);
        self.set_origin(previous);
        r
    }

    /// Sets who the following commands act for (`None`: the local person); answers the previous
    /// setting, to put back. For front ends that run commands through their own entry points.
    pub fn set_origin(&mut self, origin: Option<Origin>) -> Option<Origin> {
        let previous = std::mem::replace(&mut self.origin, origin);
        let scope = scope_of(&self.origin());
        for st in &mut self.docs {
            st.use_scope(scope);
        }
        previous
    }

    /// [`Session::undo`] in `scope` (`None`: the caller's, engine rule E14a).
    pub fn undo_in(&mut self, scope: Option<UndoScope>) -> bool {
        self.in_scope(scope, Self::undo)
    }

    /// [`Session::redo`] in `scope` (`None`: the caller's).
    pub fn redo_in(&mut self, scope: Option<UndoScope>) -> bool {
        self.in_scope(scope, Self::redo)
    }

    fn in_scope(&mut self, scope: Option<UndoScope>, step: fn(&mut Self) -> bool) -> bool {
        let own = scope_of(&self.origin());
        let scope = scope.unwrap_or(own);
        for st in &mut self.docs {
            st.use_scope(scope);
        }
        let done = step(self);
        for st in &mut self.docs {
            st.use_scope(own);
        }
        done
    }

    /// Writes what the finished command changed outside an edit, in every document (a command
    /// can change another document than the active one).
    pub(crate) fn sync_kdm_after_command(&mut self) {
        let origin = self.origin();
        for st in &mut self.docs {
            st.sync_kdm_loose(&origin);
        }
    }

    /// The active document's replica, made now if it has none yet.
    fn active_kdm(&mut self) -> Result<&mut PhotoSession> {
        let origin = self.origin();
        let st = self.active_mut().ok_or(EngineError::NoDocument)?;
        st.sync_kdm_loose(&origin);
        st.ensure_kdm().ok_or_else(|| EngineError::Other("the document can't be held in a replica (layer groups nested too deep?)".into()))
    }

    /// Imports a collaborator's (or stored) changes into the active document's replica. Their
    /// blobs must be added first ([`Session::add_blobs`]). The document follows; edits not
    /// written yet are written first, so nothing local is lost. Refused changes leave the
    /// document as it was (KitAgent A18); the steps before then undo from their snapshots if the
    /// replica lost its own.
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<Damage> {
        if self.active_job().is_some() {
            return Err(EngineError::Other("a background job is running on the document".into()));
        }
        let result = self.active_kdm()?.import_changes(bytes);
        let st = self.active_mut().ok_or(EngineError::NoDocument)?;
        match result {
            Ok(damage) => {
                st.follow_kdm();
                Ok(damage)
            }
            Err(refused) => {
                if refused.damaged {
                    // The replica holds part of what it refused: the document stays as it was
                    // and leaves it.
                    st.detach_kdm();
                } else if refused.steps_cleared {
                    st.history.mark_snapshot_steps();
                    st.other_history.mark_snapshot_steps();
                    st.kdm_snapshot_steps = true;
                }
                Err(kdm_error(refused))
            }
        }
    }

    /// Applies only the file difference, outside both undo histories, retaining local steps.
    pub fn reconcile_reread(&mut self, fresh: &Document) -> Result<()> {
        if self.active_job().is_some() {
            return Err(EngineError::Other("a background job is running on the document".into()));
        }
        let st = self.active_mut().ok_or(EngineError::NoDocument)?;
        if st.kdm_snapshot_steps {
            return Err(EngineError::Other("snapshot history cannot reconcile a file reread".into()));
        }
        let kdm = st.ensure_kdm().ok_or_else(|| EngineError::Other("the document cannot be held in a replica".into()))?;
        kdm.record(&Origin::new(kdm_model::kition_kit::kdm::OriginKind::Import, "file reread"), fresh).map_err(kdm_error)?;
        st.follow_kdm();
        Ok(())
    }

    /// The active document's changes since `since` (`Version::empty()` for all of them).
    pub fn export_changes(&mut self, since: &Version) -> Result<Vec<u8>> {
        self.active_kdm()?.export_changes(since).map_err(kdm_error)
    }

    /// The active document's replica version.
    pub fn version(&mut self) -> Result<Version> {
        Ok(self.active_kdm()?.version())
    }

    /// The active document's replica as a snapshot, for a new replica ([`Session::open_snapshot`]).
    pub fn snapshot(&mut self) -> Result<Vec<u8>> {
        self.active_kdm()?.snapshot().map_err(kdm_error)
    }

    /// SHA-256 of the active document's replicated state: equal on replicas that converged.
    pub fn state_hash(&mut self) -> Result<String> {
        self.active_kdm()?.state_hash().map_err(kdm_error)
    }

    /// The blobs the active document's replica references or can still bring back (for a peer
    /// to [`Session::add_blobs`]).
    pub fn blobs(&mut self) -> Result<&MemoryBlobs> {
        Ok(self.active_kdm()?.blobs())
    }

    /// Adds a peer's blobs to the active document's replica, kept until its next import.
    pub fn add_blobs(&mut self, blobs: &MemoryBlobs) -> Result<()> {
        self.active_kdm()?.add_blobs(blobs);
        Ok(())
    }

    /// The active document's replica blobs a peer at `since` still needs (`Version::empty()`
    /// for every blob the current state references): for live sync, so a change ships only the
    /// blobs it needs rather than the whole store ([`Session::blobs`]).
    pub fn blobs_since(&mut self, since: &Version) -> Result<MemoryBlobs> {
        Ok(MemoryBlobs(self.active_kdm()?.blobs_since(since).into_iter().collect()))
    }

    /// The active document's KDM identity, `None` before it has made a replica (it makes one
    /// lazily; this never forces that, unlike [`Session::version`] and the rest).
    pub fn doc_id(&self) -> Option<String> {
        Some(self.active()?.kdm()?.replica().doc_id().to_string())
    }

    /// Opens the replica a host stored for a `.kphoto` (`stored`, its history) with the file's
    /// content snapshot (`file`) taken into it (`kdm_model::PhotoSession::open_stored_with_file`),
    /// as a new document made active.
    pub fn open_stored_with_file(&mut self, stored: &[u8], file: &[u8], blobs: MemoryBlobs, name: &str) -> Result<usize> {
        let kdm = PhotoSession::open_stored_with_file(stored, file, blobs).map_err(kdm_error)?;
        Ok(self.open_session(kdm, name))
    }

    /// Opens another replica's snapshot as a new document (a new replica of the same KDM
    /// document, so the two exchange changes) and makes it active.
    pub fn open_snapshot(&mut self, bytes: &[u8], blobs: MemoryBlobs, name: &str) -> Result<usize> {
        let kdm = PhotoSession::open_snapshot(bytes, blobs).map_err(kdm_error)?;
        Ok(self.open_session(kdm, name))
    }

    fn open_session(&mut self, mut kdm: PhotoSession, name: &str) -> usize {
        let mut doc = kdm.document().clone();
        // The replica holds no rendered appearance of type, shape and object layers: render
        // it, so the image opened from a `.kphoto` draws and measures as the one saved did.
        if rebuild_caches(&mut doc) {
            kdm.set_view(doc.clone());
        }
        doc.name = name.to_string();
        self.add_document_with(doc, None, Some(kdm))
    }
}

/// The undo scope `origin`'s steps join (KitModel 1.1 M12a): the agents' for an agent, else the
/// person's (imports and repairs are no undo steps and record nothing in a history).
pub(crate) fn scope_of(origin: &Origin) -> UndoScope {
    UndoScope::of(origin.kind).unwrap_or(UndoScope::Person)
}

fn other_scope(scope: UndoScope) -> UndoScope {
    match scope {
        UndoScope::Person => UndoScope::Agent,
        UndoScope::Agent => UndoScope::Person,
    }
}
