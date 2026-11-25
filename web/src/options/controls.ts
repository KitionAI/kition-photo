import type { OptionKey } from '../tools/options'

/**
 * The options bar's fields as Kit UI elements: `control:options.<field>`, one id per meaning shared
 * by every tool that shows it, so a host hides "Opacity" for every tool at once. A label is
 * upstream's English label (drawn through `phrase()`) or, where upstream has none without a colon
 * or with the right sense, a key of Photo's own strings.
 */
export const OPTION_CONTROLS = {
  size: 'Size',
  hardness: 'Hardness',
  mode: 'Mode',
  opacity: 'Opacity',
  flow: 'Flow',
  smoothing: 'Smoothing',
  pressureOpacity: 'Pressure for Opacity',
  pressureSize: 'Pressure for Size',
  autoErase: 'Auto Erase',
  wet: 'Wet',
  load: 'Load',
  mix: 'Mix',
  sampleAllLayers: 'Sample All Layers',
  selectionMode: 'options.selectionMode',
  feather: 'Feather',
  antiAlias: 'Anti-alias',
  style: 'Style',
  width: 'Width',
  height: 'Height',
  swapSize: 'Swaps height and width',
  refineSelection: 'Refine Selection…',
  contrast: 'Contrast',
  frequency: 'Frequency',
  tolerance: 'Tolerance',
  contiguous: 'Contiguous',
  selectSubject: 'Select Subject',
  enhanceEdge: 'Enhance Edge',
  fill: 'Fill',
  gradientType: 'options.gradientType',
  gradientStyle: 'options.gradientStyle',
  reverse: 'Reverse',
  dither: 'Dither',
  ratio: 'Ratio',
  clear: 'Clear',
  deleteCropped: 'Delete Cropped Pixels',
  commit: 'options.commit',
  cancel: 'cancel',
  autoSelect: 'options.autoSelect',
  autoSelectTarget: 'options.autoSelectTarget',
  showTransform: 'Show Transform Controls',
  align: 'Align',
  alignDistribute: 'Align and distribute',
  sampleSize: 'options.sampleSize',
  sample: 'options.sample',
  orientation: 'Toggle text orientation',
  font: 'Font',
  fontStyle: 'options.fontStyle',
  fontSize: 'fontSize',
  textAntiAlias: 'options.textAntiAlias',
  textAlign: 'options.textAlign',
  textColor: 'Text color',
  shapeMode: 'Shape',
  stroke: 'Stroke',
  radius: 'Radius',
  sides: 'Sides',
  weight: 'Weight',
  customShape: 'options.customShape',
  penMode: 'options.penMode',
  strength: 'options.strength',
  protectDetail: 'Protect Detail',
  fingerPainting: 'Finger Painting',
  range: 'options.range',
  exposure: 'Exposure',
  protectTones: 'Protect Tones',
  vibrance: 'Vibrance',
  aligned: 'Aligned',
  spotType: 'options.spotType',
  patch: 'options.patch',
  structure: 'options.structure',
  colorAdapt: 'options.colorAdapt',
  sampling: 'options.sampling',
  limits: 'options.limits',
  protectForeground: 'Protect Foreground Color',
  spongeMode: 'options.spongeMode',
  camMode: 'options.camMode',
  magneticWidth: 'options.magneticWidth',
  bgTolerance: 'options.bgTolerance',
  clearAll: 'Clear All',
  ruleOfThirds: 'Overlay: Rule of Thirds',
  noteAuthor: 'options.noteAuthor',
  noteColor: 'options.noteColor',
  scrubbyZoom: 'Scrubby Zoom',
  fitScreen: 'Fit Screen',
  actualPixels: '100%',
  straightenLayer: 'Straighten Layer',
  slicesFromGuides: 'Slices From Guides',
  // Free Transform's bar (plan 2026-11-24, stage 2).
  transformX: 'transform.x',
  transformY: 'transform.y',
  transformW: 'transform.w',
  transformH: 'transform.h',
  transformAngle: 'transform.angle',
} as const

export type OptionControl = keyof typeof OPTION_CONTROLS

/**
 * The field an option is drawn as, where its name differs: the Mixer Brush's flow is a Flow field,
 * the fills' opacity an Opacity field, so hiding one hides it for every tool.
 */
export const CONTROL_OF: Partial<Record<OptionKey, OptionControl>> = {
  mixerFlow: 'flow',
  mixerSampleAll: 'sampleAllLayers',
  gradientClassic: 'gradientType',
  gradientMode: 'mode',
  fillOpacity: 'opacity',
  eraserOpacity: 'opacity',
  fillSource: 'fill',
  shapeFill: 'fill',
  strokeWidth: 'stroke',
  cloneSample: 'sample',
  eyedropperSample: 'sample',
  patchMode: 'patch',
  marqueeStyle: 'style',
  marqueeWidth: 'width',
  marqueeHeight: 'height',
  cropRatio: 'ratio',
  cropOverlay: 'ruleOfThirds',
  bgSampling: 'sampling',
  bgLimits: 'limits',
}

/** The field option `key` is drawn as. */
export const controlOf = (key: OptionKey): OptionControl => CONTROL_OF[key] ?? (key as OptionControl)

/** The Kit UI id of an options-bar field. */
export const optionId = (field: OptionControl) => `control:options.${field}`

/** Every options-bar field, for `defineKitUi`. */
export const OPTION_ELEMENTS = (Object.entries(OPTION_CONTROLS) as [OptionControl, string][]).map(([field, label]) => ({ id: optionId(field), label }))
