import { createContext, useContext, type ReactNode } from 'react'
import { type IconComponent } from '@kition/sdk/editor'
import { BarSeparator, Button, Checkbox, Choice as KitChoice, Hint as KitHint, IconButton, KitElement, NumberField, Segmented, useKitEditor } from '@kition/sdk/editor'
import { BLEND_MODES, RANGES, type OptionKey, type ToolOptions } from '../tools/options'
import type { BarProps } from './bar'
import { controlOf, optionId, type OptionControl } from './controls'

/**
 * The options bar's fields. Each is a Kit UI element (`control:options.<field>`): not drawn when
 * the host hides it, its inputs disabled when the host disables it. The option fields take an
 * option's name (`k`) and read and change it in the bar's options, with its range from `RANGES`.
 */

const BarContext = createContext<BarProps | null>(null)

export const BarProvider = BarContext.Provider

/** The bar's tool, options, and editor state. */
export function useBar() {
  const bar = useContext(BarContext)
  if (!bar) throw new Error('an options-bar field outside OptionsBar')
  return bar
}

type Key<T> = { [K in OptionKey]: ToolOptions[K] extends T ? K : never }[OptionKey]

/** An upstream label without its trailing colon, where a control draws its own label. */
export const bare = (label: string) => label.replace(/\s*[:：]\s*$/, '')

/** `text` with its `{name}` placeholders filled (upstream's phrases carry key names this way). */
export const fill = (text: string, values: Record<string, string>) => text.replace(/\{(\w+)\}/g, (whole, name: string) => values[name] ?? whole)

/** A field as a Kit UI element; `option` names the option it shows (the flow test reads it to hold each bar to its tool's field list). */
export function Opt({ field, option, children }: { field: OptionControl; option?: OptionKey; children: ReactNode }) {
  return (
    <KitElement id={optionId(field)}>
      {option ? (
        <span className="photo-option" data-option={option}>
          {children}
        </span>
      ) : (
        children
      )}
    </KitElement>
  )
}

/** A scrubbable number option; `min`, `max`, and `step` narrow its range where a bar needs it. */
export function Num({ k, label, unit, min, max, step = 1, disabled }: { k: Key<number>; label: string; unit?: string; min?: number; max?: number; step?: number; disabled?: boolean }) {
  const { o, set } = useBar()
  const field = controlOf(k)
  const [low, high] = RANGES[k as keyof typeof RANGES]
  return (
    <Opt field={field} option={k}>
      <NumberField label={bare(label)} value={o[k]} min={min ?? low} max={max ?? high} step={step} unit={unit} disabled={disabled} onChange={(value) => set({ [k]: value })} testId={`photo-option-${field}`} />
    </Opt>
  )
}

export function Check({ k, label, disabled }: { k: Key<boolean>; label: string; disabled?: boolean }) {
  const { o, set } = useBar()
  const field = controlOf(k)
  return (
    <Opt field={field} option={k}>
      <Checkbox label={bare(label)} checked={o[k]} disabled={disabled} onChange={(checked) => set({ [k]: checked })} testId={`photo-option-${field}`} />
    </Opt>
  )
}

/** A drop-down option; `quiet` draws no label, as upstream's unlabeled drop-downs. `onChange` sets more than the option itself. */
export function Choice({ k, label, quiet, choices, disabled, onChange }: { k: Key<string>; label: string; quiet?: boolean; choices: readonly (readonly [string, string])[]; disabled?: boolean; onChange?: (value: string) => void }) {
  const { o, set } = useBar()
  const field = controlOf(k)
  return (
    <Opt field={field} option={k}>
      <KitChoice label={bare(label)} quiet={quiet} value={o[k]} choices={choices} disabled={disabled} onChange={onChange ?? ((value) => set({ [k]: value }))} testId={`photo-option-${field}`} />
    </Opt>
  )
}

export type IconChoice = { value: string; icon: IconComponent; label: string }

/** Icon buttons of which one is pressed (selection modes, gradient styles, alignment). */
export function Icons({ k, label, choices }: { k: Key<string>; label: string; choices: readonly IconChoice[] }) {
  const { o, set } = useBar()
  const field = controlOf(k)
  return (
    <Opt field={field} option={k}>
      <Segmented label={label} value={o[k]} segments={choices} onChange={(value) => set({ [k]: value })} testId={`photo-option-${field}`} />
    </Opt>
  )
}

/** An icon button that switches a yes-or-no option. */
export function Toggle({ k, icon, label }: { k: Key<boolean>; icon: IconComponent; label: string }) {
  const { o, set } = useBar()
  return <Icon field={controlOf(k)} option={k} icon={icon} label={label} pressed={o[k]} onClick={() => set({ [k]: !o[k] })} />
}

/** One icon button: a switch when `pressed` is given, else an action. */
export function Icon({ field, option, icon, label, pressed, disabled, keepFocus, onClick }: { field: OptionControl; option?: OptionKey; icon: IconComponent; label: string; pressed?: boolean; disabled?: boolean; keepFocus?: boolean; onClick: () => void }) {
  return (
    <Opt field={field} option={option}>
      <IconButton icon={icon} label={label} pressed={pressed} disabled={disabled} onMouseDown={keepFocus ? (event) => event.preventDefault() : undefined} onClick={onClick} data-testid={`photo-option-${field}`} />
    </Opt>
  )
}

/** A text button running an action (upstream's secondary buttons). */
export function Act({ field, label, disabled, title, onClick }: { field: OptionControl; label: string; disabled?: boolean; title?: string; onClick: () => void }) {
  return (
    <Opt field={field}>
      <Button disabled={disabled} title={title} onClick={onClick} data-testid={`photo-option-${field}`}>
        {label}
      </Button>
    </Opt>
  )
}

export const Sep = BarSeparator

/** Upstream's muted usage hint. */
export function Hint({ children, testId }: { children: ReactNode; testId?: string }) {
  return <KitHint testId={testId}>{children}</KitHint>
}

/** A blend-mode drop-down (the brush's mode, the Gradient's). */
export function BlendMode({ k }: { k: 'mode' | 'gradientMode' }) {
  const { phrase } = useKitEditor()
  return <Choice k={k} label={phrase('Mode')} choices={BLEND_MODES.map(([value, label]) => [value, phrase(label)] as const)} />
}

/** Tolerance, Anti-alias, Contiguous, and Sample All Layers (the Paint Bucket calls the last All Layers), shared by the Magic Wand, Paint Bucket, and Magic Eraser. */
export function FillSampling({ allLayers }: { allLayers?: string }) {
  const { phrase } = useKitEditor()
  return (
    <>
      <Num k="tolerance" label={phrase('Tolerance')} />
      <Check k="antiAlias" label={phrase('Anti-alias')} />
      <Check k="contiguous" label={phrase('Contiguous')} />
      <Check k="sampleAllLayers" label={allLayers ?? phrase('Sample All Layers')} />
    </>
  )
}
