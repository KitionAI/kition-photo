import type { MenuRow } from '@kition/sdk/editor'

const HOST_FILE = 'The host opens, saves, and closes documents.'
const HOST_OPENS = 'The host opens documents.'

/**
 * Commands a browser editor never shows, with the reason each is hidden: the host's own file
 * commands, those that need a local file system or OS integration (an audit of the menus, and the
 * rows the web engine refuses, which `parity/check.test.mjs` finds), and those that open more
 * documents in the engine (menu rows and panel buttons alike).
 */
const HIDDEN: Readonly<Record<string, string>> = {
  'file.new': HOST_FILE,
  'file.open': HOST_FILE,
  'file.close': HOST_FILE,
  'file.closeAll': HOST_FILE,
  'file.closeOthers': HOST_FILE,
  'file.save': HOST_FILE,
  'file.saveAs': HOST_FILE,
  'file.exit': HOST_FILE,
  'file.revert': 'The host reverts a document to its saved copy.',
  'file.saveACopy': 'Writes a second file to a local folder.',
  'file.openAs': 'Opens a local file in a chosen format.',
  'file.placeLinked': 'Links a local file that stays on disk.',
  'file.scripts.browse': 'Runs a script from a local file.',
  'file.import.videoFramesToLayers': 'Imports frames from a local video file.',
  'file.import.notes': 'Imports notes from a local file.',
  'file.import.variableDataSets': 'Imports data sets from a local file.',
  'file.import.wiaSupport': 'Imports from a scanner or camera through the operating system.',
  'file.export.renderVideo': 'Renders a video file to a local folder.',
  'file.export.dataSetsAsFiles': 'Writes one file per data set to a local folder.',
  'file.export.layersToFiles': 'Writes one file per layer to a local folder.',
  'file.export.layerCompsToFiles': 'Writes one file per layer comp to a local folder.',
  'file.export.artboardsToFiles': 'Writes one file per artboard to a local folder.',
  'file.revealInFinder': 'Shows the file in the operating system file manager.',
  'layer.objectLayers.revealInFinder': 'Shows the linked file in the operating system file manager.',
  'layer.objectLayers.relinkToFile': 'Links an object layer to a local file.',
  'layer.videoLayers.newVideoLayerFromFile': 'Makes a video layer from a local video file.',
  'edit.presets.migratePresets': 'Reads presets from a local installation.',
  'plugin.install': 'Installs a plug-in from a local file.',
  'edit.preferences.historyLog': 'Writes a history log to a local file; the editor keeps no log.',
  // Engine commands the web engine refuses: they read or write local files.
  'file.scripts.imageProcessor': 'Converts files from a local folder into another.',
  'file.scripts.loadFilesIntoStack': 'Stacks files read from a local folder.',
  'file.scripts.statistics': 'Stacks files read from a local folder.',
  'file.export.artboardsToPdf': 'Writes a PDF to a local folder.',
  'file.print': 'Prints through the operating system.',
  'file.printOneCopy': 'Prints through the operating system.',
  'file.package': 'Copies the linked files into a local folder.',
  'layer.quickExportAsPng': 'Writes a PNG to a local folder; exporting through the host is not built yet.',
  'layer.exportAs': 'Writes a file to a local folder; exporting through the host is not built yet.',
  // The agent's export (kit.json `agent.export`): it answers the image's bytes and writes nothing.
  'file.exportImage': "Answers the image's bytes to an agent; the host exports files.",
  // Commands that open more documents in the engine: the editor shows one, which the host opened.
  'file.newFromClipboard': HOST_OPENS,
  'file.automate.cropAndStraightenPhotos': 'Opens a new document per photo it finds; the host opens documents.',
  'file.automate.photomerge': 'Merges files from local folders, or several open documents, into a new document.',
  'file.automate.mergeToHdrPro': 'Merges files from local folders, or several open documents, into a new document.',
  // The History panel's "Create new document from current state" button.
  'history.newDocument': 'Opens a history state as a new document in the engine; the host opens documents.',
}

const AUTOMATE = 'file.automate.'
/** File > Automate runs batches over local folders (or opens documents, listed above), except these two, which edit the open image. */
const AUTOMATE_KEPT = new Set(['file.automate.fitImage', 'file.automate.conditionalModeChange'])

/** Why a browser editor hides `command`, or null when it shows it. */
export function hiddenReason(command: string): string | null {
  if (HIDDEN[command]) return HIDDEN[command]
  if (command.startsWith(AUTOMATE) && !AUTOMATE_KEPT.has(command)) return 'Runs over files in local folders.'
  return null
}

/** Whether a catalog row is never shown on the web. */
export function isHiddenOnWeb(row: MenuRow) {
  return hiddenReason(row.command) != null
}

const PREFERENCES = 'The preferences pages are not built yet: the row would only add an empty history step.'
const CUT = 'Needs a window of its own, cut for the launch (plan 2026-11-24, section 1).'
const NO_GUIDES = 'The canvas does not draw guides yet (plan 2026-11-24, stage 7).'
const VARIABLES = 'Variables need a data-set editor, not built yet; without one the engine refuses the row.'
const MEASUREMENT = 'Measurement needs the Measurement Log panel, cut for the launch.'

/**
 * "…" rows a browser could run but the editor does not show yet, with why: a dialog-less run
 * would apply defaults or fail with the engine's English error, and nothing useful can be asked
 * for them yet (plan 2026-11-24, stage 1). Unlike `HIDDEN`, these rows stay `todo` in the parity
 * status: they come back when their dialog or panel is built.
 */
const NOT_BUILT: Readonly<Record<string, string>> = {
  ...Object.fromEntries(
    ['integrations', 'interface', 'workspace', 'tools', 'fileHandling', 'export', 'performance', 'scratchDisks', 'cursors', 'transparencyAndGamut', 'unitsAndRulers', 'guidesGridAndSlices', 'plugIns', 'type', 'enhancedControls', 'rawDefaults'].map((page) => [`edit.preferences.${page}`, PREFERENCES]),
  ),
  'file.export.exportPreferences': PREFERENCES,
  'edit.keyboardShortcuts': 'The keyboard shortcut editor is not built yet (Help › Keyboard Shortcuts lists them).',
  'edit.menus': 'The menu editor is not built yet; Customize hides and shows rows.',
  'edit.toolbar': 'The toolbar editor is not built yet; Customize hides and shows tools.',
  'edit.presets.presetManager': 'The preset manager is not built yet.',
  'edit.presets.exportImportPresets': 'The preset manager is not built yet.',
  'file.export.saveForWebLegacy': 'Writes files to a local folder; exporting through the host is not built yet.',
  'file.scripts.scriptEventsManager': 'Scripts run from local files, which a browser editor cannot.',
  'image.variables.define': VARIABLES,
  'image.variables.dataSets': VARIABLES,
  'image.applyDataSet': VARIABLES,
  'image.analysis.setMeasurementScale': MEASUREMENT,
  'image.analysis.selectDataPoints': MEASUREMENT,
  'image.analysis.placeScaleMarker': MEASUREMENT,
  'filter.filterGallery': CUT,
  'filter.vanishingPoint': CUT,
  'filter.liquify': CUT,
  'filter.rawDevelop': CUT,
  'filter.distort.displace': 'Needs a displacement map file, which the editor cannot pick yet; the engine refuses it without one.',
  'view.newGuide': NO_GUIDES,
  'view.newGuideLayout': NO_GUIDES,
  // "…" rows whose values the generic dialog cannot ask (a file, a JSON map): without a dialog of their own they would run bare.
  'layer.objectLayers.replaceContents': 'Needs a file picker for the new contents, not built yet.',
  'layer.objectLayers.exportContents': 'Writes the contents to a local file; exporting through the host is not built yet.',
  'layer.objectLayers.convertToLinked': 'Links the contents to a local file, which a browser editor cannot.',
  'layer.videoLayers.replaceFootage': 'Video layers need the Timeline, cut for the launch.',
  'layer.videoLayers.interpretFootage': 'Video layers need the Timeline, cut for the launch.',
  'text.resolveMissingFonts': 'Needs a font-mapping dialog, not built yet.',
  'layer.layerContentOptions': "Needs the fill layer's own dialog, not built yet; Properties edits a fill layer meanwhile.",
  'image.trap': 'Needs a trap-width dialog, not built yet.',
  'layer.new.artboardFromGroup': 'Needs the artboard dialog, not built yet.',
}

/**
 * Upstream rows a Kition row of its own stands for, with why: Edit › Free Transform is Kition's
 * action at the top of the Edit menu (the engine's `edit.transform` takes the box the canvas draws,
 * plan 2026-11-24, stage 2), so upstream's row, which the engine lacks, is not drawn twice.
 */
const KIT_OWN: Readonly<Record<string, string>> = {
  'edit.freeTransform': "Kition's own Edit › Free Transform row (Cmd+T) opens the box on the canvas.",
  // Plan 2026-11-24, stage 5: floating windows of Kition's own (the dock's groups are upstream's).
  'window.panel.brushSettings': "Kition's own Window › Brush Settings row (F5) opens the Brush Settings window.",
  'window.panel.brushes': "Kition's own Window › Brushes row opens the Brushes window.",
}

/** Why the menus leave out `command` for a Kition row of its own, or null. */
export function kitOwnReason(command: string): string | null {
  return KIT_OWN[command] ?? null
}

/** Why the menus leave out `command` until its dialog or panel is built, or null. */
export function notBuiltReason(command: string): string | null {
  return NOT_BUILT[command] ?? null
}

/** Whether the menus leave out `command`: never shown on the web, or not built yet. */
export function isHiddenCommand(command: string) {
  return hiddenReason(command) != null || notBuiltReason(command) != null || kitOwnReason(command) != null
}

/** Whether the menus leave out a catalog row (`catalogTabs`'s `hidden`): never shown on the web, or not built yet. */
export function isHiddenInMenus(row: MenuRow) {
  return isHiddenCommand(row.command)
}
