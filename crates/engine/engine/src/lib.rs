//! The Kition Photo engine façade: open documents, history, and the command registry.
//!
//! Every user-visible action is a command with a stable id (Photoshop-style, such as
//! `layer.newAdjustmentLayer.invert`) and JSON parameters. Every frontend goes through the same
//! [`Session::execute`] entry point: the kit's editor (through `bindings/wasm`) and the headless kit
//! with its automation session, so people and agents run the same commands.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod actions_cmds;
pub mod adjust_cmds;
pub mod adjust_params;
pub mod align_cmds;
pub mod analysis_cmds;
pub mod artboard_cmds;
pub mod automate_cmds;
pub mod brush_cmds;
pub mod brush_key_cmds;
pub mod brush_preset_cmds;
pub mod build_info;
mod canvas_geom;
pub mod channel_cmds;
pub mod color_cmds;
pub mod color_to_alpha_cmds;
pub mod commands;
pub mod comps_cmds;
pub mod cutout_cmds;
pub mod display_color;
pub mod distort_cmds;
pub mod document_preset_cmds;
pub mod edit_cmds;
pub mod edit_menu_cmds;
pub mod eraser_cmds;
pub mod extra_cmds;
pub mod file_cmds;
pub mod fill_cmds;
pub mod fill_key_cmds;
pub mod filters;
pub mod filters_ext;
pub mod float_cmds;
mod frame_cmds;
pub mod fx_view_cmds;
pub mod fx_visibility_cmds;
pub mod gallery_cmds;
pub mod gradient_fill_cmds;
pub mod group_view_cmds;
pub mod hidden_target;
pub mod history_cmds;
pub mod image_cmds;
pub mod inspect;
pub mod jobs;
pub mod kdm_bridge;
pub mod kition_photo_bridge;
pub mod layer_copy_cmds;
pub mod layer_label_cmds;
pub mod layer_menu_cmds;
pub mod layer_multi_cmds;
pub mod layer_nav_cmds;
pub mod layer_style;
pub mod lens_cmds;
pub mod magnetic_cmds;
pub mod mask_view_cmds;
mod migrate_cmds;
pub mod mode_cmds;
pub mod multichannel_cmds;
pub mod notes_cmds;
pub mod object_layer_cmds;
pub mod paint_cmds;
mod path_edit_cmds;
pub mod pattern_cmds;
pub mod photo_cmds;
pub mod pick_cmds;
mod pixels;
pub mod plugin_cmds;
pub mod prefs;
pub mod preset_import_cmds;
pub mod preset_store;
pub mod presets;
pub mod print_cmds;
pub mod proof_sim;
pub mod redeye_cmds;
pub mod render_cmds;
pub mod retouch_cmds;
pub mod sample_cmds;
pub mod select_extra_cmds;
pub mod selection_cmds;
pub mod slice_cmds;
pub mod smartselect_cmds;
pub mod snap;
pub mod solid_fill_cmds;
pub mod stamp_cmds;
pub mod swatch_cmds;
pub mod symmetry_cmds;
mod timeline_cmds;
pub mod transform_cmds;
mod trap_cmds;
pub mod type_caret_cmds;
pub mod type_cmds;
pub mod type_extra_cmds;
pub mod type_spell_cmds;
pub mod type_styles_cmds;
mod variables_cmds;
pub mod vector_cmds;
mod video_cmds;
pub use video_cmds::render_video_with;
pub mod vp_cmds;
pub mod warp_cmds;
pub mod web_cmds;
mod wia_cmds;

use std::sync::Arc;

use kition_photo_doc::{Document, LayerId};
use kition_photo_ops::{History, LayerTarget};
use serde_json::Value;

pub use commands::{CommandSpec, command_specs};

/// Does the command take a `"target"` param (where its pixels go: the layer's pixels, its mask,
/// the quick mask, or an alpha channel)? These are the commands the channel view routes.
pub fn takes_target(id: &str) -> bool {
    channel_cmds::routed(id)
}
pub mod check;
pub use kdm_model;
pub use kdm_model::kition_kit::kdm::{Origin, UndoScope, Version};
pub use kdm_model::kition_kit::session::{Damage, MemoryBlobs};
pub use kition_photo_doc as doc;
pub use kition_photo_paint as paint;
pub use kition_photo_paint::BrushSettings;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("command `{0}` is not available right now: {1}")]
    Disabled(String, String),
    #[error("invalid parameters for `{cmd}`: {msg}")]
    BadParams { cmd: String, msg: String },
    #[error("no active document")]
    NoDocument,
    #[error("no such layer {0:?}")]
    NoLayer(LayerId),
    #[error("{0}")]
    Other(String),
    /// A background job was cancelled (see [`jobs`]); nothing changed.
    #[error("cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// The pixels of a raster layer (typically one a command just created), as an error instead
/// of a panic if the layer has none.
pub(crate) fn pixels_mut(l: &mut kition_photo_doc::Layer) -> Result<&mut kition_photo_raster::Surface> {
    let id = l.id;
    l.surface_mut().ok_or_else(|| EngineError::Other(format!("layer {id:?} has no pixels")))
}

/// The active document's active layer, for `enabled` predicates.
pub(crate) fn active_layer_of(s: &Session) -> std::result::Result<&kition_photo_doc::Layer, String> {
    let d = s.active().ok_or("no document open")?;
    d.active_layer.and_then(|id| d.doc.layer(id)).ok_or_else(|| "no active layer".into())
}

/// Per-document editing state.
///
/// The shared document lives in a KDM replica ([`kdm_bridge`]): `doc` is the engine's working
/// view of it, plus this person's own state (`selection`, `quick_mask`), which is never
/// replicated. Commands change `doc`; [`Session::edit`] writes the change to the replica as one
/// transaction. `history` is this person's undo stack: its labels and snapshots drive the History
/// panel, the history brush and Fade, while undo and redo run on the replica (so they never take
/// back a collaborator's edit) and only restore the selection from the snapshots.
#[derive(Debug)]
pub struct DocState {
    pub doc: Arc<Document>,
    /// The undo history of the scope the session acts in now ([`DocState::history_scope`]: the
    /// person's, or the agents' while a command runs as an agent, KitModel 1.1 M12a).
    pub history: History,
    /// The other scope's history, swapped with `history` when the session's origin changes scope.
    pub(crate) other_history: History,
    /// Whose steps `history` holds.
    pub(crate) history_scope: UndoScope,
    /// The replica, made on the first change to the shared document (or the first request for
    /// it), so opening a document costs nothing extra. `None` until then, or after the document
    /// left a replica that refused it ([`DocState::kdm_resets`]); it then undoes from `history`'s
    /// snapshots, as before the bridge.
    pub(crate) kdm: Option<kdm_model::PhotoSession>,
    /// The history holds shared changes no replica has (taken while `kdm` was `None` after a
    /// reset): the next replica marks them as snapshot steps.
    pub(crate) kdm_unbacked: bool,
    /// The history holds snapshot steps (see `kdm_unbacked`): they are forgotten when the first
    /// collaborator's change arrives, since restoring a snapshot would take that change back.
    pub(crate) kdm_snapshot_steps: bool,
    /// Making a replica failed for a reason other than a too-deep layer tree: don't retry.
    pub(crate) kdm_blocked: bool,
    /// How many times the document left its replica (see [`DocState::kdm_resets`]).
    pub(crate) kdm_resets: u64,
    /// `doc` as last written to (or read from) `kdm`: a different pointer means there is
    /// something to write.
    pub(crate) synced: Arc<Document>,
    /// Undo steps this view took in `kdm` (for [`analysis_cmds::compound`]).
    pub(crate) kdm_steps: u64,
    /// The primary ("key") layer: what single-layer commands act on. Always a member of
    /// `selected_layers` when set.
    pub active_layer: Option<LayerId>,
    /// Every selected layer in the Layers panel (⌘/⇧-click), in selection order. Like
    /// `active_layer`, selecting is not a history step, but undo and redo restore the layers each
    /// state targeted when the next step was taken; see [`DocState::selected_layers`].
    pub selected_layers: Vec<LayerId>,
    /// Anchor of ⇧-click range selection (the last plainly or ⌘-clicked layer).
    pub layer_anchor: Option<LayerId>,
    pub path: Option<String>,
    /// An Affinity document (or its preview) must not acquire the native source as its Save path.
    pub source_read_only: bool,
    /// Increments on every change; UIs re-render when it moves.
    pub revision: u64,
    pub saved_revision: u64,
    /// Area changed by the latest revision (None = assume everything changed).
    pub last_damage: Option<kition_photo_geom::Rect>,
    /// Coalescing key of the latest history step (see [`Session::execute`]'s `coalesce` param).
    pub coalesce: Option<String>,
    /// Channels panel: targeted channel and eye toggles (view state, not history).
    pub channel_view: channel_cmds::ChannelView,
    /// Select › Isolate Layers: the Layers panel lists only these layers (empty = off; view state).
    pub isolated_layers: Vec<LayerId>,
    /// Painting symmetry preset or sampled path (tool state, not document pixels).
    pub symmetry: Option<symmetry_cmds::PaintingSymmetry>,
    /// Layers panel: layers whose effects list is collapsed under their row (the fx triangle;
    /// view state, not history). Effects lists start open.
    pub fx_collapsed: Vec<LayerId>,
    /// ⌥-click on a layer's eye (`layer.showOnly`): the layer shown alone and every layer's
    /// visibility before, so the next ⌥-click restores it (view state, not history).
    pub show_only: Option<(LayerId, Vec<(LayerId, bool)>)>,
    /// A floating selection (`select.float`): the cut piece and where it floats, until dropped
    /// (view state: the document is unchanged until `select.drop`).
    pub floating: Option<float_cmds::Floating>,
    /// The number the next layer made in this document takes, at least: above every layer id
    /// it ever held, so an id an agent noted never names another layer later in the session.
    pub(crate) next_layer: u64,
    /// The document is kept as a PSD stores it: every edit's values enter as the file stores them
    /// ([`kition_photo_io::stored::settle_psd`]), so its saved file opens as the document in
    /// memory. Set by the host that saves it as PSD.
    pub psd_values: bool,
}

/// A detached copy, for comparing states: the replica stays with the original (the copy has none
/// and undoes from its snapshots, as documents without a replica do).
impl Clone for DocState {
    fn clone(&self) -> Self {
        Self {
            kdm: None,
            kdm_unbacked: true,
            kdm_snapshot_steps: false,
            kdm_blocked: self.kdm_blocked,
            kdm_resets: self.kdm_resets,
            doc: self.doc.clone(),
            history: self.history.clone(),
            other_history: self.other_history.clone(),
            history_scope: self.history_scope,
            synced: self.synced.clone(),
            kdm_steps: self.kdm_steps,
            active_layer: self.active_layer,
            selected_layers: self.selected_layers.clone(),
            layer_anchor: self.layer_anchor,
            path: self.path.clone(),
            source_read_only: self.source_read_only,
            revision: self.revision,
            saved_revision: self.saved_revision,
            last_damage: self.last_damage,
            coalesce: self.coalesce.clone(),
            channel_view: self.channel_view.clone(),
            isolated_layers: self.isolated_layers.clone(),
            symmetry: self.symmetry.clone(),
            fx_collapsed: self.fx_collapsed.clone(),
            show_only: self.show_only.clone(),
            floating: self.floating.clone(),
            next_layer: self.next_layer,
            psd_values: self.psd_values,
        }
    }
}

impl DocState {
    pub fn new(doc: Document, path: Option<String>) -> Self {
        Self::with_replica(doc, path, None)
    }

    /// [`DocState::new`] over a replica that already holds `doc` (its view must be `doc`).
    pub(crate) fn with_replica(doc: Document, path: Option<String>, kdm: Option<kdm_model::PhotoSession>) -> Self {
        let active_layer = doc.top_layer();
        let next_layer = doc.next_layer_number();
        let mut history = History::default();
        history.set_current_layers(LayerTarget { active: active_layer, selected: active_layer.into_iter().collect() });
        let doc = Arc::new(doc);
        Self {
            synced: doc.clone(),
            doc,
            other_history: history.clone(),
            history,
            history_scope: UndoScope::Person,
            kdm,
            kdm_unbacked: false,
            kdm_snapshot_steps: false,
            kdm_blocked: false,
            kdm_resets: 0,
            kdm_steps: 0,
            active_layer,
            selected_layers: active_layer.into_iter().collect(),
            layer_anchor: active_layer,
            path,
            source_read_only: false,
            revision: 1,
            saved_revision: 1,
            last_damage: None,
            coalesce: None,
            channel_view: Default::default(),
            isolated_layers: Vec::new(),
            symmetry: None,
            fx_collapsed: Vec::new(),
            show_only: None,
            floating: None,
            next_layer,
            psd_values: false,
        }
    }

    /// The number the next layer made in this document takes.
    pub(crate) fn next_layer_number(&self) -> u64 {
        self.next_layer.max(self.doc.next_layer_number())
    }

    /// Numbers the layers this document makes until the guard drops ([`kition_photo_doc::numbering`]).
    pub(crate) fn numbering(&self) -> kition_photo_doc::Numbering {
        kition_photo_doc::numbering(self.doc.id, self.next_layer_number())
    }

    /// Gives a number in this document ([`Document::number_layers`]) to every layer made since the
    /// document was last synced where no numbering ran (a command that changed the document
    /// outside an edit), and to a copy that kept another layer's id. The view state naming such a
    /// layer follows it. Run before the document is written to its replica.
    pub(crate) fn number_layers(&mut self) {
        let had: std::collections::HashSet<LayerId> = self.synced.walk().into_iter().map(|(_, _, l)| l.id).collect();
        let keep = |id: LayerId| had.contains(&id);
        let needed = {
            let mut seen = std::collections::HashSet::new();
            self.doc.walk().iter().any(|(_, _, l)| !(l.id.0 != 0 && (l.id.is_numbered() || keep(l.id)) && seen.insert(l.id)))
        };
        if needed {
            let next = self.next_layer_number();
            let changes = Arc::make_mut(&mut self.doc).number_layers_keeping(next, keep);
            let moved: std::collections::HashMap<LayerId, LayerId> = changes.into_iter().filter(|(old, _)| !old.is_numbered()).collect();
            let follow = |id: &mut LayerId| {
                if let Some(new) = moved.get(id) {
                    *id = *new;
                }
            };
            self.active_layer.iter_mut().chain(self.layer_anchor.iter_mut()).for_each(follow);
            self.selected_layers.iter_mut().chain(self.isolated_layers.iter_mut()).chain(self.fx_collapsed.iter_mut()).for_each(follow);
        }
        self.next_layer = self.next_layer_number();
    }
    /// The selected layers in bottom-to-top document order, always including the active layer.
    /// Robust against stale state: ids no longer in the document are skipped, and if the active
    /// layer was changed without updating the set, the selection is just the active layer.
    pub fn selected_layers(&self) -> Vec<LayerId> {
        let Some(active) = self.active_layer else { return Vec::new() };
        let set: &[LayerId] = if self.selected_layers.contains(&active) { &self.selected_layers } else { std::slice::from_ref(&active) };
        self.doc.walk().into_iter().map(|(_, _, l)| l.id).filter(|id| set.contains(id)).collect()
    }
    pub fn is_layer_selected(&self, id: LayerId) -> bool {
        self.active_layer == Some(id) || (self.active_layer.is_some_and(|a| self.selected_layers.contains(&a)) && self.selected_layers.contains(&id))
    }
    pub fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
    /// The targeted layers, as history stores them with each state.
    fn layer_target(&self) -> LayerTarget {
        LayerTarget { active: self.active_layer, selected: self.selected_layers.clone() }
    }
}

/// App-wide tool state that commands read (foreground colour, brush…).
#[derive(Clone, Debug)]
pub struct ToolState {
    pub foreground: [f32; 4],
    pub background: [f32; 4],
    pub brush: kition_photo_paint::BrushSettings,
    /// Brush presets (built-ins plus user presets; see `brush.presets.*`).
    pub presets: Vec<kition_photo_paint::BrushPreset>,
    /// Bumped whenever `presets` changes ([`Session::brush_presets_changed`]); the preset store
    /// syncs when it moves.
    pub presets_rev: u64,
    /// Mixer Brush paint carried between strokes.
    pub mixer: kition_photo_paint::mixer::MixerState,
    /// The coalescing key of the running `tools.setBrush` gesture and the brush before it, so the
    /// gesture journals as one call ([`brush_cmds::coalesce_journal`]).
    pub brush_gesture: Option<(String, kition_photo_paint::BrushSettings)>,
    /// Name of the preset the current brush was last picked from. Kept across later edits (the
    /// preset stays "the current brush" so `brush.presets.update` can overwrite it); cleared when
    /// the brush is reset or the preset is gone.
    pub current_preset: Option<String>,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            foreground: [0.0, 0.0, 0.0, 1.0],
            background: [1.0, 1.0, 1.0, 1.0],
            // Photoshop's Brush tool starts at 10 % Smoothing.
            brush: kition_photo_paint::BrushSettings {
                smoothing: kition_photo_paint::brush::Smoothing { amount: 0.1, ..Default::default() },
                ..Default::default()
            },
            presets: kition_photo_paint::presets::builtin(),
            presets_rev: 0,
            mixer: Default::default(),
            brush_gesture: None,
            current_preset: None,
        }
    }
}

#[derive(Default)]
pub struct Session {
    docs: Vec<DocState>,
    active: Option<usize>,
    pub tools: ToolState,
    /// Log of executed commands (for action recording and debugging).
    pub journal: Vec<(String, Value)>,
    /// Coalescing key of the command being executed.
    coalesce_request: Option<String>,
    /// Pixels copied with Edit › Copy / Cut (shared by all documents, like Photoshop).
    pub clipboard: Option<edit_cmds::Clip>,
    /// Layer › Layer Style › Copy Layer Style: effects, blend mode, fill opacity and the Advanced
    /// Blending switches.
    pub style_clipboard: Option<(kition_photo_doc::Effects, kition_photo_color::BlendMode, f32, kition_photo_doc::AdvancedBlending)>,
    /// Shape path context menu: copied vector fill and stroke styles.
    pub path_fill_clipboard: Option<kition_photo_doc::Fill>,
    pub path_stroke_clipboard: Option<kition_photo_doc::ShapeStroke>,
    /// Colour management: proofing state, monitor profile, display transforms.
    pub color: color_cmds::ColorState,
    /// Open Edit Contents documents and the object layers they update.
    pub smart_links: Vec<object_layer_cmds::SmartLink>,
    /// Type › Save Default Type Styles: character and paragraph style new type layers start from.
    pub type_defaults: Option<(kition_photo_doc::text::CharStyle, kition_photo_doc::text::ParagraphStyle)>,
    /// Quick Mask Options (colour, opacity, colour indicates) used when entering Quick Mask.
    pub quick_mask_options: channel_cmds::QuickMaskOptions,
    /// Colour channel the running command may change (a single colour channel is targeted).
    color_restrict: Option<(LayerId, usize)>,
    /// Edit › Preferences, keyboard shortcuts, menu and toolbar customisation (see `prefs`).
    pub prefs: prefs::PrefsStore,
    /// Edit menu state: Fade source, custom shape library (see `edit_menu_cmds`).
    pub edit_state: edit_menu_cmds::EditState,
    /// The pattern library (Window › Patterns; see `pattern.*`).
    pub patterns: pattern_cmds::PatternLibrary,
    /// Image › Analysis: Measurement Log and Select Data Points (see `analysis_cmds`).
    pub analysis: analysis_cmds::AnalysisState,
    /// Window › Gradients, Patterns (groups), Styles, Shapes, Tool Presets and Clone Source.
    pub presets: presets::PresetState,
    /// File menu state: Lock Slices, Image Assets, last Print / Save for Web settings, script
    /// event log (see `automate_cmds`).
    pub file_menu: automate_cmds::FileMenuState,
    /// Persistent brush preset store (desktop only; `None` keeps presets session-only, as in
    /// headless and test sessions). See `preset_store`. The same store holds the Actions list.
    pub preset_store: Option<preset_store::PresetStore>,
    /// Window › Actions. The list persists with the preset store when one is attached.
    pub actions: actions_cmds::ActionState,
    /// Gate for every command the session runs, including the ones a command runs on its own
    /// behalf and `actions.play` steps. Untrusted sessions (MCP, the control channel) install
    /// the same check a top-level request sees. `None` runs everything, which is what a local
    /// UI and `kition-photo-cli run` do.
    pub authorize: Option<fn(&str, &serde_json::Value) -> Result<()>>,
    /// The host owns every file (a kit's documents, in the page and headless; KHP A29, A37, A45,
    /// section 10.6), set by `kition-photo-kit` when the SDK says so ([`Session::set_host_files_only`]):
    /// the engine reads and writes no file or folder by path, starts no program and scans no font
    /// folder; a linked object layer's source is read only through the host.
    pub host_files_only: bool,
    /// The check every command run for anyone but a person passes (an agent, a script's or the
    /// host's batch), directly or as a step another command dispatches ([`Session::agent_check`]):
    /// a kit sets the commands an agent may not run (those that name files). `None` runs
    /// everything.
    pub agent_gate: Option<fn(&str, &serde_json::Value) -> Result<()>>,
    /// Background jobs (see [`jobs`]).
    jobs: jobs::Jobs,
    /// Who the running command acts for ([`Session::execute_as`]); `None` is the local person.
    origin: Option<Origin>,
    /// Commands running, counting the ones a command runs itself (0 between commands).
    depth: u32,
    /// Inside [`analysis_cmds::compound`]: edits leave the replica's undo groups alone.
    kdm_hold: u32,
}

/// Move item `i` of `v` to position `to`, clamped to the end. Returns where it went; `None` when
/// `i` is out of range.
pub fn move_item<T>(v: &mut Vec<T>, i: usize, to: usize) -> Option<usize> {
    if i >= v.len() {
        return None;
    }
    let x = v.remove(i);
    let to = to.min(v.len());
    v.insert(to, x);
    Some(to)
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// From now on the host owns every file ([`Session::host_files_only`]). The by-path switches of
    /// the format and text crates are process-wide and one-way (a kit's process serves only
    /// documents whose files belong to the host); `lender` reads a linked object layer's source
    /// through the host ([`kition_photo_format::host_files::read_linked`]).
    pub fn set_host_files_only(&mut self, lender: fn(&str) -> Option<Vec<u8>>) {
        self.host_files_only = true;
        kition_photo_format::host_files::set_host_files_only(lender);
        kition_photo_text::no_system_access();
    }

    /// [`Session::agent_gate`] for the command about to run, when it runs for anyone but a person.
    pub fn agent_check(&self, id: &str, params: &Value) -> Result<()> {
        if !self.non_person() {
            return Ok(());
        }
        if matches!(id, "filter.lastFilter" | "edit.transform.again" | "edit.transform.againCopy") {
            return Err(EngineError::Other(format!(
                "`{id}` replays session history that the premise guard cannot hold; agents must give the filter or transform and its parameters explicitly"
            )));
        }
        if matches!(id, "edit.undo" | "edit.redo" | "edit.toggleLastState")
            && let Some(st) = self.active()
        {
            let scope = match params.get("scope").and_then(Value::as_str) {
                Some("person") => UndoScope::Person,
                Some("agent") => UndoScope::Agent,
                _ => st.history_scope,
            };
            let history = if scope == st.history_scope { &st.history } else { &st.other_history };
            let redo = id == "edit.redo" || id == "edit.toggleLastState" && history.can_redo();
            let steps = if redo { history.redo_external() } else { history.undo_external() };
            if st.kdm.is_none() || steps == kition_photo_ops::SNAPSHOT_STEP {
                return Err(EngineError::Other(format!(
                    "`{id}` would restore a document snapshot without collaborator-safe replica history; it is not available to agents"
                )));
            }
        }
        self.agent_gate.map_or(Ok(()), |gate| gate(id, params))
    }

    fn non_person(&self) -> bool {
        self.origin().kind != kdm_model::kition_kit::kdm::OriginKind::Person
            || kdm_model::kition_kit::dispatch::role().is_some_and(|role| role != kdm_model::kition_kit::dispatch::Role::Person)
    }

    /// What a step a command dispatches itself (an action's, a script's, a droplet's, a script
    /// event's) passes before it runs, besides [`Session::authorize`]: the host's gate for whom the
    /// call runs (`kition_kit::dispatch::may_run`, KHP A28, A29) and the agent's check
    /// ([`Session::agent_check`]), as the same command called directly would.
    pub fn may_run_step(&self, id: &str, params: &Value) -> Result<()> {
        kdm_model::kition_kit::dispatch::may_run(id, params).map_err(EngineError::Other)?;
        self.agent_check(id, params)
    }

    pub fn documents(&self) -> &[DocState] {
        &self.docs
    }
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }
    pub fn active(&self) -> Option<&DocState> {
        self.active.and_then(|i| self.docs.get(i))
    }
    pub fn active_mut(&mut self) -> Option<&mut DocState> {
        self.active.and_then(|i| self.docs.get_mut(i))
    }
    pub fn set_active(&mut self, index: usize) -> bool {
        if index < self.docs.len() {
            self.active = Some(index);
            true
        } else {
            false
        }
    }

    /// Move the document at `from` to tab position `to` (clamped to the last), keeping the active
    /// document active. Returns its new index; `None` when `from` is out of range.
    pub fn move_document(&mut self, from: usize, to: usize) -> Option<usize> {
        let active = self.active().map(|d| d.doc.id);
        let to = move_item(&mut self.docs, from, to)?;
        self.active = active.and_then(|id| self.docs.iter().position(|d| d.doc.id == id));
        Some(to)
    }

    /// Add a document (from File → New, an import, etc.) and make it active.
    /// Preserve its identity unless another open document already owns it.
    /// Identity-dependent callers must read the admitted document at the returned index.
    pub fn add_document(&mut self, doc: Document, path: Option<String>) -> usize {
        self.add_document_with(doc, path, None)
    }

    /// [`Session::add_document`], over `kdm` if given (a replica whose view is `doc`), else a new
    /// replica.
    pub(crate) fn add_document_with(&mut self, mut doc: Document, path: Option<String>, kdm: Option<kdm_model::PhotoSession>) -> usize {
        // Persisted IDs can overlap the allocator, so check every replacement too.
        while self.docs.iter().any(|st| st.doc.id == doc.id) {
            doc.id = kition_photo_doc::DocId::fresh();
        }
        // Layers keep the ids their file or source document gave them. A document a command makes
        // (its layers made outside any document's numbering) is numbered from 1 in walk order, so
        // its ids never depend on what else is open; one handed in from outside a command keeps
        // the ids it has, which its maker may name. A replica's view is numbered already.
        if kdm.is_none() {
            if self.depth > 0 {
                doc.number_layers(1);
            } else {
                doc.number_layers_keeping(1, |_| true);
            }
        }
        let mut st = match kdm {
            Some(mut kdm) => {
                kdm.set_view(doc.clone());
                DocState::with_replica(doc, path, Some(kdm))
            }
            None => DocState::new(doc, path),
        };
        for history in [&mut st.history, &mut st.other_history] {
            history.max_states = self.prefs.get().performance.history_states.max(1) as usize;
            history.max_bytes = self.prefs.get().performance.history_budget_bytes();
        }
        // A document made while an agent's command runs starts in the agent's scope.
        st.history_scope = kdm_bridge::scope_of(&self.origin());
        self.docs.push(st);
        let i = self.docs.len() - 1;
        self.active = Some(i);
        i
    }

    pub fn close(&mut self, index: usize) -> Option<DocState> {
        if index >= self.docs.len() {
            return None;
        }
        object_layer_cmds::on_close(self, index);
        if let Some(id) = self.docs.get(index).map(|d| d.doc.id) {
            self.cancel_jobs_on(id);
        }
        let d = self.docs.remove(index);
        self.active = if self.docs.is_empty() {
            None
        } else {
            Some(match self.active {
                Some(active) if active > index => active - 1,
                Some(active) if active < index => active,
                _ => index.min(self.docs.len() - 1),
            })
        };
        Some(d)
    }

    /// Run a command by id with JSON params. Returns a JSON result.
    ///
    /// Any command accepts an optional `"coalesce": "<key>"` param: consecutive edits with the same
    /// key (and no other edit, undo or redo in between) share one history step, like Photoshop's
    /// single "Edit Type Layer" step for a whole typing session or one step per slider drag.
    ///
    /// Runs synchronously, including job-capable commands (see [`jobs`]; [`Session::start`] runs
    /// those in the background).
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value> {
        match self.dispatch(id, params, false)? {
            jobs::Started::Done(v) => Ok(v),
            // Not reached: inline dispatch never starts a job. Waiting is still correct.
            jobs::Started::Job(j) => self.wait_job(j),
        }
    }

    /// Is the command currently runnable? (drives menu enablement)
    pub fn is_enabled(&self, id: &str) -> bool {
        self.is_enabled_with(id, &Value::Null)
    }

    /// [`Self::is_enabled`] for a call with `params`: their `"target"` can enable a command, as a
    /// targeted layer mask enables Image › Adjustments › Invert on an adjustment layer (#780).
    pub fn is_enabled_with(&self, id: &str, params: &Value) -> bool {
        self.disabled_reason_with(id, params).is_none()
    }

    /// Why the command can't run now (None = it can): its own precondition, or a background job
    /// running on the active document.
    pub fn disabled_reason(&self, id: &str) -> Option<String> {
        self.disabled_reason_with(id, &Value::Null)
    }

    /// [`Self::disabled_reason`] for a call with `params`.
    pub fn disabled_reason_with(&self, id: &str, params: &Value) -> Option<String> {
        let Some(s) = commands::find(id) else { return Some(format!("unknown command `{id}`")) };
        self.precondition(s, &channel_cmds::inject_target(self, id, params.clone())).err().or_else(|| self.job_conflict(id, s.journal))
    }

    /// The command's own precondition for a call with `params` (their target filled in).
    fn precondition(&self, spec: &commands::CommandSpec, params: &Value) -> std::result::Result<(), String> {
        if let Some(result) = channel_cmds::mask_target_enabled(self, spec.id, params) {
            return result;
        }
        if let Some(id) = params.get("layer").and_then(Value::as_u64)
            && filters::params_for(spec.id, params).is_some()
        {
            return filters::has_filterable_target(self, Some(LayerId(id)));
        }
        (spec.enabled)(self)
    }

    /// Apply an undoable edit to the active document.
    pub fn edit<R>(&mut self, label: &str, f: impl FnOnce(&mut Document, &mut Option<LayerId>) -> Result<R>) -> Result<R> {
        let restrict = self.color_restrict;
        let origin = self.origin();
        let st = self.active_mut().ok_or(EngineError::NoDocument)?;
        // Changes made outside an edit belong to the step before this one, not to this one.
        st.sync_kdm_loose(&origin);
        let before = st.doc.clone();
        // Selecting layers is not a step, so the state this edit leaves behind targets what was
        // selected just before it (#1356): undoing a stroke keeps the painted layer active.
        let prior = st.layer_target();
        let mut doc = (*before).clone();
        let mut active = st.active_layer;
        // Layers the edit makes take this document's next numbers (KHP A46).
        let numbering = st.numbering();
        let r = f(&mut doc, &mut active)?;
        if st.psd_values {
            kition_photo_io::stored::settle_psd(&mut doc);
        }
        st.next_layer = st.next_layer.max(numbering.next());
        drop(numbering);
        if let Some((id, k)) = restrict {
            channel_cmds::restrict_to_color(&before, &mut doc, id, k);
        }
        st.doc = Arc::new(doc);
        st.active_layer = active;
        fix_selection(st);
        channel_cmds::fix_view(st);
        // An edit that changes no pixels (a selection, guides, a layer's name) leaves the canvas
        // as it is; anything else recomposites the whole document unless the command reports
        // its own damage after this. A new lasso selection on a large document used to cost a
        // full recomposite (seconds on the CPU path, #1773).
        let unchanged = layer_multi_cmds::same_pixels(&before, &st.doc);
        let key = self.coalesce_request.clone();
        let hold = self.kdm_hold > 0;
        let st = self.active_mut().ok_or(EngineError::NoDocument)?;
        let layers = st.layer_target();
        let new_step = key.is_none() || st.coalesce != key || !st.history.can_undo();
        if new_step {
            st.history.set_current_layers(prior);
            st.history.record(label, before, layers);
            st.history.trim(&st.doc);
        } else {
            st.history.set_current_layers(layers);
        }
        // One transaction per edit; a coalescing gesture's edits share one undo group, as they
        // share one history step.
        if new_step && !hold {
            st.begin_kdm_step(key.is_some());
        }
        let steps = st.sync_kdm(&origin);
        st.history.add_current_external(steps);
        st.trim_kdm();
        st.coalesce = key;
        st.revision += 1;
        st.last_damage = unchanged.then_some(kition_photo_geom::Rect::EMPTY);
        Ok(r)
    }

    /// Replace the active document's active layer without creating a history step.
    pub fn select_layer(&mut self, id: LayerId) -> Result<()> {
        let st = self.active_mut().ok_or(EngineError::NoDocument)?;
        if st.doc.layer(id).is_none() {
            return Err(EngineError::NoLayer(id));
        }
        st.active_layer = Some(id);
        st.selected_layers = vec![id];
        st.layer_anchor = Some(id);
        // Selecting a layer is not an edit: keep a clean document clean.
        let clean = st.saved_revision == st.revision;
        st.revision += 1;
        if clean {
            st.saved_revision = st.revision;
        }
        st.last_damage = Some(kition_photo_geom::Rect::EMPTY);
        Ok(())
    }

    /// Re-renders the pixels of type layers whose font arrived after they were drawn (the web
    /// build fetches served fonts on demand, `kition_photo_text::served`). The layers' model is
    /// unchanged, so this is no history step and leaves a clean document clean. Unknown documents
    /// and layers are skipped. A document a background job is computing from must not move under
    /// it: its layers are returned, to try again later.
    pub fn refresh_type_layers(&mut self, layers: &[(kition_photo_doc::DocId, LayerId)]) -> Vec<(kition_photo_doc::DocId, LayerId)> {
        let mut busy = Vec::new();
        let mut docs: Vec<kition_photo_doc::DocId> = Vec::new();
        for (d, _) in layers {
            if !docs.contains(d) {
                docs.push(*d);
            }
        }
        for doc_id in docs {
            let mine = layers.iter().filter(|(d, _)| *d == doc_id);
            if self.job_on(doc_id).is_some() {
                busy.extend(mine);
                continue;
            }
            let Some(st) = self.docs.iter_mut().find(|st| st.doc.id == doc_id) else { continue };
            let snapshot = st.doc.clone();
            let mut doc = (*snapshot).clone();
            let mut changed = false;
            for (_, id) in mine {
                if let Some(kition_photo_doc::Layer { content: kition_photo_doc::LayerContent::Text(t), .. }) = doc.layer_mut(*id) {
                    type_cmds::refresh(&snapshot, t);
                    changed = true;
                }
            }
            if changed {
                let clean = st.saved_revision == st.revision;
                st.doc = Arc::new(doc);
                st.revision += 1;
                if clean {
                    st.saved_revision = st.revision;
                }
                st.last_damage = None;
            }
        }
        busy
    }

    pub fn undo(&mut self) -> bool {
        // A background job is computing from the current state: it must not move under it.
        if self.active_job().is_some() {
            return false;
        }
        let origin = self.origin();
        let non_person = self.non_person();
        let Some(st) = self.active_mut() else { return false };
        st.coalesce = None;
        st.sync_kdm_loose(&origin);
        let steps = st.history.undo_external();
        if non_person && (st.kdm.is_none() || steps == kition_photo_ops::SNAPSHOT_STEP) {
            return false;
        }
        if !st.kdm_can_step(steps, true) {
            return false;
        }
        match st.history.undo(st.doc.clone()) {
            Some((mut d, layers)) => {
                // Save As changes file identity outside history, just like the saved path.
                if d.name != st.doc.name {
                    Arc::make_mut(&mut d).name.clone_from(&st.doc.name);
                }
                // The shared document steps back on the replica, keeping collaborators' edits;
                // only this person's state (the selection) comes from the snapshot.
                let d = st.kdm_step(d, steps, true);
                // Pixels this step can have touched, so the canvas recomposites only that
                // (it recomposited everything before).
                let damage = layer_multi_cmds::step_damage(&d, &st.doc);
                st.doc = d;
                restore_target(st, layers);
                st.trim_kdm();
                st.revision += 1;
                st.last_damage = damage;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        if self.active_job().is_some() {
            return false;
        }
        let origin = self.origin();
        let non_person = self.non_person();
        let Some(st) = self.active_mut() else { return false };
        st.coalesce = None;
        st.sync_kdm_loose(&origin);
        let steps = st.history.redo_external();
        if non_person && (st.kdm.is_none() || steps == kition_photo_ops::SNAPSHOT_STEP) {
            return false;
        }
        if !st.kdm_can_step(steps, false) {
            return false;
        }
        match st.history.redo(st.doc.clone()) {
            Some((mut d, layers)) => {
                if d.name != st.doc.name {
                    Arc::make_mut(&mut d).name.clone_from(&st.doc.name);
                }
                let d = st.kdm_step(d, steps, false);
                let damage = layer_multi_cmds::step_damage(&st.doc, &d);
                st.doc = d;
                restore_target(st, layers);
                st.trim_kdm();
                st.revision += 1;
                st.last_damage = damage;
                true
            }
            None => false,
        }
    }
}

/// Undo / redo: target the layers the restored state targeted, as far as they still exist.
fn restore_target(st: &mut DocState, layers: LayerTarget) {
    if layers.active.is_some_and(|id| st.doc.layer(id).is_some()) {
        st.active_layer = layers.active;
        st.selected_layers = layers.selected;
    }
    fix_active(st);
}

fn fix_active(st: &mut DocState) {
    if st.active_layer.is_none_or(|id| st.doc.layer(id).is_none()) {
        st.active_layer = st.doc.top_layer();
    }
    fix_selection(st);
    channel_cmds::fix_view(st);
}

/// Keep the layer selection valid after the document or the active layer changed: drop deleted
/// layers, and collapse to the active layer when a command made a non-member active.
pub(crate) fn fix_selection(st: &mut DocState) {
    let Some(active) = st.active_layer else {
        st.selected_layers.clear();
        return;
    };
    if !st.selected_layers.contains(&active) {
        st.selected_layers = vec![active];
    } else {
        let live: std::collections::HashSet<LayerId> = st.doc.walk().into_iter().map(|(_, _, l)| l.id).collect();
        st.selected_layers.retain(|id| live.contains(id));
        let mut seen = std::collections::HashSet::new();
        st.selected_layers.retain(|id| seen.insert(*id));
    }
    if st.layer_anchor.is_some_and(|a| !st.selected_layers.contains(&a)) {
        st.layer_anchor = Some(active);
    }
}

#[cfg(test)]
mod fill_layer_mode_tests;
#[cfg(test)]
mod pattern_mode_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod c8_late_writer_tests;

#[cfg(test)]
mod fx_mode_tests;
