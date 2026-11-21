import { catalogMenuTree, CommandButton, commandIcon, RibbonButton, RibbonGroup, RibbonStack, useKitEditor, useUi, type CatalogNode, type FrameTab, type MenuRow, type MenuTree } from '@kition/sdk/editor'

const rowId = (row: MenuRow) => row.command.startsWith('kit:') ? `control:${row.command.slice(4)}` : `command:${row.command}`
const rowsIn = (nodes: readonly CatalogNode[]): MenuRow[] => nodes.flatMap((node) => node.kind === 'row' ? [node.row] : node.kind === 'submenu' ? rowsIn(node.children) : [])

/** A catalog action in Photo's ribbon, with the same menu policies and dispatch as its menu row. */
function PhotoMenuAction({ row, tree, primary }: { row: MenuRow; tree: MenuTree; primary: boolean }) {
  const { byId, phrase, testId } = useKitEditor()
  const ui = useUi()
  const id = rowId(row)
  const state = ui.state(id, row.path)
  if (state.hidden || ui.state(id).hidden) return null
  const label = phrase(row.label)
  const pressed = tree.checked?.(row.command)
  const icon = commandIcon(byId.get(row.command) ?? { id: row.command.replace(/^kit:/, ''), title: row.label })
  if (row.command.startsWith('kit:')) {
    return <RibbonButton label={label} icon={icon} primary={primary} pressed={pressed} disabled={state.disabled} onClick={() => { if (!state.denied) tree.onKitCommand?.(row.command) }} data-testid={`${testId}-${id.replace(':', '-')}`} data-kit-ui-id={id} />
  }
  return <CommandButton command={row.command} label={label} icon={icon} primary={primary} pressed={pressed} disabled={state.disabled} />
}

/**
 * Common commands remain inline in Single line; every catalog row stays available in its group's
 * popover and in Classic. Use the SDK's public ribbon composition API: catalogTabs has no
 * menu-level primary option. Menus mode continues to use the tab's original catalog.
 */
export function PhotoMenuRibbon({ tab, tree, primary }: { tab: FrameTab; tree: MenuTree; primary: readonly string[] }) {
  const { phrase } = useKitEditor()
  const ui = useUi()
  const nodes = catalogMenuTree(tree.rows,
    (row) => !tree.hidden?.(row) && !ui.state(rowId(row), row.path).hidden && !ui.state(rowId(row)).hidden,
    (path) => ui.buttons(`menu:${path}`).length > 0 && !ui.state(`menu:${path}`).hidden,
  ).find((menu) => menu.label === tab.id)?.children ?? []
  const groups = [
    { path: tab.id, label: tab.label, rows: nodes.flatMap((node) => node.kind === 'row' ? [node.row] : []) },
    ...nodes.flatMap((node) => node.kind === 'submenu' ? [{ path: node.path, label: phrase(node.label), rows: rowsIn(node.children) }] : []),
  ]
  return <>{groups.filter((group) => group.rows.length || ui.buttons(`menu:${group.path}`).length).map((group) => (
    <RibbonGroup key={group.path} id={`menu:${group.path}`} label={group.label}>
      <RibbonStack>
        {group.rows.map((row, index) => <PhotoMenuAction key={`${row.command}@${index}`} row={row} tree={tree} primary={primary.includes(row.command)} />)}
      </RibbonStack>
    </RibbonGroup>
  ))}</>
}
