//! `.kphoto` open and save with the document's KDM replica (Kition phase 3a, Task 4).
//!
//! The native bundle stores the replica snapshot (`kdm.bin`) and the blobs it references next
//! to the usual manifest and tiles (`kition_photo_format`). Opening a bundle that has one restores
//! the replica through [`Session::open_snapshot`], so the document keeps its KDM identity and
//! undo history across saves; a bundle without one (or any other format) opens and saves exactly
//! as before, and the replica is then made lazily on the first shared edit (see [`crate::kdm_bridge`]).

use std::sync::Arc;

use crate::kdm_bridge::kdm_error;
use crate::{DocState, EngineError, MemoryBlobs, Result, Session};

/// Whether `name` names a `.kphoto` bundle by its extension, as `kition_photo_io::import` decides.
pub fn is_kition_photo_name(name: &str) -> bool {
    name.rsplit(['/', '\\'])
        .next()
        .and_then(|base| base.rsplit_once('.'))
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case(kition_photo_format::EXTENSION))
}

/// The replica a file of this kit embeds, as a KDM snapshot (`document.identity`, KitModel M7a):
/// a `.kphoto` bundle's `kdm.bin`, which [`Session::open_file`] opens the document from, so a byte
/// copy of the bundle is the same document. `None` for a bundle saved without a replica, a PSD or
/// PSB (Photoshop's format embeds none: the kit makes a replica under a new id each time it opens
/// one), and an empty file (one made for the kit before it saved there). Other bytes are not one
/// of the kit's files and are refused; never writes.
pub fn embedded_replica(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    if bytes.is_empty() || bytes.starts_with(b"8BPS") {
        return Ok(None);
    }
    if !kition_photo_format::is_kphoto(bytes) {
        return Err(EngineError::Other("not a Kition Photo document (.kphoto) or a Photoshop document (.psd, .psb)".into()));
    }
    kition_photo_format::read_kdm_snapshot(bytes).map_err(kdm_error)
}

/// `bytes` (a `.kphoto` bundle with a replica) holding the document `snapshot` instead of its own
/// (`document.fork`, KitModel M7a): `kdm.bin` is `snapshot`, and the replica's blobs are only the
/// ones its current state references, so no earlier identity or history (a deleted layer's tiles,
/// what an undo step kept) stays in the file. The manifest, tiles, previews and entries this build
/// does not know are kept: they hold the same image.
pub fn replace_replica(bytes: &[u8], snapshot: &[u8]) -> Result<Vec<u8>> {
    use kdm_model::kition_kit::kdm::{ActorId, Replica};
    use kdm_model::kition_kit::session::KitModel;
    let replica = Replica::load(kdm_model::PhotoModel::schema(), snapshot, ActorId::random()).map_err(kdm_error)?;
    let keep = kdm_model::state_blobs(&replica);
    kition_photo_format::replace_kdm(bytes, snapshot, &|hash| keep.contains(hash)).map_err(kdm_error)
}

impl Session {
    /// Opens any file `kition_photo_io::import` supports as a new document; a `.kphoto` whose
    /// bundle holds a KDM replica opens it with [`Session::open_snapshot`] instead, keeping its
    /// identity and undo history across saves. Answers the new document's index (made active).
    pub fn open_file(&mut self, name: &str, bytes: &[u8]) -> Result<usize> {
        if (is_kition_photo_name(name) || kition_photo_format::is_kphoto(bytes))
            && let Some(bundle) = kition_photo_format::read_kdm(bytes).map_err(kdm_error)?
        {
            let blobs = MemoryBlobs(bundle.blobs.into_iter().map(|(hash, data)| (hash, Arc::<[u8]>::from(data))).collect());
            return self.open_snapshot(&bundle.snapshot, blobs, name);
        }
        let mut imported = kition_photo_io::import(name, bytes).map_err(|error| EngineError::Other(error.to_string()))?;
        // A native bundle without a replica may omit rendered previews too. Resolve only
        // missing caches, through the host's lender when files belong to the host.
        if self.host_files_only && (is_kition_photo_name(name) || kition_photo_format::is_kphoto(bytes)) {
            crate::kdm_bridge::rebuild_caches(&mut imported.document);
        }
        Ok(self.add_document(imported.document, Some(name.to_owned())))
    }
}

impl DocState {
    /// Saves this document as a `.kphoto` bundle, with its KDM replica if it has made one yet
    /// (see [`DocState::kdm`]); one that hasn't saves without it, as before (and gets one lazily
    /// on its first shared edit).
    pub fn save_kphoto(&self) -> Result<Vec<u8>> {
        let doc = &self.doc;
        let kdm = self
            .kdm()
            .map(|kdm| -> Result<kition_photo_format::KdmSave> {
                Ok(kition_photo_format::KdmSave {
                    snapshot: kdm.snapshot().map_err(kdm_error)?,
                    blobs: kdm.blobs().0.iter().map(|(hash, data)| (hash.clone(), data.clone())).collect(),
                })
            })
            .transpose()?;
        let opts = kition_photo_format::SaveOptions {
            thumbnail: Some(kition_photo_compose::thumbnail(doc, 256)),
            composite: Some(kition_photo_compose::thumbnail(doc, 1024)),
            kdm,
        };
        kition_photo_format::save_to_bytes(doc, &opts).map_err(|error| EngineError::Other(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn painted(session: &mut Session) {
        session.execute("layer.new.layer", json!({"name": "Ink"})).unwrap();
        session.execute("paint.stroke", json!({"points": [[4, 4, 1], [50, 30, 1]], "size": 6, "color": "#ff0000"})).unwrap();
    }

    #[test]
    fn a_kition_photo_without_a_replica_round_trips_as_before() {
        let mut a = Session::new();
        a.execute("file.new", json!({"width": 64, "height": 40, "background": "white"})).unwrap();
        let bytes = a.active().unwrap().save_kphoto().unwrap();
        assert!(kition_photo_format::read_kdm(&bytes).unwrap().is_none(), "no replica was ever made");

        let mut b = Session::new();
        let index = b.open_file("round.kphoto", &bytes).unwrap();
        assert!(b.set_active(index));
        assert_eq!(b.active().unwrap().doc.size, a.active().unwrap().doc.size);
    }

    /// Edit, save, load: the reopened document keeps the replica's identity (same version and
    /// `state_hash`). Undo is this person's own editing session (`DocState::history`'s
    /// snapshots, which steps the replica back as collaborators' edits arrive); it is not part
    /// of the replica bytes (KDM's own undo manager, loro's, is rebuilt fresh from a loaded
    /// snapshot too), so a freshly reopened document starts with none of its own, cleanly: it is
    /// not stuck, it can take new edits (and undo those) right away. The document that saved it
    /// keeps its own undo history untouched by the save.
    #[test]
    fn a_kition_photo_with_a_replica_keeps_its_identity_across_a_reload() {
        let mut a = Session::new();
        a.execute("file.new", json!({"width": 64, "height": 40, "background": "white"})).unwrap();
        painted(&mut a);
        let version_before = a.version().unwrap();
        let hash_before = a.state_hash().unwrap();
        let bytes = a.active().unwrap().save_kphoto().unwrap();
        assert!(kition_photo_format::read_kdm(&bytes).unwrap().is_some());

        let mut b = Session::new();
        let index = b.open_file("round.kphoto", &bytes).unwrap();
        assert!(b.set_active(index));
        assert_eq!(b.version().unwrap(), version_before, "the replica's version survived the round trip");
        assert_eq!(b.state_hash().unwrap(), hash_before);

        assert!(!b.undo(), "history restarts cleanly: nothing of b's own to undo yet");
        painted(&mut b);
        assert!(b.undo(), "b can undo its own edits right away");

        assert!(a.undo(), "saving did not disturb a's own undo history");
    }

    /// A saves a `.kphoto`; B opens it (so both start from the same replica); A paints some more;
    /// B imports A's changes (with blob transfer) and converges to the same state.
    #[test]
    fn two_documents_exchange_changes_through_a_kition_photo_round_trip() {
        let mut a = Session::new();
        a.execute("file.new", json!({"width": 64, "height": 40, "background": "white"})).unwrap();
        painted(&mut a);
        let bytes = a.active().unwrap().save_kphoto().unwrap();

        let mut b = Session::new();
        let index = b.open_file("shared.kphoto", &bytes).unwrap();
        assert!(b.set_active(index));

        a.execute("paint.stroke", json!({"points": [[10, 10, 1], [40, 25, 1]], "size": 8, "color": "#00ff00"})).unwrap();

        let since = b.version().unwrap();
        let changes = a.export_changes(&since).unwrap();
        b.add_blobs(a.blobs().unwrap()).unwrap();
        b.import_changes(&changes).unwrap();

        assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
        assert_eq!(a.active().unwrap().doc.layers.len(), b.active().unwrap().doc.layers.len());
    }

    #[test]
    fn is_kition_photo_name_checks_the_extension() {
        assert!(is_kition_photo_name("doc.kphoto"));
        assert!(is_kition_photo_name("DOC.kphoto"));
        assert!(!is_kition_photo_name("doc.psd"));
        assert!(!is_kition_photo_name("kphoto"));
    }
}
