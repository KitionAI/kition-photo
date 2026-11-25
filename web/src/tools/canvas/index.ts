import { HAND_TOOL, type KitTool } from '@kition/sdk/editor'
import { toolFor, type PhotoToolId } from '../catalog'
import { toolParams } from '../options'
import type { PhotoToolEnv } from './common'
import { brushTool, clickFill, gradientTool, quickSelectionTool, withAltPick } from './paint'
import { magicWandTool, marqueeTool, objectSelectionTool } from './select'
import { clickLassoTool, lassoTool } from './lasso'
import { selectionDragTool, sliceSelectTool, sliceTool } from './transformLike'
import { cropTool } from './crop'
import { moveTool } from './move'
import { directSelectionTool, lineTool, pathSelectionTool, penTool, shapeTool } from './shape'
import { typeTool } from './type'
import { eyedropperTool, markTool, rulerTool, zoomTool } from './view'

export type { CropFrame, PendingClicks, PhotoToolEnv, TransformBox } from './common'
export { drawCropProposal, drawPathOutline, drawSelectionAnts, drawTransformBox, type PathData } from './common'
export { transformTool } from './move'
export { forgetStrokeEnd } from './paint'
export { ALT_PICKS, picksColour } from '../altPicks'

type Factory = (env: PhotoToolEnv) => KitTool

/** A brush-like tool: the shared brush factory, running the tool's command. */
const brush =
  (id: PhotoToolId): Factory =>
  (env) =>
    brushTool(id, toolFor(id)!.command, env)

/** Every tool's canvas behaviour, by id; the record's type makes it name every tool. */
const FACTORIES: Record<PhotoToolId, Factory> = {
  Move: moveTool,
  RectMarquee: (env) => marqueeTool('RectMarquee', false, env),
  EllipseMarquee: (env) => marqueeTool('EllipseMarquee', true, env),
  Lasso: lassoTool,
  PolygonLasso: (env) => clickLassoTool('PolygonLasso', env),
  MagneticLasso: (env) => clickLassoTool('MagneticLasso', env),
  ObjectSelection: objectSelectionTool,
  QuickSelection: quickSelectionTool,
  MagicWand: magicWandTool,
  Crop: cropTool,
  Slice: sliceTool,
  SliceSelect: sliceSelectTool,
  Eyedropper: eyedropperTool,
  Ruler: rulerTool,
  Note: (env) => markTool('Note', env),
  Count: (env) => markTool('Count', env),
  SpotHealing: brush('SpotHealing'),
  Healing: brush('Healing'),
  Patch: (env) => selectionDragTool('Patch', env),
  SmartMove: (env) => selectionDragTool('SmartMove', env),
  Brush: brush('Brush'),
  Pencil: brush('Pencil'),
  MixerBrush: brush('MixerBrush'),
  CloneStamp: brush('CloneStamp'),
  HistoryBrush: brush('HistoryBrush'),
  Eraser: brush('Eraser'),
  BackgroundEraser: brush('BackgroundEraser'),
  MagicEraser: (env) => clickFill('MagicEraser', 'paint.magicEraser', () => toolParams('MagicEraser', env.options), env),
  Gradient: (env) => withAltPick('Gradient', gradientTool(env), env),
  PaintBucket: (env) => withAltPick('PaintBucket', clickFill('PaintBucket', 'paint.bucket', () => ({ ...toolParams('PaintBucket', env.options), color: env.foreground }), env), env),
  Blur: brush('Blur'),
  Sharpen: brush('Sharpen'),
  Smudge: brush('Smudge'),
  Dodge: brush('Dodge'),
  Burn: brush('Burn'),
  Sponge: brush('Sponge'),
  Pen: penTool,
  Type: (env) => typeTool('Type', env),
  VerticalType: (env) => typeTool('VerticalType', env),
  PathSelection: pathSelectionTool,
  DirectSelection: directSelectionTool,
  Rectangle: (env) => shapeTool('Rectangle', env),
  EllipseShape: (env) => shapeTool('EllipseShape', env),
  Triangle: (env) => shapeTool('Triangle', env),
  Polygon: (env) => shapeTool('Polygon', env),
  Line: lineTool,
  CustomShape: (env) => shapeTool('CustomShape', env),
  Hand: () => HAND_TOOL,
  Zoom: zoomTool,
}

/** The canvas tool for a Photo tool. */
export function photoCanvasTool(id: PhotoToolId, env: PhotoToolEnv): KitTool {
  return FACTORIES[id](env)
}
