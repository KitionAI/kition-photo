// Icons straight from Tessera Icons: plain-Node tests (`test:parity`) import this catalog, and
// kit-editor's entry does not load under plain Node.
import {
  ArrowLeftRightIcon as ArrowLeftRight,
  BandageIcon as Bandage,
  BlendIcon as Blend,
  BrushIcon as Brush,
  BrushCleaningIcon as BrushCleaning,
  CircleIcon as Circle,
  CircleDashedIcon as CircleDashed,
  CircleDotIcon as CircleDot,
  CloudIcon as Cloud,
  CropIcon as Crop,
  DropletIcon as Droplet,
  EraserIcon as Eraser,
  FlameIcon as Flame,
  HandIcon as Hand,
  RotateCcwClockIcon as History,
  LassoIcon as Lasso,
  LassoSelectIcon as LassoSelect,
  LollipopIcon as Lollipop,
  MagnetIcon as Magnet,
  MessageSquareIcon as MessageSquare,
  MousePointerIcon as MousePointer,
  MousePointer2Icon as MousePointer2,
  MoveIcon as Move,
  PaintBucketIcon as PaintBucket,
  PaletteIcon as Palette,
  PenToolIcon as PenTool,
  PencilIcon as Pencil,
  PentagonIcon as Pentagon,
  PipetteIcon as Pipette,
  PointerIcon as Pointer,
  RectangleHorizontalIcon as RectangleHorizontal,
  RulerIcon as Ruler,
  ShapesIcon as Shapes,
  SlashIcon as Slash,
  SliceIcon as Slice,
  SparkleIcon as Sparkle,
  SparklesIcon as Sparkles,
  SquareDashedIcon as SquareDashed,
  SquareDashedMousePointerIcon as SquareDashedMousePointer,
  SquareMousePointerIcon as SquareMousePointer,
  StampIcon as Stamp,
  TriangleIcon as Triangle,
  TypeIcon as Type,
  WandSparklesIcon as WandSparkles,
  ZoomInIcon as ZoomIn,
  type IconComponent,
} from '@kition/tessera/icons/react'

/** The `command` of a tool that only moves the view (Hand, Zoom) and runs no engine command. */
export const VIEW = 'view'

/**
 * A tool on Photo's rail, as upstream's `TOOL_SECTIONS` places it: `section` (between hairlines),
 * `slot` within the section, and `group`, the rail-wide slot its flyout shares. `key` is upstream's
 * letter (none for Blur's slot), `label` upstream's own English label (drawn through `phrase()`,
 * upstream's translations), and `command` the engine command it drives (or `VIEW`). `brush` tools
 * take the brush size from the options bar.
 */
export type PhotoTool = {
  id: PhotoToolId
  section: number
  slot: number
  group: number
  key: string | null
  label: string
  icon: IconComponent
  command: string
  brush?: boolean
}

type Entry = readonly [id: string, key: string | null, icon: IconComponent, command: string, brush?: 'brush']

/** Upstream's sections of slots of tools, in its order. */
const SECTIONS = [
  [[['Move', 'V', Move, 'layer.translate']]],
  [
    [
      ['RectMarquee', 'M', SquareDashed, 'select.rect'],
      ['EllipseMarquee', 'M', CircleDashed, 'select.rect'],
    ],
    [
      ['Lasso', 'L', Lasso, 'select.lasso'],
      ['PolygonLasso', 'L', Pentagon, 'select.lasso'],
      ['MagneticLasso', 'L', Magnet, 'select.magneticLasso'],
    ],
    [
      ['ObjectSelection', 'W', SquareDashedMousePointer, 'select.object'],
      ['QuickSelection', 'W', Sparkles, 'select.quick', 'brush'],
      ['MagicWand', 'W', WandSparkles, 'select.magicWand'],
    ],
    [
      ['Crop', 'C', Crop, 'image.crop'],
      ['Slice', 'C', Slice, 'slice.new'],
      ['SliceSelect', 'C', SquareMousePointer, 'slice.set'],
    ],
    [
      ['Eyedropper', 'I', Pipette, 'file.sampleColor'],
      ['Ruler', 'I', Ruler, 'image.analysis.rulerTool'],
      ['Note', 'I', MessageSquare, 'note.add'],
      ['Count', 'I', CircleDot, 'count.add'],
    ],
  ],
  [
    [
      ['SpotHealing', 'J', Bandage, 'paint.spotHealing', 'brush'],
      ['Healing', 'J', Bandage, 'paint.healingBrush', 'brush'],
      ['Patch', 'J', LassoSelect, 'paint.patch'],
      ['SmartMove', 'J', ArrowLeftRight, 'paint.smartMove'],
    ],
    [
      ['Brush', 'B', Brush, 'paint.stroke', 'brush'],
      ['Pencil', 'B', Pencil, 'paint.pencil', 'brush'],
      ['MixerBrush', 'B', Palette, 'paint.mixerBrush', 'brush'],
    ],
    [['CloneStamp', 'S', Stamp, 'paint.cloneStamp', 'brush']],
    [['HistoryBrush', 'Y', History, 'paint.historyBrush', 'brush']],
    [
      ['Eraser', 'E', Eraser, 'paint.stroke', 'brush'],
      ['BackgroundEraser', 'E', BrushCleaning, 'paint.backgroundEraser', 'brush'],
      ['MagicEraser', 'E', Sparkle, 'paint.magicEraser'],
    ],
    [
      ['Gradient', 'G', Blend, 'paint.gradient'],
      ['PaintBucket', 'G', PaintBucket, 'paint.bucket'],
    ],
    [
      ['Blur', null, Droplet, 'paint.blur', 'brush'],
      ['Sharpen', null, Triangle, 'paint.sharpen', 'brush'],
      ['Smudge', null, Pointer, 'paint.smudge', 'brush'],
    ],
    [
      ['Dodge', 'O', Lollipop, 'paint.dodge', 'brush'],
      ['Burn', 'O', Flame, 'paint.burn', 'brush'],
      ['Sponge', 'O', Cloud, 'paint.sponge', 'brush'],
    ],
  ],
  [
    [['Pen', 'P', PenTool, 'path.set']],
    [
      ['Type', 'T', Type, 'text.add'],
      ['VerticalType', 'T', Type, 'text.add'],
    ],
    [
      ['PathSelection', 'A', MousePointer2, 'path.transform'],
      ['DirectSelection', 'A', MousePointer, 'path.moveAnchors'],
    ],
    [
      ['Rectangle', 'U', RectangleHorizontal, 'shape.add'],
      ['EllipseShape', 'U', Circle, 'shape.add'],
      ['Triangle', 'U', Triangle, 'shape.add'],
      ['Polygon', 'U', Pentagon, 'shape.add'],
      ['Line', 'U', Slash, 'shape.add'],
      ['CustomShape', 'U', Shapes, 'shape.presets.place'],
    ],
  ],
  [[['Hand', 'H', Hand, VIEW]], [['Zoom', 'Z', ZoomIn, VIEW]]],
] as const satisfies readonly (readonly (readonly Entry[])[])[]

export type PhotoToolId = (typeof SECTIONS)[number][number][number][0]

/** Upstream's labels (`Tool::label`). */
const LABELS: Record<PhotoToolId, string> = {
  Move: 'Move Tool',
  RectMarquee: 'Rectangular Marquee Tool',
  EllipseMarquee: 'Elliptical Marquee Tool',
  Lasso: 'Lasso Tool',
  PolygonLasso: 'Polygonal Lasso Tool',
  MagneticLasso: 'Magnetic Lasso Tool',
  ObjectSelection: 'Object Selection Tool',
  QuickSelection: 'Quick Selection Tool',
  MagicWand: 'Magic Wand Tool',
  Crop: 'Crop Tool',
  Slice: 'Slice Tool',
  SliceSelect: 'Slice Select Tool',
  Eyedropper: 'Eyedropper Tool',
  Ruler: 'Ruler Tool',
  Note: 'Note Tool',
  Count: 'Count Tool',
  SpotHealing: 'Spot Healing Brush Tool',
  Healing: 'Healing Brush Tool',
  Patch: 'Patch Tool',
  SmartMove: 'Smart Move Tool',
  Brush: 'Brush Tool',
  Pencil: 'Pencil Tool',
  MixerBrush: 'Mixer Brush Tool',
  CloneStamp: 'Clone Stamp Tool',
  HistoryBrush: 'History Brush Tool',
  Eraser: 'Eraser Tool',
  BackgroundEraser: 'Background Eraser Tool',
  MagicEraser: 'Magic Eraser Tool',
  Gradient: 'Gradient Tool',
  PaintBucket: 'Paint Bucket Tool',
  Blur: 'Blur Tool',
  Sharpen: 'Sharpen Tool',
  Smudge: 'Smudge Tool',
  Dodge: 'Dodge Tool',
  Burn: 'Burn Tool',
  Sponge: 'Sponge Tool',
  Pen: 'Pen Tool',
  Type: 'Horizontal Type Tool',
  VerticalType: 'Vertical Type Tool',
  PathSelection: 'Path Selection Tool',
  DirectSelection: 'Direct Selection Tool',
  Rectangle: 'Rectangle Tool',
  EllipseShape: 'Ellipse Tool',
  Triangle: 'Triangle Tool',
  Polygon: 'Polygon Tool',
  Line: 'Line Tool',
  CustomShape: 'Custom Shape Tool',
  Hand: 'Hand Tool',
  Zoom: 'Zoom Tool',
}

/** Upstream's 49 tools, in its order. */
export const PHOTO_TOOLS: readonly PhotoTool[] = (SECTIONS as readonly (readonly (readonly Entry[])[])[]).flatMap((slots, section) =>
  slots.flatMap((tools, slot) =>
    tools.map(([name, key, icon, command, brush]) => {
      const id = name as PhotoToolId
      return { id, section, slot, group: section * 10 + slot, key, label: LABELS[id], icon, command, brush: brush === 'brush' }
    }),
  ),
)

export function toolFor(id: string) {
  return PHOTO_TOOLS.find((tool) => tool.id === id)
}

/** Whether `key` (lower case) is a tool's letter. */
export function isToolKey(key: string) {
  return PHOTO_TOOLS.some((tool) => tool.key?.toLowerCase() === key)
}

/**
 * The tool a letter key chooses, or null, as upstream (`shortcuts.rs`): from another slot, the
 * slot's first tool; in the active tool's slot, the slot's next tool. By default Shift makes no
 * difference; with Preferences › Tools › Use Shift Key for Tool Switch (`shiftSwitch`), only Shift
 * and the key cycles and the key alone keeps the active tool. A tool `allowed` refuses is passed over.
 */
export function toolForKey(key: string, shift: boolean, shiftSwitch: boolean, current: string, allowed: (id: string) => boolean) {
  const slot = PHOTO_TOOLS.filter((tool) => tool.key?.toLowerCase() === key)
  const index = slot.findIndex((tool) => tool.id === current)
  if (index < 0) return slot.find((tool) => allowed(tool.id))?.id ?? null
  if (shiftSwitch && !shift) return current
  for (let step = 1; step < slot.length; step += 1) {
    const next = slot[(index + step) % slot.length].id
    if (allowed(next)) return next
  }
  return null
}
