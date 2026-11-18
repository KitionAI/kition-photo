//! "Host files only" (KHP A29, A42, section 10.6): a process that serves a host's documents (a
//! Kition kit, headless or in the page) reads and writes no file by path. Every by-path entry point
//! of this crate ([`crate::read_file`], [`crate::atomic_write`], directory bundles, auto-save and
//! recovery) refuses from then on, and so do the engine's and the PSD exporter's own reads, which
//! ask [`refuse`] first. Files a document links to (a linked object layer's source) are read only
//! through the host's lender ([`read_linked`]), which resolves them inside the workspace and
//! refuses the rest; a link that names a place outside the workspace is never even asked for.
//!
//! The switch is one-way and process-wide: a kit's process serves only documents whose files
//! belong to the host. `kition-photo-kit` sets it when the SDK says the host owns every file
//! (`kition_kit::host::hosting`, `KitDocument::hosted`).

use std::io;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static ONLY: AtomicBool = AtomicBool::new(false);
type FileLender = fn(&str) -> Option<Vec<u8>>;
static LENDER: OnceLock<FileLender> = OnceLock::new();

/// From now on this process reaches no file by path; `lender` reads the files documents link to
/// through the host (a path relative to the document's folder, inside the workspace).
pub fn set_host_files_only(lender: fn(&str) -> Option<Vec<u8>>) {
    let _ = LENDER.set(lender);
    ONLY.store(true, Ordering::SeqCst);
}

/// Whether the host owns every file ([`set_host_files_only`]).
#[must_use]
pub fn host_files_only() -> bool {
    ONLY.load(Ordering::SeqCst)
}

/// What every by-path access answers while the host owns every file.
#[must_use]
pub fn refused(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("{}: this kit works only on the files of its workspace, through its host, and reads or writes no file by path", path.display()),
    )
}

/// `Err` ([`refused`]) while the host owns every file: callers that reach the file system on their
/// own ask this first.
pub fn refuse(path: &Path) -> io::Result<()> {
    if host_files_only() { Err(refused(path)) } else { Ok(()) }
}

/// Whether a document's link names a place only the file system could reach, never the host's
/// workspace: an absolute path, a drive or UNC path, a URL (`file:` and every other scheme), a
/// `..` step, a backslash, or nothing at all. Such a link stays as data and is reported missing.
#[must_use]
pub fn link_leaves_workspace(link: &str) -> bool {
    link.trim().is_empty()
        || link.starts_with('/')
        || link.starts_with('~')
        || link.contains('\\')
        || link.contains(':')
        || link.split('/').any(|part| part == "..")
        || link.chars().any(char::is_control)
}

/// The bytes of the file a document links to (a linked object layer's source), or `None` when it
/// cannot be read. While the host owns every file, only the host's lender is asked, and only for a
/// link inside the workspace ([`link_leaves_workspace`]); otherwise the file system is read.
#[must_use]
pub fn read_linked(link: &str) -> Option<Vec<u8>> {
    if link.is_empty() {
        return None;
    }
    if host_files_only() {
        if link_leaves_workspace(link) {
            return None;
        }
        return LENDER.get().and_then(|lend| lend(link));
    }
    crate::read::read_file(Path::new(link)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_that_leave_the_workspace() {
        for link in ["/etc/passwd", "../outside/a.png", "a/../../b.png", "file:///tmp/a.png", "https://example.com/a.png"] {
            assert!(link_leaves_workspace(link), "{link}");
        }
        for link in ["C:\\a.png", "\\\\host\\share\\a.png", "~/a.png", "", " "] {
            assert!(link_leaves_workspace(link), "{link}");
        }
        for link in ["a.png", "images/a.png", "./a.png", "a..b.png"] {
            assert!(!link_leaves_workspace(link), "{link}");
        }
    }
}
