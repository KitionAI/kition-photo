/**
 * View's rulers, extras and snapping (plan 2026-11-24, stage 7): what View › Rulers, View › Show,
 * View › Snap and View › Snap To set, kept for the person across sessions (the kit store), and what
 * each of those menu rows does and shows as its check mark. Pure, so `guides.test.mjs` tests it.
 */
export type SnapTo = { guides: boolean; grid: boolean; layers: boolean; documentBounds: boolean }

export type ViewSettings = {
  rulers: boolean
  /** View › Extras (Cmd+H): every item of View › Show at once. */
  extras: boolean
  guides: boolean
  grid: boolean
  pixelGrid: boolean
  smartGuides: boolean
  selectionEdges: boolean
  targetPath: boolean
  layerEdges: boolean
  snap: boolean
  snapTo: SnapTo
  lockGuides: boolean
}

/**
 * Photoshop's defaults, except that Snap To Layers and Document Bounds and Smart Guides start off:
 * on, every existing drag near a layer's edge would land somewhere else than it was dragged to,
 * which the editor's gesture cases pin. A person turns them on in View › Snap To and View › Show.
 */
export const DEFAULT_VIEW: ViewSettings = {
  rulers: false,
  extras: true,
  guides: true,
  grid: false,
  pixelGrid: true,
  smartGuides: false,
  selectionEdges: true,
  targetPath: true,
  layerEdges: false,
  snap: true,
  snapTo: { guides: true, grid: true, layers: false, documentBounds: false },
  lockGuides: false,
}

/** A stored value as settings: each known field of the right kind, the default for the rest. */
export function validView(raw: unknown): ViewSettings | undefined {
  if (!raw || typeof raw !== 'object') return undefined
  const value = raw as Record<string, unknown>
  const flag = (key: keyof ViewSettings) => (typeof value[key] === 'boolean' ? (value[key] as boolean) : (DEFAULT_VIEW[key] as boolean))
  const snapTo = (value.snapTo && typeof value.snapTo === 'object' ? value.snapTo : {}) as Record<string, unknown>
  const to = (key: keyof SnapTo) => (typeof snapTo[key] === 'boolean' ? (snapTo[key] as boolean) : DEFAULT_VIEW.snapTo[key])
  return {
    rulers: flag('rulers'),
    extras: flag('extras'),
    guides: flag('guides'),
    grid: flag('grid'),
    pixelGrid: flag('pixelGrid'),
    smartGuides: flag('smartGuides'),
    selectionEdges: flag('selectionEdges'),
    targetPath: flag('targetPath'),
    layerEdges: flag('layerEdges'),
    snap: flag('snap'),
    snapTo: { guides: to('guides'), grid: to('grid'), layers: to('layers'), documentBounds: to('documentBounds') },
    lockGuides: flag('lockGuides'),
  }
}

/** The View › Show items, by their row's command. */
const SHOW: Readonly<Record<string, keyof ViewSettings>> = {
  'view.show.layerEdges': 'layerEdges',
  'view.show.selectionEdges': 'selectionEdges',
  'view.show.targetPath': 'targetPath',
  'view.show.grid': 'grid',
  'view.show.guides': 'guides',
  'view.show.smartGuides': 'smartGuides',
  'view.show.pixelGrid': 'pixelGrid',
}
const SNAP_TO: Readonly<Record<string, keyof SnapTo>> = {
  'view.snapTo.guides': 'guides',
  'view.snapTo.grid': 'grid',
  'view.snapTo.layers': 'layers',
  'view.snapTo.documentBounds': 'documentBounds',
}

/** View rows the editor itself answers (the engine has no command for them). */
export const VIEW_SETTING_ROWS: ReadonlySet<string> = new Set(['view.extras', ...Object.keys(SHOW), 'view.show.all', 'view.rulers', 'view.snap', ...Object.keys(SNAP_TO), 'view.snapTo.all', 'view.lockGuides'])

/** The settings after a View row is chosen (unchanged for any other command). */
export function viewRow(settings: ViewSettings, command: string): ViewSettings {
  if (command === 'view.rulers') return { ...settings, rulers: !settings.rulers }
  if (command === 'view.extras') return { ...settings, extras: !settings.extras }
  if (command === 'view.snap') return { ...settings, snap: !settings.snap }
  if (command === 'view.lockGuides') return { ...settings, lockGuides: !settings.lockGuides }
  // Show › All: every item shown, and the extras with them.
  if (command === 'view.show.all') return { ...settings, extras: true, ...Object.fromEntries(Object.values(SHOW).map((key) => [key, true])) }
  if (command === 'view.snapTo.all') return { ...settings, snap: true, snapTo: { guides: true, grid: true, layers: true, documentBounds: true } }
  const show = SHOW[command]
  // Showing one item while the extras are hidden shows the extras again (Photoshop's way).
  if (show) return settings[show] && settings.extras ? { ...settings, [show]: false } : { ...settings, [show]: true, extras: true }
  const to = SNAP_TO[command]
  if (to) return { ...settings, snapTo: { ...settings.snapTo, [to]: !settings.snapTo[to] } }
  return settings
}

/** A View row's check mark, or undefined for a row that has none. */
export function viewChecked(settings: ViewSettings, command: string): boolean | undefined {
  if (command === 'view.rulers') return settings.rulers
  if (command === 'view.extras') return settings.extras
  if (command === 'view.snap') return settings.snap
  if (command === 'view.lockGuides') return settings.lockGuides
  const show = SHOW[command]
  if (show) return settings.extras && Boolean(settings[show])
  const to = SNAP_TO[command]
  if (to) return settings.snapTo[to]
  return undefined
}

/** Whether a View › Show item is on the canvas now (its own setting and the extras). */
export function shown(settings: ViewSettings, item: 'guides' | 'grid' | 'pixelGrid' | 'smartGuides' | 'selectionEdges' | 'targetPath' | 'layerEdges') {
  return settings.extras && settings[item]
}

/** Where the grid has lines: every inch of the document's resolution, in four parts (Photoshop's default of 25 mm and 4). */
export function gridSpacing(resolution: number): { major: number; minor: number } {
  const major = Math.max(8, resolution > 0 ? resolution : 72)
  return { major, minor: major / 4 }
}
