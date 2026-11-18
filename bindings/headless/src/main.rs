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
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

fn main() -> std::process::ExitCode {
    kition_photo_kit::gate_commands(kition_photo_automation::workspace::authorize_engine_step);
    kition_headless::run_manifest_model::<kition_photo_kit::PhotoDocument>(
        include_str!("../../../kit.json"),
        kition_photo_kit::SANDBOXED,
        kition_photo_kit::EDITOR_STATE,
    )
}
