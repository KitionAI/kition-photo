import { useLayoutEffect, type KeyboardEvent, type RefObject } from 'react'
import { catalogTabs, shortcutMatches, useKitEditor, useUi, type FrameTab, type KitCommand, type MenuRow, type Phrases } from '@kition/sdk/editor'
import { PHOTO_MENUS } from './catalog'
import { PhotoMenuRibbon } from './PhotoMenuRibbon'
import { isHiddenOnWeb } from './hidden'
import { windowShown, type DockTabs, type WindowState } from './kitMenus'

/** A key handler the shell calls before its own: true when it took the key. */
export type MenuKeys = (event: KeyboardEvent<HTMLDivElement>) => boolean

const KIT = 'kit:'
/** Menus kept in a read-only document. */
const VIEW_MENUS = ['View', 'Window', 'Help']

/** Catalog rows whose shortcut the shell does not bind: Kition's own rows, and engine rows the engine gives another key or none (rows the engine lacks have no key). */
const unbound = (row: MenuRow, engine: ReadonlyMap<string, KitCommand>) =>
  Boolean(row.shortcut) && row.command !== '---' && !isHiddenOnWeb(row) && (row.command.startsWith(KIT) || (engine.has(row.command) && engine.get(row.command)!.shortcut !== row.shortcut))

/** The rows each menu tab shows in Single line's row (Photoshop's most-used), by menu; `kit:` rows keep their prefix. */
const PHOTO_PRIMARY: Record<string, readonly string[]> = {
  Edit: ['edit.undo', 'edit.redo', 'edit.fill', 'edit.stroke', 'edit.transform.again', 'edit.transform.flipHorizontal'],
  Image: ['image.imageSize', 'image.canvasSize', 'image.crop', 'image.trim', 'image.autoTone', 'image.autoContrast', 'image.autoColor', 'image.adjustments.levels', 'image.adjustments.curves', 'image.adjustments.hueSaturation'],
  Layer: ['layer.new.layer', 'layer.new.layerViaCopy', 'layer.duplicate', 'layer.delete', 'layer.mergeDown', 'layer.mergeVisible', 'layer.flattenImage'],
  Select: ['select.all', 'select.deselect', 'select.inverse', 'select.colorRange', 'select.subject', 'select.modify.feather', 'select.transformSelection'],
  Filter: ['filter.lastFilter', 'filter.blur.gaussianBlur', 'filter.sharpen.unsharpMask', 'filter.noise.addNoise', 'filter.noise.reduceNoise', 'filter.liquify'],
  View: ['kit:view.zoomIn', 'kit:view.zoomOut', 'kit:view.fitOnScreen', 'kit:view.actualPixels'],
}

/**
 * Photo's menus as the editor frame's tabs: upstream's catalog without the rows a browser cannot
 * run, and Kition's own rows (Window and Type > Panels rows, checked while their panel or bar
 * shows, with `dock` saying which tab each group shows; View's zoom rows; Help).
 */
export function photoMenuTabs({ windows, dock, onKitCommand, phrase }: { windows: WindowState; dock: DockTabs; onKitCommand: (command: string) => void; phrase: Phrases }): FrameTab[] {
  return catalogTabs(PHOTO_MENUS, { phrase, hidden: isHiddenOnWeb, onKitCommand, checked: (command) => windowShown(windows, command, dock), view: VIEW_MENUS }).map((tab) =>
    PHOTO_PRIMARY[tab.id] && tab.menu ? { ...tab, ribbon: <PhotoMenuRibbon tab={tab} tree={tab.menu} primary={PHOTO_PRIMARY[tab.id]} /> } : tab,
  )
}

/**
 * Answers the catalog shortcuts the shell does not bind (F7 Layers, Cmd+Alt+I Image Size) through
 * `keys`, refusing what the host denies. The keys of commands hidden on the web (Cmd+Alt+W Close
 * All) are the shell's to skip (`hiddenCommands`), so a host button can still take them. It draws
 * nothing; it sits inside the editor provider for the editor's state.
 */
export function PhotoMenuKeys({ onKitCommand, keys }: { onKitCommand: (command: string) => void; keys: RefObject<MenuKeys | null> }) {
  const { byId, enabled, run } = useKitEditor()
  const ui = useUi()
  useLayoutEffect(() => {
    keys.current = (event) => {
      const row = PHOTO_MENUS.find((item) => unbound(item, byId) && shortcutMatches(item.shortcut!, event, undefined, true))
      if (!row) return false
      if (row.command.startsWith(KIT)) {
        if (ui.state(`control:${row.command.slice(KIT.length)}`, row.path).denied) return false
        onKitCommand(row.command)
        return true
      }
      if (!enabled(row.command) || ui.state(`command:${row.command}`, row.path).denied) return false
      void run(row.command)
      return true
    }
  })
  return null
}
