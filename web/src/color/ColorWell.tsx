import { useRef, type CSSProperties, type MouseEvent as ReactMouseEvent, type ReactNode } from 'react'
import { useColorPicker } from './ColorPickerHost'

/**
 * A colour chip that opens the editor's Color Picker (plan 2026-11-24, stage 5). It is still a
 * colour input underneath, so its value reads and fills as one (a script, a test, an assistive
 * tool), but a click opens the Color Picker instead of the browser's; outside an editor (no picker
 * around) the browser's opens as before.
 *
 * `onChange` gets the chosen colour once, on OK (`final` true), or each colour a fill gives the
 * input (`final` false, then true when the input lets go); `onLive` the colours the person moves
 * through in the picker, and the starting one back on Cancel.
 */
export function ColorWell({
  value,
  label,
  title = label,
  onChange,
  onLive,
  disabled,
  className = 'photo-swatch',
  style,
  testId,
  children,
}: {
  value: string
  label: string
  /** The picker's title; the label by default. */
  title?: string
  onChange: (value: string, final: boolean) => void
  onLive?: (value: string) => void
  disabled?: boolean
  className?: string
  style?: CSSProperties
  testId?: string
  children?: ReactNode
}) {
  const pick = useColorPicker()
  // A fill's last colour, given once more as final when the input lets go.
  const filled = useRef<string | null>(null)
  const settle = () => {
    if (filled.current != null) onChange(filled.current, true)
    filled.current = null
  }
  const open = (event: ReactMouseEvent<HTMLInputElement>) => {
    settle()
    if (!pick) return
    // The browser's own picker never opens: the editor's does.
    event.preventDefault()
    if (disabled) return
    void pick({ title, value, onLive }).then((chosen) => {
      if (chosen && chosen !== value.toLowerCase()) onChange(chosen, true)
    })
  }
  return (
    <label className={className} style={{ background: value, ...style }} title={label} data-disabled={disabled}>
      <input
        type="color"
        value={value}
        disabled={disabled}
        aria-label={label}
        onClick={open}
        onChange={(event) => {
          filled.current = event.target.value
          onChange(event.target.value, false)
        }}
        onBlur={settle}
        data-testid={testId}
      />
      {children}
    </label>
  )
}
