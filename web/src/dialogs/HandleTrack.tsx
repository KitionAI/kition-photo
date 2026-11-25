import { useRef, type KeyboardEvent, type PointerEvent } from 'react'

export type TrackHandle = {
  value: number
  label: string
  /** The handle's fill (black, grey, white for Levels). */
  shade: string
  testId?: string
}

/**
 * A track with handles under a histogram, as Levels' input and output sliders (Photoshop's
 * triangles): a handle follows the pointer while dragged and reports each value with `final`
 * false, then the last with `final` true; the arrows move the focused handle by 1 (Shift 10).
 */
export function HandleTrack({ handles, min = 0, max = 255, onChange, gradient, testId }: { handles: readonly TrackHandle[]; min?: number; max?: number; onChange: (index: number, value: number, final: boolean) => void; gradient?: string; testId?: string }) {
  const track = useRef<HTMLDivElement>(null)
  const dragging = useRef<{ index: number; pointerId: number } | null>(null)
  const valueAt = (clientX: number) => {
    const box = track.current?.getBoundingClientRect()
    if (!box || box.width <= 0) return min
    return min + Math.min(1, Math.max(0, (clientX - box.left) / box.width)) * (max - min)
  }
  const down = (index: number) => (event: PointerEvent<HTMLSpanElement>) => {
    if (event.button !== 0 || dragging.current) return
    event.preventDefault()
    event.currentTarget.focus({ preventScroll: true })
    event.currentTarget.setPointerCapture?.(event.pointerId)
    dragging.current = { index, pointerId: event.pointerId }
  }
  const move = (event: PointerEvent<HTMLSpanElement>) => {
    const drag = dragging.current
    if (drag && drag.pointerId === event.pointerId) onChange(drag.index, valueAt(event.clientX), false)
  }
  const up = (event: PointerEvent<HTMLSpanElement>) => {
    const drag = dragging.current
    if (!drag || drag.pointerId !== event.pointerId) return
    dragging.current = null
    onChange(drag.index, valueAt(event.clientX), true)
  }
  const key = (index: number) => (event: KeyboardEvent<HTMLSpanElement>) => {
    const step = event.shiftKey ? 10 : 1
    const by = event.key === 'ArrowRight' || event.key === 'ArrowUp' ? step : event.key === 'ArrowLeft' || event.key === 'ArrowDown' ? -step : 0
    if (!by) return
    event.preventDefault()
    event.stopPropagation()
    onChange(index, Math.min(max, Math.max(min, handles[index].value + by)), true)
  }
  return (
    <div className="photo-handle-track" ref={track} data-testid={testId}>
      {gradient ? <div className="photo-handle-track-ramp" style={{ background: gradient }} /> : null}
      {handles.map((handle, index) => (
        <span
          key={handle.label}
          className="photo-handle"
          role="slider"
          tabIndex={0}
          aria-label={handle.label}
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={Math.round(handle.value * 100) / 100}
          title={handle.label}
          style={{ left: `${((handle.value - min) / (max - min)) * 100}%`, ['--photo-handle-fill' as string]: handle.shade }}
          onPointerDown={down(index)}
          onPointerMove={move}
          onPointerUp={up}
          onPointerCancel={up}
          onKeyDown={key(index)}
          data-testid={handle.testId}
        />
      ))}
    </div>
  )
}
