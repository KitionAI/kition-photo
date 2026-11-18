//! # kition-photo-kit
//!
//! The Photo engine for Kition's in-page editor. Kition draws the whole editor (menus, tools,
//! panels) in its own interface and renders the canvas into a `<canvas>` element; this crate is
//! the engine behind it, compiled to WebAssembly: open a file's bytes, render any part of the
//! image at any scale, run engine commands, inspect the document, and save it.
//!
//! [`PhotoDocument`] implements the Kition SDK's [`KitDocument`] contract and is exported with
//! [`kition_kit::export_kit_document!`]; the native tests check it with
//! [`kition_kit::testing::check_contract`]. Photo's own extras (`selectionOutline`, and the panels'
//! `layerThumbnail`, `maskThumbnail`, `channelThumbnails`, `maskTargetCommands`, `histogram`, and
//! `navigatorImage`) sit in [`web`].
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod acts;
mod collab;
mod content;
mod context;
pub mod described;
pub mod document_text;
pub mod files;
mod mutates;
mod names;
mod outline;
mod panels;
pub mod params;
pub mod place;
mod point;
mod render;
mod resource_rests;
mod rests;

use kition_kit::{KitCommand, KitDocument, MAX_RENDER_SIDE};
use kition_photo_engine::{Session, command_specs};
use kition_photo_geom::Rect;
use serde_json::{Value, json};

/// Commands whose file parameters the engine reads through lent files (`kition-photo`'s SANDBOXED):
/// none, the engine reads its own.
pub const SANDBOXED: &[&str] = &[];

pub use context::EDITOR_STATE;

/// The gate every document's session runs each command through ([`gate_commands`]).
static COMMAND_GATE: std::sync::OnceLock<fn(&str, &Value) -> kition_photo_engine::Result<()>> = std::sync::OnceLock::new();

/// Gates every command every document runs from now on, including the ones a command runs on its
/// own behalf and `actions.play` steps ([`Session::authorize`]): a headless kit (`kition-photo`) sets
/// the automation engine's check for commands that reach the file system on their own, before it
/// opens a document. Documents gated this way leave the commands the gate always refuses (with no
/// parameters) out of [`KitDocument::commands`]. The page's editor sets none.
pub fn gate_commands(gate: fn(&str, &Value) -> kition_photo_engine::Result<()>) {
    let _ = COMMAND_GATE.set(gate);
}

/// A new editing session, behind the gate when one is set. Anyone but a person runs no command
/// that names files ([`files::refuse_for_agent`]). When the SDK says a host owns every file
/// (`kition_kit::host::hosting`, set by `kition-headless` before it opens anything), the session
/// reaches no file by path from the start, so nothing is read by path while the file opens
/// ([`Session::set_host_files_only`]); [`KitDocument::hosted`] says the same once it is open.
pub(crate) fn new_session() -> Session {
    let mut session = Session::new();
    session.authorize = COMMAND_GATE.get().copied();
    session.agent_gate = Some(files::refuse_for_agent);
    if kition_kit::host::hosting().files_only {
        session.set_host_files_only(lend);
    }
    session
}

/// A file a document links to (a linked object layer's source), read through the host: the path
/// is resolved inside the workspace, against the document's folder, and refused outside it.
fn lend(path: &str) -> Option<Vec<u8>> {
    kition_kit::host::read(path).ok()
}

/// One open image and its editing session.
pub struct PhotoDocument {
    session: Session,
    name: String,
    /// The editor states answered to hosts, for putting one back ([`context`]).
    kept: std::sync::Mutex<context::Kept>,
    pick_snapshots: std::sync::Mutex<point::Snapshots>,
}

impl PhotoDocument {
    /// The image `session` holds, saved to `name`: a `.psd` or `.psb` keeps every value as the
    /// file stores it (`DocState::psd_values`), so what an agent reads of it is what the file
    /// opens as.
    pub(crate) fn saved_as(mut session: Session, name: &str) -> Self {
        let lower = name.to_ascii_lowercase();
        if let Some(state) = session.active_mut() {
            state.psd_values = lower.ends_with(".psd") || lower.ends_with(".psb");
        }
        Self { session, name: name.to_owned(), kept: Default::default(), pick_snapshots: Default::default() }
    }

    fn state(&self) -> Result<&kition_photo_engine::DocState, String> {
        self.session.active().ok_or_else(|| "no document is open".to_owned())
    }

    /// Whether the session's gate, if any, lets the command run at all (with no parameters).
    fn admitted(&self, id: &str) -> bool {
        self.session.authorize.is_none_or(|gate| gate(id, &json!({})).is_ok())
    }

    /// The selection's outline inside the area (`x0`, `y0`)–(`x1`, `y1`), traced every `step`
    /// pixels, as flat segment coordinates `[ax, ay, bx, by, …]`; empty with no selection. The
    /// canvas draws these as marching ants.
    pub fn selection_outline(&self, x0: i32, y0: i32, x1: i32, y1: i32, step: u32) -> Result<Vec<i32>, String> {
        let doc = &self.state()?.doc;
        let Some(mask) = doc.selection.as_ref() else { return Ok(Vec::new()) };
        let bounds = mask.content_bounds().intersect(&Rect::new(x0, y0, x1, y1));
        Ok(outline::outline(mask, bounds, step).into_iter().flat_map(|([ax, ay], [bx, by])| [ax, ay, bx, by]).collect())
    }
}

impl PhotoDocument {
    /// Runs `command` for the local person (`origin` `None`) or for `origin`: the binding's own
    /// commands (export, the agent's preview and measurements), and an agent's command with what
    /// its call gave of the state it reads, for that call only ([`context`]).
    pub(crate) fn run(&mut self, command: &str, params: Value, origin: Option<&kition_photo_engine::Origin>) -> Result<Value, String> {
        params::check(command, &params)?;
        if command == EXPORT_IMAGE {
            return self.export_image(&params);
        }
        if command == render::RENDER {
            return self.render_view(&params);
        }
        if command == render::MEASURE {
            return self.measure(&params);
        }
        let agent = self.is_agent() || origin.is_some_and(|origin| origin.kind == kition_kit::kdm::OriginKind::Agent);
        let engine = move |doc: &mut Self, params: Value| {
            let was_open = doc.session.active().is_some();
            let result = match origin {
                Some(origin) => doc.session.execute_as(command, params, origin).map_err(engine_error),
                None => doc.session.execute(command, params).map_err(engine_error),
            }?;
            Ok(if was_open && doc.session.active().is_none() { json!({"closed":true}) } else { result })
        };
        // The engine reads a command's `layer` as an id only, and takes any other value for the
        // active layer: a name or `"#n"` names a layer only for a command that reads the active
        // one, which resolves it below or in `given`.
        if !acts::reads_of(command).active
            && let Some(value) = params.get("layer").filter(|value| !value.is_null() && !value.is_u64())
        {
            return Err(context::invalid(format!(
                "`layer` is {value}: `{command}` takes a layer by its id as inspect lists it, not by a name or a place in the stack"
            )));
        }
        if agent && !context::moves_the_editor(command) {
            let mut params = params;
            let given = self.given(command, &mut params)?;
            return self.bracketed(given, |doc| engine(doc, params));
        }
        // `layer` given as "#n" or a name names that layer for anyone, never the active one.
        let mut params = params;
        if acts::reads_of(command).active
            && let Some(text) = params.get("layer").and_then(Value::as_str).map(str::to_owned)
        {
            let doc = &self.state()?.doc;
            let id = place::layer_value(doc, &Value::String(text))?;
            params["layer"] = json!(id.0);
        }
        engine(self, params)
    }
}

/// `file.exportImage {format}`: the image flattened to PNG, JPEG, TIFF or WebP, its bytes in the
/// answer (base64 `data`), for the agent's `<prefix>_export` (KHP A41). The document is unchanged.
const EXPORT_IMAGE: &str = "file.exportImage";

impl PhotoDocument {
    fn export_image(&self, params: &Value) -> Result<Value, String> {
        let format = params.get("format").and_then(Value::as_str).unwrap_or("png");
        let bytes = KitDocument::export(self, format, params)?;
        let mime = match format {
            "jpg" => "image/jpeg",
            "tiff" => "image/tiff",
            "webp" => "image/webp",
            _ => "image/png",
        };
        Ok(json!({ "data": base64_encode(&bytes), "mime": mime, "bytes": bytes.len() }))
    }
}

impl kition_kit::session::BlobSource for PhotoDocument {
    fn get(&self, hash: &str) -> Option<std::sync::Arc<[u8]>> {
        self.pick_snapshots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(hash)
            .or_else(|| kition_kit::session::BlobSource::get(self.state().ok()?.kdm()?.blobs(), hash))
    }
}

/// An engine error as a typed failure (`kition_kit::failure`): what the host answers as
/// `command_not_found`, `command_disabled` or `invalid_params` without reading the wording.
pub(crate) fn engine_error(error: kition_photo_engine::EngineError) -> String {
    use kition_kit::failure::{CommandFailure, failure};
    use kition_photo_engine::EngineError;
    let reason = match &error {
        EngineError::UnknownCommand(_) => CommandFailure::NotFound,
        EngineError::BadParams { .. } | EngineError::NoLayer(_) => CommandFailure::InvalidParams,
        EngineError::Disabled(..) | EngineError::NoDocument | EngineError::Cancelled => CommandFailure::Disabled,
        EngineError::Other(_) => return error.to_string(),
    };
    failure(reason, error)
}

/// Standard base64 with padding.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let at = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (at(0) << 16) | (at(1) << 8) | at(2);
        for k in 0..4 {
            let digit = TABLE.get(((n >> (18 - 6 * k)) & 63) as usize).copied().unwrap_or(b'A');
            out.push(if k <= chunk.len() { char::from(digit) } else { '=' });
        }
    }
    out
}

impl KitDocument for PhotoDocument {
    fn batch_settler(&self) -> Option<kition_kit::BatchSettler> {
        let psd = self.state().is_ok_and(|state| state.psd_values);
        Some(Box::new(move |tx| {
            kition_photo_engine::kdm_model::settle_batch(tx, psd).map_err(|error| error.to_string())?;
            // The engine stores f32; PSD stores layer fractions as bytes as well.
            for (ty, fields) in [("Image", &["dpi"][..]), ("Layer", &["opacity", "fillOpacity"][..])] {
                for id in tx.objects_of_type(ty) {
                    for field in fields {
                        let value = tx.get(&id, field).map_err(|error| error.to_string())?;
                        let number = value.as_f64().ok_or_else(|| format!("{ty}.{field} must be a number"))? as f32;
                        let stored = if psd && ty == "Layer" {
                            kition_photo_io::stored::psd_fraction(number)
                        } else if psd && ty == "Image" {
                            // PSD ResolutionInfo is 16.16 fixed point (the schema caps it at 30,000).
                            ((f64::from(number) * 65536.0).round() / 65536.0) as f32
                        } else {
                            number
                        };
                        let stored = json!(stored);
                        if stored != value {
                            tx.set(&id, field, stored).map_err(|error| error.to_string())?;
                        }
                    }
                }
            }
            Ok(())
        }))
    }

    /// Opens a PSD, PSB, `.kphoto`, or flat image from its bytes. `name` picks the format. A
    /// `.kphoto` whose bundle holds a KDM replica (Kition phase 3a, Task 4) restores it, so the
    /// document keeps its collaboration identity and undo history across saves.
    fn open(name: &str, bytes: &[u8]) -> Result<Self, String> {
        let mut session = new_session();
        session.open_file(name, bytes).map_err(|error| error.to_string())?;
        Ok(Self::saved_as(session, name))
    }

    fn create(name: &str, width: u32, height: u32) -> Result<Self, String> {
        let mut session = new_session();
        // Named after its file, as an opened document is.
        session.execute("file.new", json!({ "width": width, "height": height, "background": "white", "name": name })).map_err(|error| error.to_string())?;
        Ok(Self::saved_as(session, name))
    }

    fn width(&self) -> u32 {
        self.state().map_or(0, |state| state.doc.size.width)
    }

    fn height(&self) -> u32 {
        self.state().map_or(0, |state| state.doc.size.height)
    }

    /// Size, color mode, bit depth, and how many layers.
    fn summary(&self) -> String {
        self.state().map_or_else(
            |_| "no image".to_owned(),
            |state| {
                let doc = &state.doc;
                format!("{} x {} px, {:?}, {}-bit, {} layers", doc.size.width, doc.size.height, doc.mode, doc.depth.bits(), doc.walk().len())
            },
        )
    }

    fn revision(&self) -> u64 {
        self.state().map_or(0, |state| state.revision)
    }

    fn dirty(&self) -> bool {
        self.state().is_ok_and(kition_photo_engine::DocState::is_dirty)
    }

    fn render(&self, x: f64, y: f64, scale: f64, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let doc = &self.state()?.doc;
        let (width, height) = (width.clamp(1, MAX_RENDER_SIDE), height.clamp(1, MAX_RENDER_SIDE));
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let span_x = ((f64::from(width) / scale).ceil() as i32).max(1);
        let span_y = ((f64::from(height) / scale).ceil() as i32).max(1);
        let area = Rect::new(x0, y0, x0.saturating_add(span_x), y0.saturating_add(span_y));
        if scale <= 1.0 {
            return Ok(kition_photo_compose::render_reduced_rect(doc, area, width, height).to_rgba8().pixels);
        }
        // Enlarge exact pixels: render the small area once, then repeat each pixel.
        let source = kition_photo_compose::render(doc, area).to_rgba8();
        let (sw, sh) = (source.width as usize, source.height as usize);
        let mut out = vec![0u8; width as usize * height as usize * 4];
        let fx = x - f64::from(x0);
        let fy = y - f64::from(y0);
        for row in 0..height as usize {
            let sy = ((row as f64 / scale) + fy).floor() as usize;
            if sy >= sh {
                break;
            }
            for col in 0..width as usize {
                let sx = ((col as f64 / scale) + fx).floor() as usize;
                if sx >= sw {
                    break;
                }
                let from = (sy * sw + sx) * 4;
                let to = (row * width as usize + col) * 4;
                if let (Some(dst), Some(src)) = (out.get_mut(to..to + 4), source.pixels.get(from..from + 4)) {
                    dst.copy_from_slice(src);
                }
            }
        }
        Ok(out)
    }

    fn render_area(&self, x0: i32, y0: i32, x1: i32, y1: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let doc = &self.state()?.doc;
        let (width, height) = (width.clamp(1, MAX_RENDER_SIDE), height.clamp(1, MAX_RENDER_SIDE));
        let area = Rect::new(x0, y0, x1.max(x0.saturating_add(1)), y1.max(y0.saturating_add(1)));
        Ok(kition_photo_compose::render_reduced_rect(doc, area, width, height).to_rgba8().pixels)
    }

    fn execute(&mut self, command: &str, params: Value) -> Result<Value, String> {
        self.run(command, params, None)
    }

    /// A host that owns every file (KHP A29, section 10.6): from now on the engine reaches no file
    /// by path, starts no program and scans no font folder ([`Session::set_host_files_only`]).
    fn hosted(&mut self, hosting: &kition_kit::host::Hosting) {
        if hosting.files_only {
            self.session.set_host_files_only(lend);
        }
    }

    fn editor_state(&self) -> Value {
        self.take_editor_state()
    }

    fn restore_editor_state(&mut self, saved: &Value) -> Result<(), String> {
        self.put_back(saved)
    }

    fn carry_replica_objects(&mut self, previous: &Self) -> Result<(), String> {
        previous.take_editor_state();
        self.kept = std::sync::Mutex::new(previous.kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone());
        self.pick_snapshots = std::sync::Mutex::new(previous.pick_snapshots.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone());
        Ok(())
    }

    fn reconcile_reread(&mut self, fresh: &Self) -> Result<bool, String> {
        let saved = self.take_editor_state();
        self.session.reconcile_reread(&fresh.state()?.doc).map_err(|error| error.to_string())?;
        self.put_back(&saved)?;
        Ok(true)
    }

    fn fork_for_premises(&self) -> Result<Self, String> {
        let mut fork = Self::open(&self.name, &self.save(None)?)?;
        fork.carry_replica_objects(self)?;
        fork.restore_editor_state(&self.editor_state())?;
        Ok(fork)
    }

    fn blob_source(&self) -> Option<&dyn kition_kit::session::BlobSource> {
        self.state().ok()?.kdm()?;
        Some(self)
    }

    fn set_context(&mut self, context: &kition_kit::CallContext) -> Result<(), String> {
        self.set_call_context(context)
    }

    fn under(&self, command: &str, params: &Value) -> Result<Option<kition_kit::Under>, String> {
        self.under_call(command, params)
    }

    fn covered(&self, command: &str, params: &Value, objects: &[kition_kit::CoveredAsk]) -> Result<Option<kition_kit::CoveredAt>, String> {
        self.covered_at(command, params, objects)
    }

    fn content_at(&self, ask: &kition_kit::ContentAsk, history: &dyn kition_kit::HistoricalBlobs) -> Result<Option<bool>, String> {
        content::changed(self, ask, history)
    }

    fn covered_at(
        &self,
        command: &str,
        params: &Value,
        objects: &[kition_kit::CoveredAsk],
        history: &dyn kition_kit::HistoricalBlobs,
    ) -> Result<Option<kition_kit::CoveredAt>, String> {
        if command == "channel.rename" {
            let now = self.derived_values(&[json!(place::IMAGE)], &["channelContents".to_owned()])?;
            return Ok(now
                .first()
                .and_then(|v| v.values.get("channelContents"))
                .and_then(|now| resource_rests::channel_covered(params.get("channel").unwrap_or(&Value::Null), true, objects, now)));
        }
        if matches!(command, "notes.set" | "view.moveGuide") {
            let state = self.state()?;
            let Some(kdm) = state.kdm() else { return Ok(None) };
            let id = kition_kit::kdm::ObjectId::from_string(place::IMAGE.to_owned());
            let fields = json!({"docState":kdm.replica().get(&id,"docState").map_err(|e|e.to_string())?,
                "guides":kdm.replica().get(&id,"guides").map_err(|e|e.to_string())?});
            return Ok(resource_rests::indexed_covered(command, params, objects, &fields));
        }
        let answer = point::covered(self, command, params, objects, history)?;
        resource_rests::sources_covered(self, command, params, objects, answer)
    }

    fn derived(&self, ids: &[Value], names: &[String]) -> Result<Vec<kition_kit::DerivedValues>, String> {
        self.derived_values(ids, names)
    }

    /// The document as JSON: size, mode, depth, layers, history, selection.
    fn inspect(&self) -> Result<Value, String> {
        Ok(kition_photo_engine::inspect::document(self.state()?))
    }

    fn commands(&self) -> Vec<KitCommand> {
        let mut commands: Vec<KitCommand> = command_specs()
            .iter()
            .filter(|spec| self.admitted(spec.id))
            .map(|spec| {
                KitCommand {
                    id: spec.id.to_owned(),
                    title: spec.label.to_owned(),
                    menu: (!spec.menu.is_empty()).then(|| spec.menu.iter().map(|part| (*part).to_owned()).collect()),
                    shortcut: spec.shortcut.map(str::to_owned),
                    params: Some(spec.params.to_owned()),
                    mutates: mutates::changes_document(spec),
                    enabled: self.session.is_enabled(spec.id),
                    ..Default::default()
                }
                .with_kit_schema()
            })
            .collect();
        // The agent's `<prefix>_export` (kit.json `agent.export`): the bytes in the answer, so no
        // path reaches the engine and it runs headless.
        commands.push(
            KitCommand {
                id: EXPORT_IMAGE.to_owned(),
                title: "Export Image".to_owned(),
                menu: Some(vec!["File".to_owned(), "Export".to_owned()]),
                params: Some(r#"{"format": "png|jpg|tiff|webp"} → {data: base64, mime, bytes}"#.to_owned()),
                mutates: false,
                enabled: self.state().is_ok(),
                ..Default::default()
            }
            .with_kit_schema(),
        );
        commands.extend(render::commands(self.state().is_ok()));
        described::fill(&mut commands);
        kition_kit::files::declare(&mut commands, files::FILES);
        params::declare(&mut commands);
        acts::declare(&mut commands);
        names::declare(&mut commands);
        commands
    }

    /// The commands that cannot run now, without building the full list and its schemas: what
    /// Kition asks after every change for menu and shortcut states.
    fn disabled_commands(&self) -> Vec<String> {
        command_specs().iter().filter(|spec| self.admitted(spec.id) && !self.session.is_enabled(spec.id)).map(|spec| spec.id.to_owned()).collect()
    }

    /// The image as a file in the format of `name`'s extension (PSD by default). Saving a
    /// `.kphoto` embeds the document's KDM replica, if it has made one yet (Kition phase 3a,
    /// Task 4), so reopening it keeps the same collaboration identity and undo history.
    fn save(&self, name: Option<&str>) -> Result<Vec<u8>, String> {
        let target = name.unwrap_or(&self.name);
        let state = self.state()?;
        if kition_photo_engine::kition_photo_bridge::is_kition_photo_name(target) {
            return state.save_kphoto().map_err(|error| error.to_string());
        }
        let exported = kition_photo_io::export(&state.doc, target, &kition_photo_io::ExportOptions::default()).map_err(|error| error.to_string())?;
        Ok(exported.bytes)
    }

    /// The flat image formats `document.export` writes (KHP A41), as the manifest's `exports` lists them.
    fn exports() -> Vec<&'static str> {
        vec!["png", "jpg", "tiff", "webp"]
    }

    /// The image flattened to `format`, with the exporter's defaults; the document is unchanged.
    fn export(&self, format: &str, _options: &Value) -> Result<Vec<u8>, String> {
        if !Self::exports().contains(&format) {
            return Err(format!("Photo exports png, jpg, tiff or webp, not {format}"));
        }
        let state = self.state()?;
        let exported =
            kition_photo_io::export(&state.doc, &format!("export.{format}"), &kition_photo_io::ExportOptions::default()).map_err(|error| error.to_string())?;
        Ok(exported.bytes)
    }

    /// What is wrong with the image (KHP A21): see [`kition_photo_engine::check`].
    fn check(&self) -> Vec<kition_kit::diagnostics::Diagnostic> {
        self.state().map(|state| kition_photo_engine::check::check_document(&state.doc)).unwrap_or_default()
    }

    /// `document.text` (KitText 1.0): the visible type layers' words, a part per layer, top
    /// first (see [`document_text`]). It changes nothing.
    fn text(&self, limit: usize) -> Result<kition_kit::text::DocumentText, String> {
        Ok(document_text::document_text(&self.state()?.doc, limit))
    }

    fn mark_saved(&mut self) {
        let target = self.name.clone();
        if let Some(state) = self.session.active_mut() {
            state.saved_to(target);
        }
    }
}

kition_kit::export_kit_document!(PhotoDocument as "PhotoDocument");

/// The scripts a face added with [`add_font`] stands in for, when the bundled faces lack them.
const FALLBACK_SCRIPTS: [&str; 4] = ["Hans", "Hant", "Jpan", "Kore"];

/// Adds a font file's faces for type the bundled faces do not cover, such as Chinese in a web
/// build, which has no system fonts (standard E9): the faces join the text engine, and the first
/// becomes the fallback for Chinese, Japanese, and Korean, as a served font listed for those
/// scripts does in upstream's web build (`kition_photo_text::served`). Returns the faces added.
pub fn add_font(bytes: &[u8]) -> usize {
    let mut text = kition_photo_text::shared().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let families = text.fonts.register_font_data(bytes.to_vec());
    if let Some(family) = families.first() {
        kition_photo_text::served::add_fonts(&[kition_photo_text::served::ServedFont {
            family: family.clone(),
            file: String::new(),
            scripts: FALLBACK_SCRIPTS.iter().map(|script| (*script).to_owned()).collect(),
            integrity: None,
        }]);
        // Registration refreshed the font database before this family's script coverage was
        // known. Refresh again now, so the first Chinese layout uses it rather than tofu.
        text.fonts.refresh_generics();
    }
    families.len()
}

/// Photo's own additions to the exported class.
#[cfg(target_arch = "wasm32")]
pub mod web {
    use kition_kit::wasm_bindgen::prelude::*;
    use serde_json::json;

    use crate::__kition_kit_export::WebDocument;

    /// Adds a font file's faces for type the bundled faces do not cover; returns how many.
    #[wasm_bindgen(js_name = addFont, wasm_bindgen = kition_kit::wasm_bindgen)]
    pub fn add_font(bytes: &[u8]) -> u32 {
        crate::add_font(bytes) as u32
    }

    fn error(message: String) -> JsError {
        JsError::new(&message)
    }

    #[wasm_bindgen(js_class = "PhotoDocument", wasm_bindgen = kition_kit::wasm_bindgen)]
    impl WebDocument {
        #[wasm_bindgen(js_name = selectionOutline)]
        pub fn selection_outline(&self, x0: i32, y0: i32, x1: i32, y1: i32, step: u32) -> Result<Vec<i32>, JsError> {
            self.0.selection_outline(x0, y0, x1, y1, step).map_err(error)
        }

        /// Layer `layer`'s thumbnail on the canvas: `[width u32 LE][height u32 LE]` then RGBA.
        #[wasm_bindgen(js_name = layerThumbnail)]
        pub fn layer_thumbnail(&self, layer: u32, max: u32) -> Result<Vec<u8>, JsError> {
            self.0.layer_thumbnail(layer, max).map_err(error)
        }

        /// Layer `layer`'s mask thumbnail, encoded like `layerThumbnail`.
        #[wasm_bindgen(js_name = maskThumbnail)]
        pub fn mask_thumbnail(&self, layer: u32, max: u32) -> Result<Vec<u8>, JsError> {
            self.0.mask_thumbnail(layer, max).map_err(error)
        }

        /// The Channels panel's thumbnails, all from one render: a count, then each encoded like `layerThumbnail`.
        #[wasm_bindgen(js_name = channelThumbnails)]
        pub fn channel_thumbnails(&self, max: u32) -> Result<Vec<u8>, JsError> {
            self.0.channel_thumbnails(max).map_err(error)
        }

        /// The commands a targeted layer mask aims, and those of them that cannot run so aimed, as JSON.
        #[wasm_bindgen(js_name = maskTargetCommands)]
        pub fn mask_target_commands(&self) -> String {
            self.0.mask_target_commands().to_string()
        }

        /// Each layer's KDM object id by its local id, as JSON `{ "<layer>": "<objectId>" }`.
        #[wasm_bindgen(js_name = layerObjectIds)]
        pub fn layer_object_ids(&self) -> String {
            self.0.layer_object_ids().to_string()
        }

        /// The composite's 256 histogram bins of `rgb`, `red`, `green`, `blue`, or `luminosity`.
        pub fn histogram(&self, channel: &str) -> Result<Vec<u32>, JsError> {
            self.0.histogram(channel).map_err(error)
        }

        /// The Navigator's image of the composite, reduced, encoded like `layerThumbnail`.
        #[wasm_bindgen(js_name = navigatorImage)]
        pub fn navigator_image(&self, max: u32) -> Result<Vec<u8>, JsError> {
            self.0.navigator_image(max).map_err(error)
        }

        // Kition phase 3a, Task 4: the collaboration members (engine contract 2). Shapes mirror
        // `kition-sdk`'s `session::web` (`export_kit_session!` does not apply here; see
        // `crate::collab`).

        pub fn version(&mut self) -> Result<Vec<u8>, JsError> {
            self.0.version().map_err(error)
        }

        #[wasm_bindgen(js_name = exportChanges)]
        pub fn export_changes(&mut self, since: &[u8]) -> Result<Vec<u8>, JsError> {
            self.0.export_changes(since).map_err(error)
        }

        #[wasm_bindgen(js_name = importChanges)]
        pub fn import_changes(&mut self, bytes: &[u8]) -> Result<String, JsError> {
            self.0.import_changes(bytes).map(|damage| serde_json::to_string(&damage).unwrap_or_else(|_| "null".to_owned())).map_err(error)
        }

        pub fn snapshot(&mut self) -> Result<Vec<u8>, JsError> {
            self.0.snapshot().map_err(error)
        }

        pub fn undo(&mut self) -> Result<String, JsError> {
            self.0.undo().map(|(done, damage)| json!({ "done": done, "damage": damage }).to_string()).map_err(error)
        }

        pub fn redo(&mut self) -> Result<String, JsError> {
            self.0.redo().map(|(done, damage)| json!({ "done": done, "damage": damage }).to_string()).map_err(error)
        }

        #[wasm_bindgen(js_name = canUndo)]
        pub fn can_undo(&self) -> bool {
            self.0.can_undo()
        }

        #[wasm_bindgen(js_name = canRedo)]
        pub fn can_redo(&self) -> bool {
            self.0.can_redo()
        }

        /// `origin` is the KDM's commit form, the JSON array `[kind, principal, label]`.
        #[wasm_bindgen(js_name = executeAs)]
        pub fn execute_as(&mut self, command: &str, params: &str, origin: &str) -> Result<String, JsError> {
            let params = kition_kit::parse_params(params).map_err(error)?;
            let origin = kition_photo_engine::Origin::decode(origin).ok_or_else(|| error(format!("origin is not [kind, principal, label]: {origin}")))?;
            self.0.execute_as(command, params, &origin).map(|value| value.to_string()).map_err(error)
        }

        /// Every blob the replica references or can still bring back, for a peer's `addBlobs`.
        pub fn blobs(&mut self) -> Result<Vec<u8>, JsError> {
            self.0.blobs().map_err(error)
        }

        /// Adds a peer's blobs (from their `blobs()`): kept until the next `importChanges`.
        #[wasm_bindgen(js_name = addBlobs)]
        pub fn add_blobs(&mut self, bytes: &[u8]) -> Result<(), JsError> {
            self.0.add_blobs(bytes).map_err(error)
        }

        /// The blobs newly needed since `since` (empty bytes: everything the current state
        /// references), encoded like `blobs()`: for live sync, so a change ships only the blobs
        /// it needs.
        #[wasm_bindgen(js_name = blobsSince)]
        pub fn blobs_since(&mut self, since: &[u8]) -> Result<Vec<u8>, JsError> {
            self.0.blobs_since(since).map_err(error)
        }

        /// The document's KDM identity (`undefined` before it has made a replica).
        #[wasm_bindgen(getter, js_name = docId)]
        pub fn doc_id(&self) -> Option<String> {
            self.0.doc_id()
        }

        #[wasm_bindgen(js_name = kdmResets)]
        pub fn kdm_resets(&self) -> u32 {
            self.0.kdm_resets()
        }

        /// Opens a file with the replica a host stored for it (the Kit UI protocol's `replica`
        /// capability); a replica that does not open is ignored and the file opens plainly.
        #[wasm_bindgen(js_name = openWithReplica)]
        pub fn open_with_replica(name: &str, bytes: &[u8], snapshot: &[u8], blobs: &[u8]) -> Result<WebDocument, JsError> {
            crate::PhotoDocument::open_with_replica(name, bytes, snapshot, blobs).map(Self).map_err(error)
        }

        /// Whether the document has made its replica yet (it makes one on the first shared edit).
        #[wasm_bindgen(getter, js_name = hasReplica)]
        pub fn has_replica(&self) -> bool {
            self.0.has_replica()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kition_kit::testing::check_contract;

    fn fixture() -> Vec<u8> {
        let dir = std::env::temp_dir().join(format!("kition-photo-kit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.kphoto");
        let mut engine = kition_photo_automation::Headless::trusted_local();
        engine.command_run("file.new", json!({ "width": 64, "height": 40, "background": "white" })).unwrap();
        engine.command_run("layer.new.layer", json!({ "name": "Ink" })).unwrap();
        engine.save(None, Some(&path), Some("kphoto"), &kition_photo_io::ExportOptions::default()).unwrap();
        std::fs::read(path).unwrap()
    }

    fn stroke(doc: &mut PhotoDocument, points: Value, color: &str) -> Result<(), String> {
        doc.execute("paint.stroke", json!({ "points": points, "size": 6, "color": color })).map(|_| ())
    }

    #[test]
    fn keeps_the_kit_sdk_contract() {
        check_contract::<PhotoDocument>(&fixture(), "fixture.kphoto", |doc| stroke(doc, json!([[4, 4, 1], [30, 20, 1]]), "#ff0000")).unwrap();
    }

    #[test]
    fn opens_renders_edits_and_saves() {
        let mut doc = PhotoDocument::open("fixture.kphoto", &fixture()).unwrap();
        assert_eq!((doc.width(), doc.height()), (64, 40));
        let full = doc.render(0.0, 0.0, 1.0, 64, 40).unwrap();
        assert_eq!(&full[..4], &[255, 255, 255, 255], "white background");
        stroke(&mut doc, json!([[4, 4, 1], [30, 20, 1]]), "#ff0000").unwrap();
        assert_eq!(doc.inspect().unwrap()["width"], 64);
    }

    #[test]
    fn renders_zoomed_in_and_out() {
        let doc = PhotoDocument::create("new.psd", 100, 50).unwrap();
        assert_eq!(doc.render(0.0, 0.0, 0.25, 25, 13).unwrap().len(), 25 * 13 * 4);
        let zoomed = doc.render(10.0, 10.0, 4.0, 40, 40).unwrap();
        assert_eq!(zoomed.len(), 40 * 40 * 4);
        assert_eq!(&zoomed[..4], &[255, 255, 255, 255]);
        assert!(doc.render(0.0, 0.0, f64::NAN, 10, 10).is_ok());
    }

    #[test]
    fn renders_an_exact_area() {
        let mut doc = PhotoDocument::create("new.psd", 100, 50).unwrap();
        doc.execute("paint.stroke", json!({ "points": [[0, 0, 1], [99, 49, 1]], "size": 4, "color": "#000000" })).unwrap();
        let full = doc.render(0.0, 0.0, 1.0, 100, 50).unwrap();
        let exact = doc.render_area(20, 5, 24, 9, 4, 4).unwrap();
        for row in 0..4 {
            let from = ((5 + row) * 100 + 20) * 4;
            assert_eq!(&exact[row * 16..row * 16 + 16], &full[from..from + 16]);
        }
    }

    #[test]
    fn takes_a_fallback_font_for_chinese() {
        assert_eq!(add_font(b"not a font"), 0);
        let font = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kition-fonts/fonts/droid-sans-fallback/DroidSansFallbackFull.ttf");
        let Ok(bytes) = std::fs::read(font) else { return };
        assert!(add_font(&bytes) > 0);
        assert!(kition_photo_text::served::script_fallback_families().iter().any(|family| family == "Droid Sans Fallback"));
    }

    #[test]
    fn traces_the_selection() {
        let mut doc = PhotoDocument::create("new.psd", 100, 50).unwrap();
        assert!(doc.selection_outline(0, 0, 100, 50, 1).unwrap().is_empty());
        doc.execute("select.rect", json!({ "x": 10, "y": 5, "width": 20, "height": 10 })).unwrap();
        let segments = doc.selection_outline(0, 0, 100, 50, 1).unwrap();
        assert_eq!(segments.len(), 4 * 4, "{segments:?}");
        assert!(segments.chunks(4).any(|s| s == [10, 5, 30, 5]));
    }

    #[test]
    fn commands_carry_menus_and_shortcuts() {
        let doc = PhotoDocument::create("new.psd", 10, 10).unwrap();
        let commands = doc.commands();
        let layer = commands.iter().find(|c| c.id == "layer.new.layer").unwrap();
        assert_eq!(layer.menu.as_deref().and_then(<[String]>::first).map(String::as_str), Some("Layer"));
        assert!(layer.enabled);
        assert_eq!(commands.iter().find(|c| c.id == "edit.undo").and_then(|c| c.shortcut.as_deref()), Some("Cmd+Z"));
    }

    #[test]
    fn mutates_means_the_command_changes_the_document() {
        let doc = PhotoDocument::create("new.psd", 10, 10).unwrap();
        let commands = doc.commands();
        let mutates = |id: &str| commands.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("{id} is listed")).mutates;
        for id in [
            "layer.new.layer",
            "paint.stroke",
            "edit.undo",
            "edit.redo",
            "file.new",
            "view.newGuide",
            "select.saveSelection",
            "actions.play",
            "timeline.nextFrame",
        ] {
            assert!(mutates(id), "{id} changes the document");
        }
        for id in [
            "select.all",
            "select.rect",
            "select.deselect",
            "layer.selectAbove",
            "tools.swapColors",
            "channel.target",
            "edit.copy",
            "file.close",
            "file.print",
            "file.saveACopy",
            "session.inspect",
            "timeline.info",
        ] {
            assert!(!mutates(id), "{id} does not change the document");
        }
    }

    /// Every command flagged as not changing the document that runs on a sample document leaves
    /// its replica where it was.
    #[test]
    fn commands_that_do_not_mutate_leave_the_replica() {
        let doc = PhotoDocument::create("new.psd", 10, 10).unwrap();
        let mut moved = Vec::new();
        for command in doc.commands().into_iter().filter(|command| !command.mutates && command.id != "test.panic") {
            let mut doc = PhotoDocument::create("new.psd", 40, 30).unwrap();
            doc.execute("layer.new.layer", json!({ "name": "A" })).unwrap();
            stroke(&mut doc, json!([[0, 0, 1], [39, 29, 1]]), "#000000").unwrap();
            doc.execute("select.rect", json!({ "x": 2, "y": 2, "width": 20, "height": 10 })).unwrap();
            let before = doc.version().unwrap();
            if doc.execute(&command.id, json!({})).is_err() {
                continue;
            }
            if doc.version().is_ok_and(|after| after != before) {
                moved.push(command.id);
            }
        }
        assert!(moved.is_empty(), "these moved the replica: {moved:?}");
    }
}
