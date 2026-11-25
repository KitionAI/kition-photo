import { TesseraIcons } from '@kition/sdk/editor'
import { Choice as KitChoice, IconButton, MenuButton, MenuItem, MenuSeparator, NumberField, Segmented, parseHex, useEngineQuery, useKitEditor, useUi } from '@kition/sdk/editor'
import { CROP_RATIOS, cropRatioKey, cropRatioSize } from '../tools/options'
import { useFontChoices } from './typeLayer'
import { useTypeStyle } from '../panels/useTypeStyle'
import { antiAliasChoices } from '../panels/TypeSections'
import { ColorSwatch } from '../panels/PropsParts'
import { Act, BlendMode, Check, Choice, FillSampling, Hint, Icon, Icons, Num, Opt, Sep, Toggle, bare, fill, useBar } from './fields'


const AlignCenter = TesseraIcons.TextAlignCenterIcon
const AlignCenterVertical = TesseraIcons.AlignCenterVerticalIcon
const AlignEndVertical = TesseraIcons.AlignEndVerticalIcon
const AlignLeft = TesseraIcons.TextAlignStartIcon
const AlignRight = TesseraIcons.TextAlignEndIcon
const AlignStartVertical = TesseraIcons.AlignStartVerticalIcon
const ArrowLeftRight = TesseraIcons.ArrowLeftRightIcon
const Ban = TesseraIcons.BanIcon
const Blend = TesseraIcons.BlendIcon
const CheckIcon = TesseraIcons.CheckIcon
const Circle = TesseraIcons.CircleIcon
const Diamond = TesseraIcons.DiamondIcon
const Ellipsis = TesseraIcons.EllipsisIcon
const Grid3x3 = TesseraIcons.Grid3x3Icon
const RotateCw = TesseraIcons.RotateCwIcon
const TextCursor = TesseraIcons.TextCursorIcon

type Stop = [number, string]
type Preset = { stops: Stop[]; transparency: [number, number][] }

/** A gradient's opacity at `t` (0 to 1) from its transparency stops, as a fraction. */
function opacityAt(transparency: readonly [number, number][], t: number) {
  if (!transparency.length) return 1
  const after = transparency.findIndex(([at]) => at >= t)
  if (after <= 0) return transparency[after < 0 ? transparency.length - 1 : 0][1] / 100
  const [a, va] = transparency[after - 1]
  const [b, vb] = transparency[after]
  return (va + ((vb - va) * (t - a)) / Math.max(1e-6, b - a)) / 100
}

/** A colour with an opacity, as CSS. */
function withOpacity(hex: string, opacity: number) {
  const [r, g, b] = parseHex(hex) ?? [0, 0, 0]
  return `rgba(${r}, ${g}, ${b}, ${opacity})`
}

/** The Gradient tool's swatch: the current preset (live) or foreground to background (classic), reversed with Reverse. */
function GradientSwatch() {
  const { phrase } = useKitEditor()
  const { o, foreground, background } = useBar()
  const current = useEngineQuery<{ current: Preset }>('gradient.presets.list')?.current
  const preset: Preset =
    !o.gradientClassic && current
      ? current
      : {
          stops: [
            [0, 'foreground'],
            [1, 'background'],
          ],
          transparency: [],
        }
  const colour = (value: string) => (value === 'foreground' ? foreground : value === 'background' ? background : value)
  const stops = preset.stops.map(([at, value]) => `${withOpacity(colour(value), opacityAt(preset.transparency, at))} ${(o.reverse ? 1 - at : at) * 100}%`)
  if (o.reverse) stops.reverse()
  // Upstream's tooltips: the live swatch names where its gradient comes from; the classic one has no action behind its words.
  const title = o.gradientClassic ? phrase('Click to edit the gradient') : phrase('The current gradient (pick one in Window › Gradients)')
  return <span className="photo-gradient-swatch" style={{ background: `linear-gradient(to right, ${stops.join(', ')}), repeating-conic-gradient(var(--kit-border) 0 25%, var(--kit-surface) 0 50%) 0 0 / 8px 8px` }} title={title} aria-hidden="true" />
}

export function GradientBar() {
  const { phrase, t } = useKitEditor()
  const { o, set } = useBar()
  return (
    <>
      <Opt field="gradientType" option="gradientClassic">
        <KitChoice
          label={phrase('Gradient')}
          quiet
          value={o.gradientClassic ? 'classic' : 'live'}
          choices={[
            ['live', phrase('Gradient')],
            ['classic', phrase('Classic gradient')],
          ]}
          onChange={(value) => set({ gradientClassic: value === 'classic' })}
          testId="photo-option-gradientType"
        />
      </Opt>
      <GradientSwatch />
      <Sep />
      <Icons
        k="gradientStyle"
        label={t('options.gradientStyle')}
        choices={[
          { value: 'linear', icon: Blend, label: phrase('Linear Gradient') },
          { value: 'radial', icon: Circle, label: phrase('Radial Gradient') },
          { value: 'angle', icon: RotateCw, label: phrase('Angle Gradient') },
          { value: 'reflected', icon: ArrowLeftRight, label: phrase('Reflected Gradient') },
          { value: 'diamond', icon: Diamond, label: phrase('Diamond Gradient') },
        ]}
      />
      <Sep />
      <BlendMode k="gradientMode" />
      <Num k="fillOpacity" label={phrase('Opacity')} unit="%" />
      <Check k="reverse" label={phrase('Reverse')} />
      <Check k="dither" label={phrase('Dither')} />
    </>
  )
}

export function BucketBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <Choice
        k="fillSource"
        label={phrase('Fill')}
        choices={[
          ['foreground', phrase('Foreground')],
          ['pattern', phrase('Pattern')],
        ]}
      />
      <Num k="fillOpacity" label={phrase('Opacity')} unit="%" />
      <FillSampling allLayers={phrase('All Layers')} />
    </>
  )
}

export function CropBar() {
  const { phrase, session } = useKitEditor()
  const { o, set, crop } = useBar()
  const doc = session.doc
  // The preset's numbers, or the typed ones (upstream shows them in the ratio fields).
  const [width, height] = cropRatioSize(o.cropRatio, doc?.width ?? 0, doc?.height ?? 0) ?? [0, 0]
  const preset = CROP_RATIOS.some(([key]) => key === o.cropRatio)
  const step = Number.isInteger(width) && Number.isInteger(height) ? 1 : 0.01
  return (
    <>
      <Choice k="cropRatio" label={phrase('Ratio')} quiet choices={[...CROP_RATIOS.map(([key, label]) => [key, phrase(label)] as const), ...(preset ? [] : ([[o.cropRatio, o.cropRatio.replace(':', ' : ')]] as const))]} />
      <Opt field="width">
        <NumberField label={bare(phrase('Width'))} value={width} min={0} max={99999} step={step} onChange={(next) => set({ cropRatio: cropRatioKey(next, height) })} testId="photo-option-width" />
      </Opt>
      <Icon field="swapSize" icon={ArrowLeftRight} label={phrase('Swaps height and width')} onClick={() => set({ cropRatio: cropRatioKey(height, width) })} />
      <Opt field="height">
        <NumberField label={bare(phrase('Height'))} value={height} min={0} max={99999} step={step} onChange={(next) => set({ cropRatio: cropRatioKey(width, next) })} testId="photo-option-height" />
      </Opt>
      <Sep />
      <Act field="clear" label={phrase('Clear')} onClick={() => set({ cropRatio: '' })} />
      <Toggle k="cropOverlay" icon={Grid3x3} label={phrase('Overlay: Rule of Thirds')} />
      <Check k="deleteCropped" label={phrase('Delete Cropped Pixels')} />
      <span className="photo-options-end">
        <Icon field="cancel" icon={Ban} label={phrase('Cancel current crop operation  (Esc)')} disabled={!crop.proposed} onClick={crop.cancel} />
        <Opt field="commit">
          <IconButton icon={CheckIcon} label={fill(phrase('Commit current crop operation  ({key})'), { key: 'Enter' })} disabled={!crop.proposed} onClick={crop.commit} data-testid="photo-crop-apply" />
        </Opt>
      </span>
    </>
  )
}

const ALIGN_EDGES = [
  ['layer.align.leftEdges', AlignStartVertical, 'Align left edges'],
  ['layer.align.horizontalCenters', AlignCenterVertical, 'Align horizontal centers'],
  ['layer.align.rightEdges', AlignEndVertical, 'Align right edges'],
] as const

export function MoveBar() {
  const { phrase, run, enabled, byId } = useKitEditor()
  const commands = [...byId.values()]
  const section = (prefix: string) =>
    commands
      .filter((command) => command.id.startsWith(prefix))
      .map((command) => <MenuItem key={command.id} label={phrase(command.title)} disabled={!enabled(command.id)} onSelect={() => void run(command.id, {})} testId={`photo-option-${command.id}`} />)
  return (
    <>
      <Check k="autoSelect" label={phrase('Auto-Select:')} />
      <Choice
        k="autoSelectTarget"
        label={phrase('Auto-Select:')}
        quiet
        choices={[
          ['layer', phrase('Layer')],
          ['group', phrase('Group')],
        ]}
      />
      <Sep />
      <Check k="showTransform" label={phrase('Show Transform Controls')} />
      <Sep />
      <Opt field="align">
        <span className="kui-segmented" role="group" aria-label={phrase('Align')}>
          {ALIGN_EDGES.map(([command, icon, label]) => (
            <IconButton key={command} icon={icon} label={phrase(label)} disabled={!enabled(command)} onClick={() => void run(command, {})} data-testid={`photo-option-${command}`} />
          ))}
        </span>
      </Opt>
      <Opt field="alignDistribute">
        <MenuButton trigger={({ open, toggle }) => <IconButton icon={Ellipsis} label={phrase('Align and distribute')} pressed={open} onClick={toggle} data-testid="photo-option-alignDistribute" />}>
          <div className="photo-menu-heading">{phrase('Align')}</div>
          {section('layer.align.')}
          <MenuSeparator />
          <div className="photo-menu-heading">{phrase('Distribute')}</div>
          {section('layer.distribute.')}
        </MenuButton>
      </Opt>
      <Hint>{phrase('Drag to move the active layer')}</Hint>
    </>
  )
}

export function EyedropperBar() {
  const { phrase, t } = useKitEditor()
  return (
    <>
      <Choice k="sampleSize" label={t('options.sampleSize')} choices={[['point', t('options.pointSample')], ...(['3', '5', '11', '31', '51', '101'] as const).map((n) => [n, t('options.average', { n })] as const)]} />
      <Choice
        k="eyedropperSample"
        label={t('options.sample')}
        choices={[
          ['current', phrase('Current Layer')],
          ['currentAndBelow', phrase('Current & Below')],
          ['all', phrase('All Layers')],
          ['allNoAdjustments', t('options.allNoAdjustments')],
          ['currentAndBelowNoAdjustments', t('options.currentAndBelowNoAdjustments')],
        ]}
      />
      <Sep />
      <Hint>{fill(phrase('Click to sample the foreground colour  ·  {key}-click for background'), { key: 'Alt' })}</Hint>
    </>
  )
}

/**
 * The Type bar (upstream's `type_tool::options_bar`) through the editor's one type style
 * (`useTypeStyle`): with a type layer selected, that layer's style, each change applied to it
 * (a size drag one history step) and kept as the tool's; else, and while new text is typed (the
 * bar's changes are for it), the tool's options and the foreground colour.
 */
export function TypeBar() {
  const { phrase, t, host } = useKitEditor()
  const { tool, o, set, foreground, setForeground, layer, text } = useBar()
  const ui = useUi()
  const denied = host.readOnly || ui.state(`tool:${tool}`).denied
  // A pending draft still owns the bar: later options are for new text, never the old active layer.
  // Text edited on the canvas owns the bar: its selected range (or all its text) takes the changes.
  const style = useTypeStyle({ layer: text.layer ?? (layer?.kind === 'Type' && !text.editing ? layer.id : undefined), options: o, setOptions: set, foreground, setForeground })
  const { layer: target, values, apply } = style
  const { fonts: fontChoices, styles: styleChoices } = useFontChoices(values.font, values.fontStyle)
  return (
    <>
      <Icon field="orientation" icon={TextCursor} label={phrase('Toggle text orientation')} disabled={!target} onClick={() => target && style.setOrientation(!target.vertical)} />
      <Opt field="font" option="font">
        <KitChoice label={phrase('Font')} quiet value={values.font} choices={fontChoices} onChange={style.setFont} testId="photo-option-font" />
      </Opt>
      <Opt field="fontStyle" option="fontStyle">
        <KitChoice label={t('options.fontStyle')} quiet value={values.fontStyle} choices={styleChoices} onChange={(fontStyle) => apply({ fontStyle }, 'fontStyle')} testId="photo-option-fontStyle" />
      </Opt>
      <Opt field="fontSize" option="fontSize">
        <NumberField label={phrase('Size')} value={values.size} min={target ? 0.1 : 1} max={1296} step={target ? 0.1 : 1} unit="pt" onChange={(size, final) => apply({ size }, 'size', final)} testId="photo-option-fontSize" />
      </Opt>
      <Opt field="textAntiAlias" option="textAntiAlias">
        <KitChoice label={phrase('Anti-alias')} quiet value={style.antialias} choices={antiAliasChoices(phrase, style.antialias)} onChange={style.setAntialias} testId="photo-option-textAntiAlias" />
      </Opt>
      <Sep />
      <Opt field="textAlign" option="textAlign">
        <Segmented
          label={t('options.textAlign')}
          value={values.align}
          segments={[
            { value: 'left', icon: AlignLeft, label: phrase('Left align text') },
            { value: 'center', icon: AlignCenter, label: phrase('Center text') },
            { value: 'right', icon: AlignRight, label: phrase('Right align text') },
          ]}
          onChange={(align) => apply({ align }, 'align')}
          testId="photo-option-textAlign"
        />
      </Opt>
      <Sep />
      <Opt field="textColor">
        <ColorSwatch label={phrase('Set the text color')} value={values.color} onChange={style.setColor} testId="photo-option-textColor" />
      </Opt>
      {text.editing && !text.submitting ? (
        <span className="photo-options-end">
          <Icon field="cancel" icon={Ban} label={phrase('Cancel any current edits (Esc)')} keepFocus onClick={text.cancel} />
          <Icon field="commit" icon={CheckIcon} label={fill(phrase('Commit any current edits ({key})'), { key: t('type.commitKey') })} keepFocus disabled={denied} onClick={text.commit} />
        </span>
      ) : (
        <>
          <Sep />
          <Hint>{phrase('Click to add text')}</Hint>
        </>
      )}
    </>
  )
}

export function ShapeBar() {
  const { phrase, t } = useKitEditor()
  const { tool, o, foreground } = useBar()
  const presets = useEngineQuery<{ groups: { name: string; shapes: { name: string }[] }[] }>('shape.presets.list')
  const shapes = presets?.groups.flatMap((group) => group.shapes.map((shape) => shape.name)) ?? []
  return (
    <>
      <Opt field="shapeMode">
        <KitChoice label={phrase('Shape')} quiet value="shape" choices={[['shape', phrase('Shape')]]} onChange={() => undefined} testId="photo-option-shapeMode" />
      </Opt>
      <Sep />
      <Check k="shapeFill" label={phrase('Fill')} />
      <span className="photo-swatch photo-swatch-static" style={{ background: foreground }} aria-hidden="true" />
      <Num k="strokeWidth" label={phrase('Stroke')} unit="px" />
      {tool === 'Rectangle' ? (
        <>
          <Sep />
          <Num k="radius" label={phrase('Radius')} unit="px" />
        </>
      ) : null}
      {tool === 'Polygon' ? (
        <>
          <Sep />
          <Num k="sides" label={phrase('Sides')} />
        </>
      ) : null}
      {tool === 'Line' ? (
        <>
          <Sep />
          <Num k="weight" label={phrase('Weight')} unit="px" />
        </>
      ) : null}
      {tool === 'CustomShape' ? (
        <>
          <Sep />
          <Choice k="customShape" label={t('options.customShape')} choices={(shapes.includes(o.customShape) ? shapes : [o.customShape, ...shapes]).map((name) => [name, name] as const)} />
        </>
      ) : null}
    </>
  )
}

export function PenBar() {
  const { phrase, t } = useKitEditor()
  return (
    <>
      <Choice
        k="penMode"
        label={t('options.penMode')}
        quiet
        choices={[
          ['path', phrase('Path')],
          ['shape', phrase('Shape')],
        ]}
      />
      <Sep />
      <Hint>{fill(phrase('Click: corner · Drag: smooth · Click first point: close · {key} finish · Esc cancel'), { key: 'Enter' })}</Hint>
    </>
  )
}

export function PathSelectionBar() {
  const { phrase } = useKitEditor()
  const { tool } = useBar()
  if (tool === 'DirectSelection') return <Hint>{fill(phrase('Click: select anchor · Drag: move anchor, handle or segment · {shift}-click: add · {alt}-click: whole subpath'), { shift: 'Shift', alt: 'Alt' })}</Hint>
  return <Hint>{phrase("Drag to move the active shape's path or the Work Path")}</Hint>
}

/** Hand and Zoom: Fit Screen and 100 % (Zoom adds Scrubby Zoom first, as upstream). */
export function ViewBar() {
  const { phrase } = useKitEditor()
  const { tool, view } = useBar()
  return (
    <>
      {tool === 'Zoom' ? (
        <>
          <Check k="scrubbyZoom" label={phrase('Scrubby Zoom')} />
          <Hint>{fill(phrase('Click to zoom in  ·  {key}-click to zoom out  ·  drag right/left to zoom in/out'), { key: 'Alt' })}</Hint>
        </>
      ) : (
        <Hint>{phrase('Drag to pan  ·  hold Space with any tool')}</Hint>
      )}
      <Act field="fitScreen" label={phrase('Fit Screen')} onClick={view.fit} />
      <Act field="actualPixels" label="100%" onClick={view.actual} />
    </>
  )
}

/** The Ruler, Count, Note, and Slice tools (upstream's `analysis_ui` and `slice_ui` bars, without their read-outs and groups). */
export function MeasureBar() {
  const { phrase, run, enabled } = useKitEditor()
  const { tool, o, set } = useBar()
  const button = (field: 'straightenLayer' | 'clear' | 'clearAll' | 'slicesFromGuides', label: string, command: string, params: Record<string, unknown> = {}) => (
    <Act field={field} label={label} disabled={!enabled(command)} onClick={() => void run(command, params)} />
  )
  switch (tool) {
    case 'Ruler':
      return (
        <>
          {button('straightenLayer', phrase('Straighten Layer'), 'image.analysis.straightenLayer')}
          {button('clear', phrase('Clear'), 'image.analysis.rulerTool', { clear: true })}
        </>
      )
    case 'Count':
      return button('clear', phrase('Clear'), 'count.clear')
    case 'Note':
      return (
        <>
          <Opt field="noteAuthor" option="noteAuthor">
            <label className="kui-choice">
              <span>{phrase('Author:')}</span>
              <input className="kui-field photo-note-author" value={o.noteAuthor} onChange={(event) => set({ noteAuthor: event.target.value })} aria-label={bare(phrase('Author:'))} data-testid="photo-option-noteAuthor" />
            </label>
          </Opt>
          <Opt field="noteColor" option="noteColor">
            <span className="photo-options-label">{phrase('Color:')}</span>
            <label className="photo-swatch" style={{ background: o.noteColor }}>
              <input type="color" value={o.noteColor} onChange={(event) => set({ noteColor: event.target.value })} aria-label={bare(phrase('Color:'))} data-testid="photo-option-noteColor" />
            </label>
          </Opt>
          {button('clearAll', phrase('Clear All'), 'notes.delete', { all: true })}
        </>
      )
    case 'Slice':
      return (
        <>
          <Hint>{phrase('Style: Normal')}</Hint>
          <Sep />
          {button('slicesFromGuides', phrase('Slices From Guides'), 'slice.fromGuides')}
        </>
      )
    default:
      return null
  }
}
