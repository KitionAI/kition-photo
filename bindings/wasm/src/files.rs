//! The parameters of this kit's commands that name files or folders on disk (KHP A28), declared
//! as typed files in the commands' schemas (`kition_kit::files`): a host lends them, and a
//! headless kit keeps those commands from agents (the host opens, saves and exports documents).

use kition_kit::files::FileParam;

/// Every file or folder parameter of the kit's commands.
pub const FILES: &[FileParam] = &[
    FileParam::read("layer.newAdjustmentLayer.colorLookup", "file", &["cube", "3dl", "look"]),
    FileParam::read("image.adjustments.colorLookup", "file", &["cube", "3dl", "look"]),
    FileParam::read("brush.presets.importAbr", "path", &["abr"]),
    FileParam::read("gradient.presets.importGrd", "path", &["grd"]),
    FileParam::read("swatches.import", "path", &["aco", "ase"]),
    FileParam::create("swatches.export", "path", &["aco", "ase"]),
    FileParam::create("file.saveACopy", "path", &[]),
    FileParam::read("file.openAs", "path", &[]),
    FileParam::read("file.placeEmbedded", "path", &[]),
    FileParam::read("file.placeLinked", "path", &[]),
    FileParam::read("file.scripts.loadFilesIntoStack", "paths", &[]),
    FileParam::folder("file.export.layersToFiles", "dir"),
    FileParam::create("file.export.colorLookupTables", "path", &["cube"]),
    FileParam::read("layer.objectLayers.replaceContents", "path", &[]),
    FileParam::create("layer.objectLayers.exportContents", "path", &[]),
    FileParam::read("layer.objectLayers.relinkToFile", "path", &[]),
    FileParam::create("layer.objectLayers.convertToLinked", "path", &[]),
    FileParam::read("file.automate.photomerge", "paths", &[]),
    FileParam::read("file.automate.mergeToHdrPro", "paths", &[]),
    FileParam::create("layer.quickExportAsPng", "path", &["png"]),
    FileParam::create("layer.exportAs", "path", &[]),
    FileParam::read("pattern.import", "path", &["pat"]),
    FileParam::create("pattern.export", "path", &["pat"]),
    FileParam::folder("file.export.layerCompsToFiles", "dir"),
    FileParam::folder("file.export.artboardsToFiles", "dir"),
    FileParam::create("file.export.artboardsToPdf", "path", &["pdf"]),
    FileParam::create("measurementLog.export", "path", &["csv"]),
    FileParam::read("file.import.notes", "path", &["psd", "psb", "kphoto"]),
    FileParam::folder("file.export.saveForWebLegacy", "dir"),
    FileParam::create("file.export.saveForWebLegacy", "path", &[]),
    FileParam::create("file.export.quickExport", "path", &[]),
    FileParam::folder("file.generate.imageAssets", "dir"),
    FileParam::create("file.automate.createDroplet", "path", &["pcdroplet"]),
    FileParam::read("file.scripts.browse", "path", &[]),
    FileParam::folder("file.package", "dir"),
    FileParam::create("file.export.pathsToIllustrator", "path", &["ai"]),
    FileParam::read("edit.presets.migratePresets", "path", &[]),
    FileParam::read("file.import.variableDataSets", "path", &["csv"]),
    FileParam::folder("file.export.dataSetsAsFiles", "dir"),
    FileParam::read("plugin.install", "path", &["wasm"]),
    FileParam::folder("plugin.reload", "path"),
];

/// Refuses `command` for anyone but a person (an agent, a script's or the host's batch; the
/// engine's [`kition_photo_engine::Session::agent_gate`], KHP A28, A29): it names files or folders
/// by path, and a kit works only on its workspace's files, through its host. Whoever runs it
/// (directly, inside `model.batch`, or as a step of `actions.play` or a script) is refused alike.
pub fn refuse_for_agent(command: &str, _params: &serde_json::Value) -> kition_photo_engine::Result<()> {
    if FILES.iter().any(|file| file.command == command) {
        return Err(kition_photo_engine::EngineError::Other(format!(
            "`{command}` reaches files or folders by path, and this kit works only on files of its workspace, through its host: \
             it is not available to the agent. The host opens, saves and exports the image (file.exportImage gives its pixels)"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use kition_kit::KitDocument;

    use super::*;

    /// Parameters whose names read like files but are not: vector paths (the work path, a saved
    /// path, path data; a new fill or adjustment layer's `path` becomes its vector mask), brush
    /// preset folders, preference keys.
    const NOT_FILES: &[(&str, &str)] = &[
        ("layer.newFillLayer.solidColor", "path"),
        ("layer.newFillLayer.gradient", "path"),
        ("layer.newFillLayer.pattern", "path"),
        ("layer.newAdjustmentLayer.brightnessContrast", "path"),
        ("layer.newAdjustmentLayer.levels", "path"),
        ("layer.newAdjustmentLayer.curves", "path"),
        ("layer.newAdjustmentLayer.exposure", "path"),
        ("layer.newAdjustmentLayer.vibrance", "path"),
        ("layer.newAdjustmentLayer.hueSaturation", "path"),
        ("layer.newAdjustmentLayer.colorBalance", "path"),
        ("layer.newAdjustmentLayer.blackWhite", "path"),
        ("layer.newAdjustmentLayer.photoFilter", "path"),
        ("layer.newAdjustmentLayer.channelMixer", "path"),
        ("layer.newAdjustmentLayer.invert", "path"),
        ("layer.newAdjustmentLayer.posterize", "path"),
        ("layer.newAdjustmentLayer.threshold", "path"),
        ("layer.newAdjustmentLayer.gradientMap", "path"),
        ("layer.newAdjustmentLayer.selectiveColor", "path"),
        ("layer.newAdjustmentLayer.colorLookup", "path"),
        ("filter.blurGallery.pathBlur", "paths"),
        ("filter.render.flame", "path"),
        ("shape.create", "path"),
        ("shape.edit", "path"),
        ("shape.presets.new", "path"),
        ("path.set", "path"),
        ("layer.vectorMask.add", "path"),
        ("layer.vectorMask.edit", "path"),
        ("edit.defineCustomShape", "path"),
        ("file.export.pathsToIllustrator", "paths"),
        ("brush.presets.move", "folder"),
        ("brush.presets.moveGroup", "folder"),
        ("brush.presets.renameGroup", "folder"),
        ("brush.presets.deleteGroup", "folder"),
        ("prefs.get", "path"),
        ("prefs.set", "path"),
        ("prefs.reset", "path"),
    ];

    #[test]
    fn every_file_parameter_is_declared() {
        let doc = crate::PhotoDocument::create("Untitled.psd", 16, 16).unwrap_or_else(|error| panic!("{error}"));
        let problems = kition_kit::files::check(&doc.commands(), FILES, NOT_FILES);
        assert!(problems.is_empty(), "{problems:#?}");
    }
}
