import type { ComponentType } from 'react'
import { BarSeparator, IconButton, KitElement, NumberField, TesseraIcons, useKitEditor } from '@kition/sdk/editor'
import { toolFor, type PhotoToolId } from '../tools/catalog'
import type { BarProps } from './bar'
import { BackgroundEraserBar, BrushBar, MagicEraserBar, MixerBar, PencilBar, RetouchBar } from './brushBars'
import { LassoBar, MagicWandBar, MarqueeBar, ObjectSelectionBar, QuickSelectionBar } from './selectionBars'
import { BarProvider, Opt, bare } from './fields'
import { ColorReplacementBar, ConvertPointBar, PatternStampBar, RedEyeBar } from './addedBars'
import { BucketBar, CropBar, EyedropperBar, GradientBar, MeasureBar, MoveBar, PathSelectionBar, PenBar, ShapeBar, TypeBar, ViewBar } from './otherBars'

type Bar = ComponentType

/** Each tool's bar: one per arm of upstream's `options_bar`. */
const BARS: Record<PhotoToolId, Bar> = {
  Move: MoveBar,
  RectMarquee: MarqueeBar,
  EllipseMarquee: MarqueeBar,
  Lasso: LassoBar,
  PolygonLasso: LassoBar,
  MagneticLasso: LassoBar,
  ObjectSelection: ObjectSelectionBar,
  QuickSelection: QuickSelectionBar,
  MagicWand: MagicWandBar,
  Crop: CropBar,
  Slice: MeasureBar,
  SliceSelect: () => null,
  Eyedropper: EyedropperBar,
  Ruler: MeasureBar,
  Note: MeasureBar,
  Count: MeasureBar,
  SpotHealing: RetouchBar,
  Healing: RetouchBar,
  Patch: RetouchBar,
  SmartMove: RetouchBar,
  Brush: BrushBar,
  Pencil: PencilBar,
  MixerBrush: MixerBar,
  CloneStamp: RetouchBar,
  HistoryBrush: RetouchBar,
  Eraser: BrushBar,
  BackgroundEraser: BackgroundEraserBar,
  MagicEraser: MagicEraserBar,
  Gradient: GradientBar,
  PaintBucket: BucketBar,
  Blur: RetouchBar,
  Sharpen: RetouchBar,
  Smudge: RetouchBar,
  Dodge: RetouchBar,
  Burn: RetouchBar,
  Sponge: RetouchBar,
  Pen: PenBar,
  Type: TypeBar,
  VerticalType: TypeBar,
  PathSelection: PathSelectionBar,
  DirectSelection: PathSelectionBar,
  Rectangle: ShapeBar,
  EllipseShape: ShapeBar,
  Triangle: ShapeBar,
  Polygon: ShapeBar,
  Line: ShapeBar,
  CustomShape: ShapeBar,
  Hand: ViewBar,
  Zoom: ViewBar,
  // The tools added in plan 2026-11-24, stage 7.
  SingleRowMarquee: LassoBar,
  SingleColumnMarquee: LassoBar,
  RedEye: RedEyeBar,
  ColorReplacement: ColorReplacementBar,
  PatternStamp: PatternStampBar,
  ConvertPoint: ConvertPointBar,
}

/**
 * Free Transform's bar while its box is open (plan 2026-11-24, stage 2): the reference point's X and
 * Y, W and H in % of the layer's, the angle, and Cancel and Commit (Escape, Enter).
 */
function TransformBar({ transform }: { transform: NonNullable<BarProps['transform']> }) {
  const { t } = useKitEditor()
  const { fields, set } = transform
  const round = (value: number) => Math.round(value * 100) / 100
  const field = (key: 'x' | 'y' | 'w' | 'h' | 'angle', control: 'transformX' | 'transformY' | 'transformW' | 'transformH' | 'transformAngle', unit: string, min: number, max: number) => (
    <Opt field={control}>
      <NumberField label={bare(t(`transform.${key}`))} value={round(fields[key])} min={min} max={max} step={key === 'angle' ? 0.1 : 1} unit={unit} onChange={(value) => set({ [key]: value })} testId={`photo-transform-${key}`} />
    </Opt>
  )
  return (
    <>
      {field('x', 'transformX', 'px', -300000, 300000)}
      {field('y', 'transformY', 'px', -300000, 300000)}
      <BarSeparator />
      {field('w', 'transformW', '%', 0.1, 100000)}
      {field('h', 'transformH', '%', 0.1, 100000)}
      <BarSeparator />
      {field('angle', 'transformAngle', '°', -180, 180)}
      <span className="photo-options-end">
        <Opt field="cancel">
          <IconButton icon={TesseraIcons.BanIcon} label={t('transform.cancel')} onClick={transform.cancel} data-testid="photo-transform-cancel" />
        </Opt>
        <Opt field="commit">
          <IconButton icon={TesseraIcons.CheckIcon} label={t('transform.commit')} onClick={transform.commit} data-testid="photo-transform-commit" />
        </Opt>
      </span>
    </>
  )
}

/** Photoshop's options bar (Kit UI `control:options`): the active tool, then its options as upstream draws them (Free Transform's while its box is open). */
export function OptionsBar(props: BarProps) {
  const { phrase } = useKitEditor()
  const tool = toolFor(props.tool)
  const Bar = BARS[props.tool]
  if (props.transform) {
    return (
      <KitElement id="control:options">
        <div className="photo-options" data-testid="photo-options-bar" data-transform="">
          <strong className="photo-options-tool" data-testid="photo-options-tool" data-tool="FreeTransform">
            {phrase('Free Transform')}
          </strong>
          <BarSeparator />
          <TransformBar transform={props.transform} />
        </div>
      </KitElement>
    )
  }
  return (
    <KitElement id="control:options">
      <div className="photo-options" data-testid="photo-options-bar">
        <strong className="photo-options-tool" data-testid="photo-options-tool" data-tool={props.tool}>
          {tool ? <tool.icon size={16} aria-hidden="true" /> : null}
          {tool ? phrase(tool.label) : props.tool}
        </strong>
        <BarSeparator />
        <BarProvider value={props}>
          <Bar />
        </BarProvider>
      </div>
    </KitElement>
  )
}
