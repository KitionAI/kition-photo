import { useState } from 'react'
import { RGB_IDENTITY, SLIDER_GRADIENTS, Segmented, useKitEditor, type CurvePoint, type KitCommand } from '@kition/sdk/editor'
import { DialogFrame } from './DialogFrame'
import { CheckRow, ChoiceRow, SliderRow } from './controls'
import { dialogTitle } from './labels'
import { curvesParams, LEVELS_IDENTITY, levelsParams, type Levels, type RgbChannel } from './levels'
import { CurvesEditor, LevelsEditor, type Histograms } from './ToneEditors'

/**
 * The adjustments' own dialogs (Photoshop's): Levels, Curves, Hue/Saturation, Brightness/Contrast
 * and Color Balance, each previewing on the canvas. The same dialog serves Image › Adjustments
 * (destructive) and Layer › New Adjustment Layer (the layer's settings): the command differs, the
 * values are the same. Each opens with the values last OK'd for that kind in this session.
 */

type Props = { command: KitCommand; onClose: () => void }

const remembered = new Map<string, unknown>()

/** A dialog's values: the last OK'd ones for `kind`, else `initial`; `keep` remembers them. */
function useKept<T>(kind: string, initial: () => T) {
  const [value, setValue] = useState<T>(() => (remembered.has(kind) ? (remembered.get(kind) as T) : initial()))
  return [value, setValue, () => remembered.set(kind, value)] as const
}

const closeWith = (keep: () => void, onClose: () => void) => (committed: boolean) => {
  if (committed) keep()
  onClose()
}

const levelsIdentity = (): Record<RgbChannel, Levels> => ({ rgb: LEVELS_IDENTITY, red: LEVELS_IDENTITY, green: LEVELS_IDENTITY, blue: LEVELS_IDENTITY })

export function LevelsDialog({ command, histograms, onClose }: Props & { histograms: Histograms | null }) {
  const words = useKitEditor()
  const [levels, setLevels, keep] = useKept('levels', levelsIdentity)
  const [channel, setChannel] = useState<RgbChannel>('rgb')
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={levelsParams(levels)} previewing onReset={() => setLevels(levelsIdentity())} onClose={closeWith(keep, onClose)}>
      <LevelsEditor levels={levels} channel={channel} onChannel={setChannel} histograms={histograms} onChange={(next) => setLevels(next)} testId="photo-dialog" />
    </DialogFrame>
  )
}

const curvesIdentity = (): Record<RgbChannel, readonly CurvePoint[]> => ({ rgb: RGB_IDENTITY, red: RGB_IDENTITY, green: RGB_IDENTITY, blue: RGB_IDENTITY })

export function CurvesDialog({ command, histograms, onClose }: Props & { histograms: Histograms | null }) {
  const words = useKitEditor()
  const [curves, setCurves, keep] = useKept('curves', curvesIdentity)
  const [channel, setChannel] = useState<RgbChannel>('rgb')
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={curvesParams(curves)} previewing onReset={() => setCurves(curvesIdentity())} onClose={closeWith(keep, onClose)}>
      <CurvesEditor curves={curves} channel={channel} onChannel={setChannel} histograms={histograms} onChange={(next) => setCurves(next)} testId="photo-dialog" />
    </DialogFrame>
  )
}

/** Hue/Saturation's colour ranges, as the engine names them, and their middle hue (for the colour bars). */
export const HUE_RANGES = [
  ['reds', 0],
  ['yellows', 60],
  ['greens', 120],
  ['cyans', 180],
  ['blues', 240],
  ['magentas', 300],
] as const
type HueRange = (typeof HUE_RANGES)[number][0]
type Hsl = { hue: number; saturation: number; lightness: number }
type HueSat = { master: Hsl; ranges: Record<HueRange, Hsl>; colorize: boolean; tint: Hsl }

const ZERO: Hsl = { hue: 0, saturation: 0, lightness: 0 }
const hueSatInitial = (): HueSat => ({ master: ZERO, ranges: Object.fromEntries(HUE_RANGES.map(([range]) => [range, ZERO])) as Record<HueRange, Hsl>, colorize: false, tint: { hue: 0, saturation: 25, lightness: 0 } })

/** Hue/Saturation's parameters: the master values (or Colorize's), and each range that moves. */
export function hueSatParams(state: HueSat): Record<string, unknown> {
  if (state.colorize) return { colorize: true, ...state.tint }
  const params: Record<string, unknown> = { colorize: false, ...state.master }
  for (const [range] of HUE_RANGES) {
    const values = state.ranges[range]
    if (values.hue || values.saturation || values.lightness) params[range] = { ...values }
  }
  return params
}

/** A hue strip from 0 to 360°, each hue moved by `shift` (the bar under Hue/Saturation's). */
const hueStrip = (shift: number, saturation = 80, lightness = 50) => `linear-gradient(to right, ${Array.from({ length: 13 }, (_, i) => `hsl(${i * 30 + shift} ${saturation}% ${lightness}%)`).join(', ')})`

export function HueSaturationDialog({ command, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [state, setState, keep] = useKept('hueSaturation', hueSatInitial)
  const [edit, setEdit] = useState<'master' | HueRange>('master')
  const colorize = state.colorize
  const target = colorize ? state.tint : edit === 'master' ? state.master : state.ranges[edit]
  const set = (key: keyof Hsl) => (value: number) =>
    setState((current) => {
      if (current.colorize) return { ...current, tint: { ...current.tint, [key]: value } }
      if (edit === 'master') return { ...current, master: { ...current.master, [key]: value } }
      return { ...current, ranges: { ...current.ranges, [edit]: { ...current.ranges[edit], [key]: value } } }
    })
  // The lower bar shows where each hue goes: the whole strip shifts with the master hue, a range's part with its own.
  const shift = colorize ? 0 : state.master.hue + (edit === 'master' ? 0 : state.ranges[edit].hue)
  const tintBar = colorize ? `linear-gradient(to right, hsl(${state.tint.hue} 0% 50%), hsl(${state.tint.hue} 100% 50%))` : hueStrip(shift)
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={hueSatParams(state)} previewing onReset={() => setState(hueSatInitial())} onClose={closeWith(keep, onClose)}>
      <div className="photo-dialog-fields">
        <ChoiceRow
          label={t('dialog.edit')}
          value={colorize ? 'master' : edit}
          disabled={colorize}
          choices={[['master', t('dialog.master')] as const, ...HUE_RANGES.map(([range]) => [range, t(`dialog.${range}`)] as const)]}
          onChange={setEdit}
          testId="photo-dialog-range"
        />
        <SliderRow label={t('dialog.hue')} value={target.hue} min={colorize ? 0 : -180} max={colorize ? 360 : 180} fallback={0} stops={SLIDER_GRADIENTS.hue} onChange={set('hue')} testId="photo-dialog-hue" />
        <SliderRow label={t('dialog.saturation')} value={target.saturation} min={colorize ? 0 : -100} max={100} fallback={colorize ? 25 : 0} stops={['hsl(0 0% 50%)', `hsl(${target.hue} 100% 50%)`]} onChange={set('saturation')} testId="photo-dialog-saturation" />
        <SliderRow label={t('dialog.lightness')} value={target.lightness} min={-100} max={100} fallback={0} stops={['#000', `hsl(${target.hue} 60% 50%)`, '#fff']} onChange={set('lightness')} testId="photo-dialog-lightness" />
        <CheckRow label={t('dialog.colorize')} checked={colorize} onChange={(on) => setState((current) => ({ ...current, colorize: on }))} testId="photo-dialog-colorize" />
        <div className="photo-hue-bars" role="img" aria-label={t('dialog.hueBars')} data-testid="photo-dialog-hue-bars">
          <div className="photo-hue-bar" style={{ background: hueStrip(0) }} />
          <div className="photo-hue-bar" style={{ background: tintBar }} />
        </div>
      </div>
    </DialogFrame>
  )
}

export function BrightnessContrastDialog({ command, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [state, setState, keep] = useKept('brightnessContrast', () => ({ brightness: 0, contrast: 0, legacy: false }))
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={state} previewing onReset={() => setState({ brightness: 0, contrast: 0, legacy: false })} onClose={closeWith(keep, onClose)}>
      <div className="photo-dialog-fields">
        <SliderRow label={t('dialog.brightness')} value={state.brightness} min={-150} max={150} fallback={0} onChange={(brightness) => setState((current) => ({ ...current, brightness }))} testId="photo-dialog-brightness" />
        <SliderRow label={t('dialog.contrast')} value={state.contrast} min={state.legacy ? -100 : -50} max={100} fallback={0} onChange={(contrast) => setState((current) => ({ ...current, contrast }))} testId="photo-dialog-contrast" />
        <CheckRow label={t('dialog.useLegacy')} checked={state.legacy} onChange={(legacy) => setState((current) => ({ ...current, legacy, contrast: legacy ? current.contrast : Math.max(-50, current.contrast) }))} testId="photo-dialog-legacy" />
      </div>
    </DialogFrame>
  )
}

const TONES = ['shadows', 'midtones', 'highlights'] as const
type Tone = (typeof TONES)[number]
type Balance = Record<Tone, [number, number, number]> & { preserveLuminosity: boolean }
const balanceInitial = (): Balance => ({ shadows: [0, 0, 0], midtones: [0, 0, 0], highlights: [0, 0, 0], preserveLuminosity: true })

/** Color Balance's three axes, with the colours at each end. */
const AXES = [
  ['dialog.cyanRed', ['#00b7c3', '#e5484d']],
  ['dialog.magentaGreen', ['#d6409f', '#30a46c']],
  ['dialog.yellowBlue', ['#f5d90a', '#3e63dd']],
] as const

export function ColorBalanceDialog({ command, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [state, setState, keep] = useKept('colorBalance', balanceInitial)
  const [tone, setTone] = useState<Tone>('midtones')
  const values = state[tone]
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={state} previewing onReset={() => setState(balanceInitial())} onClose={closeWith(keep, onClose)}>
      <div className="photo-dialog-fields">
        <div className="photo-dialog-caption">{t('dialog.colorLevels')}</div>
        {AXES.map(([label, stops], index) => (
          <SliderRow
            key={label}
            label={t(label)}
            value={values[index]}
            min={-100}
            max={100}
            fallback={0}
            stops={stops}
            onChange={(value) => setState((current) => ({ ...current, [tone]: current[tone].map((item, at) => (at === index ? value : item)) as [number, number, number] }))}
            testId={`photo-dialog-balance-${index}`}
          />
        ))}
        <div className="photo-dialog-caption">{t('dialog.toneBalance')}</div>
        <div className="photo-dialog-row" data-testid="photo-dialog-tone">
          <Segmented label={t('dialog.toneBalance')} value={tone} segments={TONES.map((value) => ({ value, label: t(`dialog.${value}`) }))} onChange={setTone} />
        </div>
        <CheckRow label={t('dialog.preserveLuminosity')} checked={state.preserveLuminosity} onChange={(preserveLuminosity) => setState((current) => ({ ...current, preserveLuminosity }))} testId="photo-dialog-preserve" />
      </div>
    </DialogFrame>
  )
}
