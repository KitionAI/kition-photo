import { useCallback, useLayoutEffect, useRef, type PointerEvent as ReactPointerEvent } from 'react'
import type { KitCanvasView } from '@kition/sdk/editor'
import { useLive, type Live } from '../panels/live'
import type { CanvasPoint } from '../tools/BrushCursor'
import { accent } from '../tools/canvas/common'
import { guidePosition, rulerTicks } from './snap'
import type { GuideDraft } from './draw'

/** The rulers' thickness, in CSS pixels (the canvas moves in by as much while they show). */
export const RULER_SIZE = 18

type Side = 'top' | 'left'

/**
 * View › Rulers (plan 2026-11-24, stage 7): a ruler along the top and the left of the canvas,
 * numbered in document pixels as the view is zoomed and scrolled, the pointer's place marked on
 * each. A drag out of the top ruler makes a horizontal guide, out of the left one a vertical guide
 * (Alt the other way, Shift to the ruler's ticks): `onDraft` shows it while it is dragged, `onDrop`
 * adds it where it is let go over the canvas; let go over a ruler, it is not made.
 */
export function Rulers({ view, pointer, surface, onDraft, onDrop, label }: { view: Live<KitCanvasView | null>; pointer: Live<CanvasPoint>; surface: () => DOMRect | null; onDraft: (draft: GuideDraft | null) => void; onDrop: (orientation: 'horizontal' | 'vertical', position: number) => void; label: (side: Side) => string }) {
  const shownView = useLive(view)
  const at = useLive(pointer)
  const top = useRef<HTMLCanvasElement>(null)
  const left = useRef<HTMLCanvasElement>(null)
  useLayoutEffect(() => {
    for (const side of ['top', 'left'] as const) {
      const canvas = (side === 'top' ? top : left).current
      if (canvas && shownView) drawRuler(canvas, side, shownView, at)
    }
  }, [shownView, at])
  const drag = useRef<{ orientation: 'horizontal' | 'vertical'; position: number; inside: boolean } | null>(null)
  const place = useCallback(
    (event: ReactPointerEvent<HTMLCanvasElement>, orientation: 'horizontal' | 'vertical') => {
      const now = view.get()
      const box = surface()
      if (!now || !box) return null
      const zoom = now.zoom || 1
      const raw = orientation === 'horizontal' ? now.y + (event.clientY - box.top) / zoom : now.x + (event.clientX - box.left) / zoom
      const inside = event.clientX >= box.left && event.clientX < box.right && event.clientY >= box.top && event.clientY < box.bottom
      return { orientation, position: guidePosition(raw, zoom, event.shiftKey), inside }
    },
    [view, surface],
  )
  const handlers = (side: Side) => ({
    onPointerDown: (event: ReactPointerEvent<HTMLCanvasElement>) => {
      if (event.button !== 0) return
      event.preventDefault()
      event.currentTarget.setPointerCapture(event.pointerId)
      const orientation = (side === 'top') !== event.altKey ? 'horizontal' : 'vertical'
      drag.current = place(event, orientation)
    },
    onPointerMove: (event: ReactPointerEvent<HTMLCanvasElement>) => {
      const current = drag.current
      if (!current) return
      const next = place(event, current.orientation)
      if (!next) return
      drag.current = next
      onDraft({ orientation: next.orientation, position: next.position, index: null, leaving: !next.inside })
    },
    onPointerUp: (event: ReactPointerEvent<HTMLCanvasElement>) => {
      const current = drag.current
      drag.current = null
      onDraft(null)
      if (!current) return
      const next = place(event, current.orientation) ?? current
      if (next.inside) onDrop(next.orientation, next.position)
    },
    onPointerCancel: () => {
      drag.current = null
      onDraft(null)
    },
  })
  return (
    <>
      <div className="photo-ruler-corner" aria-hidden="true" />
      <canvas ref={top} className="photo-ruler photo-ruler-top" data-testid="photo-ruler-top" aria-label={label('top')} {...handlers('top')} />
      <canvas ref={left} className="photo-ruler photo-ruler-left" data-testid="photo-ruler-left" aria-label={label('left')} {...handlers('left')} />
    </>
  )
}

/** One ruler: its ticks and numbers in the ruler's text colour, and the pointer's place in the accent colour. */
function drawRuler(canvas: HTMLCanvasElement, side: Side, view: KitCanvasView, pointer: CanvasPoint) {
  const ratio = window.devicePixelRatio || 1
  const length = side === 'top' ? view.width : view.height
  const width = side === 'top' ? length : RULER_SIZE
  const height = side === 'top' ? RULER_SIZE : length
  if (canvas.width !== Math.round(width * ratio) || canvas.height !== Math.round(height * ratio)) {
    canvas.width = Math.round(width * ratio)
    canvas.height = Math.round(height * ratio)
  }
  const context = canvas.getContext('2d')
  if (!context) return
  context.setTransform(ratio, 0, 0, ratio, 0, 0)
  context.clearRect(0, 0, width, height)
  const style = getComputedStyle(canvas)
  context.strokeStyle = style.color
  context.fillStyle = style.color
  context.lineWidth = 1
  context.font = '9px system-ui, sans-serif'
  const ticks = rulerTicks(side === 'top' ? view.x : view.y, view.zoom, length)
  context.beginPath()
  for (const at of ticks.minor) {
    const p = Math.round(at) + 0.5
    if (side === 'top') {
      context.moveTo(p, RULER_SIZE - 4)
      context.lineTo(p, RULER_SIZE)
    } else {
      context.moveTo(RULER_SIZE - 4, p)
      context.lineTo(RULER_SIZE, p)
    }
  }
  for (const { at } of ticks.major) {
    const p = Math.round(at) + 0.5
    if (side === 'top') {
      context.moveTo(p, 0)
      context.lineTo(p, RULER_SIZE)
    } else {
      context.moveTo(0, p)
      context.lineTo(RULER_SIZE, p)
    }
  }
  context.stroke()
  for (const { at, label } of ticks.major) {
    if (side === 'top') context.fillText(label, Math.round(at) + 3, 9)
    else {
      context.save()
      context.translate(9, Math.round(at) + 3)
      context.rotate(Math.PI / 2)
      context.fillText(label, 0, 0)
      context.restore()
    }
  }
  if (pointer) {
    const p = Math.round(side === 'top' ? pointer.x : pointer.y) + 0.5
    context.beginPath()
    if (side === 'top') {
      context.moveTo(p, 0)
      context.lineTo(p, RULER_SIZE)
    } else {
      context.moveTo(0, p)
      context.lineTo(RULER_SIZE, p)
    }
    context.strokeStyle = accent()
    context.stroke()
  }
  // The edge against the canvas.
  context.beginPath()
  if (side === 'top') {
    context.moveTo(0, RULER_SIZE - 0.5)
    context.lineTo(width, RULER_SIZE - 0.5)
  } else {
    context.moveTo(RULER_SIZE - 0.5, 0)
    context.lineTo(RULER_SIZE - 0.5, height)
  }
  context.strokeStyle = style.borderColor || style.color
  context.stroke()
}
