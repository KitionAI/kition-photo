//! Explicit call context (kition-sdk `KitDocument::editor_state`, `set_context`; Kit Process
//! Protocol section 7a; task card C-photo): the editor state a command may read besides its
//! parameters, taken and put back without an edit, set for one call from the layers the call acts
//! on, and kept out of an agent's commands that say what they act on.
//!
//! **The editor state** of an image is the person's editing session, never the file's content:
//! the active layer (what single-layer commands act on), the selected layers (what multi-layer
//! commands act on) and the range anchor, the pixel selection (where pixel commands act), the
//! targeted channel and the channels' eyes, the Layers panel's isolation, folded effects and
//! ⌥-click solo view, the painting symmetry, and the foreground and background colors. The view
//! (zoom, scroll), the tool and its options are the editor page's own and never reach the engine.
//! The Quick Mask mode and the history position are the document's (entering Quick Mask and
//! undoing are steps of the history), so no call context sets them; inspect lists them as editor
//! state ([`EDITOR_STATE`]) since they are not what the file holds.
//!
//! **A call's part**: an image has none. Its artboards are layer groups that every command
//! addresses in the image's own coordinates, so a call names one as a layer (`on`, a command's
//! `layer`). `at.part` is refused as invalid parameters; `at.time` as disabled (a video
//! timeline's current frame is the document's, and changing it is an edit).
//!
//! **`on`**: layers by their ids as inspect lists them (`12` or `"12"`): they become the selected
//! layers, the first the active one. An index spelling (`"#3"`) is refused: `on` takes ids.
//!
//! **An agent's command** (the host's dispatch runs it for an agent, or the document runs it as
//! an agent's) that says what it acts on gets exactly that, for that call only: `layer` makes
//! that layer the active and only selected layer for a command that reads the active layer,
//! `region` makes the pixel selection that area for a command that reads it, and
//! `foregroundColor` / `backgroundColor` the colors for a command that reads them ([`Given`]).
//! What the call set is put back after it, unless the command itself moved it (a layer it made
//! becomes active, as for the person). Commands whose work is moving the editor ([`moves_the_editor`])
//! are not bracketed.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use kition_kit::CallContext;
use kition_kit::failure::{CommandFailure, failure};
use kition_photo_engine::channel_cmds::ChannelView;
use kition_photo_engine::doc::LayerId;
use kition_photo_engine::symmetry_cmds::PaintingSymmetry;
use kition_photo_engine::{DocState, Session};
use kition_photo_raster::Surface;
use serde_json::{Value, json};

use crate::PhotoDocument;

/// What `inspect()` reports that is the editing session, not the file (kition-headless's volatile
/// keys, `testing::check_context`): JSON Pointers to members of the inspect's top level, and
/// `selected`, the flag on every layer of the tree that says the Layers panel selects it.
pub const EDITOR_STATE: &[&str] = &[
    "/activeLayer",
    "/selectedLayers",
    "/isolatedLayers",
    "/effectsCollapsed",
    "/hasSelection",
    "/selectionBounds",
    "/layerMaskView",
    "/symmetry",
    "/quickMask",
    "/channels/target",
    "/historyIndex",
    "/historyRedo",
    "selected",
];

/// How many snapshots [`PhotoDocument::take_editor_state`] keeps for
/// [`PhotoDocument::put_back`]: a host restores the one it took after a call, and an editor
/// running an agent's steps holds two or three at a time.
const KEPT: usize = 64;

/// The editor state of the active document and the session's colors.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    active_layer: Option<LayerId>,
    selected_layers: Vec<LayerId>,
    layer_anchor: Option<LayerId>,
    isolated_layers: Vec<LayerId>,
    fx_collapsed: Vec<LayerId>,
    show_only: Option<(LayerId, Vec<(LayerId, bool)>)>,
    channel_view: ChannelView,
    symmetry: Option<PaintingSymmetry>,
    selection: Option<Surface>,
    foreground: [f32; 4],
    background: [f32; 4],
}

impl Snapshot {
    fn take(session: &Session) -> Option<Self> {
        let st = session.active()?;
        Some(Self {
            active_layer: st.active_layer,
            selected_layers: st.selected_layers.clone(),
            layer_anchor: st.layer_anchor,
            isolated_layers: st.isolated_layers.clone(),
            fx_collapsed: st.fx_collapsed.clone(),
            show_only: st.show_only.clone(),
            channel_view: st.channel_view.clone(),
            symmetry: st.symmetry.clone(),
            selection: st.doc.selection.clone(),
            foreground: session.tools.foreground,
            background: session.tools.background,
        })
    }

    /// Puts the state back on the active document, dropping layers it no longer has. No edit:
    /// the revision, the dirty state and the history stay.
    fn put(&self, session: &mut Session) {
        session.tools.foreground = self.foreground;
        session.tools.background = self.background;
        let Some(st) = session.active_mut() else { return };
        let has = |id: &LayerId| st.doc.layer(*id).is_some();
        // Keep a deleted target's identity until the person explicitly selects another layer.
        // A stale agent call must name that missing object, never fall through to a neighbour.
        st.active_layer = self.active_layer;
        st.selected_layers = self.selected_layers.iter().copied().filter(has).collect();
        st.layer_anchor = self.layer_anchor.filter(has);
        st.isolated_layers = self.isolated_layers.iter().copied().filter(has).collect();
        st.fx_collapsed = self.fx_collapsed.iter().copied().filter(has).collect();
        st.show_only = self.show_only.clone().filter(|(id, _)| has(id));
        st.channel_view = self.channel_view.clone();
        st.symmetry = self.symmetry.clone();
        set_selection(st, self.selection.clone());
    }

    /// The same for the same state: pointers of the selection's tiles, which a snapshot keeps alive.
    fn fingerprint(&self) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            self.active_layer,
            self.selected_layers,
            self.layer_anchor,
            self.isolated_layers,
            self.fx_collapsed,
            self.show_only,
            self.channel_view,
            self.symmetry,
            self.foreground,
            self.background
        )
        .hash(&mut hasher);
        if let Some(selection) = &self.selection {
            for (coord, tile) in selection.tiles() {
                (coord.tx, coord.ty, Arc::as_ptr(tile) as usize).hash(&mut hasher);
            }
            format!("{:?}", selection.default_pixel()).hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }
}

/// The pixel selection of `st`, set without an edit (the selection is the person's, never the
/// replica's); the document is copied only when it changes.
pub(crate) fn set_selection(st: &mut DocState, selection: Option<Surface>) {
    if st.doc.selection != selection {
        Arc::make_mut(&mut st.doc).selection = selection;
    }
}

/// The snapshots answered so far, by their fingerprint (the newest last).
#[derive(Clone, Default)]
pub(crate) struct Kept {
    states: VecDeque<(String, Snapshot)>,
}

pub(crate) fn invalid(message: impl std::fmt::Display) -> String {
    failure(CommandFailure::InvalidParams, message)
}

pub(crate) fn disabled(message: impl std::fmt::Display) -> String {
    failure(CommandFailure::Disabled, message)
}

/// Where inspect lists what a call may name.
const LISTED: &str = "layers' ids are in photo_inspect's `layers` (`id`)";

/// A layer id as `on`, `layer` and a region's `layer` take it: a whole number or its digits.
pub(crate) fn layer_of(value: &Value) -> Option<LayerId> {
    value.as_u64().or_else(|| value.as_str().and_then(|text| text.trim().parse().ok())).map(LayerId)
}

/// Whether a call aims at the targeted layer's mask (`"target": "mask"`).
pub(crate) fn targets_mask(params: &Value) -> bool {
    params.get("target").and_then(Value::as_str) == Some("mask")
}

/// Commands whose work is moving the editor (the pixel selection, the layer selection, the
/// targeted channel, the colors, the brush, the view): they keep their effect for an agent too.
pub(crate) fn moves_the_editor(command: &str) -> bool {
    ["select.", "view.", "tools.", "brush.", "channel.target", "layer.select", "symmetry."].iter().any(|prefix| command.starts_with(prefix))
        || matches!(command, "layer.showOnly" | "layer.isolate" | "layer.setEffectsExpanded")
}

/// What an agent's call gave of the state a command reads, for that call ([`PhotoDocument::bracketed`]).
#[derive(Default)]
pub(crate) struct Given {
    pub layer: Option<LayerId>,
    pub region: Option<Surface>,
    pub region_given: bool,
    pub foreground: Option<[f32; 4]>,
    pub background: Option<[f32; 4]>,
}

/// `#rrggbb` (or `#rrggbbaa`) as the engine's color.
fn color(value: &Value, key: &str) -> Result<[f32; 4], String> {
    let text = value.as_str().ok_or_else(|| invalid(format!("`{key}` is {value}: a color is \"#rrggbb\"")))?;
    let hex = text.trim_start_matches('#');
    let byte = |at: usize| hex.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()).map(|v| f32::from(v) / 255.0);
    match hex.len() {
        6 => Some([byte(0), byte(2), byte(4), Some(1.0)]),
        8 => Some([byte(0), byte(2), byte(4), byte(6)]),
        _ => None,
    }
    .and_then(|parts| Some([parts[0]?, parts[1]?, parts[2]?, parts[3]?]))
    .ok_or_else(|| invalid(format!("`{key}` is {value}: a color is \"#rrggbb\"")))
}

/// Four whole numbers `[x, y, width, height]`.
fn rect(value: &Value, what: &str) -> Result<[i64; 4], String> {
    let items = value.as_array().filter(|items| items.len() == 4);
    let numbers: Option<Vec<i64>> =
        items.and_then(|items| items.iter().map(|item| item.as_f64().filter(|n| n.is_finite()).map(|n| n.round() as i64)).collect());
    match numbers.as_deref() {
        Some([x, y, w, h]) if *w > 0 && *h > 0 => Ok([*x, *y, *w, *h]),
        _ => Err(invalid(format!("a region's `{what}` is [x, y, width, height] in pixels, width and height above 0, not {value}"))),
    }
}

/// The forms `region` takes, for messages and the schema.
pub(crate) const REGION_FORMS: &str = "{\"rect\": [x, y, width, height]}, {\"ellipse\": [x, y, width, height]}, {\"polygon\": [[x, y], …]} (3 points or more), {\"layer\": id} (that layer's pixels, by their opacity), {\"channel\": name or index} (a saved selection or alpha channel), or \"all\"; add \"invert\": true to take everything else";

/// The pixel selection `region` names, computed by the engine's own selection commands on a copy
/// of the document (no edit of this one).
pub(crate) fn region_mask(st: &DocState, region: &Value) -> Result<Option<Surface>, String> {
    if region.is_null() {
        return Ok(None);
    }
    let mut scratch = Session::new();
    scratch.add_document((*st.doc).clone(), None);
    let run = |scratch: &mut Session, command: &str, params: Value| -> Result<(), String> {
        scratch.execute(command, params).map(|_| ()).map_err(|error| invalid(format!("the region: {error}")))
    };
    let invert = region.get("invert").and_then(Value::as_bool).unwrap_or(false);
    match region {
        Value::String(text) if text == "all" => run(&mut scratch, "select.all", json!({}))?,
        Value::Object(fields) => {
            let named: Vec<&String> = fields.keys().filter(|key| *key != "invert").collect();
            let [key] = named.as_slice() else {
                return Err(invalid(format!("`region` is {region}: it names one area, {REGION_FORMS}")));
            };
            let value = &fields[key.as_str()];
            match key.as_str() {
                "rect" | "ellipse" => {
                    let [x, y, width, height] = rect(value, key)?;
                    run(&mut scratch, "select.rect", json!({ "x": x, "y": y, "width": width, "height": height, "ellipse": *key == "ellipse" }))?;
                }
                "polygon" => {
                    let points = value
                        .as_array()
                        .filter(|points| points.len() >= 3)
                        .ok_or_else(|| invalid(format!("a region's `polygon` is [[x, y], …], 3 points or more, not {value}")))?;
                    run(&mut scratch, "select.lasso", json!({ "points": points }))?;
                }
                "layer" => {
                    let id = layer_of(value)
                        .filter(|id| st.doc.layer(*id).is_some())
                        .ok_or_else(|| invalid(format!("a region's `layer` is {value}, which names no layer: {LISTED}")))?;
                    run(&mut scratch, "select.loadSelection", json!({ "channel": "transparency", "layer": id.0 }))?;
                }
                "channel" => {
                    let known = st
                        .doc
                        .channels
                        .iter()
                        .enumerate()
                        .any(|(index, channel)| value.as_str() == Some(channel.name.as_str()) || value.as_u64() == Some(index as u64));
                    if !known {
                        let names: Vec<&str> = st.doc.channels.iter().map(|channel| channel.name.as_str()).collect();
                        return Err(invalid(format!(
                            "a region's `channel` is {value}, which names no saved selection or alpha channel; the image has {names:?}"
                        )));
                    }
                    run(&mut scratch, "select.loadSelection", json!({ "channel": value }))?;
                }
                _ => return Err(invalid(format!("`region` is {region}: {REGION_FORMS}"))),
            }
        }
        _ => return Err(invalid(format!("`region` is {region}: {REGION_FORMS}"))),
    }
    if invert {
        run(&mut scratch, "select.inverse", json!({}))?;
    }
    Ok(scratch.active().and_then(|scratch| scratch.doc.selection.clone()))
}

impl PhotoDocument {
    /// Whether the command running now runs for an agent: the host's dispatch says so (headless),
    /// or the document runs it as an agent's (the open editor's `executeAs`).
    pub(crate) fn is_agent(&self) -> bool {
        kition_kit::dispatch::role() == Some(kition_kit::dispatch::Role::Agent) || self.session.origin().kind == kition_kit::kdm::OriginKind::Agent
    }

    /// The editor's state, kept for [`PhotoDocument::put_back`], as `{snapshot, active,
    /// selection, area, foreground, background}` (`area`: the pixel selection's bounds,
    /// kition-sdk's `EditorState::Area`); `null` with no image open.
    pub(crate) fn take_editor_state(&self) -> Value {
        let Some(state) = Snapshot::take(&self.session) else { return Value::Null };
        let number = state.fingerprint();
        let hex = |c: [f32; 4]| {
            format!(
                "#{:02x}{:02x}{:02x}",
                (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                (c[2].clamp(0.0, 1.0) * 255.0).round() as u8
            )
        };
        let answer = json!({
            "snapshot": number,
            "active": state.active_layer.map(|id| id.0),
            "selection": state.selected_layers.iter().map(|id| id.0).collect::<Vec<_>>(),
            "area": state.selection.as_ref().map(|mask| kition_photo_engine::inspect::xywh(mask.content_bounds())),
            "foreground": hex(state.foreground),
            "background": hex(state.background),
        });
        let mut kept = self.kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        kept.states.retain(|(n, _)| *n != number);
        kept.states.push_back((number, state));
        while kept.states.len() > KEPT {
            kept.states.pop_front();
        }
        answer
    }

    /// Puts back what [`PhotoDocument::take_editor_state`] answered; no edit.
    pub(crate) fn put_back(&mut self, saved: &Value) -> Result<(), String> {
        if saved.is_null() {
            return Ok(());
        }
        let number = saved.get("snapshot").and_then(Value::as_str).ok_or_else(|| invalid(format!("{saved} is not an editor state this kit answered")))?;
        let state = {
            let kept = self.kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            kept.states.iter().find(|(n, _)| n == number).map(|(_, state)| state.clone())
        };
        let state = state.ok_or_else(|| invalid(format!("editor state {number} is no longer kept (the kit keeps the last {KEPT}); take it again")))?;
        state.put(&mut self.session);
        Ok(())
    }

    /// Sets the layers a call acts on (`on`) as the selected layers, the first active; refuses a
    /// part, a time and what the image lacks, before changing anything. No edit.
    pub(crate) fn set_call_context(&mut self, context: &CallContext) -> Result<(), String> {
        if let Some(part) = &context.part {
            return Err(invalid(format!(
                "the part {part}: an image has no parts. Its artboards are layer groups in the image's coordinates: name one with `on` or the command's `layer` ({LISTED})"
            )));
        }
        if context.time.is_some() {
            return Err(disabled(
                "a call cannot set the current time: a video timeline's current frame is the document's own (`timeline.goToFrame` changes it, as an edit)",
            ));
        }
        if context.on.is_empty() {
            return Ok(());
        }
        let st = self.session.active().ok_or_else(|| disabled("no image is open"))?;
        let mut ids: Vec<LayerId> = Vec::new();
        for item in &context.on {
            if item.as_str().is_some_and(|text| text.trim_start().starts_with('#')) {
                return Err(invalid(format!(
                    "`on` holds {item}, a place in the layer stack: `on` takes layer ids ({LISTED}); give the place as the command's `layer` instead"
                )));
            }
            let id =
                layer_of(item).ok_or_else(|| invalid(format!("`on` holds {item}, which is not a layer id: a layer is named by its numeric id; {LISTED}")))?;
            if st.doc.layer(id).is_none() {
                return Err(invalid(format!("the image has no layer {}: {LISTED}", id.0)));
            }
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let st = self.session.active_mut().ok_or_else(|| disabled("no image is open"))?;
        st.active_layer = ids.first().copied();
        st.layer_anchor = ids.first().copied();
        st.selected_layers = ids;
        Ok(())
    }

    /// What an agent's call gives of the state `command` reads: `layer` (a command that reads
    /// the active layer), `region` (one that reads the pixel selection), `foregroundColor`,
    /// `backgroundColor` (one that reads them). The binding's own keys are taken out of `params`.
    pub(crate) fn given(&self, command: &str, params: &mut Value) -> Result<Given, String> {
        let reads = crate::acts::reads_of(command);
        let mut given = Given::default();
        let Some(fields) = params.as_object_mut() else { return Ok(given) };
        let st = self.session.active();
        if crate::acts::picks_by_point(command)
            && let Some(at) = fields.remove("at")
        {
            if fields.get("layer").is_some_and(|layer| !layer.is_null()) {
                return Err(invalid("give the layer by `layer` or by `at`, not both"));
            }
            let st = st.ok_or_else(|| disabled("no image is open"))?;
            let (x, y) = crate::place::point(&at)?;
            let id = crate::place::pick(&st.doc, x, y)
                .ok_or_else(|| invalid(format!("no layer has a visible pixel at [{x}, {y}] (hidden and fully locked layers are passed over)")))?;
            fields.insert("layer".to_owned(), json!(id.0));
        }
        if reads.active
            && let Some(value) = fields.get("layer").filter(|value| !value.is_null()).cloned()
        {
            let st = st.ok_or_else(|| disabled("no image is open"))?;
            let id = crate::place::layer_value(&st.doc, &value)?;
            fields.insert("layer".to_owned(), json!(id.0));
            given.layer = Some(id);
        }
        if let Some(region) = fields.remove("region") {
            if !reads.region {
                return Err(invalid(format!("`{command}` does not act inside the selection: it takes no `region`")));
            }
            let st = st.ok_or_else(|| disabled("no image is open"))?;
            given.region = region_mask(st, &region)?;
            given.region_given = true;
        }
        for (key, slot) in [("foregroundColor", &mut given.foreground), ("backgroundColor", &mut given.background)] {
            if let Some(value) = fields.remove(key) {
                if !reads.colors {
                    return Err(invalid(format!("`{command}` does not use the foreground or background color: it takes no `{key}`")));
                }
                *slot = Some(color(&value, key)?);
            }
        }
        Ok(given)
    }

    /// Runs an agent's command with what its call gave ([`Given`]) and puts back what the call set
    /// that the command did not move itself, also when it fails.
    pub(crate) fn bracketed(&mut self, given: Given, run: impl FnOnce(&mut Self) -> Result<Value, String>) -> Result<Value, String> {
        let Some(before) = Snapshot::take(&self.session) else { return run(self) };
        if let Some(color) = given.foreground {
            self.session.tools.foreground = color;
        }
        if let Some(color) = given.background {
            self.session.tools.background = color;
        }
        if let Some(st) = self.session.active_mut() {
            if let Some(id) = given.layer {
                st.active_layer = Some(id);
                st.selected_layers = vec![id];
                st.layer_anchor = Some(id);
            }
            if given.region_given {
                set_selection(st, given.region.clone());
            }
        }
        let set = Snapshot::take(&self.session);
        let result = run(self);
        let (Some(set), Some(now)) = (set, Snapshot::take(&self.session)) else { return result };
        let mut back = now.clone();
        if given.layer.is_some() && now.active_layer == set.active_layer && now.selected_layers == set.selected_layers {
            back.active_layer = before.active_layer;
            back.selected_layers = before.selected_layers.clone();
            back.layer_anchor = before.layer_anchor;
        }
        if given.region_given && now.selection == set.selection {
            back.selection = before.selection.clone();
        }
        if given.foreground.is_some() && now.foreground == set.foreground {
            back.foreground = before.foreground;
        }
        if given.background.is_some() && now.background == set.background {
            back.background = before.background;
        }
        if back != now {
            back.put(&mut self.session);
        }
        result
    }
}
