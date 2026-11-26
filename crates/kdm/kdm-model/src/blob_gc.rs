//! Which blobs a [`crate::PhotoSession`] still needs, so the rest can be dropped.
//!
//! A blob is kept while the replica's current state references it, while an undo or redo step
//! this session can still take might bring it back, or while it waits to be imported (blobs a
//! peer sent before its changes). Everything else (the tiles of a stroke undone and redone out of
//! reach, of a filter preview step trimmed from history) is garbage: without this a long session
//! keeps every tile it ever painted.
//!
//! The current state's references are counted per object, updated from the objects each
//! transaction, undo, redo or import changed, so nothing rescans the whole document. Each undo
//! step keeps the blobs its transaction displaced (to undo it) and added (to redo it after an
//! undo, even if a collaborator overwrote them meanwhile); a redo step keeps the same for the undo
//! that made it. Steps are kept per undo scope (the person's and the agents', KitModel 1.1 M12a),
//! as the replica keeps them.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use kition_kit::kdm::{ObjectId, Replica, UndoScope};
use kition_kit::session::MemoryBlobs;
use serde_json::Value;

/// The blob hashes one replica operation stopped and started referencing.
#[derive(Default)]
pub(crate) struct Moved {
    pub displaced: HashSet<String>,
    pub added: HashSet<String>,
}

impl Moved {
    fn all(self) -> HashSet<String> {
        let mut all = self.displaced;
        all.extend(self.added);
        all
    }
}

#[derive(Default)]
pub(crate) struct BlobIndex {
    by_object: HashMap<ObjectId, Vec<String>>,
    refs: HashMap<String, u32>,
    /// The person's steps and the agents' (see [`slot`]).
    steps: [Steps; 2],
    /// Blobs added for changes not imported yet.
    pending: HashSet<String>,
}

/// One undo scope's steps.
#[derive(Default)]
struct Steps {
    /// What each undo step can bring back, oldest first.
    undo: VecDeque<HashSet<String>>,
    /// What each redo step can bring back, the next redo last.
    redo: Vec<HashSet<String>>,
}

fn slot(scope: UndoScope) -> usize {
    match scope {
        UndoScope::Person => 0,
        UndoScope::Agent => 1,
    }
}

/// Every `{"hash": …, "mime": …}` (a blob reference) inside `value`.
fn collect(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let (Some(Value::String(hash)), Some(Value::String(_))) = (map.get("hash"), map.get("mime")) {
                out.push(hash.clone());
            }
            for item in map.values() {
                collect(item, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| collect(item, out)),
        _ => {}
    }
}

/// The blobs object `id` references (none if it no longer exists).
fn object_blobs(replica: &Replica, id: &ObjectId) -> Vec<String> {
    let mut out = Vec::new();
    if !replica.exists(id) {
        return out;
    }
    let (registers, tiles): (&[&str], bool) = match replica.object_type(id).as_deref() {
        Ok("Image") => (&["icc", "docState"], false),
        Ok("Layer") => (&["data", "extra", "styleSettings"], true),
        Ok("Surface") => (&["props"], true),
        Ok("Style") => (&["effect"], false),
        Ok("Comp") => (&["states"], false),
        Ok("Path") => (&["psdRaw"], false),
        Ok("Pattern") => (&["pixels"], false),
        _ => (&[], false),
    };
    for field in registers {
        if let Ok(value) = replica.get(id, field) {
            collect(&value, &mut out);
        }
    }
    if tiles && let Ok(entries) = replica.map_entries(id, "tiles") {
        for (_, value) in entries {
            collect(&value, &mut out);
        }
    }
    out
}

impl BlobIndex {
    /// Indexes every object of a freshly loaded or created replica.
    pub fn build(replica: &Replica) -> Self {
        let mut index = Self::default();
        let all: BTreeSet<ObjectId> =
            ["Image", "Layer", "Surface", "Style", "Comp", "Path", "Pattern"].iter().flat_map(|ty| replica.objects_of_type(ty)).collect();
        index.update(replica, &all);
        index
    }

    /// Re-reads the references of the `changed` objects.
    ///
    /// An object's field can list the same hash more than once (two tiles sharing one default
    /// pixel): `before` and `now` are counted as multisets, and only a hash's *net* change in
    /// how many times this one object lists it touches `refs`. Without that, a hash an object
    /// keeps listing (one tile painted while another of the same object is untouched) would
    /// drop to a transient zero and look "gained" again from nothing, even though nothing about
    /// it changed; that falsely marked it `Moved::added` for every later change to the object,
    /// not just the one that actually first added it.
    pub fn update(&mut self, replica: &Replica, changed: &BTreeSet<ObjectId>) -> Moved {
        let mut dropped = Vec::new();
        let mut gained = Vec::new();
        for id in changed {
            let now = object_blobs(replica, id);
            let before = if now.is_empty() { self.by_object.remove(id) } else { self.by_object.insert(id.clone(), now.clone()) }.unwrap_or_default();
            let mut before_counts: HashMap<&str, i64> = HashMap::new();
            for hash in &before {
                *before_counts.entry(hash.as_str()).or_insert(0) += 1;
            }
            let mut now_counts: HashMap<&str, i64> = HashMap::new();
            for hash in &now {
                *now_counts.entry(hash.as_str()).or_insert(0) += 1;
            }
            let mut hashes: BTreeSet<&str> = before_counts.keys().copied().collect();
            hashes.extend(now_counts.keys().copied());
            for hash in hashes {
                let delta = now_counts.get(hash).copied().unwrap_or(0) - before_counts.get(hash).copied().unwrap_or(0);
                if delta > 0 {
                    let count = self.refs.entry(hash.to_owned()).or_insert(0);
                    if *count == 0 {
                        gained.push(hash.to_owned());
                    }
                    *count += u32::try_from(delta).unwrap_or(u32::MAX);
                } else if delta < 0
                    && let Some(count) = self.refs.get_mut(hash)
                {
                    *count = count.saturating_sub(u32::try_from(-delta).unwrap_or(u32::MAX));
                    if *count == 0 {
                        self.refs.remove(hash);
                        dropped.push(hash.to_owned());
                    }
                }
            }
        }
        let mut moved = Moved::default();
        for hash in dropped {
            if !self.refs.contains_key(&hash) {
                moved.displaced.insert(hash);
            }
        }
        for hash in gained {
            if !moved.displaced.remove(&hash) {
                moved.added.insert(hash);
            }
        }
        moved
    }

    fn scope(&mut self, scope: UndoScope) -> &mut Steps {
        let [person, agent] = &mut self.steps;
        if slot(scope) == 0 { person } else { agent }
    }

    /// A transaction that became a new undo step of `scope`.
    pub fn push_step(&mut self, scope: UndoScope, moved: Moved) {
        let steps = self.scope(scope);
        steps.undo.push_back(moved.all());
        steps.redo.clear();
    }

    /// A transaction that joined the latest undo step of `scope` (an undo group).
    pub fn join_step(&mut self, scope: UndoScope, moved: Moved) {
        let steps = self.scope(scope);
        match steps.undo.back_mut() {
            Some(step) => step.extend(moved.all()),
            None => steps.undo.push_back(moved.all()),
        }
        steps.redo.clear();
    }

    /// An undo of `scope` took `taken` steps off the replica's stack (those it passed over, with
    /// nothing left to change, and the one it took back, `done`): one redo step brings them back
    /// together (`UndoOutcome`).
    pub fn undone(&mut self, scope: UndoScope, taken: usize, done: bool, moved: Moved) {
        let steps = self.scope(scope);
        for _ in 0..taken {
            steps.undo.pop_back();
        }
        if done {
            steps.redo.push(moved.all());
        }
    }

    /// The mirror of [`BlobIndex::undone`] for a redo.
    pub fn redone(&mut self, scope: UndoScope, taken: usize, done: bool, moved: Moved) {
        let steps = self.scope(scope);
        for _ in 0..taken {
            steps.redo.pop();
        }
        if done {
            steps.undo.push_back(moved.all());
        }
    }

    pub fn undo_steps(&self, scope: UndoScope) -> usize {
        self.steps.get(slot(scope)).map_or(0, |s| s.undo.len())
    }

    pub fn redo_steps(&self, scope: UndoScope) -> usize {
        self.steps.get(slot(scope)).map_or(0, |s| s.redo.len())
    }

    /// Every hash the current state references (not the extra copies undo, redo or a pending
    /// import keep around): what a peer starting from nothing needs.
    pub fn referenced(&self) -> Vec<String> {
        self.refs.keys().cloned().collect()
    }

    /// Keeps only the newest `undo` undo steps and the next `redo` redo steps of `scope`: the
    /// older ones can no longer be taken, so their blobs can go.
    pub fn retain(&mut self, scope: UndoScope, undo: usize, redo: usize) {
        let steps = self.scope(scope);
        while steps.undo.len() > undo {
            steps.undo.pop_front();
        }
        if steps.redo.len() > redo {
            let extra = steps.redo.len() - redo;
            steps.redo.drain(..extra);
        }
    }

    pub fn protect(&mut self, hashes: impl IntoIterator<Item = String>) {
        self.pending.extend(hashes);
    }

    pub fn imported(&mut self) {
        self.pending.clear();
    }

    /// Drops every blob nothing above needs.
    pub fn collect_garbage(&self, blobs: &mut MemoryBlobs) {
        blobs.0.retain(|hash, _| {
            self.refs.contains_key(hash)
                || self.pending.contains(hash)
                || self.steps.iter().any(|steps| steps.undo.iter().chain(&steps.redo).any(|s| s.contains(hash)))
        });
    }
}
