import { useState } from 'react'
import { Button, Checkbox, Choice, Hint, NumberField, toast, useEngineQuery, useHostPick, useKitEditor } from '@kition/kit-editor'
import type { PhotoLayer } from '../photo'
import { useGestures } from './gesture'
import { ColorSwatch, Row } from './PropsParts'

/**
 * An adjustment layer's settings in the Properties panel (upstream `adjust_editors::layer_editor`
 * and `adjust_ui`'s Selective Color and Color Lookup editors), as slider rows, drop-downs, and
 * check boxes: each change runs `layer.setAdjustment` with only what changed (the engine merges it
 * over the layer's settings), one history step per gesture. Upstream's graphical editors (the
 * Levels histogram and sliders, the Curves graph, the hue-range bar, gradient stops) come in a
 * later version; Levels shows its composite's five numbers meanwhile.
 */

type Params = Record<string, unknown>
type Set = (patch: Params, field: string, final: boolean) => void

const num = (value: unknown, fallback: number) => (typeof value === 'number' && Number.isFinite(value) ? value : fallback)
const nums = (value: unknown, fallback: readonly number[]) => fallback.map((item, index) => num(Array.isArray(value) ? value[index] : undefined, item))
const obj = (value: unknown): Params => (value && typeof value === 'object' && !Array.isArray(value) ? (value as Params) : {})
const flag = (value: unknown, fallback: boolean) => (typeof value === 'boolean' ? value : fallback)

/** The hue ranges (upstream `RANGE_LABELS`), and Selective Color's, which adds the tones. */
const HUE_RANGES = [
  ['reds', 'Reds'],
  ['yellows', 'Yellows'],
  ['greens', 'Greens'],
  ['cyans', 'Cyans'],
  ['blues', 'Blues'],
  ['magentas', 'Magentas'],
] as const
const COLOR_RANGES = [...HUE_RANGES, ['whites', 'Whites'], ['neutrals', 'Neutrals'], ['blacks', 'Blacks']] as const

/** Black & White's default weights (`adjust_params::BW_DEFAULTS`). */
const BW_DEFAULTS = [40, 60, 40, 60, 20, 80]

/** Photo Filter's presets (`adjust_params::PHOTO_FILTERS`): id, label, and colour. */
export const PHOTO_FILTERS = [
  ['warming85', 'Warming Filter (85)', '#ec8a00'],
  ['warmingLBA', 'Warming Filter (LBA)', '#fa9600'],
  ['warming81', 'Warming Filter (81)', '#ebb113'],
  ['cooling80', 'Cooling Filter (80)', '#006dff'],
  ['coolingLBB', 'Cooling Filter (LBB)', '#005dff'],
  ['cooling82', 'Cooling Filter (82)', '#00b5ff'],
  ['red', 'Red', '#ea1a1a'],
  ['orange', 'Orange', '#f38417'],
  ['yellow', 'Yellow', '#f9e31c'],
  ['green', 'Green', '#19c919'],
  ['cyan', 'Cyan', '#1dcbea'],
  ['blue', 'Blue', '#1d35ea'],
  ['violet', 'Violet', '#9b1dea'],
  ['magenta', 'Magenta', '#e318e3'],
  ['sepia', 'Sepia', '#ac7a33'],
  ['deepRed', 'Deep Red', '#ff0000'],
  ['deepBlue', 'Deep Blue', '#0022cd'],
  ['deepEmerald', 'Deep Emerald', '#008c00'],
  ['deepYellow', 'Deep Yellow', '#ffd500'],
  ['underwater', 'Underwater', '#00c2b1'],
] as const

type Stop = [number, string]

/** Gradient Map's presets (upstream `gradient_map`); Foreground to Background takes the editor's colours. */
function gradientPresets(foreground: string, background: string): (readonly [string, Stop[]])[] {
  return [
    ['Black, White', [[0, '#000000'], [1, '#ffffff']]],
    ['Foreground to Background', [[0, foreground], [1, background]]],
    ['Violet, Orange', [[0, '#290a59'], [1, '#ff7c00']]],
    ['Blue, Red, Yellow', [[0, '#0a00b2'], [0.5, '#ff0000'], [1, '#fffc00']]],
    ['Copper', [[0, '#97461a'], [0.4, '#fbd8c5'], [0.7, '#6c2e16'], [1, '#efdbcd']]],
  ]
}

/** A CSS gradient through `stops`, reversed with Reverse. */
export function stopsCss(stops: readonly Stop[], reverse: boolean) {
  const sorted = [...stops].sort((a, b) => a[0] - b[0]).map(([at, color]) => [reverse ? 1 - at : at, color] as const)
  if (reverse) sorted.reverse()
  return `linear-gradient(to right, ${sorted.map(([at, color]) => `${color} ${Math.round(at * 1000) / 10}%`).join(', ')})`
}

/**
 * A numeric setting: a slider row (upstream's `slider_row`), or with `track={false}` a bare
 * scrubbable field (upstream's `value_field`, as Levels' numbers); a drag is one history step.
 */
function Field({ field, label, value, min, max, step = 1, unit, track = true, set, patch }: { field: string; label: string; value: number; min: number; max: number; step?: number; unit?: string; track?: boolean; set: Set; patch: (value: number) => Params }) {
  return <NumberField label={label} value={value} min={min} max={max} step={step} unit={unit} track={track} onChange={(next, final) => set(patch(next), field, final)} testId={`photo-adjustment-${field}`} />
}

/** A colour setting: one picker session is one history step. */
function Swatch({ field, label, value, disabled, set, patch }: { field: string; label: string; value: string; disabled?: boolean; set: Set; patch: (value: string) => Params }) {
  return <ColorSwatch label={label} value={value} disabled={disabled} onChange={(next, final) => set(patch(next), field, final)} testId={`photo-adjustment-${field}`} />
}

export function AdjustmentSettings({ layer, foreground, background }: { layer: PhotoLayer; foreground: string; background: string }) {
  const { run, phrase, t } = useKitEditor()
  const gestures = useGestures(`adjustment-${layer.id}`)
  const [range, setRange] = useState(0)
  const [tone, setTone] = useState('midtones')
  const [output, setOutput] = useState(0)
  const [selective, setSelective] = useState(0)
  const [useColor, setUseColor] = useState<boolean | null>(null)
  const kind = layer.adjustment?.kind ?? ''
  const p = layer.adjustment?.params ?? {}
  /** Runs one change; a gesture's changes (and a picker's ending, `{}`) share its key. */
  const set: Set = (patch, field, final) => {
    const coalesce = gestures.key(field, final)
    if (Object.keys(patch).length) void run('layer.setAdjustment', { layer: layer.id, ...patch, coalesce })
  }
  const check = (field: string, label: string, checked: boolean, patch: (on: boolean) => Params) => (
    <Checkbox label={label} checked={checked} onChange={(on) => set(patch(on), field, true)} testId={`photo-adjustment-${field}`} />
  )
  const field = (key: string, label: string, min: number, max: number, fallback: number, unit?: string, step?: number, track?: boolean) => (
    <Field key={key} field={key} label={label} value={num(p[key], fallback)} min={min} max={max} step={step} unit={unit} track={track} set={set} patch={(value) => ({ [key]: value })} />
  )

  switch (kind) {
    case 'brightnessContrast': {
      const legacy = flag(p.legacy, false)
      const contrast = num(p.contrast, 0)
      return (
        <div className="photo-props-fields">
          {field('brightness', phrase('Brightness'), -150, 150, 0)}
          {field('contrast', phrase('Contrast'), legacy ? -100 : -50, 100, 0)}
          {check('legacy', phrase('Use Legacy'), legacy, (on) => (on ? { legacy: true } : { legacy: false, contrast: Math.max(-50, contrast) }))}
        </div>
      )
    }
    case 'levels':
      return (
        <div className="photo-props-fields">
          <Hint>{t('properties.levelsLater')}</Hint>
          {field('inBlack', t('properties.inBlack'), 0, 253, 0, undefined, 1, false)}
          {field('gamma', t('properties.gamma'), 0.01, 9.99, 1, undefined, 0.01, false)}
          {field('inWhite', t('properties.inWhite'), 2, 255, 255, undefined, 1, false)}
          {field('outBlack', t('properties.outBlack'), 0, 255, 0, undefined, 1, false)}
          {field('outWhite', t('properties.outWhite'), 0, 255, 255, undefined, 1, false)}
        </div>
      )
    case 'curves':
      return <Hint testId="photo-adjustment-later">{t('properties.curvesLater')}</Hint>
    case 'exposure':
      return (
        <div className="photo-props-fields">
          {field('exposure', phrase('Exposure'), -20, 20, 0, undefined, 0.01)}
          {field('offset', phrase('Offset'), -0.5, 0.5, 0, undefined, 0.0001)}
          {field('gamma', phrase('Gamma Correction'), 0.01, 9.99, 1, undefined, 0.01)}
        </div>
      )
    case 'vibrance':
      return (
        <div className="photo-props-fields">
          {field('vibrance', phrase('Vibrance'), -100, 100, 0, '%')}
          {field('saturation', phrase('Saturation'), -100, 100, 0, '%')}
        </div>
      )
    case 'hueSaturation': {
      const colorize = flag(p.colorize, false)
      const at = colorize ? 0 : range
      const key = at === 0 ? null : HUE_RANGES[at - 1][0]
      const values = key ? obj(p[key]) : p
      const sub = (name: string) => (value: number) => (key ? { [key]: { [name]: value } } : { [name]: value })
      const hue = num(p.hue, 0)
      return (
        <div className="photo-props-fields">
          <Choice
            label={phrase('Master')}
            quiet
            value={String(at)}
            choices={[['0', phrase('Master')], ...HUE_RANGES.map(([, label], index) => [String(index + 1), phrase(label)] as const)]}
            disabled={colorize}
            onChange={(value) => setRange(Number(value))}
            testId="photo-adjustment-range"
          />
          <Field field={`${key ?? 'master'}-hue`} label={phrase('Hue')} value={num(values.hue, 0)} min={colorize ? 0 : -180} max={colorize ? 360 : 180} unit="°" set={set} patch={sub('hue')} />
          <Field field={`${key ?? 'master'}-saturation`} label={phrase('Saturation')} value={num(values.saturation, 0)} min={colorize ? 0 : -100} max={100} unit="%" set={set} patch={sub('saturation')} />
          <Field field={`${key ?? 'master'}-lightness`} label={phrase('Lightness')} value={num(values.lightness, 0)} min={-100} max={100} unit="%" set={set} patch={sub('lightness')} />
          {check('colorize', phrase('Colorize'), colorize, (on) =>
            on ? { colorize: true, hue: ((hue % 360) + 360) % 360, saturation: Math.max(25, num(p.saturation, 25)) } : { colorize: false, hue: hue > 180 ? hue - 360 : hue },
          )}
        </div>
      )
    }
    case 'colorBalance': {
      const values = nums(p[tone], [0, 0, 0])
      const axes = ['Cyan  ·  Red', 'Magenta  ·  Green', 'Yellow  ·  Blue']
      return (
        <div className="photo-props-fields">
          <Choice
            label={phrase('Tone:').replace(/[:：]\s*$/, '')}
            value={tone}
            choices={[
              ['shadows', phrase('Shadows')],
              ['midtones', phrase('Midtones')],
              ['highlights', phrase('Highlights')],
            ]}
            onChange={setTone}
            testId="photo-adjustment-tone"
          />
          {axes.map((label, index) => (
            <Field key={label} field={`${tone}-${index}`} label={phrase(label)} value={values[index]} min={-100} max={100} set={set} patch={(value) => ({ [tone]: values.map((item, at) => (at === index ? value : item)) })} />
          ))}
          {check('preserveLuminosity', phrase('Preserve Luminosity'), flag(p.preserveLuminosity, true), (on) => ({ preserveLuminosity: on }))}
        </div>
      )
    }
    case 'blackWhite': {
      const tint = flag(p.tint, false)
      return (
        <div className="photo-props-fields">
          {HUE_RANGES.map(([key, label], index) => field(key, phrase(label), -200, 300, BW_DEFAULTS[index], '%'))}
          <Row>
            {check('tint', phrase('Tint'), tint, (on) => ({ tint: on }))}
            <Swatch field="tintColor" label={phrase('Tint')} value={typeof p.tintColor === 'string' ? p.tintColor : '#e1d3b3'} disabled={!tint} set={set} patch={(value) => ({ tintColor: value })} />
            <Button onClick={() => set(Object.fromEntries(HUE_RANGES.map(([key], index) => [key, BW_DEFAULTS[index]])), 'default', true)} data-testid="photo-adjustment-default">
              {phrase('Default')}
            </Button>
          </Row>
        </div>
      )
    }
    case 'photoFilter': {
      const color = typeof p.color === 'string' ? p.color.toLowerCase() : '#ec8a00'
      const preset = PHOTO_FILTERS.find(([, , value]) => value === color)?.[0] ?? null
      const byColor = useColor ?? preset == null
      return (
        <div className="photo-props-fields">
          <Row>
            <Choice
              label={phrase('Filter:').replace(/[:：]\s*$/, '')}
              value={byColor ? 'color' : (preset ?? 'custom')}
              choices={[...(preset ? [] : ([['custom', phrase('Custom')]] as const)), ...PHOTO_FILTERS.map(([id, label]) => [id, phrase(label)] as const), ['color', phrase('Color:').replace(/[:：]\s*$/, '')]]}
              onChange={(value) => {
                setUseColor(value === 'color')
                if (value !== 'color' && value !== 'custom') set({ filter: value }, 'filter', true)
              }}
              testId="photo-adjustment-filter"
            />
            <Swatch field="color" label={phrase('Color:').replace(/[:：]\s*$/, '')} value={color} disabled={!byColor} set={set} patch={(value) => ({ color: value })} />
          </Row>
          {field('density', phrase('Density'), 0, 100, 25, '%')}
          {check('preserveLuminosity', phrase('Preserve Luminosity'), flag(p.preserveLuminosity, true), (on) => ({ preserveLuminosity: on }))}
        </div>
      )
    }
    case 'channelMixer': {
      const mono = flag(p.monochrome, false)
      const out = mono ? 0 : Math.min(output, 2)
      const key = mono ? 'gray' : (['red', 'green', 'blue'] as const)[out]
      const identity = mono ? [40, 40, 20, 0] : [0, 1, 2].map((index) => (index === out ? 100 : 0)).concat(0)
      const row = nums(p[key] ?? (mono ? p.red : undefined), identity)
      const patch = (index: number) => (value: number) => ({ [key]: row.map((item, at) => (at === index ? value : item)) })
      const total = row[0] + row[1] + row[2]
      return (
        <div className="photo-props-fields">
          <Choice
            label={phrase('Output Channel:').replace(/[:：]\s*$/, '')}
            value={String(out)}
            choices={mono ? [['0', phrase('Gray')]] : [['0', phrase('Red')], ['1', phrase('Green')], ['2', phrase('Blue')]]}
            onChange={(value) => setOutput(Number(value))}
            testId="photo-adjustment-output"
          />
          {['Red', 'Green', 'Blue'].map((label, index) => (
            <Field key={label} field={`${key}-${index}`} label={phrase(label)} value={row[index]} min={-200} max={200} unit="%" set={set} patch={patch(index)} />
          ))}
          <Row>
            <span className="photo-props-label">{phrase('Total:')}</span>
            <span className="photo-props-value" data-warn={total > 100}>{`${total > 0 ? '+' : ''}${Math.round(total)}%`}</span>
          </Row>
          <Field field={`${key}-3`} label={phrase('Constant')} value={row[3]} min={-200} max={200} unit="%" set={set} patch={patch(3)} />
          {check('monochrome', phrase('Monochrome'), mono, (on) => ({ monochrome: on }))}
        </div>
      )
    }
    case 'invert':
      return <Hint>{phrase('Invert has no settings.')}</Hint>
    case 'posterize':
      return <div className="photo-props-fields">{field('levels', phrase('Levels'), 2, 255, 4)}</div>
    case 'threshold':
      return <div className="photo-props-fields">{field('level', phrase('Threshold Level'), 1, 255, 128)}</div>
    case 'gradientMap': {
      const stops = (Array.isArray(p.stops) ? p.stops : []).filter((stop): stop is Stop => Array.isArray(stop) && typeof stop[0] === 'number' && typeof stop[1] === 'string')
      const reverse = flag(p.reverse, false)
      const presets = gradientPresets(foreground, background)
      return (
        <div className="photo-props-fields">
          <Choice
            label={phrase('Preset:').replace(/[:：]\s*$/, '')}
            value="custom"
            choices={[['custom', phrase('Custom')], ...presets.map(([label], index) => [String(index), phrase(label)] as const)]}
            onChange={(value) => {
              const chosen = presets[Number(value)]
              if (chosen) set({ stops: chosen[1] }, 'preset', true)
            }}
            testId="photo-adjustment-preset"
          />
          <div className="photo-gradient-preview" style={{ background: stopsCss(stops.length >= 2 ? stops : presets[0][1], reverse) }} data-testid="photo-adjustment-gradient" />
          <Hint>{t('properties.stopsLater')}</Hint>
          <Row>
            {check('reverse', phrase('Reverse'), reverse, (on) => ({ reverse: on }))}
            {check('dither', phrase('Dither'), flag(p.dither, false), (on) => ({ dither: on }))}
          </Row>
        </div>
      )
    }
    case 'selectiveColor': {
      const [key, label] = COLOR_RANGES[selective]
      const values = nums(p[key], [0, 0, 0, 0])
      return (
        <div className="photo-props-fields">
          <Choice label={phrase('Colors')} value={key} choices={COLOR_RANGES.map(([value, name]) => [value, phrase(name)] as const)} onChange={(value) => setSelective(COLOR_RANGES.findIndex(([item]) => item === value))} testId="photo-adjustment-colors" />
          {['Cyan', 'Magenta', 'Yellow', 'Black'].map((name, index) => (
            <Field key={`${label}-${name}`} field={`${key}-${index}`} label={phrase(name)} value={values[index]} min={-100} max={100} unit="%" set={set} patch={(value) => ({ [key]: values.map((item, at) => (at === index ? value : item)) })} />
          ))}
          <Choice
            label={phrase('Method')}
            value={p.method === 'absolute' ? 'absolute' : 'relative'}
            choices={[
              ['relative', phrase('Relative')],
              ['absolute', phrase('Absolute')],
            ]}
            onChange={(method) => set({ method }, 'method', true)}
            testId="photo-adjustment-method"
          />
        </div>
      )
    }
    case 'colorLookup':
      return <ColorLookupSettings lookup={layer.adjustment?.lookup ?? ''} params={p} set={set} />
    default:
      // An adjustment kept as imported (upstream `Unsupported`): nothing to edit.
      return <Hint>{t('properties.unsupported')}</Hint>
  }
}

/** The largest look file the editor reads (a 65-point .cube is about 9 MB). */
const LUT_LIMIT = 32 * 1024 * 1024

/**
 * Color Lookup (upstream `adjust_ui::color_lookup_editor`): a built-in look, or a look file from
 * the workspace through the host (`host/pick`, then `host/read`; upstream's path field), and the
 * interpolation and dithering.
 */
function ColorLookupSettings({ lookup, params, set }: { lookup: string; params: Params; set: Set }) {
  const { phrase, t } = useKitEditor()
  const looks = useEngineQuery<{ id: string; label: string }[]>('image.adjustments.colorLookup.list') ?? []
  const files = useHostPick()
  const current = lookup ? (looks.find((look) => look.label === lookup)?.id ?? 'custom') : 'none'
  const load = async () => {
    const file = await files.pick('colorLookup', ['cube', '3dl', 'look', 'csp'])
    if (!file) return
    if (file.bytes.byteLength > LUT_LIMIT) {
      toast('warning', t('properties.lutTooLarge', { name: file.name, mb: Math.round(LUT_LIMIT / 1024 / 1024) }))
      return
    }
    set({ data: new TextDecoder('utf-8', { fatal: false }).decode(file.bytes), fileName: file.name }, 'lutFile', true)
  }
  return (
    <div className="photo-props-fields">
      <Row>
        <Choice
          label={phrase('3D LUT File')}
          value={current}
          choices={[['none', phrase('Load 3D LUT…')], ...looks.map((look) => [look.id, phrase(look.label)] as const), ...(current === 'custom' ? ([['custom', lookup]] as const) : [])]}
          onChange={(lut) => {
            if (lut !== 'custom' && lut !== current) set({ lut }, 'lut', true)
          }}
          testId="photo-adjustment-lut"
        />
        {/* A workspace file stands in for upstream's path field, when the host lets the editor pick one. */}
        {files.can ? (
          <Button onClick={() => void load()} data-testid="photo-adjustment-lutFile">
            {phrase('Load')}
          </Button>
        ) : null}
      </Row>
      <Choice
        label={phrase('Interpolation')}
        value={params.interpolation === 'tetrahedral' ? 'tetrahedral' : 'trilinear'}
        choices={[
          ['trilinear', phrase('Trilinear')],
          ['tetrahedral', phrase('Tetrahedral')],
        ]}
        onChange={(interpolation) => set({ interpolation }, 'interpolation', true)}
        testId="photo-adjustment-interpolation"
      />
      <Checkbox label={phrase('Dither')} checked={flag(params.dither, false)} onChange={(dither) => set({ dither }, 'dither', true)} testId="photo-adjustment-dither" />
    </div>
  )
}
