import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { Button, GradientSlider, PreviewDialog, useEngineQuery, useKitEditor } from '@kition/sdk/editor'
import { DIALOG_BUTTONS } from '../dialogs/DialogFrame'
import { ColorWell } from '../color/ColorWell'
import { pushEscape } from '../color/escapeStack'
import { addColorStop, addOpacityStop, cssGradient, fromEngine, midpointPlaces, moveStop, removeStop, resolveColor, setMidpoint, setStop, type EngineGradient, type GradientDraft } from './gradient'

/** The editor's test id; its parts add `-bar`, `-colors` and `-opacities` (the stop rows), `-color-<i>`, `-opacity-<i>`, `-midpoint-<i>`, `-preset-<i>`, `-location`, `-stop-color`, `-stop-opacity`, `-delete`. */
export const GRADIENT_EDITOR = 'photo-gradient-editor'

type Selected = { which: 'color' | 'opacity'; index: number } | { which: 'midpoint'; index: number } | null
type Preset = { name: string; stops: [number, string][]; transparency: [number, number][] }

/** How far (px) a stop dragged off its row is taken away on release, as Photoshop's. */
const DRAG_OFF = 28

/**
 * Photoshop's Gradient Editor (plan 2026-11-24, stage 5): the presets, the bar with opacity stops
 * over it and colour stops under it, a midpoint between each pair of colour stops, and the
 * selected stop's colour or opacity and location. A press on a stop row adds a stop there, a stop
 * drags along the bar (dragged off the row it goes on release), Delete takes the selected one
 * away, a colour stop's colour opens the Color Picker. OK answers the gradient, Cancel null.
 */
export function GradientEditorDialog({ title, value, midpoints, onLive, foreground, background, onDone }: { title: string; value: GradientDraft; midpoints: boolean; onLive?: (draft: GradientDraft) => void; foreground: string; background: string; onDone: (draft: GradientDraft | null) => void }) {
  const { t, phrase } = useKitEditor()
  const [draft, setDraftState] = useState(value)
  const draftNow = useRef(draft)
  const [selected, setSelected] = useState<Selected>({ which: 'color', index: 0 })
  const onLiveNow = useRef(onLive)
  onLiveNow.current = onLive
  const setDraft = (next: GradientDraft) => {
    draftNow.current = next
    setDraftState(next)
    onLiveNow.current?.(next)
  }
  const doneNow = useRef(onDone)
  doneNow.current = onDone
  const cancel = () => {
    if (draftNow.current !== value) onLiveNow.current?.(value)
    doneNow.current(null)
  }
  const cancelNow = useRef(cancel)
  cancelNow.current = cancel
  useEffect(() => pushEscape(() => cancelNow.current()), [])
  const presetList = useEngineQuery<{ groups?: { name: string; presets: Preset[] }[] }>('gradient.presets.list')
  const presets = (presetList?.groups ?? []).flatMap((group) => group.presets).slice(0, 48)
  const bar = useRef<HTMLDivElement>(null)
  const fraction = (clientX: number) => {
    const box = bar.current?.getBoundingClientRect()
    return box ? Math.min(1, Math.max(0, (clientX - box.left) / Math.max(1, box.width))) : 0
  }

  /** A press on a stop row: on a stop, select and drag it; elsewhere, add one there and drag it. */
  const pressRow = (which: 'color' | 'opacity') => (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return
    event.preventDefault()
    const target = (event.target as HTMLElement).closest<HTMLElement>('[data-stop]')
    let index: number
    let working = draftNow.current
    if (target) index = Number(target.dataset.stop)
    else {
      const added = which === 'color' ? addColorStop(working, fraction(event.clientX), foreground, background) : addOpacityStop(working, fraction(event.clientX))
      working = added.draft
      index = added.index
      setDraft(working)
    }
    setSelected({ which, index })
    const row = event.currentTarget
    row.setPointerCapture?.(event.pointerId)
    const startY = event.clientY
    const move = (moved: PointerEvent) => {
      const off = Math.abs(moved.clientY - startY) > DRAG_OFF
      row.dataset.removing = off ? 'true' : 'false'
      setDraft(moveStop(draftNow.current, which, index, fraction(moved.clientX)))
    }
    const up = (released: PointerEvent) => {
      row.removeEventListener('pointermove', move)
      row.removeEventListener('pointerup', up)
      row.removeEventListener('pointercancel', up)
      delete row.dataset.removing
      if (Math.abs(released.clientY - startY) > DRAG_OFF) {
        const fewer = removeStop(draftNow.current, which, index)
        if (fewer) {
          setDraft(fewer)
          setSelected(null)
        }
      }
    }
    row.addEventListener('pointermove', move)
    row.addEventListener('pointerup', up)
    row.addEventListener('pointercancel', up)
  }
  const pressMidpoint = (segment: number) => (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    setSelected({ which: 'midpoint', index: segment })
    const handle = event.currentTarget
    handle.setPointerCapture?.(event.pointerId)
    const move = (moved: PointerEvent) => setDraft(midpointTo(segment, fraction(moved.clientX)))
    const up = () => {
      handle.removeEventListener('pointermove', move)
      handle.removeEventListener('pointerup', up)
      handle.removeEventListener('pointercancel', up)
    }
    handle.addEventListener('pointermove', move)
    handle.addEventListener('pointerup', up)
    handle.addEventListener('pointercancel', up)
  }
  /** The draft with segment `segment`'s midpoint under bar position `at`. */
  const midpointTo = (segment: number, at: number) => {
    const now = draftNow.current
    const sorted = [...now.stops].sort((a, b) => a.at - b.at)
    const a = sorted[segment]?.at ?? 0
    const b = sorted[segment + 1]?.at ?? 1
    return setMidpoint(now, segment, b - a <= 0 ? 0.5 : (at - a) / (b - a))
  }

  const removeSelected = () => {
    if (!selected || selected.which === 'midpoint') return
    const fewer = removeStop(draftNow.current, selected.which, selected.index)
    if (!fewer) return
    setDraft(fewer)
    setSelected(null)
  }
  const colorStop = selected?.which === 'color' ? draft.stops[selected.index] : undefined
  const opacityStop = selected?.which === 'opacity' ? draft.opacity[selected.index] : undefined
  const location = colorStop?.at ?? opacityStop?.at ?? (selected?.which === 'midpoint' ? midpointPlaces(draft)[selected.index] : undefined)
  const places = midpointPlaces(draft)
  return (
    <PreviewDialog title={title} onOk={() => doneNow.current(draftNow.current)} onCancel={cancel} ids={DIALOG_BUTTONS} testId={GRADIENT_EDITOR}>
      <div className="photo-gradient-editor">
        {presets.length ? (
          <div className="photo-gradient-presets" role="listbox" aria-label={t('gradient.presets')}>
            {presets.map((preset, index) => (
              <button
                key={`${preset.name}-${index}`}
                type="button"
                role="option"
                aria-selected={false}
                className="photo-gradient-preset"
                title={phrase(preset.name)}
                aria-label={phrase(preset.name)}
                style={{ backgroundImage: `${cssGradient(fromEngine(preset as EngineGradient), foreground, background)}, var(--photo-checker)` }}
                onClick={() => {
                  setDraft(fromEngine(preset as EngineGradient))
                  setSelected({ which: 'color', index: 0 })
                }}
                data-testid={`${GRADIENT_EDITOR}-preset-${index}`}
              />
            ))}
          </div>
        ) : null}
        <div className="photo-gradient-ramp">
          <div className="photo-gradient-row photo-gradient-row-opacity" onPointerDown={pressRow('opacity')} title={t('gradient.addOpacityStop')} data-testid={`${GRADIENT_EDITOR}-opacities`}>
            {draft.opacity.map((stop, index) => (
              <span
                key={index}
                className="photo-gradient-stop photo-gradient-stop-opacity"
                data-stop={index}
                data-selected={selected?.which === 'opacity' && selected.index === index}
                style={{ left: `${stop.at * 100}%`, background: `rgba(0, 0, 0, ${stop.opacity / 100})` }}
                title={`${Math.round(stop.opacity)}%`}
                data-testid={`${GRADIENT_EDITOR}-opacity-${index}`}
              />
            ))}
          </div>
          <div ref={bar} className="photo-gradient-bar" style={{ backgroundImage: `${cssGradient(draft, foreground, background)}, var(--photo-checker)` }} data-testid={`${GRADIENT_EDITOR}-bar`} />
          <div className="photo-gradient-row photo-gradient-row-color" onPointerDown={pressRow('color')} title={t('gradient.addColorStop')} data-testid={`${GRADIENT_EDITOR}-colors`}>
            {draft.stops.map((stop, index) => (
              <span
                key={index}
                className="photo-gradient-stop photo-gradient-stop-color"
                data-stop={index}
                data-selected={selected?.which === 'color' && selected.index === index}
                style={{ left: `${stop.at * 100}%`, background: resolveColor(stop.color, foreground, background) }}
                data-color={stop.color}
                data-testid={`${GRADIENT_EDITOR}-color-${index}`}
              />
            ))}
            {places.length
              ? places.map((place, segment) => (
                  <button
                    key={`mid-${segment}`}
                    type="button"
                    className="photo-gradient-midpoint"
                    data-selected={selected?.which === 'midpoint' && selected.index === segment}
                    style={{ left: `${place * 100}%` }}
                    aria-label={t('gradient.midpoint')}
                    title={t('gradient.midpoint')}
                    onPointerDown={pressMidpoint(segment)}
                    data-testid={`${GRADIENT_EDITOR}-midpoint-${segment}`}
                  />
                ))
              : null}
          </div>
        </div>
        <div className="photo-gradient-stop-fields">
          <span className="photo-dialog-caption">{t('gradient.stops')}</span>
          {colorStop ? (
            <label className="photo-gradient-field">
              <span>{t('dialog.color')}</span>
              <ColorWell
                value={resolveColor(colorStop.color, foreground, background)}
                label={t('dialog.color')}
                title={t('gradient.stopColor')}
                onChange={(color) => selected && selected.which === 'color' && setDraft(setStop(draftNow.current, 'color', selected.index, color))}
                testId={`${GRADIENT_EDITOR}-stop-color`}
              />
            </label>
          ) : null}
          {opacityStop ? (
            <GradientSlider label={t('dialog.opacity')} value={Math.round(opacityStop.opacity)} min={0} max={100} onChange={(opacity) => selected && selected.which === 'opacity' && setDraft(setStop(draftNow.current, 'opacity', selected.index, opacity))} testId={`${GRADIENT_EDITOR}-stop-opacity`} />
          ) : null}
          {location !== undefined && selected ? (
            <GradientSlider
              label={t('gradient.location')}
              value={Math.round(location * 100)}
              min={0}
              max={100}
              onChange={(percent) => {
                if (selected.which === 'midpoint') setDraft(midpointTo(selected.index, percent / 100))
                else setDraft(moveStop(draftNow.current, selected.which, selected.index, percent / 100))
              }}
              testId={`${GRADIENT_EDITOR}-location`}
            />
          ) : (
            <p className="photo-dialog-hint">{t('gradient.hint')}</p>
          )}
          <Button onClick={removeSelected} disabled={!selected || selected.which === 'midpoint' || (selected.which === 'color' && draft.stops.length <= 2)} data-testid={`${GRADIENT_EDITOR}-delete`}>
            {t('gradient.deleteStop')}
          </Button>
        </div>
      </div>
    </PreviewDialog>
  )
}
