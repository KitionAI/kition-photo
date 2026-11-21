import { defineKitUi } from '@kition/sdk/editor'
import { PHOTO_TOOLS } from './tools/catalog'
import { PHOTO_MENUS } from './menus/catalog'
import { isHiddenOnWeb } from './menus/hidden'
import { OPTION_ELEMENTS } from './options/controls'
import { DOCK_PANELS, PANEL_CONTROLS } from './panels/dock'

/**
 * Every element the Photo editor draws (Kit UI protocol section 9): its menu catalog (the rows the
 * menu bar can draw, with Kition's own rows) and its own tools, panels (each dock tab,
 * `panel:<tab>`; a group's strip carries its active tab's id), and controls (the rail's column toggle, colours, and Quick Mask button,
 * the options bar and each of its fields, `control:options.<field>`, the status bar and its zoom
 * field, `control:status.zoom`, and the panels' own, `control:<panel>.<name>`).
 * Ids are public: a host hides or disables them by these names.
 */
export const PHOTO_UI = defineKitUi({
  tabs: [{ id: 'tab:home', label: 'homeTab' }],
  tools: PHOTO_TOOLS.map((tool) => ({ id: `tool:${tool.id}`, label: tool.label })),
  panels: DOCK_PANELS,
  controls: [
    { id: 'control:colors', label: 'colors' },
    { id: 'control:quickMask', label: 'Edit in Quick Mask Mode' },
    { id: 'control:toolRail.columns', label: 'railTwoColumns' },
    { id: 'control:options', label: 'toolOptions' },
    { id: 'control:status', label: 'Status Bar' },
    { id: 'control:status.zoom', label: 'Zoom', parent: 'control:status' },
    ...OPTION_ELEMENTS,
    ...PANEL_CONTROLS,
  ],
  menus: PHOTO_MENUS.filter((row) => !isHiddenOnWeb(row)),
})
