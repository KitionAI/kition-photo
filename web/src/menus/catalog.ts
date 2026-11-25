// Imports carry `.ts` and the JSON its import attribute so `node --test` loads these modules as they are.
import type { MenuRow } from '@kition/sdk/editor'
import upstream from '../../parity/photo-upstream.json' with { type: 'json' }
import upstreamIds from '../../parity/photo-upstream-ids.json' with { type: 'json' }
import { withKitRows } from './kitMenus.ts'

/**
 * Upstream submenus the engine names differently, with the engine's name. Kit UI enforces a
 * command's hides under the engine's `menu` path, so the menus are drawn under it too.
 */
const ENGINE_SUBMENUS: ReadonlyArray<{ upstream: readonly string[]; engine: readonly string[] }> = [{ upstream: ['Filter', 'Other…'], engine: ['Filter', 'Other'] }]

/**
 * Rows upstream lists a second time away from the engine's menu, by command, with the second
 * copy's path. The copy stays where upstream draws it; its Kit UI id is the same command's.
 */
export const SECOND_COPIES: Readonly<Record<string, string>> = { 'edit.keyboardShortcuts': 'Window/Workspace' }

/**
 * The engine's id of a command upstream's manifest names (`parity/photo-upstream-ids.json`:
 * upstream id → the engine's id under the command naming standard); most ids are the same.
 */
export const engineId = (command: string): string => (upstreamIds as Readonly<Record<string, string>>)[command] ?? command

/** `rows` with upstream's command ids replaced by the engine's. */
export const engineIds = (rows: readonly MenuRow[]): MenuRow[] => rows.map((row) => ({ ...row, command: engineId(row.command) }))

const startsWith = (path: readonly string[], prefix: readonly string[]) => prefix.every((part, index) => path[index] === part)

/** `rows` with upstream's submenu names replaced by the engine's. */
export function enginePaths(rows: readonly MenuRow[]): MenuRow[] {
  return rows.map((row) => {
    const rename = ENGINE_SUBMENUS.find((item) => startsWith(row.path, item.upstream))
    return rename ? { ...row, path: [...rename.engine, ...row.path.slice(rename.upstream.length)] } : row
  })
}

/**
 * Photo's menus: what upstream draws, in upstream's order (the parity manifest: its catalog, and
 * the engine's commands the catalog lacks where upstream places them), under the engine's menu
 * paths and with the engine's command ids, with Kition's Window toggles and Help menu. Rows the
 * engine lacks are drawn disabled.
 */
export const PHOTO_MENUS: readonly MenuRow[] = withKitRows(engineIds(enginePaths(upstream.menus)))

/** Kition's own rows in `PHOTO_MENUS`: the Window toggles and Help. */
export const KIT_ROWS: readonly MenuRow[] = PHOTO_MENUS.filter((row) => row.command.startsWith('kit:'))
