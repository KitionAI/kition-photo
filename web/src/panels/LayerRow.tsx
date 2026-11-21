import type { MouseEvent as ReactMouseEvent, PointerEvent as ReactPointerEvent } from 'react'
import { TesseraIcons } from '@kition/sdk/editor'
import { EyeButton, KitElement, RenameField, Thumbnail } from '@kition/sdk/editor'
import type { PhotoLayer } from '../photo'
import { ADJUSTMENTS } from './AdjustmentsPanel'

const Brush = TesseraIcons.BrushIcon
const ChevronDown = TesseraIcons.ChevronDownIcon
const ChevronRight = TesseraIcons.ChevronRightIcon
const Contrast = TesseraIcons.ContrastIcon
const Folder = TesseraIcons.FolderIcon
const Link = TesseraIcons.LinkIcon
const Lock = TesseraIcons.LockIcon
const Sparkles = TesseraIcons.SparklesIcon

/** Kinds the panel draws as an icon tile instead of a thumbnail (upstream's folder and contrast glyphs). */
export const ICON_KINDS: Record<string, typeof Folder> = { Group: Folder, Adjustment: Contrast }

/** The thumbnails' side (CSS pixels): upstream's default thumbnails, 34 pt in rows of about 44. */
export const THUMB = 34

/**
 * The line under a layer's name (upstream `layer_row`'s kind sub-label): an adjustment's own name,
 * a group's layer count, or the kind (Type, Shape, Fill, Object); none for a pixel layer.
 * `count` is a group's number of child layers.
 */
export function kindLine(layer: PhotoLayer, count: number, label: (english: string) => string) {
  if (layer.kind === 'Pixel' || !layer.kind) return null
  if (layer.kind === 'Adjustment') return label(ADJUSTMENTS.find(([kind]) => kind === layer.adjustment?.kind)?.[2] ?? 'Adjustment')
  if (layer.kind === 'Group') {
    // An upstream plural row: its English forms, or the translation's (one form, or the singular and the plural).
    const forms = label('Group · {n} layer|Group · {n} layers').split('|')
    return (forms[count === 1 ? 0 : forms.length - 1] ?? forms[0]).replace('{n}', String(count))
  }
  return label(layer.kind)
}

/** The part of a row a pointer event hit: its eye, thumbnail, mask thumbnail, or name. */
export const partOf = (event: { target: EventTarget }) => (event.target as HTMLElement).closest<HTMLElement>('[data-part]')?.dataset.part

export type LayerRowProps = {
  layer: PhotoLayer
  /** The line under the name (`kindLine`), or null. */
  kind: string | null
  /** The active layer's row. */
  primary: boolean
  /** The active layer's mask is what painting edits (its thumbnail framed instead of the layer's). */
  maskTarget: boolean
  image: ImageBitmap | null
  maskImage: ImageBitmap | null
  /** Where a row dragged over this one would land. */
  drop: 'above' | 'below' | 'into' | undefined
  effectsOpen: boolean
  /** The text of an in-place rename of this row, or null. */
  renaming: string | null
  /** Only the active row is in the tab order (the list moves the focus with the arrow keys). */
  focusable: boolean
  /** Someone else is painting this layer (holds its lease, `layerLeases.ts`): their name, and the mark's tip. */
  leasedBy?: { name: string; title: string } | null
  label: (english: string) => string
  on: {
    press: (event: ReactPointerEvent<HTMLElement>) => void
    click: (event: ReactMouseEvent<HTMLElement>) => void
    doubleClick: (event: ReactMouseEvent<HTMLElement>) => void
    menu: (event: ReactMouseEvent<HTMLElement>) => void
    eye: (alt: boolean) => void
    sweep: (layer: number) => void
    disclosure: (all: boolean) => void
    effects: (all: boolean) => void
    effectEye: (index: number | null, visible: boolean) => void
    rename: (text: string) => void
    renamed: (commit: boolean) => void
  }
}

/**
 * One Layers panel row (upstream's `layer_row`, `layer_row_ui::indicators`, `effect_rows`): the
 * eye, the group disclosure, the clipping arrow, the thumbnail and mask thumbnail (the targeted
 * one framed), the name (renamed in place; the Background's in italics) over its kind, the blend mode, link, fx, and lock marks, then the
 * layer's effects rows while they are open.
 */
export function LayerRow({ layer, kind, primary, maskTarget, image, maskImage, drop, effectsOpen, renaming, focusable, leasedBy, label, on }: LayerRowProps) {
  const id = layer.id!
  const Icon = ICON_KINDS[layer.kind]
  const showBlend = layer.blend && layer.blend !== 'Normal' && layer.blend !== 'Pass Through'
  const selected = layer.selected || primary
  const groupTip = label(layer.expanded ? 'Collapse group  ({key}-click: all groups)' : 'Expand group  ({key}-click: all groups)').replace('{key}', 'Alt')
  const fxTip = label(effectsOpen ? "Hide the layer's effects  ({key}-click: all layers)" : "Show the layer's effects  ({key}-click: all layers)").replace('{key}', 'Alt')
  return (
    <li className="photo-layer-block">
      <div
        className="photo-layer"
        data-layer-row={id}
        data-selected={selected}
        data-hidden={!layer.visible}
        data-drop={drop}
        onPointerDown={on.press}
        onClick={on.click}
        onDoubleClick={on.doubleClick}
        onContextMenu={on.menu}
      >
        <span className="photo-layer-eye" data-part="eye">
          <KitElement id="control:layers.visibility">
            <EyeButton visible={layer.visible} label={label('Toggle visibility')} onPress={on.eye} tabIndex={-1} sweep={{ id: String(id), group: 'layers', onOver: (over) => on.sweep(Number(over)) }} testId={`photo-layer-visibility-${id}`} />
          </KitElement>
        </span>
        <span className="photo-layer-indent" style={{ width: layer.depth * 14 }} />
        {layer.kind === 'Group' ? (
          <button
            type="button"
            className="photo-disclosure"
            aria-expanded={layer.expanded}
            title={groupTip}
            aria-label={groupTip}
            tabIndex={-1}
            onPointerDown={(event) => event.stopPropagation()}
            onClick={(event) => {
              event.stopPropagation()
              on.disclosure(event.altKey)
            }}
            data-testid={`photo-layer-disclosure-${id}`}
          >
            {layer.expanded ? <ChevronDown size={13} aria-hidden="true" /> : <ChevronRight size={13} aria-hidden="true" />}
          </button>
        ) : null}
        {layer.clipped ? (
          <span className="photo-layer-clip" aria-hidden="true">
            ↳
          </span>
        ) : null}
        <span className="photo-layer-thumb" data-part="thumb" data-tile={layer.kind in ICON_KINDS ? 'icon' : layer.kind === 'Type' ? 'type' : undefined} data-target={primary && !maskTarget} data-testid={`photo-layer-thumb-${id}`}>
          {Icon ? <Icon size={18} aria-hidden="true" /> : layer.kind === 'Type' ? <span className="photo-type-thumb">T</span> : <Thumbnail image={image} size={THUMB} />}
        </span>
        {layer.hasMask ? (
          <>
            {layer.maskLinked ? <Link size={10} className="photo-layer-chain" aria-hidden="true" /> : <span className="photo-layer-chain" />}
            <span className="photo-layer-thumb" data-part="mask" data-target={primary && maskTarget} data-disabled={!layer.maskEnabled} data-testid={`photo-layer-mask-${id}`}>
              <Thumbnail image={maskImage} size={THUMB} />
            </span>
          </>
        ) : null}
        {renaming != null ? (
          <RenameField value={renaming} label={label('Rename Layer…')} onChange={on.rename} onDone={on.renamed} testId="photo-layer-rename" />
        ) : (
          <span className="photo-layer-text">
            <button type="button" className="photo-layer-name" data-part="name" data-italic={layer.background} aria-pressed={selected} aria-current={primary || undefined} tabIndex={focusable ? 0 : -1} data-layer-focus={id} data-testid={`photo-layer-${id}`}>
              {layer.name}
            </button>
            {kind ? (
              <span className="photo-layer-kind" data-testid={`photo-layer-kind-${id}`}>
                {kind}
              </span>
            ) : null}
          </span>
        )}
        <span className="photo-layer-marks">
          {leasedBy ? (
            <span className="photo-layer-lease" title={leasedBy.title} aria-label={leasedBy.title} data-testid={`photo-layer-lease-${id}`} data-holder={leasedBy.name}>
              <Brush size={11} aria-hidden="true" />
              <span className="photo-layer-lease-name">{leasedBy.name}</span>
            </span>
          ) : null}
          {showBlend ? <span className="photo-layer-blend">{label(layer.blend!)}</span> : null}
          {layer.linked ? <Link size={12} aria-label={label('Link Layers')} /> : null}
          {layer.effects ? (
            <button
              type="button"
              className="photo-layer-fx"
              aria-expanded={effectsOpen}
              aria-label={fxTip}
              title={fxTip}
              tabIndex={-1}
              onPointerDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                event.stopPropagation()
                on.effects(event.altKey)
              }}
              data-testid={`photo-layer-fx-${id}`}
            >
              <i>fx</i>
              {effectsOpen ? <ChevronDown size={11} aria-hidden="true" /> : <ChevronRight size={11} aria-hidden="true" />}
            </button>
          ) : null}
          {layer.locks.transparency || layer.locks.position || layer.locks.all ? <Lock size={12} aria-label={label('Lock:')} /> : null}
        </span>
      </div>
      {layer.effects && effectsOpen ? (
        <ul className="photo-effects" style={{ paddingLeft: 30 + layer.depth * 14 + THUMB + 10 }}>
          {[{ name: label('Effects'), on: layer.effects.enabled, index: null as number | null }, ...layer.effects.items.map((item, index) => ({ name: label(item.kind), on: item.enabled, index }))].map((row) => (
            <li key={row.index ?? 'all'} className="photo-effect" data-off={!row.on}>
              <span className="photo-layer-eye photo-effect-eye">
                <KitElement id="control:layers.visibility">
                  <EyeButton visible={row.on} label={label('Toggle visibility')} onPress={() => on.effectEye(row.index, !row.on)} tabIndex={-1} testId={`photo-effect-visibility-${id}-${row.index ?? 'all'}`} />
                </KitElement>
              </span>
              {row.index == null ? <Sparkles size={11} aria-hidden="true" /> : null}
              <span className="photo-effect-name">{row.name}</span>
            </li>
          ))}
        </ul>
      ) : null}
    </li>
  )
}
