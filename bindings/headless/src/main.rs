//! Kition Photo: the image editor as a Kition kit, headless, for Kition's agent and the
//! conformance suite (Kit Process Protocol over `kition-headless`, the same path every kit takes).
//!
//! Every document runs its commands through the automation engine's gate
//! ([`kition_photo_automation::workspace::authorize_engine_step`]): commands that reach the file
//! system on their own (an `.icc` path, a preferences section that names folders, plug-ins) are
//! refused, including nested `actions.play` steps, and the ones it always refuses are not listed.
//! File parameters are typed (`kition_photo_kit::files`), so the commands that take them are not
//! offered to agents, and an agent's call (or anyone's but a person's) is refused them however it
//! arrives. The host owns every file (`kition-headless` says so through `kition_kit::host`): the
//! engine reads and writes no file by path, starts no program and scans no font folder, and reads a
//! linked object layer's source only through the host, never outside the workspace.
//!
//! The identity a file carries (`document.identity`, `document.fork`): a `.kphoto` bundle embeds
//! its replica (`kdm.bin`), so a byte copy is the same document and a fork holds a copy under a new
//! id ([`PhotoFiles`]); a PSD or PSB embeds none.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::sync::Arc;

use kition_headless::{EmbeddedReplica, FileIdentity, Headless, KitInfo};
use kition_kit_server::{ErrorReason, RpcError};
use kition_photo_engine::kdm_model::kition_kit::kdm::Schema;
use kition_photo_engine::kdm_model::kition_kit::session::KitModel;

/// Where this kit's files keep the document's replica (Kit Process Protocol section 5.6): a
/// `.kphoto` bundle's `kdm.bin` and `blobs/kdm/` entries
/// (`kition_photo_engine::kition_photo_bridge::embedded_replica` and `replace_replica`).
struct PhotoFiles;

fn unsupported(error: impl std::fmt::Display) -> RpcError {
    RpcError::kit_detail(ErrorReason::UnsupportedFormat, error.to_string())
}

impl EmbeddedReplica for PhotoFiles {
    fn schema(&self) -> Arc<Schema> {
        kition_photo_engine::kdm_model::PhotoModel::schema()
    }

    fn replica(&self, file: &[u8]) -> Result<Option<Vec<u8>>, RpcError> {
        kition_photo_engine::kition_photo_bridge::embedded_replica(file).map_err(unsupported)
    }

    fn replace(&self, file: &[u8], snapshot: &[u8]) -> Result<Vec<u8>, RpcError> {
        kition_photo_engine::kition_photo_bridge::replace_replica(file, snapshot).map_err(unsupported)
    }
}

fn main() -> std::process::ExitCode {
    kition_photo_kit::gate_commands(kition_photo_automation::workspace::authorize_engine_step);
    let kit = match KitInfo::from_manifest(include_str!("../../../kit.json"), kition_photo_kit::SANDBOXED, kition_photo_kit::EDITOR_STATE) {
        Ok(kit) => kit,
        Err(error) => {
            eprintln!("kit: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let backend = Headless::<kition_photo_kit::PhotoDocument>::with_replica_port(kit.clone()).with_file_identity(FileIdentity::Embedded(Arc::new(PhotoFiles)));
    kition_headless::serve_backend(&kit, backend)
}
