import { useEffect, useState } from 'react'
import { TesseraIcons } from '@kition/kit-editor'
import { ContextMenu, IconButton, KitElement, MenuItem, MenuSeparator, PanelFooter, RenameField, useKitEditor, useUi, type ContextMenuPoint } from '@kition/kit-editor'
import type { PhotoInfo } from '../photo'
import type { PathData } from '../tools/canvas'

const Circle = TesseraIcons.CircleIcon
const PaintBucket = TesseraIcons.PaintBucketIcon
const Plus = TesseraIcons.PlusIcon
const Spline = TesseraIcons.SplineIcon
const Square = TesseraIcons.SquareIcon
const SquareDashed = TesseraIcons.SquareDashedIcon
const Trash2 = TesseraIcons.TrashIcon

/**
 * Upstream's Paths panel (`vector_ui.rs` `paths_panel`, `path_rows`, `path_context_actions`): the
 * saved paths, the work path, and the active layer's shape path or vector mask, each with its
 * outline; a click selects one, the footer and each row's menu fill, stroke, load, make, and
 * delete paths. The rows come from the engine's `path.list` and `path.info`.
 */

/** A row: the name the commands take (`work`, `layer`, or a saved path's name) and what it shows. */
type PathRow = { key: string; name: string; kind: 'saved' | 'work' | 'layer'; path: PathData | null }

type PathList = { paths?: { name: string }[]; workPath?: unknown; layerPath?: { kind: string; name: string } | null }

/** The SVG outline of `path` on a `width` × `height` canvas. */
function outline(path: PathData) {
  return path.subpaths
    .filter((subpath) => subpath.knots.length)
    .map(({ knots, closed }) => {
      const segments = closed ? knots.length : knots.length - 1
      let d = `M${knots[0].anchor.join(' ')}`
      for (let index = 0; index < segments; index += 1) {
        const a = knots[index]
        const b = knots[(index + 1) % knots.length]
        d += `C${a.out.join(' ')} ${b.in.join(' ')} ${b.anchor.join(' ')}`
      }
      return closed ? `${d}Z` : d
    })
    .join('')
}

/** The first "Path N" no saved path has. */
function nextPathName(rows: readonly PathRow[]) {
  const saved = rows.filter((row) => row.kind === 'saved')
  let n = saved.length + 1
  while (saved.some((row) => row.name === `Path ${n}`)) n += 1
  return `Path ${n}`
}

export function PathsPanel({ info, foreground }: { info: PhotoInfo; foreground: string }) {
  const { run, query, enabled, phrase, session } = useKitEditor()
  const [rows, setRows] = useState<PathRow[]>([])
  const [selected, setSelected] = useState('work')
  const [renaming, setRenaming] = useState<{ key: string; text: string } | null>(null)
  const listDisabled = useUi().state('control:paths.list').disabled
  const [menu, setMenu] = useState<{ at: ContextMenuPoint; row: PathRow } | null>(null)
  const revision = session.change.revision
  const label = (english: string) => phrase(english)

  // The rows after every change: the list, then each path's outline (a failed read draws none).
  useEffect(() => {
    let current = true
    void (async () => {
      const list = await query<PathList>('path.list')
      if (!list) {
        if (current) setRows([])
        return
      }
      const named: Omit<PathRow, 'path'>[] = [
        ...(list.paths ?? []).map((path) => ({ key: path.name, name: path.name, kind: 'saved' as const })),
        ...(list.workPath ? [{ key: 'work', name: 'Work Path', kind: 'work' as const }] : []),
        ...(list.layerPath ? [{ key: 'layer', name: list.layerPath.name, kind: 'layer' as const }] : []),
      ]
      const withPaths = await Promise.all(named.map(async (row) => ({ ...row, path: (await query<{ path: PathData }>('path.info', { name: row.key }))?.path ?? null })))
      if (current) setRows(withPaths)
    })()
    return () => {
      current = false
    }
  }, [query, revision, info.activeLayer])

  const layerRow = rows.find((row) => row.kind === 'layer')
  const chosen = selected === 'layer' && !layerRow ? 'work' : selected
  const canPaint = (() => {
    const kind = info.layers.find((layer) => layer.id === info.activeLayer)?.kind
    return kind === 'Pixel'
  })()
  // A layer's own path: deleting it deletes the vector mask (a shape keeps its path).
  const deletesVectorMask = chosen === 'layer' && layerRow != null && !layerRow.name.endsWith(' Shape Path')

  const finishRename = (commit: boolean) => {
    const current = renaming
    setRenaming(null)
    const to = current?.text.trim()
    if (commit && current && to && to !== current.key) {
      void run('path.rename', { name: current.key, to })
      if (selected === current.key) setSelected(to)
    }
  }

  /** Upstream's row menu, acting on that row whatever is selected. */
  const menuItems = (row: PathRow) => {
    const item = (english: string, command: string, params: Record<string, unknown>, disabled = false) => (
      <MenuItem key={`${command}:${english}`} label={label(english)} disabled={disabled || !enabled(command)} onSelect={() => void run(command, params)} uiId="control:paths.menu" testId={`photo-paths-menu-${command}`} />
    )
    const items = [item('Make Selection', 'path.toSelection', { name: row.key }), item('Fill Path', 'path.fill', { name: row.key }, !canPaint), item('Stroke Path', 'path.stroke', { name: row.key, tool: 'brush' }, !canPaint)]
    if (row.kind === 'work') items.push(item('Save Path', 'path.rename', { name: 'work', to: nextPathName(rows) }))
    else {
      let n = 1
      while (rows.some((other) => other.name === `${row.name} copy ${n}`)) n += 1
      items.push(item('Duplicate Path', 'path.set', { name: `${row.name} copy ${n}`, path: row.path ?? { subpaths: [] } }, !row.path))
    }
    if (row.kind !== 'layer') items.push(item('Delete Path', 'path.delete', { name: row.key }))
    else if (row.name.endsWith(' Vector Mask')) items.push(item('Delete Vector Mask', 'layer.vectorMask.delete', {}))
    if (row.kind === 'saved') {
      items.push(<MenuSeparator key="separator" />, <MenuItem key="rename" label={label('Rename Path')} onSelect={() => setRenaming({ key: row.key, text: row.name })} uiId="control:paths.menu" testId="photo-paths-menu-rename" />)
    }
    return items
  }

  return (
    <div className="photo-paths" data-testid="photo-paths">
      <KitElement id="control:paths.list">
        <ul className="photo-layers-rows" aria-label={label('Paths')} data-disabled={listDisabled}>
          {rows.length === 0 ? <li className="kui-muted photo-empty">{label('Draw with the Pen tool (P) or make a work path from a selection.')}</li> : null}
          {rows.map((row) => (
            <li
              key={`${row.kind}:${row.key}`}
              className="photo-layer photo-path"
              data-selected={chosen === row.key}
              onClick={() => {
                if (!listDisabled) setSelected(row.key)
              }}
              onDoubleClick={() => {
                if (listDisabled) return
                // The work path saves as a new path; a saved path renames in place.
                if (row.kind === 'work') void run('path.rename', { name: 'work', to: nextPathName(rows) })
                else if (row.kind === 'saved') setRenaming({ key: row.key, text: row.name })
              }}
              onContextMenu={(event) => {
                event.preventDefault()
                if (!listDisabled) setMenu({ at: { x: event.clientX, y: event.clientY }, row })
              }}
              data-testid={`photo-path-${row.kind === 'saved' ? `saved-${row.key}` : row.kind}`}
            >
              <span className="photo-layer-thumb photo-path-thumb">
                <svg viewBox={`0 0 ${Math.max(1, info.width)} ${Math.max(1, info.height)}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
                  <rect width={info.width} height={info.height} className="photo-path-canvas" />
                  {row.path ? <path d={outline(row.path)} className="photo-path-shape" /> : null}
                </svg>
              </span>
              {renaming?.key === row.key && row.kind === 'saved' ? (
                <RenameField value={renaming.text} label={label('Rename Path')} onChange={(text) => setRenaming({ key: row.key, text })} onDone={finishRename} testId="photo-path-rename" />
              ) : (
                <button type="button" className="photo-layer-name" data-italic={row.kind !== 'saved'} aria-pressed={chosen === row.key}>
                  {row.kind === 'work' ? label(row.name) : row.name}
                </button>
              )}
            </li>
          ))}
        </ul>
      </KitElement>
      <PanelFooter
        start={
          <>
            <KitElement id="control:paths.fill">
              <IconButton icon={PaintBucket} label={label('Fill path with foreground color')} disabled={!canPaint || !rows.length} onClick={() => void run('path.fill', { name: chosen, color: foreground })} data-testid="photo-paths-fill" />
            </KitElement>
            <KitElement id="control:paths.stroke">
              <IconButton icon={Circle} label={label('Stroke path with brush')} disabled={!canPaint || !rows.length} onClick={() => void run('path.stroke', { name: chosen, tool: 'brush' })} data-testid="photo-paths-stroke" />
            </KitElement>
            <KitElement id="control:paths.load">
              <IconButton icon={SquareDashed} label={label('Load path as a selection')} disabled={!rows.length} onClick={() => void run('path.toSelection', { name: chosen })} data-testid="photo-paths-load" />
            </KitElement>
            <KitElement id="control:paths.workPath">
              <IconButton icon={Spline} label={label('Make work path from selection')} disabled={!info.hasSelection} onClick={() => void run('select.toWorkPath', { tolerance: 2 })} data-testid="photo-paths-work" />
            </KitElement>
            <KitElement id="control:paths.vectorMask">
              <IconButton icon={Square} label={label('Add vector mask')} disabled={!rows.length || !enabled('layer.vectorMask.fromPath')} onClick={() => void run('layer.vectorMask.fromPath', { name: chosen })} data-testid="photo-paths-mask" />
            </KitElement>
          </>
        }
        end={
          <>
            <KitElement id="control:paths.new">
              <IconButton
                icon={Plus}
                label={label('Create new path')}
                onClick={() => {
                  const name = nextPathName(rows)
                  setSelected(name)
                  void run('path.set', { name, path: { subpaths: [] } })
                }}
                data-testid="photo-paths-new"
              />
            </KitElement>
            <KitElement id="control:paths.delete">
              <IconButton
                icon={Trash2}
                label={label(deletesVectorMask ? 'Delete Vector Mask' : 'Delete path')}
                disabled={!rows.some((row) => row.key === chosen) || (chosen === 'layer' && !deletesVectorMask)}
                onClick={() => void (deletesVectorMask ? run('layer.vectorMask.delete', {}) : run('path.delete', { name: chosen }))}
                data-testid="photo-paths-delete"
              />
            </KitElement>
          </>
        }
      />
      <KitElement id="control:paths.menu">
        <ContextMenu at={menu?.at ?? null} onClose={() => setMenu(null)} label={label('Paths')} testId="photo-paths-menu">
          {menu ? menuItems(menu.row) : null}
        </ContextMenu>
      </KitElement>
    </div>
  )
}
