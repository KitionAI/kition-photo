import type { MenuRow } from '@kition/sdk/editor'

/**
 * Kition's own menu rows (`kit:` commands, Kit UI ids `control:<name>`): the Window toggles, Type ›
 * Panels, View's zoom rows, Edit › Free Transform, and Help. Their names start with `window.`,
 * `type.panels.`, `view.`, `edit.`, or `help.`, so they never collide with Photo's declared controls (`control:options`,
 * `control:colors`, `control:status`).
 */

/** Upstream's six dock groups, in its order; each group is named after its first tab. */
export const DOCK_GROUPS = ['color', 'properties', 'character', 'navigator', 'history', 'layers'] as const

export type DockGroupId = (typeof DOCK_GROUPS)[number]

/**
 * Each group's tabs that have a panel, in upstream's order (its first tab first): the tabs a
 * Window row shows. Gradients, Patterns, Actions, and Layer Comps come in wave 2.
 */
export const DOCK_TABS: Readonly<Record<DockGroupId, readonly string[]>> = {
  color: ['color', 'swatches'],
  properties: ['properties', 'adjustments'],
  character: ['character', 'paragraph'],
  navigator: ['navigator', 'histogram', 'info'],
  history: ['history'],
  layers: ['layers', 'channels', 'paths'],
}

/** Upstream's Type › Panels rows for tabs Photo has (they show their tab and never hide it). */
const TYPE_PANELS = ['character', 'paragraph'] as const

/** Upstream's View rows that move the canvas's view (`menus.rs`, `view_cmds.rs`). */
export const VIEW_ROWS = ['view.zoomIn', 'view.zoomOut', 'view.fitOnScreen', 'view.fitLayersOnScreen', 'view.actualPixels', 'view.twoHundredPercent', 'view.printSize'] as const

/**
 * Upstream's rows that Kition's own take over in place, keeping upstream's label and key (F7
 * Layers, Cmd+1 100%): upstream command → Kition command. Window › <panel> for every tab with a
 * panel, the toolbar, and the options bar; Type › Panels › Character and Paragraph; View's zoom rows;
 * Edit › Free Transform (Cmd+T).
 */
export const KIT_TAKEOVERS: Readonly<Record<string, string>> = {
  ...Object.fromEntries(Object.values(DOCK_TABS).flatMap((tabs) => tabs.map((tab) => [`window.panel.${tab}`, `kit:window.panel.${tab}`]))),
  'window.panel.options': 'kit:window.options',
  'window.panel.tools': 'kit:window.toolbar',
  ...Object.fromEntries(TYPE_PANELS.map((tab) => [`type.panels.${tab}`, `kit:type.panels.${tab}`])),
  ...Object.fromEntries(VIEW_ROWS.map((command) => [command, `kit:${command}`])),
  // Edit › Free Transform (Cmd+T) opens the box the canvas draws; the engine has no such command (plan 2026-11-24, stage 9).
  'edit.freeTransform': 'kit:edit.freeTransform',
}

/** The status bar's toggle, right after the toolbar's (upstream has no such row). */
export const STATUS_BAR_ROW: MenuRow = { path: ['Window'], label: 'Status Bar', shortcut: null, command: 'kit:window.status' }

/** Kition's Help menu (upstream has none); its labels are upstream phrases. */
export const HELP_ROWS: readonly MenuRow[] = [
  { path: ['Help'], label: 'Keyboard Shortcuts', shortcut: null, command: 'kit:help.shortcuts' },
  { path: ['Help'], label: '---', shortcut: null, command: '---' },
  { path: ['Help'], label: 'About Kition Photo', shortcut: null, command: 'kit:help.about' },
]

/** `rows` with Kition's own rows in place of upstream's (`KIT_TAKEOVERS`), the status bar's toggle, and Help at the end. */
export function withKitRows(rows: readonly MenuRow[]): MenuRow[] {
  const out = rows.map((row) => (KIT_TAKEOVERS[row.command] ? { ...row, command: KIT_TAKEOVERS[row.command] } : row))
  const toolbar = out.findIndex((row) => row.command === 'kit:window.toolbar')
  out.splice(toolbar < 0 ? out.length : toolbar + 1, 0, STATUS_BAR_ROW)
  return [...out, ...HELP_ROWS]
}

/** What the Window menu shows: each dock group, the toolbar, the options bar, and the status bar. */
export type WindowState = { panels: Readonly<Record<DockGroupId, boolean>>; toolbar: boolean; options: boolean; status: boolean }

/** Upstream's default workspace (`Panels::default`): Color, Properties, and Layers docked, every bar shown. */
export const DEFAULT_WINDOWS: WindowState = {
  panels: { color: true, properties: true, character: false, navigator: false, history: false, layers: true },
  toolbar: true,
  options: true,
  status: true,
}

const WINDOW_PANEL = 'kit:window.panel.'
const TYPE_PANEL = 'kit:type.panels.'
const BARS: Readonly<Record<string, 'toolbar' | 'options' | 'status'>> = { 'kit:window.toolbar': 'toolbar', 'kit:window.options': 'options', 'kit:window.status': 'status' }

/** A panel row's group and tab; `always` for Type › Panels rows, which show their tab and never hide it. */
function panelRow(command: string): { group: DockGroupId; tab: string; always: boolean } | null {
  const always = command.startsWith(TYPE_PANEL)
  const tab = always ? command.slice(TYPE_PANEL.length) : command.startsWith(WINDOW_PANEL) ? command.slice(WINDOW_PANEL.length) : ''
  const group = DOCK_GROUPS.find((candidate) => DOCK_TABS[candidate].includes(tab))
  return group ? { group, tab, always } : null
}

/** The tab a dock group shows and whether it is collapsed (`DockAreaHandle.activeTab`); null when the dock cannot say. */
export type DockTabs = (group: DockGroupId) => { tab: string; collapsed: boolean } | null

/** A group's tab: the dock's, else its first. */
const tabOf = (group: DockGroupId, dock?: DockTabs) => dock?.(group)?.tab ?? DOCK_TABS[group][0]

/**
 * Whether a Window or Type › Panels row's part is shown (its check mark), as upstream
 * (`view_cmds::checked`): a panel row while its group shows with its tab forward, a bar while it
 * shows; undefined for any other command.
 */
export function windowShown(state: WindowState, command: string, dock?: DockTabs): boolean | undefined {
  const panel = panelRow(command)
  if (panel) return state.panels[panel.group] && tabOf(panel.group, dock) === panel.tab
  const bar = BARS[command]
  return bar ? state[bar] : undefined
}

/**
 * What a Window or Type › Panels row does, as upstream (`view_cmds::run`): the window state after
 * it, and the tab to bring forward (expanding its group), or null. A Window panel row whose tab
 * shows, expanded, hides its group; otherwise it shows the group with its tab forward. A Type ›
 * Panels row always shows its tab. A bar row flips its bar; any other command changes nothing.
 */
export function windowCommand(state: WindowState, command: string, dock?: DockTabs): { state: WindowState; reveal: string | null } {
  const panel = panelRow(command)
  if (panel) {
    const showing = state.panels[panel.group] && tabOf(panel.group, dock) === panel.tab && !dock?.(panel.group)?.collapsed
    const shown = !showing || panel.always
    return { state: state.panels[panel.group] === shown ? state : { ...state, panels: { ...state.panels, [panel.group]: shown } }, reveal: shown ? panel.tab : null }
  }
  const bar = BARS[command]
  return { state: bar ? { ...state, [bar]: !state[bar] } : state, reveal: null }
}

/** Photoshop's zoom steps (upstream `canvas::zoom_step`): the next one in (`direction` 1) or out (-1) from `zoom`. */
export function zoomStep(zoom: number, direction: 1 | -1): number {
  const steps = [0.01, 0.02, 0.03, 0.05, 0.0667, 0.1, 0.125, 0.1667, 0.25, 0.333, 0.5, 0.6667, 1, 2, 3, 4, 5, 6, 8, 12, 16, 32]
  return direction > 0 ? (steps.find((step) => step > zoom * 1.001) ?? 32) : (steps.reverse().find((step) => step < zoom * 0.999) ?? 0.01)
}
