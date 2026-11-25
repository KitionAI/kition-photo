import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { GradientSlider, useKitEditor } from '@kition/sdk/editor'
import { createLive, useLive } from '../panels/live'
import { pushEscape } from '../color/escapeStack'
import { presetOptions, presetsOf, settingsOf, settingsPatch, type BrushPreset, type BrushSettings, type EngineBrush } from './brushes'
import { presetThumbnails } from './thumbnails'

/** The preset last picked on this page (the engine keeps it too, but does not say which). */
const pickedPreset = createLive<string | null>(null)

/** What the brush panels change on the options bar: the shared brush's size, hardness and pressure toggles. */
export type BrushOptions = { size: number; hardness: number; set: (patch: { size?: number; hardness?: number; pressureSize?: boolean; pressureOpacity?: boolean }) => void }

/**
 * The engine's brush for the brush panels (plan 2026-11-24, stage 5): read with `brush.get`,
 * changed with `tool.setBrush` (no history step: the brush is the person's tool, not the
 * document), a preset picked by name.
 */
export function useEngineBrush() {
  const { query } = useKitEditor()
  const [brush, setBrush] = useState<EngineBrush | null>(null)
  const refresh = useCallback(() => void query<EngineBrush>('brush.get').then((answer) => answer && setBrush(answer)), [query])
  useEffect(refresh, [refresh])
  const set = useCallback(
    async (patch: Record<string, unknown>) => {
      const answer = await query<EngineBrush>('tool.setBrush', patch)
      if (answer) setBrush(answer)
      return answer
    },
    [query],
  )
  return { brush, settings: settingsOf(brush), set, refresh }
}

/** The presets the engine lists, and their stroke pictures once painted. */
function usePresets() {
  const { query } = useKitEditor()
  const [presets, setPresets] = useState<BrushPreset[]>([])
  const [pictures, setPictures] = useState<Map<string, string>>(new Map())
  useEffect(() => {
    let live = true
    void query('brush.presets.list').then((answer) => {
      if (!live) return
      const list = presetsOf(answer)
      setPresets(list)
      void presetThumbnails(list).then((made) => live && setPictures(made))
    })
    return () => {
      live = false
    }
  }, [query])
  return { presets, pictures }
}

/**
 * The brush preset picker's body (Photoshop's Brush Preset picker; the Brushes panel): Size and
 * Hardness, then every preset with its stroke. A preset becomes the brush (`tool.setBrush`), its
 * size, hardness and pressure toggles the options bar's.
 */
export function BrushPresets({ options, onPicked, testId }: { options: BrushOptions; onPicked?: () => void; testId: string }) {
  const { phrase, t } = useKitEditor()
  const { presets, pictures } = usePresets()
  const { set } = useEngineBrush()
  const picked = useLive(pickedPreset)
  const pick = async (preset: BrushPreset) => {
    const answer = await set({ preset: preset.name })
    if (!answer) return
    pickedPreset.set(preset.name)
    options.set(presetOptions(answer))
    onPicked?.()
  }
  return (
    <div className="photo-brush-presets" data-testid={`${testId}-presets`}>
      <GradientSlider label={`${phrase('Size')} (px)`} value={options.size} min={1} max={5000} softMax={500} onChange={(size) => options.set({ size })} testId={`${testId}-size`} />
      <GradientSlider label={`${phrase('Hardness')} (%)`} value={options.hardness} min={0} max={100} onChange={(hardness) => options.set({ hardness })} testId={`${testId}-hardness`} />
      <ul className="photo-brush-preset-list" role="listbox" aria-label={t('brush.presets')}>
        {presets.map((preset, index) => (
          <li key={preset.name}>
            <button type="button" role="option" aria-selected={picked === preset.name} className="photo-brush-preset" onClick={() => void pick(preset)} title={phrase(preset.name)} data-name={preset.name} data-testid={`${testId}-preset-${index}`}>
              <span className="photo-brush-preset-stroke">{pictures.get(preset.name) ? <img src={pictures.get(preset.name)} alt="" draggable={false} /> : null}</span>
              <span className="photo-brush-preset-name">{phrase(preset.name)}</span>
              <span className="photo-brush-preset-size tabular-nums">{typeof preset.size === 'number' ? Math.round(preset.size) : ''}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}

/**
 * The picker as a pop-up at a point (the options bar's brush chip, a right click on the canvas with
 * a brush tool): it closes on a press outside it or Escape.
 */
export function BrushPickerPopover({ at, options, onClose }: { at: { x: number; y: number }; options: BrushOptions; onClose: () => void }) {
  const box = useRef<HTMLDivElement>(null)
  const [place, setPlace] = useState(at)
  const closeNow = useRef(onClose)
  closeNow.current = onClose
  // Kept inside the window.
  useLayoutEffect(() => {
    const rect = box.current?.getBoundingClientRect()
    if (!rect) return
    setPlace({ x: Math.max(4, Math.min(at.x, window.innerWidth - rect.width - 4)), y: Math.max(4, Math.min(at.y, window.innerHeight - rect.height - 4)) })
  }, [at])
  useEffect(() => {
    const letGo = pushEscape(() => closeNow.current())
    const down = (event: PointerEvent) => {
      if (event.target instanceof Node && box.current?.contains(event.target)) return
      closeNow.current()
    }
    window.addEventListener('pointerdown', down, true)
    return () => {
      letGo()
      window.removeEventListener('pointerdown', down, true)
    }
  }, [])
  return (
    <div ref={box} className="photo-brush-picker kui-popover" role="dialog" style={{ left: place.x, top: place.y }} data-testid="photo-brush-picker">
      <BrushPresets options={options} testId="photo-brush-picker" />
    </div>
  )
}

/** One Brush Settings slider row: its label, track and value, in the field's unit. */
function SettingRow({ label, value, min, max, field, set, testId }: { label: string; value: number; min: number; max: number; field: keyof BrushSettings; set: (field: keyof BrushSettings, value: number | boolean) => void; testId: string }) {
  return <GradientSlider label={label} value={value} min={min} max={max} onChange={(next) => set(field, next)} testId={testId} />
}

/**
 * Brush Settings (Photoshop's panel, the commonly used part): the tip's size, hardness, angle,
 * roundness and spacing, Shape Dynamics' size jitter, Transfer's opacity jitter, and Smoothing.
 * Size, hardness and smoothing are the options bar's; the rest is the engine's brush.
 */
export function BrushSettingsPanel({ options, smoothing }: { options: BrushOptions; smoothing: { value: number; set: (value: number) => void } | null }) {
  const { phrase, t } = useKitEditor()
  const { settings, set } = useEngineBrush()
  const change = (field: keyof BrushSettings, value: number | boolean) => void set(settingsPatch(field, value))
  const id = (name: string) => `photo-brush-settings-${name}`
  const check = (field: 'spacingEnabled' | 'shapeDynamics' | 'transfer', label: string) => (
    <label className="kui-checkbox photo-brush-settings-check">
      <input type="checkbox" checked={settings[field]} onChange={(event) => change(field, event.target.checked)} data-testid={id(field)} />
      <span>{label}</span>
    </label>
  )
  return (
    <div className="photo-brush-settings" data-testid="photo-brush-settings">
      <div className="photo-dialog-caption">{phrase('Brush Tip Shape')}</div>
      <GradientSlider label={`${phrase('Size')} (px)`} value={options.size} min={1} max={5000} softMax={500} onChange={(size) => options.set({ size })} testId={id('size')} />
      <GradientSlider label={`${phrase('Hardness')} (%)`} value={options.hardness} min={0} max={100} onChange={(hardness) => options.set({ hardness })} testId={id('hardness')} />
      <SettingRow label={`${phrase('Angle')} (°)`} value={settings.angle} min={-180} max={180} field="angle" set={change} testId={id('angle')} />
      <SettingRow label={`${phrase('Roundness')} (%)`} value={settings.roundness} min={0} max={100} field="roundness" set={change} testId={id('roundness')} />
      {check('spacingEnabled', phrase('Spacing'))}
      <SettingRow label={`${phrase('Spacing')} (%)`} value={settings.spacing} min={1} max={1000} field="spacing" set={change} testId={id('spacing')} />
      <div className="photo-dialog-caption">{check('shapeDynamics', phrase('Shape Dynamics'))}</div>
      <SettingRow label={`${phrase('Size Jitter')} (%)`} value={settings.sizeJitter} min={0} max={100} field="sizeJitter" set={change} testId={id('sizeJitter')} />
      <div className="photo-dialog-caption">{check('transfer', phrase('Transfer'))}</div>
      <SettingRow label={`${phrase('Opacity Jitter')} (%)`} value={settings.opacityJitter} min={0} max={100} field="opacityJitter" set={change} testId={id('opacityJitter')} />
      {smoothing ? (
        <>
          <div className="photo-dialog-caption">{phrase('Smoothing')}</div>
          <GradientSlider label={`${phrase('Smoothing')} (%)`} value={smoothing.value} min={0} max={100} onChange={smoothing.set} testId={id('smoothing')} />
        </>
      ) : (
        <p className="photo-dialog-hint">{t('brush.noSmoothing')}</p>
      )}
    </div>
  )
}
