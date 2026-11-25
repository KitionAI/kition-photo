import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent as ReactMouseEvent } from 'react'
import { TesseraIcons } from '@kition/sdk/editor'
import { Choice, ContextMenu, IconButton, KitElement, MenuButton, MenuItem, MenuSeparator, NumberField, PanelFooter, deviceScale, useKitEditor, useLeases, useReadOnly, useUi, type ContextMenuPoint } from '@kition/sdk/editor'
import type { PhotoInfo, PhotoLayer, PhotoLocks } from '../photo'
import { BLEND_MODES } from '../tools/options'
import { hiddenReason } from '../menus/hidden'
import { ICON_KINDS, LayerRow, THUMB, kindLine, partOf } from './LayerRow'
import { useLayerDrag } from './useLayerDrag'
import { useThumbnails, type ThumbnailSource } from './thumbnails'
import { layersLeasedByOthers, useLayerObjectIds } from '../layerLeases'
import { gestureKey } from './gesture'
import { useDialogs } from '../dialogs/DialogHost'
import { useTypeEditing } from '../type/context'


const AppWindow = TesseraIcons.AppWindowIcon
const Brush = TesseraIcons.BrushIcon
const Contrast = TesseraIcons.ContrastIcon
const Folder = TesseraIcons.FolderIcon
const Grid3x3 = TesseraIcons.Grid3x3Icon
const Image = TesseraIcons.ImageIcon
const Link = TesseraIcons.LinkIcon
const Lock = TesseraIcons.LockIcon
const Move = TesseraIcons.MoveIcon
const Scan = TesseraIcons.ScanIcon
const Search = TesseraIcons.SearchIcon
const Square = TesseraIcons.SquareIcon
const SquareDot = TesseraIcons.SquareDotIcon
const SquarePlus = TesseraIcons.SquarePlusIcon
const Trash2 = TesseraIcons.TrashIcon
const Type = TesseraIcons.TypeIcon

/**
 * Upstream's Layers panel (`panels.rs` `layers`, `layer_row`, `effect_rows`, the footer;
 * `layer_tree_ui.rs`, `layer_row_ui.rs`, `layer_menu_ui.rs`): the kind filter and a name search,
 * the active layer's blend mode, Opacity, locks, and Fill, the rows (top of the stack first, groups
 * folding, effects listed under their layer), and the footer. Every action is an engine command.
 */

/** Upstream's kind filters: the key, the engine kinds it keeps, the icon, and upstream's tip. */
const KIND_FILTERS = [
  ['pixel', ['Pixel'], Image, 'Filter for pixel layers'],
  ['adjustment', ['Adjustment', 'Fill'], Contrast, 'Filter for adjustment layers'],
  ['type', ['Type'], Type, 'Filter for type layers'],
  ['shape', ['Shape'], Square, 'Filter for shape layers'],
  ['smart', ['Object'], AppWindow, 'Filter for object layers'],
] as const

/** Upstream's lock buttons (Photoshop's order). */
const LOCKS = [
  ['transparency', Grid3x3, 'Lock transparent pixels'],
  ['pixels', Brush, 'Lock image pixels'],
  ['position', Move, 'Lock position'],
  ['artboard', Scan, 'Prevent auto-nesting in and out of Artboards and Frames'],
  ['all', Lock, 'Lock all'],
] as const

/** Upstream's layer style kinds (`layer_style::KINDS`), in its menu order. */
export const LAYER_STYLES = [
  ['bevelEmboss', 'Bevel & Emboss'],
  ['stroke', 'Stroke'],
  ['innerShadow', 'Inner Shadow'],
  ['innerGlow', 'Inner Glow'],
  ['satin', 'Satin'],
  ['colorOverlay', 'Color Overlay'],
  ['gradientOverlay', 'Gradient Overlay'],
  ['patternOverlay', 'Pattern Overlay'],
  ['outerGlow', 'Outer Glow'],
  ['dropShadow', 'Drop Shadow'],
] as const

/** `layer.select`'s mode for a click: Cmd toggles, Shift selects a range, a plain click one layer. */
const selectMode = (event: { metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }) => (event.metaKey || event.ctrlKey ? 'toggle' : event.shiftKey ? 'range' : 'replace')

/** Cmd = new selection, Cmd+Shift add, Cmd+Alt subtract, Cmd+Alt+Shift intersect (thumbnail Cmd-clicks). */
export const loadOperation = (event: { shiftKey: boolean; altKey: boolean }) => (event.shiftKey && event.altKey ? 'intersect' : event.shiftKey ? 'add' : event.altKey ? 'subtract' : 'new')

/** The add-mask command for the footer button: from the selection when there is one, Alt inverting. */
export const addMaskCommand = (hasSelection: boolean, alt: boolean) =>
  hasSelection ? (alt ? 'layer.layerMask.hideSelection' : 'layer.layerMask.revealSelection') : alt ? 'layer.layerMask.hideAll' : 'layer.layerMask.revealAll'

/** A group's direct child layers in `layers` (the flattened stack, each group right above its contents); 0 for another layer. */
function childCount(layers: readonly PhotoLayer[], group: PhotoLayer) {
  if (group.kind !== 'Group') return 0
  let count = 0
  for (let index = layers.indexOf(group) + 1; index < layers.length && layers[index].depth > group.depth; index += 1) if (layers[index].depth === group.depth + 1) count += 1
  return count
}

/** Upstream's layer context menu (`layer_menu_ui::entries`): `null` is a separator. */
function menuEntries(layer: PhotoLayer, multi: boolean, hasSelection: boolean): ([string, string] | null)[] {
  const rasterize: Record<string, string> = { Type: 'layer.rasterize.type', Shape: 'layer.rasterize.shape', Object: 'layer.rasterize.objectLayer', Fill: 'layer.rasterize.fillContent' }
  return [
    ['Blending Options…', 'layer.layerStyle.blendingOptions'],
    null,
    [multi ? 'Duplicate Layers…' : 'Duplicate Layer…', 'layer.duplicate'],
    [multi ? 'Delete Layers' : 'Delete Layer', 'layer.delete'],
    ...(multi ? [] : [['Group from Layers…', 'layer.groupLayers'] as [string, string]]),
    null,
    ['Quick Export As PNG', 'layer.quickExportAsPng'],
    ['Export As…', 'layer.exportAs'],
    null,
    ['Artboard from Layers…', 'layer.new.artboardFromLayers'],
    ['Frame from Layers…', 'layer.new.frameFromLayers'],
    null,
    ['Convert to Object Layer', 'layer.objectLayers.convertToObjectLayer'],
    ...(layer.kind === 'Object' ? [['Convert to Layers', 'layer.objectLayers.convertToLayers'] as [string, string]] : []),
    [layer.kind === 'Type' ? 'Rasterize Type' : 'Rasterize Layer', rasterize[layer.kind] ?? 'layer.rasterize.layer'],
    null,
    ...(layer.hasMask
      ? ([
          [layer.maskEnabled ? 'Disable Layer Mask' : 'Enable Layer Mask', 'layer.layerMask.enabled'],
          ['Apply Layer Mask', 'layer.layerMask.apply'],
          ['Delete Layer Mask', 'layer.layerMask.delete'],
        ] as [string, string][])
      : [['Add Layer Mask', addMaskCommand(hasSelection, false)] as [string, string]]),
    layer.clipped ? ['Release Clipping Mask', 'layer.releaseClippingMask'] : ['Create Clipping Mask', 'layer.addClippingMask'],
    null,
    ['Link Layers', 'layer.linkLayers'],
    ['Select Linked Layers', 'layer.selectLinkedLayers'],
    null,
    ['Copy Layer Style', 'layer.layerStyle.copyLayerStyle'],
    ['Paste Layer Style', 'layer.layerStyle.pasteLayerStyle'],
    ['Clear Layer Style', 'layer.layerStyle.clear'],
    null,
    multi ? ['Merge Layers', 'layer.mergeLayers'] : ['Merge Down', 'layer.mergeDown'],
    ['Merge Visible', 'layer.mergeVisible'],
    ['Flatten Image', 'layer.flattenImage'],
  ]
}

export function LayersPanel({ info, maskTarget, onMaskTarget }: { info: PhotoInfo; maskTarget: boolean; onMaskTarget: (on: boolean) => void }) {
  const { run, ask, enabled, byId, phrase, t, session } = useKitEditor()
  const typeEditing = useTypeEditing()
  const dialogs = useDialogs()
  /** A "…" command's dialog (with its live preview), or the generic form for one the dialog host has none for. */
  const askFirst = (command: string) => {
    if (!dialogs.open(command)) ask(command)
  }
  const ui = useUi()
  // A viewer's document: the panel's own editing controls are greyed (the engine refuses edits).
  const readOnly = useReadOnly()
  const [kinds, setKinds] = useState<readonly string[]>([])
  const [search, setSearch] = useState('')
  const [renaming, setRenaming] = useState<{ key: number; text: string } | null>(null)
  const [menu, setMenu] = useState<{ at: ContextMenuPoint; layer: PhotoLayer } | null>(null)
  const sweep = useRef<{ visible: boolean; key: string; swept: Set<number> } | null>(null)
  const gestures = useRef(new Map<string, string>())
  const list = useRef<HTMLUListElement>(null)
  /** Set when the keyboard moved the selection: the new active row takes the focus. */
  const focusActive = useRef(false)
  // A host's `disable` of the list stops every way into it, not only its buttons.
  const listDisabled = ui.state('control:layers.list').disabled

  const active = info.layers.find((layer) => layer.id === info.activeLayer) ?? null
  const selected = info.layers.filter((layer) => layer.selected)
  const filtering = kinds.length > 0 || search.trim() !== ''
  const needle = search.trim().toLowerCase()
  // Upstream's display rows: closed groups' contents left out, unless a filter looks inside them.
  const rows = info.layers.filter((layer) => {
    if (layer.id == null) return false
    if (!filtering) return !layer.folded
    const kindOk = kinds.length === 0 || KIND_FILTERS.some(([key, engineKinds]) => kinds.includes(key) && (engineKinds as readonly string[]).includes(layer.kind))
    return kindOk && (!needle || layer.name.toLowerCase().includes(needle))
  })
  const groups = useMemo(() => new Set(info.layers.filter((layer) => layer.kind === 'Group').map((layer) => layer.id!)), [info.layers])

  const sources = useMemo<ThumbnailSource[]>(
    () =>
      rows.flatMap((layer) => [
        ...(ICON_KINDS[layer.kind] || layer.kind === 'Type' ? [] : [{ kind: 'layer' as const, layer: layer.id! }]),
        ...(layer.hasMask ? [{ kind: 'mask' as const, layer: layer.id! }] : []),
      ]),
    [rows],
  )
  const thumbnail = useThumbnails(session.doc, session.change.revision, sources, THUMB * deviceScale())
  // The layers someone else is painting right now (they hold its lease), marked on their rows.
  const { leases, available: leasing } = useLeases()
  const leased = layersLeasedByOthers(useLayerObjectIds(session.doc, session.change.revision, leasing), leases)
  const drag = useLayerDrag({ list, groups, selected: selected.map((layer) => layer.id!), run: (command, params) => void run(command, params), enabled: !readOnly && !ui.state('control:layers.list').denied, document: session.doc })

  useEffect(() => {
    if (!focusActive.current) return
    focusActive.current = false
    list.current?.querySelector<HTMLElement>(`[data-layer-focus="${info.activeLayer}"]`)?.focus()
  }, [info.activeLayer])

  const label = (english: string) => phrase(english)
  const setProps = (layer: number, props: Record<string, unknown>) => void run('layer.setProps', { layer, ...props })

  /** A press on a row's eye: Alt shows only this layer; otherwise it toggles and starts a sweep down the column. */
  const pressEye = (layer: PhotoLayer, alt: boolean) => {
    if (listDisabled) return
    if (alt) {
      void run('layer.showOnly', { layer: layer.id })
      return
    }
    const key = gestureKey('layers-eye-sweep')
    sweep.current = { visible: !layer.visible, key, swept: new Set([layer.id!]) }
    setProps(layer.id!, { visible: !layer.visible, coalesce: key })
  }
  /** The sweep reached another row's eye: it takes the first eye's visibility, in the same step. */
  const sweepEye = (id: number) => {
    const current = sweep.current
    const layer = info.layers.find((item) => item.id === id)
    if (!current || !layer || current.swept.has(id)) return
    current.swept.add(id)
    if (layer.visible !== current.visible) setProps(id, { visible: current.visible, coalesce: current.key })
  }

  const clickRow = (event: ReactMouseEvent<HTMLElement>, layer: PhotoLayer) => {
    if (drag.swallowClick() || listDisabled) return
    const part = partOf(event)
    const command = event.metaKey || event.ctrlKey
    // Cmd-click a thumbnail loads its transparency or mask as a selection.
    if (command && part === 'thumb') return void run('select.loadSelection', { channel: 'transparency', layer: layer.id, operation: loadOperation(event) })
    if (command && part === 'mask') return void run('select.loadSelection', { channel: 'mask', layer: layer.id, operation: loadOperation(event) })
    // Shift-click a mask disables or enables it; Alt-click views it.
    if (part === 'mask' && event.shiftKey) return void run('layer.layerMask.enabled', { layer: layer.id })
    if (part === 'mask' && event.altKey) return void run('view.layerMask', { layer: layer.id, mode: 'toggleGray' })
    // Clicking a thumbnail picks what painting targets; adjustment and fill layers target their mask.
    // The target changes once the selection has (a host that refuses it leaves both as they were).
    const primary = layer.id === info.activeLayer
    const target = part === 'mask' || ((layer.kind === 'Adjustment' || layer.kind === 'Fill') && layer.hasMask && part !== 'thumb') ? true : part === 'thumb' || !primary ? false : null
    void run('layer.select', { layer: layer.id, mode: selectMode(event) }).then((done) => {
      if (done != null && target != null) onMaskTarget(target)
    })
  }

  const doubleClickRow = (event: ReactMouseEvent<HTMLElement>, layer: PhotoLayer) => {
    if (listDisabled) return
    const part = partOf(event)
    if (part === 'mask' || part === 'eye') return
    // A type layer's thumbnail edits its text on the canvas, all of it selected (plan 2026-11-24, stage 4).
    if (part === 'thumb' && layer.kind === 'Type' && typeEditing && layer.id != null) return typeEditing.editLayer(layer.id)
    // The Background becomes a normal layer; a name renames in place; elsewhere opens Blending Options.
    if (layer.background) void run('layer.new.layerFromBackground', {})
    else if (part === 'name') setRenaming({ key: layer.id!, text: layer.name })
    else if (enabled('layer.layerStyle.blendingOptions')) askFirst('layer.layerStyle.blendingOptions')
  }

  /** The row menu acts on the selection: a row outside it is selected first (as upstream's), so the menu's states are that row's. */
  const openMenu = (event: ReactMouseEvent<HTMLElement>, layer: PhotoLayer) => {
    event.preventDefault()
    if (listDisabled) return
    if (!layer.selected && layer.id !== info.activeLayer) void run('layer.select', { layer: layer.id, mode: 'replace' })
    setMenu({ at: { x: event.clientX, y: event.clientY }, layer })
  }

  /** The list's keys: Up and Down move the selection (Shift extends it), Alt+Up and Alt+Down move the active layer up or down the stack (Layer › Arrange), F2 or Enter renames it. */
  const keyDown = (event: ReactKeyboardEvent<HTMLUListElement>) => {
    if (listDisabled || renaming || (event.target as HTMLElement).tagName === 'INPUT') return
    const at = rows.findIndex((layer) => layer.id === info.activeLayer)
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault()
      event.stopPropagation()
      if (event.altKey) {
        if (active) void run(event.key === 'ArrowUp' ? 'arrange.bringForward' : 'arrange.sendBackward', {})
        focusActive.current = true
        return
      }
      const next = rows[Math.min(rows.length - 1, Math.max(0, at + (event.key === 'ArrowUp' ? -1 : 1)))]
      if (!next || next.id === info.activeLayer) return
      focusActive.current = true
      void run('layer.select', { layer: next.id, mode: event.shiftKey ? 'range' : 'replace' })
    } else if ((event.key === 'F2' || event.key === 'Enter') && (event.target as HTMLElement).matches('[data-layer-focus]') && active && !active.background) {
      event.preventDefault()
      event.stopPropagation()
      setRenaming({ key: active.id!, text: active.name })
    }
  }

  const finishRename = (commit: boolean) => {
    const current = renaming
    setRenaming(null)
    focusActive.current = true
    const layer = info.layers.find((item) => item.id === current?.key)
    const name = current?.text.trim()
    if (commit && layer && name && name !== layer.name) void run('layer.renameLayer', { layer: layer.id, name })
  }

  /** A context-menu row ("…" rows open their dialog); the menu's row is already selected. */
  const choose = (command: string, title: string) => {
    if (title.endsWith('…')) askFirst(command)
    else void run(command, {})
  }

  const blendChoices = useMemo(
    () => [...(active?.kind === 'Group' ? [['Pass Through', label('Pass Through')] as const] : []), ...BLEND_MODES.map(([, english]) => [english, label(english)] as const)],
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `label` reads the stable phrase function
    [active?.kind, phrase],
  )
  const locks: PhotoLocks | null = active?.locks ?? null
  const background = active?.background ?? false
  /** Opacity or Fill: each drag (until its final value) is one history step. */
  const scrub = (key: 'opacity' | 'fill') => (value: number, final: boolean) => {
    if (!active?.id) return
    const coalesce = gestures.current.get(key) ?? gestureKey(`layers-${key}`)
    gestures.current.set(key, coalesce)
    if (final) gestures.current.delete(key)
    setProps(active.id, { [key]: value / 100, coalesce })
  }

  const onSet = (layer: PhotoLayer) => selected.length > 1 && layer.selected
  const dropOn = drag.drop && 'command' in drag.drop ? drag.drop.command : null
  const ghostLayer = drag.ghost ? info.layers.find((layer) => layer.id === drag.ghost!.layer) : null
  const focusable = rows.some((layer) => layer.id === info.activeLayer) ? info.activeLayer : rows[0]?.id

  return (
    <div className="photo-layers" data-testid="photo-kit-layers">
      <div className="photo-layers-head">
        <KitElement id="control:layers.search">
          <label className="photo-layers-search">
            <Search size={13} aria-hidden="true" />
            <input type="search" value={search} placeholder={t('layers.search')} aria-label={t('layers.search')} onChange={(event) => setSearch(event.target.value)} onKeyDown={(event) => event.stopPropagation()} data-testid="photo-layers-search" />
          </label>
        </KitElement>
        <KitElement id="control:layers.filter">
          <span className="photo-layers-filters">
            {KIND_FILTERS.map(([key, , icon, tip]) => (
              <IconButton key={key} icon={icon} label={label(tip)} pressed={kinds.includes(key)} onClick={() => setKinds((now) => (now.includes(key) ? now.filter((item) => item !== key) : [...now, key]))} data-testid={`photo-layers-filter-${key}`} />
            ))}
          </span>
        </KitElement>
      </div>
      {active ? (
        <div className="photo-layers-props">
          <div className="photo-layers-line">
            <KitElement id="control:layers.blend">
              <Choice label={label('Blend Mode')} quiet value={active.blend ?? 'Normal'} choices={blendChoices} disabled={background || readOnly} onChange={(blend) => setProps(active.id!, { blend })} testId="photo-layers-blend" />
            </KitElement>
            <KitElement id="control:layers.opacity">
              <NumberField label={label('Opacity:').replace(/[:：]\s*$/, '')} value={Math.round((active.opacity ?? 1) * 100)} min={0} max={100} unit="%" disabled={background || readOnly} onChange={scrub('opacity')} testId="photo-layers-opacity" />
            </KitElement>
          </div>
          <div className="photo-layers-line">
            <KitElement id="control:layers.locks">
              <span className="photo-layers-locks" role="group" aria-label={label('Lock:')}>
                <span className="photo-layers-label">{label('Lock:')}</span>
                {LOCKS.map(([key, icon, tip]) => (
                  <IconButton key={key} icon={icon} label={label(tip)} pressed={locks?.[key] ?? false} disabled={readOnly} onClick={() => setProps(active.id!, { locks: { [key]: !(locks?.[key] ?? false) } })} data-testid={`photo-layers-lock-${key}`} />
                ))}
              </span>
            </KitElement>
            <KitElement id="control:layers.fill">
              <NumberField label={label('Fill:').replace(/[:：]\s*$/, '')} value={Math.round((active.fill ?? 1) * 100)} min={0} max={100} unit="%" disabled={background || readOnly} onChange={scrub('fill')} testId="photo-layers-fill" />
            </KitElement>
          </div>
        </div>
      ) : null}
      <KitElement id="control:layers.list">
        <ul className="photo-layers-rows" ref={list} aria-label={label('Layers')} onKeyDown={keyDown} data-disabled={listDisabled} data-testid="photo-layers-rows">
          {rows.length === 0 ? <li className="kui-muted photo-empty">{t('noLayers')}</li> : null}
          {rows.map((layer) => {
            const id = layer.id!
            return (
              <LayerRow
                key={id}
                layer={layer}
                kind={kindLine(layer, childCount(info.layers, layer), label)}
                primary={id === info.activeLayer}
                maskTarget={maskTarget}
                image={thumbnail({ kind: 'layer', layer: id })}
                maskImage={layer.hasMask ? thumbnail({ kind: 'mask', layer: id }) : null}
                drop={drag.drop && 'target' in drag.drop && drag.drop.target === id ? drag.drop.position : undefined}
                effectsOpen={!info.effectsCollapsed.includes(id)}
                renaming={renaming?.key === id ? renaming.text : null}
                focusable={id === focusable}
                leasedBy={leased.has(id) ? { name: leased.get(id)!, title: t('lease.layer', { name: leased.get(id)! }) } : null}
                label={label}
                on={{
                  press: (event) => {
                    if (!listDisabled && !readOnly && !renaming) drag.press(event, id)
                  },
                  click: (event) => clickRow(event, layer),
                  doubleClick: (event) => doubleClickRow(event, layer),
                  menu: (event) => openMenu(event, layer),
                  eye: (alt) => pressEye(layer, alt),
                  sweep: sweepEye,
                  disclosure: (all) => void run('layer.setExpanded', { layer: id, expanded: !layer.expanded, all }),
                  effects: (all) => void run('layer.setEffectsExpanded', { layer: id, expanded: info.effectsCollapsed.includes(id), all }),
                  effectEye: (index, visible) => void run('layer.setEffectsVisible', { layer: id, visible, ...(index == null ? {} : { index }) }),
                  rename: (text) => setRenaming({ key: id, text }),
                  renamed: finishRename,
                }}
              />
            )
          })}
        </ul>
      </KitElement>
      <PanelFooter
        end={
          <>
            <KitElement id="control:layers.link">
              <IconButton icon={Link} label={label('Link layers')} disabled={!enabled('layer.linkLayers')} onClick={() => void run('layer.linkLayers', {})} data-testid="photo-layers-link" />
            </KitElement>
            <KitElement id="control:layers.style">
              <MenuButton trigger={({ toggle }) => <button type="button" className="kui-icon-button photo-fx-button" aria-label={label('Add a layer style')} title={label('Add a layer style')} disabled={!active || readOnly} onClick={toggle} data-testid="photo-layers-style"><i>fx</i></button>}>
                <MenuItem label={label('Blending Options…')} onSelect={() => askFirst('layer.layerStyle.blendingOptions')} uiId="control:layers.style" />
                <MenuSeparator />
                {LAYER_STYLES.map(([key, english]) => (
                  <MenuItem key={key} label={`${label(english)}…`} onSelect={() => askFirst(`layer.layerStyle.${key}`)} uiId="control:layers.style" testId={`photo-layers-style-${key}`} />
                ))}
              </MenuButton>
            </KitElement>
            <KitElement id="control:layers.mask">
              <IconButton
                icon={SquareDot}
                label={label('Add a mask  (from the selection; {key} inverts)').replace('{key}', 'Alt')}
                disabled={!active || active.hasMask || readOnly}
                onClick={(event) => void run(addMaskCommand(info.hasSelection, event.altKey), {})}
                data-testid="photo-layers-mask"
              />
            </KitElement>
            <KitElement id="control:layers.adjustment">
              <MenuButton trigger={({ toggle }) => <IconButton icon={Contrast} label={label('Create new fill or adjustment layer')} disabled={readOnly} onClick={toggle} data-testid="photo-layers-adjustment" />}>
                {[...byId.values()]
                  .filter((command) => command.id.startsWith('layer.newAdjustmentLayer.'))
                  .map((command) => (
                    <MenuItem key={command.id} label={label(command.title).replace(/…$/, '')} disabled={!enabled(command.id)} onSelect={() => void run(command.id, {})} uiId="control:layers.adjustment" testId={`photo-layers-${command.id}`} />
                  ))}
                <MenuSeparator />
                <MenuItem label={label('Solid Color…')} onSelect={() => void run('layer.newFillLayer.solidColor', {})} uiId="control:layers.adjustment" />
                <MenuItem label={label('Gradient…')} onSelect={() => void run('layer.newFillLayer.gradient', {})} uiId="control:layers.adjustment" />
              </MenuButton>
            </KitElement>
            <KitElement id="control:layers.newGroup">
              <span data-layer-drop="layer.groupLayers" data-over={dropOn === 'layer.groupLayers'}>
                <IconButton icon={Folder} label={label('Create a new group')} disabled={readOnly} onClick={() => void run('layer.new.group', {})} data-testid="photo-layers-group" />
              </span>
            </KitElement>
            <KitElement id="control:layers.new">
              <span data-layer-drop="layer.duplicate" data-over={dropOn === 'layer.duplicate'}>
                <IconButton icon={SquarePlus} label={label('Create a new layer')} disabled={readOnly} onClick={() => void run('layer.new.layer', {})} data-testid="photo-layer-new" />
              </span>
            </KitElement>
            <KitElement id="control:layers.delete">
              <span data-layer-drop="layer.delete" data-over={dropOn === 'layer.delete'}>
                <IconButton icon={Trash2} label={label('Delete layer')} disabled={!active || info.layers.length < 2 || readOnly} onClick={() => void run('layer.delete', {})} data-testid="photo-layer-delete" />
              </span>
            </KitElement>
          </>
        }
      />
      <KitElement id="control:layers.menu">
        <ContextMenu at={menu?.at ?? null} onClose={() => setMenu(null)} label={label('Layers')} testId="photo-layers-menu">
          {menu
            ? (() => {
                const layer = menu.layer
                const set = onSet(layer)
                const items = menuEntries(layer, set, info.hasSelection).filter((entry) => !entry || (byId.has(entry[1]) && !hiddenReason(entry[1])))
                return (
                  <>
                    {items.map((entry, index) =>
                      entry ? (
                        <MenuItem key={`${entry[1]}:${index}`} label={label(entry[0])} disabled={!enabled(entry[1])} onSelect={() => choose(entry[1], entry[0])} uiId="control:layers.menu" testId={`photo-layers-menu-${entry[1]}`} />
                      ) : (
                        <MenuSeparator key={`separator:${index}`} />
                      ),
                    )}
                    <MenuSeparator />
                    <MenuItem label={label('Rename Layer…')} onSelect={() => setRenaming({ key: layer.id!, text: layer.name })} uiId="control:layers.menu" testId="photo-layers-menu-rename" />
                  </>
                )
              })()
            : null}
        </ContextMenu>
      </KitElement>
      {drag.ghost && ghostLayer ? (
        <div className="photo-layer-ghost" style={{ left: drag.ghost.x + 12, top: drag.ghost.y + 8 }} aria-hidden="true">
          {ghostLayer.name}
        </div>
      ) : null}
    </div>
  )
}
