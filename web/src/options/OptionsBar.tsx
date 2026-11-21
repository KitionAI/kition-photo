import type { ComponentType } from 'react'
import { BarSeparator, KitElement, useKitEditor } from '@kition/sdk/editor'
import { toolFor, type PhotoToolId } from '../tools/catalog'
import type { BarProps } from './bar'
import { BackgroundEraserBar, BrushBar, MagicEraserBar, MixerBar, PencilBar, RetouchBar } from './brushBars'
import { LassoBar, MagicWandBar, MarqueeBar, ObjectSelectionBar, QuickSelectionBar } from './selectionBars'
import { BarProvider } from './fields'
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
}

/** Photoshop's options bar (Kit UI `control:options`): the active tool, then its options as upstream draws them. */
export function OptionsBar(props: BarProps) {
  const { phrase } = useKitEditor()
  const tool = toolFor(props.tool)
  const Bar = BARS[props.tool]
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
