//! What `mutates` means for Kition Photo's commands: whether running one changes the document.
//! Shared by the in-page SDK and the `kition-photo` executable (`bindings/headless`), so a read-only
//! document refuses the same commands in both.

/// Journaled commands that change only this person's own state, never the document: the
/// selection, quick mask, layer and channel targets (the KDM bridge never writes them), the
/// clipboard, tool colors and sizes, the application's settings, and file commands that only
/// close the document or produce bytes.
const PERSONAL_COMMANDS: &[&str] = &[
    "file.close",
    "file.closeAll",
    "file.closeOthers",
    "file.print",
    "file.printOneCopy",
    "file.saveACopy",
    "file.revealInFinder",
    "edit.copy",
    "edit.copyMerged",
    "edit.colorSettings",
    "edit.keyboardShortcuts",
    "edit.menus",
    "edit.toolbar",
    "layer.select",
    "layer.selectAbove",
    "layer.selectBelow",
    "layer.selectTop",
    "layer.selectBottom",
    "layer.addAboveToSelection",
    "layer.addBelowToSelection",
    "layer.selectLinkedLayers",
];

/// Families of [`PERSONAL_COMMANDS`].
const PERSONAL_FAMILIES: &[&str] = &["select.", "tools.", "channel.target", "edit.purge.", "file.export."];

/// Selection commands that do change the document: they make a path, shape, channel or layer.
const SELECTION_EDITS: &[&str] = &["select.toWorkPath", "select.convertToShape", "select.saveSelection", "select.float", "select.drop"];

/// Commands the engine does not journal that still change the document: playing an action runs
/// others, and the frame animation's timeline, its current frame included, is in the replica.
const UNJOURNALED_EDITS: &[&str] =
    &["action.play", "timeline.add", "timeline.delete", "timeline.setProps", "timeline.setFrame", "timeline.nextFrame", "timeline.previousFrame"];

/// Whether running `spec` changes the document (`KitCommand::mutates`). The engine's `journal`
/// also records commands that change only this person's state ([`PERSONAL_COMMANDS`]); `view.*`
/// commands edit guides and slices, which the document holds, so they stay edits.
pub(crate) fn changes_document(spec: &kition_photo_engine::CommandSpec) -> bool {
    let id = spec.id;
    if UNJOURNALED_EDITS.contains(&id) || SELECTION_EDITS.contains(&id) {
        return true;
    }
    spec.journal && !PERSONAL_COMMANDS.contains(&id) && !PERSONAL_FAMILIES.iter().any(|family| id.starts_with(family))
}
