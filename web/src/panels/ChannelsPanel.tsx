import { useMemo, useState, type MouseEvent as ReactMouseEvent } from 'react'
import { TesseraIcons } from '@kition/sdk/editor'
import { ContextMenu, EyeButton, IconButton, KitElement, MenuItem, MenuSeparator, PanelFooter, RenameField, Thumbnail, deviceScale, useKitEditor, useUi, type ContextMenuPoint } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { loadOperation } from './LayersPanel'
import { useChannelThumbnails, useThumbnails } from './thumbnails'

const CircleDashed = TesseraIcons.CircleDashedIcon
const Plus = TesseraIcons.PlusIcon
const SquareDashed = TesseraIcons.SquareDashedIcon
const Trash2 = TesseraIcons.TrashIcon

/**
 * Upstream's Channels panel (`channels_panel.rs`): the composite and color channels, the active
 * layer's mask, alpha and spot channels, and the Quick Mask, each with its thumbnail and eye;
 * a click targets a channel, Cmd-click loads it as a selection, and the footer loads, saves,
 * makes, and deletes channels. The target and the eyes are the engine's (`inspect().channels`).
 */

type Row =
  | { kind: 'composite' }
  | { kind: 'color'; index: number }
  | { kind: 'mask' }
  | { kind: 'alpha'; index: number }
  | { kind: 'quickMask' }

/** A row's channel as the engine's commands take it. */
function reference(row: Row): unknown {
  switch (row.kind) {
    case 'composite':
      return 'composite'
    case 'color':
      return { color: row.index }
    case 'alpha':
      return row.index
    case 'quickMask':
      return 'quickMask'
    case 'mask':
      return 'mask'
  }
}

const rowKey = (row: Row) => ('index' in row ? `${row.kind}-${row.index}` : row.kind)

const THUMB = 28

export function ChannelsPanel({ info, maskTarget, onMaskTarget }: { info: PhotoInfo; maskTarget: boolean; onMaskTarget: (on: boolean) => void }) {
  const { run, ask, enabled, phrase, t, session } = useKitEditor()
  const [renaming, setRenaming] = useState<{ key: number; text: string } | null>(null)
  const [menu, setMenu] = useState<{ at: ContextMenuPoint; row: Row } | null>(null)
  const channels = info.channels
  const active = info.layers.find((layer) => layer.id === info.activeLayer) ?? null
  const masked = active?.hasMask ? active : null
  const quick = Boolean(channels?.quickMask)
  const target = channels?.target ?? { kind: 'composite' as const }
  const maskTargeted = Boolean(masked) && maskTarget && target.kind === 'composite' && !quick
  const multichannel = info.mode === 'Multichannel'
  const colors = channels?.colors ?? []

  const rows = useMemo<Row[]>(() => {
    if (!channels) return []
    const out: Row[] = multichannel ? [] : [{ kind: 'composite' }]
    if (colors.length > 1 && !multichannel) out.push(...colors.map((_, index) => ({ kind: 'color' as const, index })))
    if (masked) out.push({ kind: 'mask' })
    out.push(...channels.alpha.map((alpha) => ({ kind: 'alpha' as const, index: alpha.index })))
    if (quick) out.push({ kind: 'quickMask' })
    return out
  }, [channels, colors, masked, multichannel, quick])
  // The engine's channel thumbnails come together (one render of the composite); the layer mask's is its own.
  const channelImages = useChannelThumbnails(session.doc, session.change.revision, true, THUMB * deviceScale())
  const maskSources = useMemo(() => (masked?.id != null ? [{ kind: 'mask' as const, layer: masked.id }] : []), [masked?.id])
  const maskImage = useThumbnails(session.doc, session.change.revision, maskSources, THUMB * deviceScale())
  /** A row's image: `channelThumbnails` answers the composite, each color channel, each alpha channel, then the Quick Mask. */
  const imageOf = (row: Row): ImageBitmap | null => {
    const count = channels?.colors.length ?? 0
    switch (row.kind) {
      case 'composite':
        return channelImages[0] ?? null
      case 'color':
        return channelImages[1 + row.index] ?? null
      case 'alpha':
        return channelImages[1 + count + row.index] ?? null
      case 'quickMask':
        return channelImages[1 + count + (channels?.alpha.length ?? 0)] ?? null
      case 'mask':
        return maskSources[0] ? maskImage(maskSources[0]) : null
    }
  }
  const listDisabled = useUi().state('control:channels.list').disabled

  const label = (english: string) => phrase(english)
  const layerMaskView = channels?.layerMaskView ?? null
  const grayView = layerMaskView?.mode === 'gray'

  /** A row's name, whether its eye is on, whether it is targeted, and its Cmd+number key. */
  const describeRow = (row: Row) => {
    const alpha = row.kind === 'alpha' ? channels!.alpha.find((item) => item.index === row.index) : null
    const composite = target.kind === 'composite' && !quick && !maskTargeted
    switch (row.kind) {
      case 'composite':
        return { name: channels!.composite, visible: channels!.compositeVisible && !grayView, selected: composite, slot: 2 }
      case 'color':
        return { name: colors[row.index].name, visible: colors[row.index].visible && !grayView, selected: composite || (target.kind === 'color' && target.index === row.index), slot: 3 + row.index }
      case 'alpha': {
        const slot = 3 + (colors.length > 1 ? colors.length : 0) + row.index
        return { name: alpha?.name ?? '', visible: alpha?.visible ?? false, selected: target.kind === 'alpha' && target.index === row.index, slot: slot <= 9 ? slot : null }
      }
      case 'quickMask':
        return { name: label('Quick Mask'), visible: channels!.quickMask?.visible ?? false, selected: target.kind === 'composite', slot: null }
      case 'mask':
        return { name: t('channels.layerMask', { name: masked?.name ?? '' }), visible: layerMaskView != null, selected: maskTargeted, slot: null }
    }
  }

  const pressEye = (row: Row, visible: boolean) => {
    const layer = masked?.id
    // The mask's eye shows it as an overlay or ends the mask view; the composite's eye over a gray mask view keeps the overlay.
    if (row.kind === 'mask') void run('view.layerMask', { layer, mode: visible ? 'off' : 'overlay' })
    else if (row.kind === 'composite' && grayView) void run('view.layerMask', { layer, mode: 'overlay' })
    else void run('channel.setVisible', { channel: reference(row), visible: !visible })
  }

  const clickRow = (event: ReactMouseEvent<HTMLElement>, row: Row) => {
    if (listDisabled) return
    if (event.metaKey || event.ctrlKey) {
      void run('select.loadSelection', row.kind === 'mask' ? { channel: 'mask', layer: masked?.id, operation: loadOperation(event) } : { channel: reference(row), operation: loadOperation(event) })
      return
    }
    // The layer mask's row targets the mask, as its thumbnail in the Layers panel does; any other
    // row ends the mask target (an alpha channel or the Quick Mask takes the edits). The target
    // changes once the channel target has (a host that refuses it leaves both as they were).
    void run('channel.target', { channel: row.kind === 'mask' || row.kind === 'quickMask' ? 'composite' : reference(row) }).then((done) => {
      if (done != null) onMaskTarget(row.kind === 'mask')
    })
  }

  const finishRename = (commit: boolean) => {
    const current = renaming
    setRenaming(null)
    const alpha = channels?.alpha.find((item) => item.index === current?.key)
    const name = current?.text.trim()
    if (commit && alpha && name && name !== alpha.name) void run('channel.rename', { channel: alpha.index, name })
  }

  const targetReference = target.kind === 'alpha' ? target.index : target.kind === 'color' ? { color: target.index } : 'composite'

  /** Upstream's row menu: per kind, then New Channel and New Spot Channel. */
  const menuItems = (row: Row) => {
    const item = (english: string, command: string, params: Record<string, unknown>, dialog = false) => (
      <MenuItem key={`${command}:${english}`} label={label(english)} disabled={!enabled(command)} onSelect={() => (dialog ? ask(command, params) : void run(command, params))} uiId="control:channels.menu" testId={`photo-channels-menu-${command}`} />
    )
    const own = (() => {
      switch (row.kind) {
        case 'alpha':
          return [
            item('Duplicate Channel', 'channel.duplicate', { channel: row.index }),
            item('Delete Channel', 'channel.delete', { channel: row.index }),
            <MenuItem key="rename" label={label('Rename Channel…')} onSelect={() => setRenaming({ key: row.index, text: channels?.alpha.find((alpha) => alpha.index === row.index)?.name ?? '' })} uiId="control:channels.menu" testId="photo-channels-menu-rename" />,
            <MenuSeparator key="separator-indicates" />,
            item('Color Indicates Masked Areas', 'channel.options', { channel: row.index, indicates: 'masked' }),
            item('Color Indicates Selected Areas', 'channel.options', { channel: row.index, indicates: 'selected' }),
          ]
        case 'quickMask':
          return [item('Exit Quick Mask', 'select.editInQuickMaskMode', { on: false })]
        case 'mask':
          return [item(masked?.maskEnabled ? 'Disable Layer Mask' : 'Enable Layer Mask', 'layer.layerMask.enabled', {}), item('Delete Layer Mask', 'layer.layerMask.delete', {})]
        default:
          return [item('Duplicate Channel', 'channel.duplicate', { channel: reference(row) })]
      }
    })()
    return [...own, <MenuSeparator key="separator-new" />, item('New Channel…', 'channel.new', {}, true), item('New Spot Channel…', 'channel.newSpot', {}, true)]
  }

  if (!channels) return <p className="kui-muted photo-empty">{label('No document')}</p>

  return (
    <div className="photo-channels" data-testid="photo-channels">
      <KitElement id="control:channels.list">
        <ul className="photo-layers-rows" aria-label={label('Channels')} data-disabled={listDisabled}>
          {rows.map((row) => {
            const { name, visible, selected, slot } = describeRow(row)
            const key = rowKey(row)
            return (
              <li
                key={key}
                className="photo-layer photo-channel"
                data-selected={selected}
                data-channel={key}
                onClick={(event) => clickRow(event, row)}
                onDoubleClick={() => {
                  if (row.kind === 'alpha' && !listDisabled) setRenaming({ key: row.index, text: name })
                }}
                onContextMenu={(event) => {
                  event.preventDefault()
                  if (!listDisabled) setMenu({ at: { x: event.clientX, y: event.clientY }, row })
                }}
                data-testid={`photo-channel-${key}`}
              >
                <span className="photo-layer-eye">
                  <KitElement id="control:channels.visibility">
                    <EyeButton visible={visible} label={label(row.kind === 'mask' ? 'Show the layer mask as an overlay' : 'Toggle visibility')} onPress={() => !listDisabled && pressEye(row, visible)} testId={`photo-channel-visibility-${key}`} />
                  </KitElement>
                </span>
                <span className="photo-layer-thumb photo-channel-thumb">
                  <Thumbnail image={imageOf(row)} size={THUMB} testId={`photo-channel-thumb-${key}`} />
                </span>
                {row.kind === 'alpha' && renaming?.key === row.index ? (
                  <RenameField value={renaming.text} label={label('Rename Channel…')} onChange={(text) => setRenaming({ key: row.index, text })} onDone={finishRename} testId="photo-channel-rename" />
                ) : (
                  <button type="button" className="photo-layer-name" data-italic={row.kind === 'mask' || row.kind === 'quickMask'} aria-pressed={selected}>
                    {name}
                  </button>
                )}
                {slot ? <kbd className="photo-channel-key">{`Cmd+${slot}`}</kbd> : null}
              </li>
            )
          })}
        </ul>
      </KitElement>
      <PanelFooter
        start={
          <>
            <KitElement id="control:channels.load">
              <IconButton icon={CircleDashed} label={label('Load channel as selection')} onClick={() => void run('select.loadSelection', { channel: targetReference })} data-testid="photo-channels-load" />
            </KitElement>
            <KitElement id="control:channels.save">
              <IconButton icon={SquareDashed} label={label('Save selection as channel')} disabled={!info.hasSelection} onClick={() => void run('select.saveSelection', {})} data-testid="photo-channels-save" />
            </KitElement>
          </>
        }
        end={
          <>
            <KitElement id="control:channels.new">
              <IconButton icon={Plus} label={label('Create new channel')} onClick={() => void run('channel.new', {})} data-testid="photo-channels-new" />
            </KitElement>
            <KitElement id="control:channels.delete">
              <IconButton icon={Trash2} label={label('Delete current channel')} disabled={target.kind !== 'alpha'} onClick={() => void run('channel.delete', {})} data-testid="photo-channels-delete" />
            </KitElement>
          </>
        }
      />
      <KitElement id="control:channels.menu">
        <ContextMenu at={menu?.at ?? null} onClose={() => setMenu(null)} label={label('Channels')} testId="photo-channels-menu">
          {menu ? menuItems(menu.row) : null}
        </ContextMenu>
      </KitElement>
    </div>
  )
}
