//! Runs the Kition Kit Protocol conformance suite against the `kition-photo` binary.
//!
//! The suite is a separate program from the Kition repository. Set `KITION_KIT_CONFORMANCE` to
//! its path to run this test; without it the test only reports that it was skipped.
//!
//! ```sh
//! cargo install --git https://github.com/KitionAI/kition kition-kit-conformance
//! KITION_KIT_CONFORMANCE="$(command -v kition-kit-conformance)" cargo test -p kition-photo --test conformance
//! ```

use std::path::PathBuf;
use std::process::Command;

use kition_photo_automation::Headless;
use kition_photo_io::ExportOptions;
use serde_json::json;

/// A small layered PSD written by the engine itself.
fn fixture(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("fixture.psd");
    let mut engine = Headless::trusted_local();
    engine.command_run("file.new", json!({ "width": 96, "height": 64, "background": "white" })).unwrap();
    engine.command_run("layer.new.layer", json!({ "name": "Base" })).unwrap();
    engine.save(None, Some(&path), Some("psd"), &ExportOptions::default()).unwrap();
    path
}

#[test]
fn kition_photo_passes_the_conformance_suite() {
    let Some(suite) = std::env::var_os("KITION_KIT_CONFORMANCE").filter(|value| !value.is_empty()) else {
        eprintln!("skipped: set KITION_KIT_CONFORMANCE to the kition-kit-conformance binary");
        return;
    };
    let dir = std::env::temp_dir().join(format!("kition-photo-conformance-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fixture = fixture(&dir);
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../kit.json");
    let output = Command::new(suite)
        .arg("--kit")
        .arg(env!("CARGO_BIN_EXE_kition-photo"))
        .arg("--manifest")
        .arg(&manifest)
        .arg("--fixture")
        .arg(&fixture)
        .args(["--edit-command", "layer.new.layer", "--edit-params", r#"{"name":"Ink"}"#])
        .output()
        .unwrap();
    let report = String::from_utf8_lossy(&output.stdout);
    println!("{report}");
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(output.status.success(), "conformance failed:\n{report}");
}
