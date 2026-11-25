import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react'
import { PreviewDialog, useKitEditor } from '@kition/sdk/editor'
import { DIALOG_BUTTONS, useDialogPlace } from '../dialogs/DialogFrame'
import { fieldsOf, fieldsOfHsb, hexToRgb, normalHex, squarePoint, squareValue, stripHue, withField, type ColorField, type ColorFields } from './colorMath'
import type { SampleAt } from './ColorPickerHost'
import { pushEscape } from './escapeStack'

/** The picker's test id; its parts add `-square`, `-hue`, `-new`, `-current`, `-hex` and one per field (`-h`, `-s`, `-b`, `-r`, `-g`, `-bl`, `-l`, `-la`, `-lb`). */
export const COLOR_DIALOG = 'photo-color-dialog'

const FIELDS: readonly { field: ColorField; label: string; unit: string; min: number; max: number; read: (fields: ColorFields) => number }[] = [
  { field: 'h', label: 'H', unit: '°', min: 0, max: 360, read: (f) => Math.round(f.hsb.h) % 360 },
  { field: 's', label: 'S', unit: '%', min: 0, max: 100, read: (f) => Math.round(f.hsb.s) },
  { field: 'b', label: 'B', unit: '%', min: 0, max: 100, read: (f) => Math.round(f.hsb.b) },
  { field: 'r', label: 'R', unit: '', min: 0, max: 255, read: (f) => f.rgb[0] },
  { field: 'g', label: 'G', unit: '', min: 0, max: 255, read: (f) => f.rgb[1] },
  { field: 'bl', label: 'B', unit: '', min: 0, max: 255, read: (f) => f.rgb[2] },
  { field: 'l', label: 'L', unit: '', min: 0, max: 100, read: (f) => Math.round(f.lab.l) },
  { field: 'la', label: 'a', unit: '', min: -128, max: 127, read: (f) => Math.round(f.lab.a) },
  { field: 'lb', label: 'b', unit: '', min: -128, max: 127, read: (f) => Math.round(f.lab.b) },
]

/**
 * Photoshop's Color Picker (plan 2026-11-24, stage 5; the reference's `color_picker_ui`): a
 * saturation and brightness square with a hue strip beside it, the new colour over the current
 * one (a click on the current one goes back to it), and HSB, RGB, Lab and hex fields that follow
 * each other. While it is open a press on the canvas picks the picture's colour there (an
 * eyedropper, the pointer dragged keeps picking). OK answers the new colour, Cancel (or Escape)
 * null, after giving `onLive` the current colour back. It sits over the dialog that opened it and
 * takes Escape and presses before that dialog can.
 */
export function ColorPickerDialog({ title, value, onLive, sample, onDone }: { title: string; value: string; onLive?: (hex: string) => void; sample: SampleAt; onDone: (hex: string | null) => void }) {
  const { t } = useKitEditor()
  const current = normalHex(value) ?? '#000000'
  const [fields, setFieldsState] = useState<ColorFields>(() => fieldsOf(hexToRgb(current) ?? [0, 0, 0]))
  const fieldsNow = useRef(fields)
  fieldsNow.current = fields
  const [typing, setTyping] = useState<{ field: ColorField | 'hex'; text: string } | null>(null)
  const onLiveNow = useRef(onLive)
  onLiveNow.current = onLive
  const place = useDialogPlace()
  const setFields = (next: ColorFields) => {
    const before = fieldsNow.current.hex
    fieldsNow.current = next
    setFieldsState(next)
    if (next.hex !== before) onLiveNow.current?.(next.hex)
  }
  const setFieldsNow = useRef(setFields)
  setFieldsNow.current = setFields
  const doneNow = useRef(onDone)
  doneNow.current = onDone
  const cancel = () => {
    if (fieldsNow.current.hex !== current) onLiveNow.current?.(current)
    doneNow.current(null)
  }
  const cancelNow = useRef(cancel)
  cancelNow.current = cancel

  // The eyedropper and Escape, before anything under the picker (its own dialog, the one that
  // opened it, the canvas's tools) sees them: window capture listeners, gone when it closes.
  useEffect(() => {
    let pressed = false
    let busy = false
    const take = (event: PointerEvent) => {
      const answer = sample(event.clientX, event.clientY)
      if (!answer) return false
      event.preventDefault()
      event.stopPropagation()
      event.stopImmediatePropagation()
      if (busy) return true
      busy = true
      void answer
        .then((hex) => {
          const rgb = hex ? hexToRgb(hex) : null
          if (rgb) setFieldsNow.current(fieldsOf(rgb, fieldsNow.current.hsb.h))
        })
        .finally(() => {
          busy = false
        })
      return true
    }
    const down = (event: PointerEvent) => {
      if (event.button !== 0) return
      if ((event.target instanceof Element && event.target.closest(`[data-testid="${COLOR_DIALOG}"]`)) || !take(event)) return
      pressed = true
    }
    const move = (event: PointerEvent) => {
      if (pressed && event.buttons & 1) take(event)
    }
    const up = (event: PointerEvent) => {
      if (!pressed) return
      pressed = false
      event.preventDefault()
      event.stopPropagation()
      event.stopImmediatePropagation()
    }
    const letGo = pushEscape(() => cancelNow.current())
    window.addEventListener('pointerdown', down, true)
    window.addEventListener('pointermove', move, true)
    window.addEventListener('pointerup', up, true)
    return () => {
      letGo()
      window.removeEventListener('pointerdown', down, true)
      window.removeEventListener('pointermove', move, true)
      window.removeEventListener('pointerup', up, true)
    }
  }, [sample])

  const dragTo = (event: ReactPointerEvent<HTMLElement>, part: 'square' | 'hue') => {
    const rect = event.currentTarget.getBoundingClientRect()
    const x = (event.clientX - rect.left) / Math.max(1, rect.width)
    const y = (event.clientY - rect.top) / Math.max(1, rect.height)
    const hsb = fieldsNow.current.hsb
    setTyping(null)
    setFields(part === 'square' ? fieldsOfHsb({ h: hsb.h, ...squareValue(x, y) }) : fieldsOfHsb({ ...hsb, h: stripHue(y) }))
  }
  const pressOn = (part: 'square' | 'hue') => ({
    onPointerDown: (event: ReactPointerEvent<HTMLElement>) => {
      if (event.button !== 0) return
      event.currentTarget.setPointerCapture?.(event.pointerId)
      dragTo(event, part)
    },
    onPointerMove: (event: ReactPointerEvent<HTMLElement>) => {
      if (event.buttons & 1 && event.currentTarget.hasPointerCapture?.(event.pointerId)) dragTo(event, part)
    },
  })
  const nudge = (part: 'square' | 'hue') => (event: ReactKeyboardEvent<HTMLElement>) => {
    const step = event.shiftKey ? 10 : 1
    const hsb = fieldsNow.current.hsb
    const moves: Record<string, () => ColorFields> =
      part === 'square'
        ? { ArrowLeft: () => fieldsOfHsb({ ...hsb, s: hsb.s - step }), ArrowRight: () => fieldsOfHsb({ ...hsb, s: hsb.s + step }), ArrowUp: () => fieldsOfHsb({ ...hsb, b: hsb.b + step }), ArrowDown: () => fieldsOfHsb({ ...hsb, b: hsb.b - step }) }
        : { ArrowUp: () => fieldsOfHsb({ ...hsb, h: (hsb.h + step) % 360 }), ArrowDown: () => fieldsOfHsb({ ...hsb, h: (hsb.h - step + 360) % 360 }) }
    const move = moves[event.key]
    if (!move) return
    event.preventDefault()
    event.stopPropagation()
    setTyping(null)
    setFields(move())
  }

  const point = squarePoint(fields.hsb)
  const pure = fieldsOfHsb({ h: fields.hsb.h, s: 100, b: 100 }).hex
  const hexShown = typing?.field === 'hex' ? typing.text : fields.hex.slice(1)
  return (
    <PreviewDialog
      title={title}
      position={place.position}
      onMove={place.onMove}
      onOk={() => {
        doneNow.current(fieldsNow.current.hex)
      }}
      onCancel={cancel}
      ids={DIALOG_BUTTONS}
      testId={COLOR_DIALOG}
    >
      <div className="photo-color-dialog" data-color={fields.hex}>
        <div
          className="photo-color-square"
          role="slider"
          tabIndex={0}
          aria-label={t('color.square')}
          aria-valuetext={`${Math.round(fields.hsb.s)}%, ${Math.round(fields.hsb.b)}%`}
          style={{ background: `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, ${pure})` }}
          onKeyDown={nudge('square')}
          {...pressOn('square')}
          data-testid={`${COLOR_DIALOG}-square`}
        >
          <span className="photo-color-square-mark" style={{ left: `${point.x * 100}%`, top: `${point.y * 100}%` }} />
        </div>
        <div
          className="photo-color-hue"
          role="slider"
          tabIndex={0}
          aria-label={t('color.hue')}
          aria-valuemin={0}
          aria-valuemax={360}
          aria-valuenow={Math.round(fields.hsb.h)}
          onKeyDown={nudge('hue')}
          {...pressOn('hue')}
          data-testid={`${COLOR_DIALOG}-hue`}
        >
          <span className="photo-color-hue-mark" style={{ top: `${(1 - fields.hsb.h / 360) * 100}%` }} />
        </div>
        <div className="photo-color-side">
          <div className="photo-color-compare">
            <span className="photo-color-compare-label">{t('color.new')}</span>
            <span className="photo-color-compare-new" style={{ background: fields.hex }} data-color={fields.hex} data-testid={`${COLOR_DIALOG}-new`} />
            <button
              type="button"
              className="photo-color-compare-current"
              style={{ background: current }}
              title={t('color.backToCurrent')}
              aria-label={t('color.backToCurrent')}
              onClick={() => {
                setTyping(null)
                setFields(fieldsOf(hexToRgb(current) ?? [0, 0, 0], fieldsNow.current.hsb.h))
              }}
              data-color={current}
              data-testid={`${COLOR_DIALOG}-current`}
            />
            <span className="photo-color-compare-label">{t('color.current')}</span>
          </div>
          <p className="photo-color-hint">{t('color.eyedropperHint')}</p>
          <div className="photo-color-fields">
            {FIELDS.map(({ field, label, unit, min, max, read }) => {
              const id = `${COLOR_DIALOG}-${field}`
              return (
                <label key={field} className="photo-color-field" htmlFor={id} data-field={field}>
                  <span className="photo-color-field-label">{label}</span>
                  <input
                    id={id}
                    className="kui-field tabular-nums"
                    type="number"
                    min={min}
                    max={max}
                    value={typing?.field === field ? typing.text : read(fields)}
                    onChange={(event) => {
                      const text = event.target.value
                      setTyping({ field, text })
                      const number = Number(text)
                      if (text.trim() !== '' && Number.isFinite(number)) setFields(withField(fieldsNow.current, field, Math.min(max, Math.max(min, number))))
                    }}
                    onBlur={() => setTyping(null)}
                    data-testid={id}
                  />
                  <span className="photo-color-field-unit">{unit}</span>
                </label>
              )
            })}
            <label className="photo-color-field photo-color-field-hex" htmlFor={`${COLOR_DIALOG}-hex`}>
              <span className="photo-color-field-label">#</span>
              <input
                id={`${COLOR_DIALOG}-hex`}
                className="kui-field tabular-nums"
                type="text"
                spellCheck={false}
                maxLength={7}
                value={hexShown}
                onChange={(event) => {
                  const text = event.target.value
                  setTyping({ field: 'hex', text })
                  const rgb = /^#?[0-9a-f]{6}$/i.test(text.trim()) ? hexToRgb(text) : null
                  if (rgb) setFields(fieldsOf(rgb, fieldsNow.current.hsb.h))
                }}
                onBlur={() => setTyping(null)}
                data-testid={`${COLOR_DIALOG}-hex`}
              />
            </label>
          </div>
        </div>
      </div>
    </PreviewDialog>
  )
}
