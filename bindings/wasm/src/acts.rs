//! What each command reads besides its parameters and what it acts on (`KitCommand::reads`,
//! `targets`, `places`, `objects`, `index`, `position_independent`), for kition-sdk's explicit
//! call context and premise guard (task card C-photo).
//!
//! `tests/acts.rs` runs
//! every command on an image in editor states that differ in one member (the active layer, the
//! selected layers, the pixel selection, the colors) and fails when a command's outcome depends
//! on a member its row here does not name, or when a row names a member the command's outcome
//! never depended on where it ran. No-op and unsteady commands need implementation checks:
//! shared filter reads are checked explicitly; remaining unobserved rows use descriptions as
//! a fallback, with exclusions for implementations that address documents or editor settings.
//!
//! - `Active` ([`ACTIVE`]): the command acts on the active layer when given no `layer`. It is
//!   published with `targets: "layer"` (`on` fills `layer`), and an agent's `layer` makes that
//!   layer active for the call ([`crate::context`]).
//! - `Selection` ([`SELECTED`]): it acts on the selected layers; `on` sets them.
//! - `Area` ([`REGION`]): it acts inside the pixel selection; an agent's `region` sets it for the
//!   call. The editor state carries it (`area`), and inspect reports it (`/hasSelection`,
//!   `/selectionBounds`, both in [`crate::EDITOR_STATE`]).
//! - the colors ([`COLORS`]): it uses the foreground or background color; an agent's
//!   `foregroundColor` / `backgroundColor` set them for the call.

use kition_kit::{EditorState, KitCommand};
use serde_json::{Value, json};

/// What one command reads besides its parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reads {
    pub active: bool,
    pub selection: bool,
    pub region: bool,
    pub colors: bool,
}

include!("acts_tables.rs");

fn listed(table: &[&str], id: &str) -> bool {
    table.binary_search(&id).is_ok()
}

/// What command `id` reads besides its parameters ([`Reads`]).
pub fn reads_of(id: &str) -> Reads {
    Reads { active: listed(ACTIVE, id), selection: listed(SELECTED, id), region: listed(REGION, id), colors: listed(COLORS, id) }
}

/// Parameters besides `layer` that name layers by id, by command (`KitCommand::objects`): a whole
/// number there is a layer an agent's write with `basis` depends on. `tests/object_params.rs`
/// fails for a parameter that takes a layer id and is not listed here.
pub const OBJECTS: &[(&str, &[&str])] = &[
    ("edit.autoAlignLayers", &["reference"]),
    ("filter.distort.displace", &["mapLayer", "region.layer"]),
    ("image.adjustments.matchColor", &["sourceLayer", "region.layer"]),
    ("image.applyImage", &["source.layer", "mask.layer", "sourceLayer", "maskLayer", "region.layer"]),
    ("image.calculations", &["source1.layer", "source2.layer", "mask.layer", "source1Layer", "source2Layer", "maskLayer"]),
    ("layer.copyToDocument", &["layers"]),
    ("layer.layerStyle.transferEffects", &["from", "to"]),
    ("layer.moveTo", &["layers", "target"]),
    ("shape.add", &["addTo"]),
    ("shape.presets.place", &["addTo"]),
    ("style.presets.apply", &["layers"]),
    ("text.characterStyle.apply", &["layer", "layers"]),
    ("text.characterStyle.clearOverride", &["layer", "layers"]),
    ("text.characterStyle.list", &["layer", "layers"]),
    ("text.characterStyle.new", &["layer", "layers"]),
    ("text.characterStyle.redefine", &["layer", "layers"]),
    ("text.paragraphStyle.apply", &["layer", "layers"]),
    ("text.paragraphStyle.clearOverride", &["layer", "layers"]),
    ("text.paragraphStyle.list", &["layer", "layers"]),
    ("text.paragraphStyle.new", &["layer", "layers"]),
    ("text.paragraphStyle.redefine", &["layer", "layers"]),
];

/// Whether what `command` does to its layer does not depend on where the layer is in the stack:
/// the families whose code was read ([`crate::place::family`]) and the layer style commands,
/// which set one layer's own fields. (No field of a layer says where it is: its pixels are in
/// image coordinates, and a move of them is a change of its `tiles`.)
pub fn position_independent(command: &str) -> bool {
    crate::place::family(command).is_some() || command.starts_with("layer.layerStyle.") && !command.ends_with("transferEffects")
}

/// Whether an agent may name `command`'s layer by a point (`at`): it reads the active layer and
/// takes no `at` of its own (a warp's handle, a caret position).
pub fn picks_by_point(command: &str) -> bool {
    reads_of(command).active && kition_photo_engine::commands::find(command).is_none_or(|spec| !spec.params.contains("\"at\""))
}

/// The `EditorState` members a command with `reads` declares: the active layer, the selected
/// layers, and the pixel selection (`Area`). The colors have no member.
const fn states(reads: Reads) -> &'static [EditorState] {
    use EditorState::{Active, Area, Selection};
    match (reads.active, reads.selection, reads.region) {
        (true, true, true) => &[Active, Selection, Area],
        (true, true, false) => &[Active, Selection],
        (true, false, true) => &[Active, Area],
        (true, false, false) => &[Active],
        (false, true, true) => &[Selection, Area],
        (false, true, false) => &[Selection],
        (false, false, true) => &[Area],
        (false, false, false) => &[],
    }
}

/// Sets each command's `reads` and `targets`, and adds the keys an agent's call gives the state
/// it reads with (`layer`, `region`, `foregroundColor`, `backgroundColor`) to its schema.
pub fn declare(commands: &mut [KitCommand]) {
    for command in commands {
        let reads = reads_of(&command.id);
        command.reads = states(reads);
        let Some(Value::Object(schema)) = command.schema.as_mut() else { continue };
        if reads.active {
            schema.insert(
                "layer".to_owned(),
                json!({
                    "oneOf": [{ "type": "integer" }, { "type": "string" }],
                    "description": "The layer: its id as inspect lists it, \"#n\" (the n-th layer of the image's top level as inspect lists it, counted from the top: \"#1\" is the top one), or its whole name when no other layer has it; the active layer by default.",
                }),
            );
            if picks_by_point(&command.id) {
                command.places = &["at"];
                schema.insert(
                "at".to_owned(),
                json!({ "type": "array", "items": { "type": "number" }, "description": "Instead of `layer`: a point [x, y] in image pixels; the command acts on the topmost visible layer, not fully locked, with a pixel there (the Move tool's Auto-Select)." }),
            );
            }
            command.targets = Some("layer");
            command.index = Some("#");
            command.index_reversed = true;
        }
        command.position_independent = position_independent(&command.id);
        if let Some((_, params)) = OBJECTS.iter().find(|(id, _)| *id == command.id) {
            command.objects = params;
        } else if !reads.active && schema.contains_key("layer") && command.objects.is_empty() {
            // A command that acts on the selected layers unless given one: `layer` names a layer.
            command.objects = &["layer"];
        }
        if reads.region && command.objects.is_empty() {
            command.objects = &["region.layer"];
        }
        if command.id == "layer.select" {
            command.targets = Some("layer");
            command.objects = &[];
        }
        if command.id == "channel.rename" {
            command.places = &["channel"];
        }
        // All these addresses resolve in the layer tree, never in the image/channel id spaces.
        if command.targets == Some("layer") {
            command.object_types = &[("layer", &["Layer"])];
            command.name_field = Some("name");
        }
        if reads.region {
            schema.insert(
                "region".to_owned(),
                json!({
                    "type": "json",
                    "description": format!(
                        "Where the command acts, for this call only: {}. Without it, inside the current selection (made with select.* earlier in the run), or everywhere when there is none.",
                        crate::context::REGION_FORMS
                    ),
                }),
            );
        }
        if reads.colors {
            for (key, which) in [("foregroundColor", "foreground"), ("backgroundColor", "background")] {
                schema.insert(
                    key.to_owned(),
                    json!({ "type": "string", "description": format!("The {which} color the command uses, \"#rrggbb\", for this call only; the current {which} color by default.") }),
                );
            }
        }
    }
}
