//! What `outside.rs` and `receipts.rs` share (task card C-photo): images of numbered blocks built
//! with this kit's engine, the `kition-photo` executable driven through the kits' MCP server's
//! tools (`kition_kits::tools`, the library `kition-kits-mcp` serves) as an agent drives it, and a
//! colleague's program other than that server that changes the file: this kit's engine, an instance
//! of its own, opening the file, running commands and saving it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use kition_kit::KitDocument;
use kition_kits::tools::{self, CallOptions};
use kition_kits::{KitDefinition, KitManager};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

pub const ME: &str = "agent-me";

pub fn kit() -> &'static KitDefinition {
    KitDefinition::from_manifest_text(include_str!("../../../../kit.json")).unwrap()
}

/// The image's grid: 256-pixel tiles, five across.
pub const TILE: i32 = 256;
pub const ACROSS: i32 = 5;
pub const BLOCK: i32 = 120;

/// Where block `L<n>` stands: `[x, y, width, height]`, 60 pixels into tile `(n % 5, n / 5)`, so
/// each layer's pixels lie in a tile of their own.
pub fn block(n: usize) -> [i32; 4] {
    let n = i32::try_from(n).unwrap();
    [TILE * (n % ACROSS) + 60, TILE * (n / ACROSS) + 60, BLOCK, BLOCK]
}

/// The point at the middle of block `n`.
pub fn middle(n: usize) -> [i32; 2] {
    let [x, y, w, h] = block(n);
    [x + w / 2, y + h / 2]
}

/// The region an agent fills on block `n`: its top-left 40 x 40 corner, 10 pixels in.
pub fn region(n: usize) -> [i32; 4] {
    let [x, y, ..] = block(n);
    [x + 10, y + 10, 40, 40]
}

/// A white image, `TILE * 5` wide and as tall as `layers` blocks need, with layers `L0`, `L1`, …
/// above the background, each a block of its own color at [`block`]; nothing selected.
pub fn image_doc(name: &str, layers: usize) -> PhotoDocument {
    let rows = i32::try_from(layers.div_ceil(ACROSS as usize)).unwrap().max(1);
    let mut doc = PhotoDocument::create(name, u32::try_from(TILE * ACROSS).unwrap(), u32::try_from(TILE * rows).unwrap()).unwrap();
    for n in 0..layers {
        doc.execute("layer.new.layer", json!({ "name": format!("L{n}") })).unwrap();
        let [x, y, w, h] = block(n);
        doc.execute("select.rect", json!({ "x": x, "y": y, "width": w, "height": h })).unwrap();
        let shade = 40 + (n * 9) % 200;
        doc.execute("edit.fill", json!({ "color": format!("#{shade:02x}{:02x}{:02x}", 255 - shade, (shade * 3) % 256) })).unwrap();
    }
    doc.execute("select.deselect", json!({})).unwrap();
    doc
}

pub fn image(name: &str, layers: usize) -> Vec<u8> {
    image_doc(name, layers).save(Some(name)).unwrap()
}

/// Sets the file's modification time an hour before its own, so that only its content tells the
/// server it changed.
fn backdate(file: &Path, before: std::time::SystemTime) {
    std::fs::File::options().write(true).open(file).unwrap().set_modified(before - Duration::from_secs(3600)).unwrap();
}

/// A colleague's program: this kit's engine, its own instance, opening the file from disk,
/// running `steps` (commands and parameters, as the person runs them) and saving the file back.
pub fn colleague(file: &Path, steps: &[(&str, Value)]) -> Vec<Value> {
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    let at = std::fs::metadata(file).unwrap().modified().unwrap();
    let mut doc = PhotoDocument::open(&name, &std::fs::read(file).unwrap()).unwrap();
    let answers =
        steps.iter().map(|(command, params)| doc.execute(command, params.clone()).unwrap_or_else(|error| panic!("colleague's {command}: {error}"))).collect();
    std::fs::write(file, doc.save(Some(&name)).unwrap()).unwrap();
    backdate(file, at);
    answers
}

/// The file as a new engine reads it.
pub fn on_disk(file: &Path) -> PhotoDocument {
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    PhotoDocument::open(&name, &std::fs::read(file).unwrap()).unwrap()
}

/// Every layer of an inspect's tree, top first.
pub fn layers_of(inspected: &Value) -> Vec<Value> {
    fn walk(layers: &Value, out: &mut Vec<Value>) {
        for layer in layers.as_array().into_iter().flatten() {
            out.push(layer.clone());
            walk(&layer["children"], out);
        }
    }
    let mut out = Vec::new();
    walk(&inspected["layers"], &mut out);
    out
}

/// Layer `id` as a new engine's inspect of the file lists it (`None` when the file has none).
pub fn layer_on_disk(file: &Path, id: u64) -> Option<Value> {
    layers_of(&on_disk(file).inspect().unwrap()).into_iter().find(|layer| layer["id"] == id)
}

/// The ids of the layers an inspect lists, by name.
pub fn ids_by_name(inspected: &Value) -> std::collections::BTreeMap<String, u64> {
    layers_of(inspected).iter().map(|layer| (layer["name"].as_str().unwrap().to_owned(), layer["id"].as_u64().unwrap())).collect()
}

static WORKSPACES: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty directory of this test process.
fn workspace() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-photo-agent-{}-{}", std::process::id(), WORKSPACES.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

pub struct Fixture {
    pub kits: KitManager,
    pub options: CallOptions,
    pub workspace: PathBuf,
    pub name: String,
}

impl Fixture {
    /// An image of `layers` blocks saved as `name` (`.kphoto` or `.psd`).
    pub fn new(name: &str, layers: usize) -> Self {
        Self::with(name, layers, CallOptions::default())
    }

    pub fn with(name: &str, layers: usize, options: CallOptions) -> Self {
        Self::from_bytes(name, &image(name, layers), options)
    }

    pub fn from_bytes(name: &str, bytes: &[u8], options: CallOptions) -> Self {
        let workspace = workspace();
        std::fs::write(workspace.join(name), bytes).unwrap();
        let kits = KitManager::new(Some(workspace.clone()), vec![(kit(), Some(PathBuf::from(env!("CARGO_BIN_EXE_kition-photo"))))]);
        Self { kits, options, workspace, name: name.to_owned() }
    }

    pub fn file(&self) -> PathBuf {
        self.workspace.join(&self.name)
    }

    pub fn colleague(&self, steps: &[(&str, Value)]) -> Vec<Value> {
        colleague(&self.file(), steps)
    }

    pub async fn call(&self, action: &str, mut input: Value) -> Result<Value, Value> {
        input["path"] = json!(self.name);
        match tools::call_as(&self.kits, &format!("photo_{action}"), &input, self.options, ME).await {
            Some(Ok(output)) => Ok(output.output),
            Some(Err(error)) => Err(error.structured()),
            None => panic!("photo_{action} is no tool"),
        }
    }

    pub async fn ok(&self, action: &str, input: Value) -> Value {
        self.call(action, input).await.unwrap_or_else(|error| panic!("photo_{action}: {error}"))
    }

    pub async fn refused(&self, action: &str, input: Value) -> Value {
        match self.call(action, input).await {
            Err(error) => error,
            Ok(answer) => panic!("photo_{action} was expected to be refused: {answer}"),
        }
    }

    /// The image as the agent inspects it (`details`), and the answer's `revision`.
    pub async fn inspect(&self) -> (Value, String) {
        let seen = self.ok("inspect", json!({})).await;
        (seen["details"].clone(), revision(&seen))
    }

    pub async fn shutdown(self) {
        self.kits.shutdown().await;
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

pub fn revision(answer: &Value) -> String {
    answer["revision"].as_str().unwrap_or_else(|| panic!("a revision: {answer}")).to_owned()
}

/// An id as an answer gives it, as text: an integer tool id is a number in receipts, `others` and
/// conflicts, a string elsewhere.
pub fn id_text(id: &Value) -> Option<String> {
    id.as_str().map(str::to_owned).or_else(|| id.as_u64().map(|id| id.to_string()))
}

/// The ids a change list names.
pub fn ids(list: &Value) -> Vec<String> {
    list.as_array().map(|items| items.iter().filter_map(|item| id_text(&item["id"])).collect()).unwrap_or_default()
}

/// The entry of `list` naming `id`.
pub fn entry<'a>(list: &'a Value, id: &str) -> &'a Value {
    list.as_array().and_then(|items| items.iter().find(|item| id_text(&item["id"]).as_deref() == Some(id))).unwrap_or_else(|| panic!("no {id} in {list}"))
}

/// A refusal of a write whose premises others changed, naming `id` with `reason`.
pub fn assert_conflict<'a>(refused: &'a Value, id: &str, reason: &str) -> &'a Value {
    assert_eq!(refused["error"]["code"], "conflict", "{refused}");
    let found = refused["conflicts"].as_array().and_then(|conflicts| conflicts.iter().find(|conflict| id_text(&conflict["id"]).as_deref() == Some(id)));
    let found = found.unwrap_or_else(|| panic!("no conflict on {id}: {refused}"));
    assert_eq!(found["reason"], reason, "{refused}");
    found
}

/// The median of `samples`, in milliseconds.
pub fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples.get(samples.len() / 2).copied().unwrap_or(f64::NAN)
}

/// A small generator of the mixed rounds (splitmix64), printed by its seed.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A whole number in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap()).unwrap()
    }
}
