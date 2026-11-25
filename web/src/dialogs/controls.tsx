import { useState, type ReactNode } from 'react'
import { Checkbox, Choice, GradientSlider } from '@kition/sdk/editor'
import { ColorWell } from '../color/ColorWell'

/**
 * The dialogs' controls, on kit-editor's: a slider row (label, track, value to type), a drop-down,
 * a check box, a colour well, a number and a text field. Each reports every change at once; the
 * preview waits for the values to settle (`useCommandPreview`).
 */

export function SliderRow({ label, value, min, max, softMin, softMax, decimals = 0, fallback, stops, onChange, testId }: { label: string; value: number; min: number; max: number; softMin?: number; softMax?: number; decimals?: number; fallback?: number; stops?: readonly string[]; onChange: (value: number) => void; testId?: string }) {
  return <GradientSlider label={label} value={value} min={min} max={max} softMin={softMin} softMax={softMax} decimals={decimals} default={fallback} stops={stops} onChange={(next) => onChange(next)} testId={testId} />
}

export function ChoiceRow<V extends string>({ label, value, choices, onChange, disabled, testId }: { label: string; value: V; choices: readonly (readonly [V, string])[]; onChange: (value: V) => void; disabled?: boolean; testId?: string }) {
  return (
    <div className="photo-dialog-row">
      <Choice label={label} value={value} choices={choices} onChange={onChange} disabled={disabled} testId={testId} />
    </div>
  )
}

export function CheckRow({ label, checked, onChange, disabled, testId }: { label: string; checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean; testId?: string }) {
  return (
    <div className="photo-dialog-row">
      <Checkbox label={label} checked={checked} onChange={onChange} disabled={disabled} testId={testId} />
    </div>
  )
}

/** A labelled row around any field. */
export function FieldRow({ label, children, htmlFor }: { label: string; children: ReactNode; htmlFor?: string }) {
  return (
    <div className="photo-dialog-row">
      <label className="photo-dialog-label" htmlFor={htmlFor}>
        {label}
      </label>
      {children}
    </div>
  )
}

export function ColorRow({ label, value, onChange, disabled, testId }: { label: string; value: string; onChange: (value: string) => void; disabled?: boolean; testId?: string }) {
  // The chip opens the Color Picker over the dialog (plan 2026-11-24, stage 5).
  return (
    <FieldRow label={label}>
      <ColorWell value={value} label={label} disabled={disabled} onChange={(next) => onChange(next)} testId={testId} />
    </FieldRow>
  )
}

/** A number to type, or empty (`null`, the engine's default). */
export function NumberRow({ label, value, min, max, decimals, unit, onChange, testId }: { label: string; value: number | null; min: number; max: number; decimals: number; unit?: string; onChange: (value: number | null) => void; testId?: string }) {
  const id = testId ? `${testId}-input` : undefined
  // The text being typed, kept while the field has focus ("2." is a number on its way).
  const [typing, setTyping] = useState<string | null>(null)
  return (
    <FieldRow label={label} htmlFor={id}>
      <span className="photo-dialog-number">
        <input
          id={id}
          className="kui-field tabular-nums"
          type="number"
          value={typing ?? (value ?? '')}
          min={min}
          max={max}
          step={10 ** -decimals}
          onBlur={() => setTyping(null)}
          onChange={(event) => {
            const text = event.target.value
            setTyping(text)
            if (text.trim() === '') return onChange(null)
            const next = Number(text)
            // Kept as typed (a first digit may be under the minimum); the engine clamps what it runs.
            if (Number.isFinite(next)) onChange(next)
          }}
          data-testid={testId}
        />
        {unit ? <span className="photo-dialog-unit">{unit}</span> : null}
      </span>
    </FieldRow>
  )
}

export function TextRow({ label, value, placeholder, onChange, testId }: { label: string; value: string; placeholder?: string; onChange: (value: string) => void; testId?: string }) {
  const id = testId ? `${testId}-input` : undefined
  return (
    <FieldRow label={label} htmlFor={id}>
      <input id={id} className="kui-field" type="text" value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} data-testid={testId} />
    </FieldRow>
  )
}
