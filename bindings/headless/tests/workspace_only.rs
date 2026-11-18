//! `kition-photo` reads and writes only inside its workspace, and only through its host (KHP A29,
//! A37, A42, A45, section 10.6): whatever an agent runs, nothing appears in the process's current
//! folder, its home folder or its temporary folder, no program is started, nothing outside the
//! workspace is listed, read into an answer, a render, a save or an export, changed or deleted,
//! and the workspace holds only what the test put there. An object layer's link is the document's
//! data: one that names a file outside the workspace (an absolute path, `../`, a `file:` URL) is
//! never read, and the kit's check reports it as `photo.missing-object-source`; one that names a
//! file inside the workspace is read through the host and shown.
//!
//! The test runs itself again as a child process whose current folder, `HOME` and `TMPDIR` are
//! fresh empty folders and whose `PATH` holds only stand-ins for the programs an editor starts
//! (each one leaves a mark and does nothing else), so every by-path write and every program start
//! lands where it can be seen. A folder beside them (`outside/`) holds this test's sentinel files,
//! which the agent names by path: directly, inside `model.batch` (no origin, an agent's, a claimed
//! person's), through the kit's own documents (`PhotoDocument::execute_as` as an agent and as a
//! person, the `actions.play` dispatcher, editable filter parameters, the object model's layer
//! properties) and from document files that link to them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit::KitDocument;
use kition_kit_server::protocol::{Origin, ScreenshotScope};
use kition_kit_server::{KitBackend, OpenRequest, ScreenshotRequest};
use kition_photo_engine::Session;
use kition_photo_engine::doc::{LayerContent, SmartSource};
use kition_photo_kit::PhotoDocument;
use serde_json::{Map, Value, json};

const MANIFEST: &str = include_str!("../../../kit.json");
/// Set in the child: the folder holding `workspace/`, `outside/` and the scratch folders.
const CHILD: &str = "KITION_PHOTO_TEST_SANDBOX";
/// The picture outside the workspace, and one inside it.
const OUTSIDE_RGB: [u8; 3] = [17, 233, 71];
const INSIDE_RGB: [u8; 3] = [201, 40, 160];
/// Text only files outside the workspace hold: it must reach no answer.
const SECRET: &str = "SENTINEL-4c2e";
/// A file outside the workspace that no call names: only a listing could find it.
const LISTED_ONLY: &str = "listed-only-4c2e.txt";
/// The programs an editor may start (reveal in the file manager, print, the OS locale, scripts):
/// stand-ins on the child's `PATH` that only leave a mark in `spawned/`.
const PROGRAMS: &[&str] =
    &["open", "xdg-open", "explorer", "lp", "lpr", "lpstat", "reg", "osascript", "say", "sysctl", "sw_vers", "powershell", "cmd", "sh-helper"];
/// The documents the test writes into the workspace.
const WORKSPACE: &[&str] = &[
    "basic.psd",
    "inside.png",
    "link-absolute.kphoto",
    "link-absolute-cached.kphoto",
    "link-parent.kphoto",
    "link-file-url.kphoto",
    "link-inside.kphoto",
    "lut-builtin.kphoto",
    "lut-outside.kphoto",
];

/// Every file and folder under `dir`, relative to it, with a file's bytes.
fn tree(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = path.strip_prefix(dir).unwrap_or(&path).to_string_lossy().into_owned();
            if path.is_dir() {
                stack.push(path.clone());
                out.push((name, None));
            } else {
                out.push((name, std::fs::read(&path).ok()));
            }
        }
    }
    out.sort();
    out
}

fn names(dir: &Path) -> Vec<String> {
    tree(dir).into_iter().map(|(name, _)| name).collect()
}

fn params(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// Whether some pixel of `rgba` is within 8 of `rgb` in every channel.
fn shows(rgba: &[u8], rgb: [u8; 3]) -> bool {
    rgba.as_chunks::<4>().0.iter().any(|px| px[3] > 0 && (0..3).all(|c| px[c].abs_diff(rgb[c]) <= 8))
}

/// Whether `bytes` holds `needle` anywhere.
fn holds(bytes: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && bytes.windows(needle.len()).any(|window| window == needle)
}

/// A 16 x 16 PNG of one colour, made by this kit's engine (not hosted: the test's own process).
fn png(rgb: [u8; 3]) -> Vec<u8> {
    let mut doc = PhotoDocument::create("pixel.png", 16, 16).unwrap();
    doc.execute("select.all", json!({})).unwrap();
    doc.execute("edit.fill", json!({ "color": format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]) })).unwrap();
    doc.save(Some("pixel.png")).unwrap()
}

/// A 40 x 40 white image with one pixel layer, as a PSD.
fn basic_psd() -> Vec<u8> {
    let mut doc = PhotoDocument::create("basic.psd", 40, 40).unwrap();
    doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap();
    doc.execute("select.rect", json!({ "x": 4, "y": 4, "width": 10, "height": 10 })).unwrap();
    doc.execute("edit.fill", json!({ "color": "#3060c0" })).unwrap();
    doc.execute("select.deselect", json!({})).unwrap();
    doc.save(Some("basic.psd")).unwrap()
}

/// A 40 x 40 `.kphoto` with an object layer placed linked from `placed` (a picture inside the
/// workspace), whose link then names `link`: what a document file may say. Without `cached` the
/// saved preview is dropped, so the layer shows only what its source gives.
fn linked(placed: &Path, link: &str, cached: bool, lut: Option<Value>) -> Vec<u8> {
    let mut session = Session::new();
    session.execute("file.new", json!({ "width": 40, "height": 40, "background": "white", "name": "linked.kphoto" })).unwrap();
    session.execute("file.placeLinked", json!({ "path": placed.to_string_lossy(), "fit": false })).unwrap();
    let state = session.active_mut().unwrap();
    let doc = std::sync::Arc::make_mut(&mut state.doc);
    let layer = doc.layers.iter_mut().find(|layer| matches!(layer.content, LayerContent::Smart(_))).unwrap();
    let LayerContent::Smart(smart) = &mut layer.content else { panic!("an object layer") };
    smart.source = SmartSource::Linked { path: link.to_owned() };
    if !cached {
        smart.cache = None;
    }
    // A Color Lookup editable filter with the parameters the file stores.
    if let Some(params) = lut {
        smart.editable_filters.push(kition_photo_engine::doc::EditableFilter {
            command: "image.adjustments.colorLookup".to_owned(),
            params,
            blend: kition_photo_engine::doc::BlendMode::Normal,
            opacity: 1.0,
            visible: true,
        });
    }
    // The document itself, without the replica the session saves from.
    kition_photo_format::save_to_bytes(doc, &Default::default()).unwrap()
}

/// The agent's session: the headless kit over the workspace, and every answer it gave.
struct Agent {
    kit: Headless<PhotoDocument>,
    folder: HostFolder,
    answers: Vec<String>,
}

impl Agent {
    fn open(&mut self, name: &str) -> String {
        self.kit.open(OpenRequest { file: &self.folder.file(name).unwrap(), read_only: false, create: None }).unwrap().id
    }

    fn keep(&mut self, what: String, answer: Result<Value, String>) -> Result<Value, String> {
        self.answers.push(format!("{what}: {answer:?}"));
        answer
    }

    /// `command` as the agent calls it directly (`commands.execute`).
    fn run(&mut self, id: &str, command: &str, value: Value) -> Result<Value, String> {
        let answer = self.kit.execute(id, command, params(value), Origin::Agent).map(|done| done.result.unwrap_or(Value::Null)).map_err(|e| format!("{e:?}"));
        self.keep(command.to_owned(), answer)
    }

    /// `command` as one `command` operation of `model.batch`, with the batch's `origin` (none, or
    /// one the caller claims).
    fn batch(&mut self, id: &str, command: &str, value: Value, origin: &Value) -> Result<Value, String> {
        let mut request = json!({ "documentId": id, "ops": [{ "op": "command", "id": command, "params": value }] });
        if !origin.is_null() {
            request["origin"] = origin.clone();
        }
        let answer = self.kit.model("model.batch", request).map_err(|e| format!("{e:?}"));
        self.keep(format!("model.batch {command} as {origin}"), answer)
    }

    /// Any object model call, kept.
    fn model(&mut self, method: &str, request: Value) -> Result<Value, String> {
        let answer = self.kit.model(method, request).map_err(|e| format!("{e:?}"));
        self.keep(method.to_owned(), answer)
    }

    /// The canvas of document `id`, as RGBA.
    fn picture(&mut self, id: &str) -> Vec<u8> {
        let shot = self.kit.screenshot(id, &ScreenshotRequest { scope: ScreenshotScope::Canvas, max_size: 64, scale: 1.0, target: None }).unwrap();
        kition_photo_codecs::decode(&shot.png).unwrap().to_rgba8()
    }

    fn check(&mut self, id: &str) -> String {
        let found = self.kit.check(id).map_err(|e| format!("{e:?}"));
        self.answers.push(format!("check: {found:?}"));
        found.map(|found| found.to_string()).unwrap_or_default()
    }
}

/// The parameter names a command's notation gives that may name a file or folder.
const FILE_KEYS: &[&str] = &[
    "path",
    "paths",
    "file",
    "files",
    "dir",
    "folder",
    "folders",
    "imagesFolder",
    "profile",
    "monitorProfile",
    "printerProfile",
    "workingRgb",
    "workingCmyk",
    "workingGray",
    "mapPath",
    "url",
    "droplet",
    "script",
    "destination",
    "template",
    "source",
    "sources",
];

/// For each engine command (listed or not), its parameters with every file-like one naming
/// `target` (a list for the plural names).
fn hostile_calls(target: &str) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for spec in kition_photo_engine::command_specs() {
        let mut given = Map::new();
        for key in FILE_KEYS {
            if spec.params.contains(&format!("\"{key}\"")) {
                let value = if key.ends_with('s') { json!([target]) } else { json!(target) };
                given.insert((*key).to_owned(), value);
            }
        }
        if !given.is_empty() {
            out.push((spec.id.to_owned(), Value::Object(given)));
        }
    }
    out
}

/// Commands that end or replace the document the rest of a sweep runs in.
fn ends_document(command: &str) -> bool {
    command.starts_with("file.close") || matches!(command, "file.revert" | "file.exit" | "file.quit" | "window.closeAll")
}

/// What the agent does in the child. Errors are fine; reaching files outside the workspace is not.
/// Returns what went wrong that the child can see (answers, renders, saves); the parent checks the
/// folders.
fn act(root: &Path) -> Vec<String> {
    let workspace = root.join("workspace");
    let outside = root.join("outside");
    let at = |name: &str| outside.join(name).to_string_lossy().into_owned();
    let secret_png = std::fs::read(outside.join("secret.png")).unwrap();
    let mut wrong = Vec::new();

    // As the headless binary: the gate, then a host that owns every file.
    kition_photo_kit::gate_commands(kition_photo_automation::workspace::authorize_engine_step);
    kition_kit::host::set_hosting(kition_kit::host::Hosting { files_only: true });
    let info = KitInfo::from_manifest(MANIFEST, kition_photo_kit::SANDBOXED, kition_photo_kit::EDITOR_STATE).unwrap();
    let mut agent = Agent { kit: Headless::with_replica_port(info), folder: HostFolder::new(&workspace).unwrap(), answers: Vec::new() };
    agent.kit.set_host(agent.folder.host());
    let id = agent.open("basic.psd");

    // Documents that link to pictures outside the workspace: never read, reported missing.
    for (name, link) in [
        ("link-absolute.kphoto", at("secret.png")),
        ("link-absolute-cached.kphoto", at("secret.png")),
        ("link-parent.kphoto", "../outside/secret.png".to_owned()),
        ("link-file-url.kphoto", format!("file://{}", at("secret.png"))),
    ] {
        let escape = agent.open(name);
        // What re-renders an object layer from its source: an editable filter, a new size, undo.
        let _ = agent.run(&escape, "layer.selectTop", json!({}));
        let _ = agent.run(&escape, "filter.blur.gaussianBlur", json!({ "radius": 0.5 }));
        let _ = agent.run(&escape, "image.imageSize", json!({ "width": 48, "height": 48 }));
        let _ = agent.run(&escape, "edit.undo", json!({}));
        let _ = agent.run(&escape, "image.imageSize", json!({ "width": 40, "height": 40 }));
        if shows(&agent.picture(&escape), OUTSIDE_RGB) {
            wrong.push(format!("{name}: the picture outside the workspace ({link}) was read into the render"));
        }
        let found = agent.check(&escape);
        if !found.contains("photo.missing-object-source") {
            wrong.push(format!("{name}: the check does not report the link outside the workspace as a missing source: {found}"));
        }
        // Saved as PSD and as the kit's own format, and exported: nothing of the outside file.
        for format in ["psd", "kphoto"] {
            let saved = agent.model("model.save", json!({ "documentId": escape, "format": format }));
            if let Ok(saved) = saved {
                let bytes = base64::decode(saved["data"].as_str().unwrap_or_default());
                if holds(&bytes, &secret_png) {
                    wrong.push(format!("{name}: saving it as {format} embedded the file outside the workspace"));
                }
                if let Ok(reopened) = PhotoDocument::open(&format!("again.{format}"), &bytes)
                    && shows(&reopened.render(0.0, 0.0, 1.0, 40, 40).unwrap(), OUTSIDE_RGB)
                {
                    wrong.push(format!("{name}: the {format} it saves shows the picture outside the workspace"));
                }
            }
        }
        let exported = agent.run(&escape, "file.exportImage", json!({ "format": "png" }));
        if let Ok(exported) = exported {
            let png = base64::decode(exported["data"].as_str().unwrap_or_default());
            if shows(&kition_photo_codecs::decode(&png).unwrap().to_rgba8(), OUTSIDE_RGB) {
                wrong.push(format!("{name}: its export shows the picture outside the workspace"));
            }
        }
    }

    // A link inside the workspace is read through the host and shown, and is not missing.
    let inside = agent.open("link-inside.kphoto");
    if let Err(error) = kition_kit::host::read("inside.png") {
        wrong.push(format!("inside.png: host could not lend the workspace source: {error}"));
    }
    let _ = agent.run(&inside, "layer.selectTop", json!({}));
    let filtered = agent.run(&inside, "filter.blur.gaussianBlur", json!({ "radius": 0.5 }));
    if !shows(&agent.picture(&inside), INSIDE_RGB) {
        wrong.push(format!("link-inside.kphoto: the linked picture in the workspace is not shown after filtering: {filtered:?}"));
    }
    let found = agent.check(&inside);
    if found.contains("photo.missing-object-source") {
        wrong.push(format!("link-inside.kphoto: the linked picture in the workspace is reported missing: {found}"));
    }

    // The object model's layer properties hold an object layer's link: the agent points it outside.
    let described = agent.model("model.describe", json!({ "documentId": inside })).unwrap_or_default();
    let mut object_layers = Vec::new();
    find_smart_layers(&described, &mut object_layers);
    for (object, props) in object_layers {
        for link in [at("secret.png"), "../outside/secret.png".to_owned()] {
            let mut props = props.clone();
            props["source"] = json!({ "Linked": { "path": link } });
            for origin in [Value::Null, json!({ "kind": "agent", "principal": "agent:test" })] {
                let mut request = json!({ "documentId": inside, "ops": [{ "op": "set", "target": object, "field": "props", "value": props }] });
                if !origin.is_null() {
                    request["origin"] = origin;
                }
                let _ = agent.model("model.batch", request);
                let _ = agent.run(&inside, "layer.selectTop", json!({}));
                let _ = agent.run(&inside, "filter.blur.gaussianBlur", json!({ "radius": 0.5 }));
                if shows(&agent.picture(&inside), OUTSIDE_RGB) {
                    wrong.push(format!("model.batch set props: the link {link} was read into the render"));
                }
            }
        }
    }

    // Commands that name files or folders, with paths outside the workspace: directly, inside
    // model.batch (no origin, the agent's, a claimed person's), and through actions.play.
    let folder = outside.to_string_lossy().into_owned();
    let mut named: Vec<(String, Value)> = hostile_calls(&at("made"));
    named.extend(hostile_calls(&at("secret.png")));
    named.extend(hostile_calls("../outside/made"));
    named.extend(hostile_calls(&format!("file://{}", at("secret.png"))));
    named.extend([
        ("image.adjustments.colorLookup".to_owned(), json!({ "file": at("secret.cube") })),
        ("layer.newAdjustmentLayer.colorLookup".to_owned(), json!({ "file": at("secret.cube") })),
        ("filter.distort.displace".to_owned(), json!({ "mapPath": at("secret.png") })),
        ("edit.assignProfile".to_owned(), json!({ "profile": at("secret.icc") })),
        ("edit.convertToProfile".to_owned(), json!({ "profile": at("secret.icc") })),
        ("prefs.set".to_owned(), json!({ "path": "plugIns", "value": { "useAdditionalPluginsFolder": true, "additionalPluginsFolder": folder } })),
        ("prefs.set".to_owned(), json!({ "values": { "plugIns.additionalPluginsFolder": folder, "plugIns.useAdditionalPluginsFolder": true } })),
        ("prefs.set".to_owned(), json!({ "path": "scratchDisks", "value": { "disks": [{ "path": outside.to_string_lossy(), "enabled": true }] } })),
        ("prefs.set".to_owned(), json!({ "path": "", "value": { "plugIns": { "additionalPluginsFolder": outside.to_string_lossy() } } })),
        ("plugin.list".to_owned(), json!({})),
        ("layer.objectLayers.revealInFinder".to_owned(), json!({})),
        ("file.print".to_owned(), json!({})),
        ("file.printOneCopy".to_owned(), json!({})),
    ]);
    let actions = |command: &str, value: &Value| json!({ "name": "Escape", "steps": [{ "command": command, "params": value }] });
    for origin in [Value::Null, json!({ "kind": "agent", "principal": "agent:test" }), json!({ "kind": "person", "principal": "person:local" })] {
        for (command, value) in &named {
            if ends_document(command) {
                continue;
            }
            let _ = agent.batch(&id, command, value.clone(), &origin);
        }
    }
    for (command, value) in &named {
        if ends_document(command) {
            continue;
        }
        let _ = agent.run(&id, command, value.clone());
        let _ = agent.run(&id, "actions.play", json!({ "steps": [{ "command": command, "params": value }] }));
        let _ = agent.run(&id, "actions.play", actions(command, value));
    }
    // A Color Lookup editable filter whose parameters name a LUT outside the workspace: set by the
    // agent, or stored in the file.
    let filtered = agent.open("lut-builtin.kphoto");
    let outside_lut = json!({ "index": 0, "params": { "lut": null, "file": at("secret.cube") } });
    let person = json!({ "kind": "person", "principal": "person:local" });
    let _ = agent.run(&filtered, "layer.editableFilter.setParams", outside_lut.clone());
    let _ = agent.batch(&filtered, "layer.editableFilter.setParams", outside_lut, &person);
    let _ = agent.run(&filtered, "layer.selectTop", json!({}));
    let _ = agent.run(&filtered, "filter.blur.gaussianBlur", json!({ "radius": 0.5 }));
    if shows(&agent.picture(&filtered), OUTSIDE_RGB) {
        wrong.push("layer.editableFilter.setParams: a LUT outside the workspace coloured the render".to_owned());
    }
    let stored = agent.open("lut-outside.kphoto");
    let _ = agent.run(&stored, "layer.selectTop", json!({}));
    let _ = agent.run(&stored, "filter.blur.gaussianBlur", json!({ "radius": 0.5 }));
    let _ = agent.run(&stored, "image.imageSize", json!({ "width": 48, "height": 48 }));
    if shows(&agent.picture(&stored), OUTSIDE_RGB) {
        wrong.push("lut-outside.kphoto: a stored LUT outside the workspace coloured the render".to_owned());
    }

    // The kit's own documents, as the headless kit makes them, without the server's gate: the
    // agent's role and a person's. Whoever calls, the engine reaches no file by path.
    for (role, origin) in [("agent", kition_photo_engine::Origin::agent("agent:test")), ("person", kition_photo_engine::Origin::person("person:local"))] {
        let mut doc = PhotoDocument::open("basic.psd", &std::fs::read(workspace.join("basic.psd")).unwrap()).unwrap();
        doc.hosted(&kition_kit::host::Hosting { files_only: true });
        for (command, value) in &named {
            if ends_document(command) {
                continue;
            }
            let answer = doc.execute_as(command, value.clone(), &origin);
            agent.answers.push(format!("{role} execute_as {command}: {answer:?}"));
            if role == "agent" && answer.is_ok() && kition_photo_kit::files::FILES.iter().any(|file| file.command == command) {
                wrong.push(format!("execute_as ran `{command}` for an agent, a command that names files"));
            }
            let answer = doc.execute_as("actions.play", json!({ "steps": [{ "command": command, "params": value }] }), &origin);
            agent.answers.push(format!("{role} actions.play {command}: {answer:?}"));
            if role == "agent" && answer.is_ok() && kition_photo_kit::files::FILES.iter().any(|file| file.command == command) {
                wrong.push(format!("actions.play ran `{command}` as a step for an agent, a command that names files"));
            }
        }
        // Every engine command with no parameters.
        for spec in kition_photo_engine::command_specs() {
            if ends_document(spec.id) {
                continue;
            }
            let answer = doc.execute_as(spec.id, json!({}), &origin);
            agent.answers.push(format!("{role} execute_as {} {{}}: {answer:?}", spec.id));
            if doc.inspect().is_err() {
                doc = PhotoDocument::open("basic.psd", &std::fs::read(workspace.join("basic.psd")).unwrap()).unwrap();
                doc.hosted(&kition_kit::host::Hosting { files_only: true });
            }
        }
    }

    // Every command the kit offers the agent, with no parameters.
    let offered: Vec<String> = agent.kit.commands(Some(&id)).unwrap().into_iter().map(|command| command.id).collect();
    for command in offered {
        if ends_document(&command) {
            continue;
        }
        let _ = agent.run(&id, &command, json!({}));
    }

    // Nothing outside the workspace reached an answer: its files' text, a listing, a folder of
    // this process.
    let home = root.join("home").to_string_lossy().into_owned();
    let tmp = root.join("tmp").to_string_lossy().into_owned();
    for answer in &agent.answers {
        for leak in [SECRET, LISTED_ONLY, home.as_str(), tmp.as_str()] {
            if answer.contains(leak) {
                wrong.push(format!("an answer holds `{leak}` from outside the workspace: {}", answer.chars().take(400).collect::<String>()));
            }
        }
    }
    wrong
}

/// The object model's object layers: `(object id, props)`.
fn find_smart_layers(value: &Value, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(fields) => {
            if let (Some(Value::String(id)), Some(props)) = (fields.get("id"), fields.get("fields").and_then(|f| f.get("props")))
                && props.get("source").is_some()
            {
                out.push((id.clone(), props.clone()));
            }
            fields.values().for_each(|item| find_smart_layers(item, out));
        }
        Value::Array(items) => items.iter().for_each(|item| find_smart_layers(item, out)),
        _ => {}
    }
}

#[test]
fn agent_commands_reach_no_file_outside_the_workspace() {
    if let Some(root) = std::env::var_os(CHILD) {
        let wrong = act(&PathBuf::from(root));
        assert!(wrong.is_empty(), "{wrong:#?}");
        return;
    }
    let root = std::env::temp_dir().join(format!("kition-photo-workspace-only-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["workspace", "cwd", "home", "tmp", "bin", "spawned", "outside/sub"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    let root = std::fs::canonicalize(root).unwrap();
    let (workspace, outside) = (root.join("workspace"), root.join("outside"));

    // Sentinels outside the workspace.
    std::fs::write(outside.join("secret.png"), png(OUTSIDE_RGB)).unwrap();
    // A LUT that turns every colour into the outside one.
    let [r, g, b] = OUTSIDE_RGB.map(|c| f64::from(c) / 255.0);
    let mut cube = format!("TITLE \"{SECRET}\"\nLUT_3D_SIZE 2\n");
    for _ in 0..8 {
        cube.push_str(&format!("{r:.6} {g:.6} {b:.6}\n"));
    }
    std::fs::write(outside.join("secret.cube"), cube).unwrap();
    for name in ["secret.icc", "secret.abr", "secret.grd", "secret.aco", "secret.pat", "secret.json", "secret.csv", "secret.txt", "secret.atn"] {
        std::fs::write(outside.join(name), format!("{SECRET}\n")).unwrap();
    }
    std::fs::write(outside.join(LISTED_ONLY), SECRET).unwrap();
    std::fs::write(outside.join("sub/older.bin"), SECRET).unwrap();
    let planted = tree(&outside);

    // The workspace: a picture, an image, and documents whose object layers link to pictures.
    std::fs::write(workspace.join("inside.png"), png(INSIDE_RGB)).unwrap();
    std::fs::write(workspace.join("basic.psd"), basic_psd()).unwrap();
    let placed = workspace.join("inside.png");
    let secret = outside.join("secret.png").to_string_lossy().into_owned();
    let cube = outside.join("secret.cube").to_string_lossy().into_owned();
    for (name, link, cached, lut) in [
        ("link-absolute.kphoto", secret.clone(), false, None),
        ("link-absolute-cached.kphoto", secret.clone(), true, None),
        ("link-parent.kphoto", "../outside/secret.png".to_owned(), false, None),
        ("link-file-url.kphoto", format!("file://{secret}"), false, None),
        ("link-inside.kphoto", "inside.png".to_owned(), false, None),
        ("lut-builtin.kphoto", "inside.png".to_owned(), true, Some(json!({ "lut": "warm" }))),
        ("lut-outside.kphoto", "inside.png".to_owned(), true, Some(json!({ "file": cube }))),
    ] {
        std::fs::write(workspace.join(name), linked(&placed, &link, cached, lut)).unwrap();
    }

    // Stand-ins for every program an editor may start: each leaves a mark and does nothing else.
    for program in PROGRAMS {
        let script = root.join("bin").join(program);
        std::fs::write(&script, format!("#!/bin/sh\necho \"$@\" > \"{}/{program}\"\n", root.join("spawned").display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "agent_commands_reach_no_file_outside_the_workspace", "--nocapture", "--test-threads=1"])
        .current_dir(root.join("cwd"))
        .env(CHILD, &root)
        .env("HOME", root.join("home"))
        .env("TMPDIR", root.join("tmp"))
        .env("PATH", root.join("bin"))
        .env("XDG_CONFIG_HOME", root.join("home/.config"))
        .env("XDG_DATA_HOME", root.join("home/.local/share"))
        .env("XDG_CACHE_HOME", root.join("home/.cache"))
        .env_remove("KITION_PHOTO_NO_SYSTEM_FONTS")
        .env_remove("KITION_PHOTO_LOCALE")
        .output()
        .unwrap();
    let mut wrong = Vec::new();
    if !output.status.success() {
        let said = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let failure = said.find("panicked").map_or(said.as_str(), |at| &said[at..]);
        wrong.push(format!("the agent's session found something outside the workspace: {}", failure.chars().take(20_000).collect::<String>()));
    }
    for dir in ["cwd", "home", "tmp", "spawned"] {
        let found = names(&root.join(dir));
        if !found.is_empty() {
            wrong.push(format!("the kit wrote into its {dir} folder, or started a program: {found:?}"));
        }
    }
    let now = tree(&outside);
    if now != planted {
        let before: Vec<&String> = planted.iter().map(|(name, _)| name).collect();
        let after: Vec<&String> = now.iter().map(|(name, _)| name).collect();
        wrong.push(format!("the folder outside the workspace changed: before {before:?}, after {after:?}"));
    }
    let mut expected: Vec<String> = WORKSPACE.iter().map(|name| (*name).to_owned()).collect();
    expected.sort();
    let found = names(&workspace);
    if found != expected {
        wrong.push(format!("the workspace holds {found:?}, not only what the test wrote ({expected:?})"));
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    let _ = std::fs::remove_dir_all(&root);
}

mod base64 {
    pub fn decode(text: &str) -> Vec<u8> {
        let value = |c: u8| match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        };
        let digits: Vec<u8> = text.bytes().filter_map(value).collect();
        let mut out = Vec::with_capacity(digits.len() * 3 / 4);
        for chunk in digits.chunks(4) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, d)| n | (u32::from(*d) << (18 - 6 * i)));
            out.extend_from_slice(&n.to_be_bytes()[1..chunk.len()]);
        }
        out
    }
}
