import { useLayoutEffect, useMemo, type KeyboardEvent, type MouseEvent, type ReactNode, type RefObject } from 'react'
import { ToolRail, useEngineQuery, useKitEditor, useKitStore, useUi } from '@kition/sdk/editor'
import { RAIL_TOOLS, isToolKey, toolFor, toolForKey, type PhotoToolId } from './catalog'

/** A key handler the editor calls before the shell's own: true when it took the key. */
export type ToolKeys = (event: KeyboardEvent<HTMLDivElement>) => boolean

/** Upstream's Preferences › Tools › Use Shift Key for Tool Switch, in the engine's preferences. */
const SHIFT_SWITCH = 'tools.useShiftKeyForToolSwitch'

/** Upstream's long press that opens a slot's flyout (`panels.rs`: 0.35 s). */
const FLYOUT_PRESS_MS = 350

/** A stored column count: one or two. */
const columnCount = (raw: unknown) => (raw === 1 || raw === 2 ? raw : undefined)

/**
 * Photo's tools: the rail (upstream's sections, in two columns until the person chooses one with
 * the chevrons at its head, a choice every Photo tab shares through the kit store), with
 * `children` under it, while `shown`; and, shown or not, the tools' letter keys through `keys`,
 * cycling as the engine's Use Shift Key for Tool Switch preference says. Every choice, from a slot,
 * a flyout, a key, or Shift and a key, goes through `choose`, which refuses a tool the host denies.
 */
export function PhotoToolRail({ active, onChoose, shown, keys, onView, children }: { active: PhotoToolId; onChoose: (id: PhotoToolId) => void; shown: boolean; keys: RefObject<ToolKeys | null>; onView?: (fit: boolean) => void; children?: ReactNode }) {
  const { phrase } = useKitEditor()
  const ui = useUi()
  const [columns, setColumns] = useKitStore<1 | 2>('kit.photo', 'railColumns', { initial: 2, validate: columnCount })
  const tools = useMemo(() => RAIL_TOOLS.map((tool) => ({ id: tool.id, label: phrase(tool.label), icon: tool.icon, shortcut: tool.key, group: tool.group, section: tool.section })), [phrase])
  // Read again after every change (a preferences command is one): a single small query.
  const shiftSwitch = useEngineQuery<boolean>('prefs.get', { path: SHIFT_SWITCH }) === true
  const allowed = (id: string) => !ui.state(`tool:${id}`).denied
  const choose = (id: string) => {
    const tool = toolFor(id)
    if (tool && allowed(id)) onChoose(tool.id)
  }
  useLayoutEffect(() => {
    keys.current = (event) => {
      const key = event.key.toLowerCase()
      if (event.metaKey || event.ctrlKey || event.altKey || !isToolKey(key)) return false
      const next = toolForKey(key, event.shiftKey, shiftSwitch, active, (id) => allowed(id) && !ui.state(`tool:${id}`).hidden)
      // A tool's letter is the tool's even when it chooses nothing (a slot the host denies).
      if (next) choose(next)
      return true
    }
  })
  if (!shown) return null
  // Double-clicking the Hand tool fits the image on screen; the Zoom tool shows it at 100 % (upstream's `toolbar.rs`).
  const doubleClick = (event: MouseEvent<HTMLDivElement>) => {
    if (event.shiftKey || event.altKey || event.metaKey || event.ctrlKey || !(event.target instanceof Element)) return
    const id = event.target.closest('[data-testid^="photo-tool-"]')?.getAttribute('data-testid')?.slice('photo-tool-'.length)
    if (id === 'Hand') onView?.(true)
    else if (id === 'Zoom') onView?.(false)
  }
  return (
    <div className="photo-rail" onDoubleClick={doubleClick}>
      <ToolRail tools={tools} active={active} onSelect={choose} columns={columns} onColumns={(next) => void setColumns(next)} columnsUiId="control:toolRail.columns" flyoutPressMs={FLYOUT_PRESS_MS} />
      {children}
    </div>
  )
}
