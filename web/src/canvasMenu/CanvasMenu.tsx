import { ContextMenu, MenuItem, MenuSeparator, useKitEditor, type ContextMenuPoint, type Phrases } from '@kition/sdk/editor'
import { rowCommand, type CanvasMenuRow } from './rows'

/**
 * The canvas's right-click menu (plan 2026-11-24, stage 7) at `at`: `rows` as `canvasMenuRows`
 * gave them, each label in the editor's language, a command the engine refuses now greyed. A
 * choice goes to `onChoose`; a press outside, Escape or a choice closes it.
 */
export function CanvasMenu({ at, rows, phrase, onChoose, onClose }: { at: ContextMenuPoint | null; rows: readonly CanvasMenuRow[]; phrase: Phrases; onChoose: (row: Extract<CanvasMenuRow, { kind: 'item' }>) => void; onClose: () => void }) {
  const { enabled } = useKitEditor()
  return (
    <ContextMenu at={at} onClose={onClose} label={phrase('Context Menu')} testId="photo-canvas-menu">
      {rows.map((row, index) => {
        if (row.kind === 'separator') return <MenuSeparator key={`sep-${index}`} />
        if (row.kind === 'heading') return <MenuItem key={`heading-${index}`} label={phrase(row.label)} disabled onSelect={() => undefined} testId="photo-canvas-menu-heading" />
        const command = rowCommand(row)
        // Upstream's phrases (its catalogs translate them); a layer's name as it is.
        const label = 'layer' in row.action ? row.label : phrase(row.label)
        const action = row.action
        const id = 'command' in action ? action.command : 'layer' in action ? `layer-${action.layer}` : `${action.own}${action.value ? `-${action.value}` : ''}`
        return (
          <MenuItem
            key={`${id}-${index}`}
            label={label}
            checked={row.checked}
            disabled={row.disabled || (command != null && !enabled(command))}
            onSelect={() => onChoose(row)}
            testId={`photo-canvas-menu-${id.replace(/[:.]/g, '-')}`}
          />
        )
      })}
    </ContextMenu>
  )
}
