//! # kition-photo-automation
//!
//! Agent-facing automation (architecture §12):
//!
//! * [`Headless`]: the synchronous session + file I/O core over an in-process
//!   [`kition_photo_engine::Session`]. The headless kit (`bindings/headless`) drives it;
//!   agents reach it through the headless kit.
//! * [`rpc`]: a headless JSON-lines server (stdio or loopback TCP) with the
//!   control protocol's envelope.
//! * [`files`]: open/save any supported format, `.kphoto` natively.
//!
//! L6, no UI-toolkit dependencies.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod budgets;
pub mod files;
pub mod headless;
pub mod rpc;
pub mod security;
pub mod workspace;

pub use headless::Headless;
pub use workspace::AuthorizedWorkspace;

#[derive(Debug, thiserror::Error)]
pub enum AutomationError {
    #[error("{0}")]
    BadRequest(String),
    #[error("I/O: {0}")]
    Io(String),
    #[error(transparent)]
    Engine(#[from] kition_photo_engine::EngineError),
    #[error(transparent)]
    Import(#[from] kition_photo_io::IoError),
    #[error(transparent)]
    Format(#[from] kition_photo_format::FormatError),
    #[error("{0}")]
    Other(String),
}
