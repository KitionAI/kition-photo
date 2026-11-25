import { useLayoutEffect, type KeyboardEvent, type RefObject } from 'react'
import { catalogTabs, shortcutMatches, useKitEditor, useUi, type FrameTab, type KitCommand, type MenuAction, type MenuRow, type Phrases } from '@kition/sdk/editor'
import { PHOTO_MENUS } from './catalog'
import { PhotoMenuRibbon } from './PhotoMenuRibbon'
import { isHiddenCommand, isHiddenInMenus } from './hidden'
import type { Dialogs } from '../dialogs/DialogHost'
import { windowShown, type DockTabs, type WindowState } from './kitMenus'

/** A key handler the shell calls before its own: true when it took the key. */
export type MenuKeys = (event: KeyboardEvent<HTMLDivElement>) => boolean

const KIT = 'kit:'
/** Menus kept in a read-only document. */
const VIEW_MENUS = ['View', 'Window', 'Help']

/** Catalog rows whose shortcut the shell does not bind: Kition's own rows, and engine rows the engine gives another key or none (rows the engine lacks have no key). */
const unbound = (row: MenuRow, engine: ReadonlyMap<string, KitCommand>) =>
  Boolean(row.shortcut) && row.command !== '---' && !isHiddenInMenus(row) && (row.command.startsWith(KIT) || (engine.has(row.command) && engine.get(row.command)!.shortcut !== row.shortcut))

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
 * run or the editor has not built yet, and Kition's own rows (Window and Type › Panels rows,
 * checked while their panel or bar shows, with `dock` saying which tab each group shows; View's
 * zoom rows; Help). An engine row runs through `onCommand` (the dialog host's `invoke`, so a "…"
 * row asks first); `before` puts Kition's own actions at the top of a menu (File › New…).
 */
export function photoMenuTabs({ windows, dock, onKitCommand, onCommand, before, phrase }: { windows: WindowState; dock: DockTabs; onKitCommand: (command: string) => void; onCommand: (command: string) => void; before?: Readonly<Record<string, readonly MenuAction[]>>; phrase: Phrases }): FrameTab[] {
  return catalogTabs(PHOTO_MENUS, { phrase, hidden: isHiddenInMenus, onKitCommand, onCommand, before, checked: (command) => windowShown(windows, command, dock), view: VIEW_MENUS }).map((tab) =>
    PHOTO_PRIMARY[tab.id] && tab.menu ? { ...tab, ribbon: <PhotoMenuRibbon tab={tab} tree={tab.menu} primary={PHOTO_PRIMARY[tab.id]} onCommand={onCommand} /> } : tab,
  )
}

/**
 * Answers the catalog shortcuts the shell does not bind (F7 Layers, Cmd+Alt+I Image Size) through
 * `keys`, refusing what the host denies, and every shortcut of a command that asks first (Cmd+L
 * Levels, Shift+F5 Fill, Shift+F6 Feather): it opens the dialog instead of the shell running the
 * command with the engine's defaults. The keys of commands hidden on the web (Cmd+Alt+W Close
 * All) are the shell's to skip (`hiddenCommands`), so a host button can still take them. It draws
 * nothing; it sits inside the editor provider for the editor's state.
 */
export function PhotoMenuKeys({ onKitCommand, keys, dialogs }: { onKitCommand: (command: string) => void; keys: RefObject<MenuKeys | null>; dialogs: Dialogs }) {
  const { byId, commands, enabled, run } = useKitEditor()
  const ui = useUi()
  useLayoutEffect(() => {
    keys.current = (event) => {
      // A command that asks first takes its key whatever happens next: the shell must not run it bare.
      const asking =
        PHOTO_MENUS.find((item) => item.shortcut && !item.command.startsWith(KIT) && shortcutMatches(item.shortcut, event, undefined, true) && !isHiddenInMenus(item) && dialogs.asks(item.command))?.command ??
        commands.find((command) => command.shortcut && shortcutMatches(command.shortcut, event, undefined, true) && !isHiddenCommand(command.id) && dialogs.asks(command.id))?.id
      if (asking) {
        dialogs.open(asking)
        return true
      }
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
