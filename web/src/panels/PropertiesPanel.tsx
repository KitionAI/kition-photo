import { type IconComponent } from '@kition/sdk/editor'
import { TesseraIcons } from '@kition/sdk/editor'
import { Button, Checkbox, Choice, IconButton, KitElement, NumberField, useEngineQuery, useKitEditor } from '@kition/sdk/editor'
import type { PhotoInfo, PhotoLayer } from '../photo'
import { hiddenReason } from '../menus/hidden'
import { ADJUSTMENTS } from './AdjustmentsPanel'
import { AdjustmentSettings } from './AdjustmentSettings'
import { useGestures } from './gesture'
import { CharacterSection, ParagraphSection, TypeOptionsSection } from './TypeSections'
import { useTypeStyle, type TypeTarget } from './useTypeStyle'
import { ColorSwatch, EditingControls, Row, Section, useRemembered } from './PropsParts'
import { DEPTHS, MODES, QUICK_ACTIONS, canvasResizeParams, kindLabel, type Box } from './properties'

const AlignCenterHorizontal = TesseraIcons.AlignCenterHorizontalIcon
const AlignCenterVertical = TesseraIcons.AlignCenterVerticalIcon
const AlignEndHorizontal = TesseraIcons.AlignEndHorizontalIcon
const AlignEndVertical = TesseraIcons.AlignEndVerticalIcon
const AlignStartHorizontal = TesseraIcons.AlignStartHorizontalIcon
const AlignStartVertical = TesseraIcons.AlignStartVerticalIcon
const AppWindow = TesseraIcons.AppWindowIcon
const File = TesseraIcons.FileIcon
const Folder = TesseraIcons.FolderIcon
const Image = TesseraIcons.ImageIcon
const Link = TesseraIcons.LinkIcon
const PaintBucket = TesseraIcons.PaintBucketIcon
const Pentagon = TesseraIcons.PentagonIcon
const RectangleHorizontal = TesseraIcons.RectangleHorizontalIcon
const RectangleVertical = TesseraIcons.RectangleVerticalIcon
const SlidersHorizontal = TesseraIcons.SlidersHorizontalIcon
const Type = TesseraIcons.TypeIcon
const Unlink = TesseraIcons.UnlinkIcon

/**
 * Upstream's docked Properties panel (`panels.rs` `properties_body`, `doc_props_ui.rs`,
 * `layer_props_ui.rs`, `props_layout.rs`, `adjustment_controls`), context-sensitive: the document
 * (canvas size, orientation, resolution, mode and depth) when no layer or the Background is
 * active; for a layer, upstream's order (`properties_body`, `layer_controls`): a header (kind
 * icon, name, kind), its X, Y, W, and H (read only), Opacity and Fill sliders, the kind's own
 * sections (a type layer's Character, Paragraph, and Type Options, a shape's appearance and live
 * shape), then Align and Distribute and Quick Actions (upstream's Photoshop theme's); for an adjustment layer, its settings, visibility, clipping, and a
 * reset. Every edit is an engine command through the editor's session (so the mask target aims
 * it); a field's drag is one history step.
 */

/** The kind icons (upstream `props_layout::kind_icon`). */
const KIND_ICONS: Record<string, IconComponent> = { Adjustment: SlidersHorizontal, Group: Folder, Fill: PaintBucket, Type, Shape: Pentagon, Object: AppWindow, Pixel: Image }

/** Align and Distribute's buttons (upstream's order and glyphs). */
const ALIGN: readonly (readonly [string, IconComponent])[] = [
  ['leftEdges', AlignStartVertical],
  ['horizontalCenters', AlignCenterVertical],
  ['rightEdges', AlignEndVertical],
  ['topEdges', AlignStartHorizontal],
  ['verticalCenters', AlignCenterHorizontal],
  ['bottomEdges', AlignEndHorizontal],
]

export type PropertiesProps = {
  info: PhotoInfo
  foreground: string
  background: string
  /** The type style's target for a type layer (the Type tool's options and the foreground; see `useTypeStyle`). */
  typeTarget: TypeTarget
}

export function PropertiesPanel(props: PropertiesProps) {
  const { info } = props
  const active = info.layers.find((layer) => layer.id === info.activeLayer)
  // Photoshop shows the document when no layer, or the Background, is active (upstream `shows_document`).
  return (
    <div className="photo-props" data-testid="photo-properties">
      {!active || active.id == null || active.background ? <DocumentProperties info={info} /> : <LayerProperties key={active.id} layer={active} {...props} />}
    </div>
  )
}

/** Runs a command as its menu row does: a "…" row asks for its values first. */
function useInvoke() {
  const { run, ask, byId } = useKitEditor()
  return (id: string) => {
    if (byId.get(id)?.title.endsWith('…')) ask(id)
    else void run(id, {})
  }
}

function DocumentProperties({ info }: { info: PhotoInfo }) {
  const { run, phrase, t } = useKitEditor()
  const invoke = useInvoke()
  const [linked, setLinked] = useRemembered('canvas-link', false)
  const size = { width: info.width, height: info.height }
  const resize = (width: number | null, height: number | null) => {
    const params = canvasResizeParams(size, width, height, linked)
    if (params) void run('image.canvasSize', params)
  }
  const portrait = info.height > info.width
  const turn = () => void run('image.canvasSize', { width: info.height, height: info.width, anchor: 'center' })
  return (
    <>
      <div className="photo-props-header">
        <File size={15} aria-hidden="true" />
        <span className="photo-props-name">{phrase('Document')}</span>
      </div>
      <Section id="canvas" title={phrase('Canvas')}>
        <Row>
          <NumberField label="W" value={info.width} min={1} max={300000} unit="px" onChange={(value, final) => final && resize(value, null)} testId="photo-props-canvas-width" />
          <IconButton icon={linked ? Link : Unlink} label={phrase('Link width and height')} pressed={linked} onClick={() => setLinked(!linked)} data-testid="photo-props-canvas-link" />
          <NumberField label="H" value={info.height} min={1} max={300000} unit="px" onChange={(value, final) => final && resize(null, value)} testId="photo-props-canvas-height" />
        </Row>
        <Row>
          {/* Photoshop greys Canvas X and Y for documents without artboards. */}
          <NumberField label="X" value={0} min={0} max={0} unit="px" disabled onChange={() => undefined} />
          <span className="photo-props-gap" />
          <NumberField label="Y" value={0} min={0} max={0} unit="px" disabled onChange={() => undefined} />
        </Row>
        <Row>
          <IconButton icon={RectangleVertical} label={phrase('Portrait')} pressed={portrait} onClick={() => !portrait && info.width !== info.height && turn()} data-testid="photo-props-portrait" />
          <IconButton icon={RectangleHorizontal} label={phrase('Landscape')} pressed={!portrait} onClick={() => portrait && turn()} data-testid="photo-props-landscape" />
        </Row>
        <p className="photo-props-text">{phrase('Resolution: {dpi} pixels/inch').replace('{dpi}', String(Math.round(info.resolution * 100) / 100))}</p>
        <Choice
          label={phrase('Mode')}
          value={info.mode}
          choices={MODES.map(([mode, label]) => [mode, phrase(label)] as const)}
          onChange={(mode) => {
            const command = MODES.find(([value]) => value === mode)?.[2]
            if (command) invoke(command)
          }}
          testId="photo-props-mode"
        />
        <Choice
          label={t('properties.depth')}
          quiet
          value={String(info.depth)}
          choices={DEPTHS.map(([depth, label]) => [depth, phrase(label)] as const)}
          onChange={(depth) => {
            const command = DEPTHS.find(([value]) => value === depth)?.[2]
            if (command) invoke(command)
          }}
          testId="photo-props-depth"
        />
      </Section>
    </>
  )
}

function LayerProperties({ layer, info, foreground, background, typeTarget }: PropertiesProps & { layer: PhotoLayer }) {
  const { run, phrase, enabled, byId } = useKitEditor()
  const invoke = useInvoke()
  const id = layer.id!
  const KindIcon = KIND_ICONS[layer.kind] ?? Image
  const adjustmentLabel = layer.adjustment ? (ADJUSTMENTS.find(([kind]) => kind === layer.adjustment!.kind)?.[2] ?? null) : null
  const box: Box | null = info.activeBounds ? [info.activeBounds.x0, info.activeBounds.y0, info.activeBounds.x1, info.activeBounds.y1] : null
  const actions = (QUICK_ACTIONS[layer.kind] ?? []).filter(([, command]) => byId.has(command) && !hiddenReason(command) && enabled(command))
  return (
    <>
      <div className="photo-props-header" data-testid="photo-props-header">
        <KindIcon size={15} aria-hidden="true" />
        <span className="photo-props-name">{layer.name}</span>
        <span className="photo-props-kind">{phrase(kindLabel(layer.kind, adjustmentLabel, layer.artboard))}</span>
      </div>
      {layer.adjustment ? (
        <KitElement id="control:properties.adjustment">
          <div className="photo-props-adjustment" data-testid="photo-props-adjustment">
            <EditingControls>
              <AdjustmentSettings key={id} layer={layer} foreground={foreground} background={background} />
              <Row>
                <Checkbox label={phrase('Adjustment visible')} checked={layer.visible} onChange={(visible) => void run('layer.setProps', { layer: id, visible })} testId="photo-props-adjustment-visible" />
                <Checkbox label={phrase('Clip to layer below')} checked={layer.clipped} onChange={(clipped) => void run('layer.setProps', { layer: id, clipped })} testId="photo-props-adjustment-clip" />
              </Row>
              <Button onClick={() => void run('layer.setAdjustment', { layer: id })} data-testid="photo-props-adjustment-reset">
                {phrase('Reset to defaults')}
              </Button>
            </EditingControls>
          </div>
        </KitElement>
      ) : (
        <>
          {box ? <Bounds box={box} /> : null}
          <Blending layer={layer} />
          {layer.kind === 'Type' ? <TypeProperties target={typeTarget} /> : null}
          {layer.kind === 'Shape' ? <ShapeSections layer={id} /> : null}
          <Section id="align" title={phrase('Align and Distribute')}>
            <Row>
              {ALIGN.map(([kind, icon]) => {
                const command = `layer.align.${kind}`
                return <IconButton key={kind} icon={icon} label={phrase('Align {what}').replace('{what}', phrase(byId.get(command)?.title ?? kind))} disabled={!enabled(command)} onClick={() => void run(command, {})} data-testid={`photo-props-align-${kind}`} />
              })}
            </Row>
          </Section>
          {actions.length ? (
            <Section id="quickActions" title={phrase('Quick Actions')}>
              <div className="photo-props-actions">
                {actions.map(([label, command]) => (
                  <Button key={command} onClick={() => invoke(command)} data-testid={`photo-props-action-${command}`}>
                    {phrase(label)}
                  </Button>
                ))}
              </div>
            </Section>
          ) : null}
        </>
      )}
    </>
  )
}

/** The content box's X, Y, W, and H, read only (upstream `layer_controls`): the Move tool and Free Transform change them. */
function Bounds({ box }: { box: Box }) {
  const values: [string, number][] = [
    ['X', box[0]],
    ['Y', box[1]],
    ['W', box[2] - box[0]],
    ['H', box[3] - box[1]],
  ]
  return (
    <KitElement id="control:properties.transform">
      <dl className="photo-props-bounds" data-testid="photo-props-bounds">
        {values.map(([name, value]) => (
          <div key={name} className="photo-props-bound">
            <dt>{name}</dt>
            <dd data-testid={`photo-props-bounds-${name.toLowerCase()}`}>{`${value} px`}</dd>
          </div>
        ))}
      </dl>
    </KitElement>
  )
}

/** Opacity and Fill as slider rows (upstream `layer_controls`' `slider_row`s), each drag one history step. */
function Blending({ layer }: { layer: PhotoLayer }) {
  const { run, phrase } = useKitEditor()
  const gestures = useGestures(`layer-${layer.id}`)
  const set = (key: 'opacity' | 'fill') => (value: number, final: boolean) => void run('layer.setProps', { layer: layer.id, [key]: value / 100, coalesce: gestures.key(key, final) })
  return (
    <div className="photo-props-fields">
      <EditingControls>
        <NumberField label={phrase('Opacity')} value={Math.round((layer.opacity ?? 1) * 100)} min={0} max={100} unit="%" track onChange={set('opacity')} testId="photo-props-opacity" uiId="control:properties.opacity" />
        <NumberField label={phrase('Fill')} value={Math.round((layer.fill ?? 1) * 100)} min={0} max={100} unit="%" track onChange={set('fill')} testId="photo-props-fill" uiId="control:properties.fill" />
      </EditingControls>
    </div>
  )
}

type ShapeInfo = {
  fill: unknown
  stroke: { width: number; fill: unknown; align: string } | null
  live: { kind: string; rect?: number[]; radii?: number[]; sides?: number; star_ratio?: number; weight?: number } | null
}

/**
 * A shape's fill or stroke paint: a colour shows as its swatch; none, a gradient, or a pattern
 * shows as its name (choosing the paint type, upstream's fill type picker, comes in wave 2).
 */
function Paint({ paint, label, onChange, testId }: { paint: unknown; label: string; onChange: (color: string, final: boolean) => void; testId: string }) {
  const { phrase } = useKitEditor()
  if (typeof paint === 'string') return <ColorSwatch label={label} value={paint} onChange={onChange} testId={testId} />
  const kind = paint && typeof paint === 'object' ? ('gradient' in paint ? 'Gradient' : 'pattern' in paint ? 'Pattern' : null) : 'None'
  return (
    <span className="photo-props-paint" title={label} data-testid={testId} data-paint={kind ?? 'other'}>
      {kind ? phrase(kind) : '—'}
    </span>
  )
}

/** A shape's Appearance (fill, stroke colour, width, and alignment) and its live shape's own fields (upstream `vector_ui::shape_properties`), each change a `shape.edit`. */
function ShapeSections({ layer }: { layer: number }) {
  const { run, phrase } = useKitEditor()
  const gestures = useGestures(`shape-${layer}`)
  const shape = useEngineQuery<ShapeInfo>('shape.info', { layer }, layer)
  if (!shape) return null
  const edit = (params: Record<string, unknown>, field: string, final: boolean) => void run('shape.edit', { layer, ...params, coalesce: gestures.key(field, final) })
  const stroke = shape.stroke
  const live = shape.live
  const rect = live?.rect
  return (
    <>
      <Section id="appearance" title={phrase('Appearance')}>
        <Row>
          <span className="photo-props-label">{phrase('Fill')}</span>
          <Paint paint={shape.fill} label={phrase('Set shape fill type')} onChange={(color, final) => edit({ fill: color }, 'fill', final)} testId="photo-props-shape-fill" />
          <span className="photo-props-label">{phrase('Stroke')}</span>
          <Paint paint={stroke ? stroke.fill : null} label={phrase('Set shape stroke type')} onChange={(color, final) => edit({ stroke: { color, width: stroke?.width ?? 3 } }, 'stroke', final)} testId="photo-props-shape-stroke" />
        </Row>
        <Row>
          <NumberField
            label={phrase('Width')}
            value={stroke?.width ?? 0}
            min={0}
            max={288}
            step={0.1}
            unit="px"
            onChange={(width, final) => edit(width <= 0 ? { stroke: null } : { stroke: { width } }, 'stroke-width', final)}
            testId="photo-props-shape-strokeWidth"
          />
          {stroke ? (
            <Choice
              label={phrase('Stroke')}
              quiet
              value={stroke.align === 'inside' || stroke.align === 'outside' ? stroke.align : 'center'}
              choices={[
                ['inside', phrase('Inside')],
                ['center', phrase('Center')],
                ['outside', phrase('Outside')],
              ]}
              onChange={(align) => edit({ stroke: { align } }, 'stroke-align', true)}
              testId="photo-props-shape-strokeAlign"
            />
          ) : null}
        </Row>
      </Section>
      {live ? (
        <Section id="shape" title={phrase('Shape')}>
          {rect && live.kind !== 'line' ? (
            <Row>
              <NumberField label="W" value={Math.round(rect[2])} min={1} max={300000} unit="px" onChange={(width, final) => edit({ rect: [rect[0], rect[1], Math.round(width), rect[3]] }, 'wh', final)} testId="photo-props-shape-width" />
              <NumberField label="H" value={Math.round(rect[3])} min={1} max={300000} unit="px" onChange={(height, final) => edit({ rect: [rect[0], rect[1], rect[2], Math.round(height)] }, 'wh', final)} testId="photo-props-shape-height" />
            </Row>
          ) : null}
          {live.kind === 'rect' ? (
            <NumberField label={phrase('Corner radius')} value={live.radii?.[0] ?? 0} min={0} max={100000} unit="px" onChange={(radii, final) => edit({ radii }, 'radius', final)} testId="photo-props-shape-radius" />
          ) : null}
          {live.kind === 'polygon' ? (
            <Row>
              <NumberField label={phrase('Sides')} value={live.sides ?? 5} min={3} max={100} onChange={(sides, final) => edit({ sides }, 'sides', final)} testId="photo-props-shape-sides" />
              <NumberField label={phrase('Star ratio')} value={Math.round((live.star_ratio ?? 1) * 100)} min={1} max={100} unit="%" onChange={(ratio, final) => edit({ starRatio: ratio / 100 }, 'star', final)} testId="photo-props-shape-starRatio" />
            </Row>
          ) : null}
          {live.kind === 'line' ? (
            <NumberField label={phrase('Weight')} value={live.weight ?? 1} min={1} max={10000} unit="px" onChange={(weight, final) => edit({ weight }, 'weight', final)} testId="photo-props-shape-weight" />
          ) : null}
        </Section>
      ) : null}
    </>
  )
}

/** A type layer's sections (upstream's `type_properties`): Character, Paragraph, and Type Options, shared with the Character and Paragraph panels. */
function TypeProperties({ target }: { target: TypeTarget }) {
  const style = useTypeStyle(target)
  if (!style.layer) return null
  return (
    <>
      <CharacterSection style={style} testId="photo-props-type" />
      <ParagraphSection style={style} testId="photo-props-type" uiId="control:properties.paragraph" />
      <TypeOptionsSection style={style} testId="photo-props-type" />
    </>
  )
}
