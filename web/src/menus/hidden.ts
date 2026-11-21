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

/** Whether a catalog row is never shown on the web (`catalogTabs`'s `hidden`). */
export function isHiddenOnWeb(row: MenuRow) {
  return hiddenReason(row.command) != null
}
