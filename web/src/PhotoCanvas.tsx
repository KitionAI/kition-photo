import { useEffect, useLayoutEffect, useMemo, useRef, type ComponentProps, type MutableRefObject } from 'react'
import { HAND_TOOL, KitDocumentCanvas, ZOOM_TOOL, useKitEditor, useUi, type KitPointer, type KitTool } from '@kition/sdk/editor'
import type { PhotoToolId } from './tools/catalog'
import { useLive, type Live } from './panels/live'
import { modifiersOf, takesSpace, type HeldKeys } from './tools/held'
import { wheelZoomFactor } from './tools/canvas/geometry'

type CanvasProps = ComponentProps<typeof KitDocumentCanvas> & {
  activeTool: PhotoToolId
  tool: KitTool
  onDenied: () => void
  /** The keys held now (Space, Cmd, Alt…), which this canvas keeps up to date. */
  held: Live<HeldKeys>
  /** The Move tool a press with Cmd held takes (Photoshop's temporary Move), or null where Cmd is the tool's own. */
  temporaryMove: KitTool | null
  /** A cursor the active tool asks for now (a handle's arrows, the brush outline's none), over its own. */
  cursor: string | null
  /** The canvas's viewport, as the tools last saw it (for the wheel's zoom about the pointer). */
  viewport: MutableRefObject<Parameters<KitTool['begin']>[1]['viewport'] | null>
}
type Context = Parameters<KitTool['begin']>[1]
type Gesture = Exclude<ReturnType<KitTool['begin']>, void>
/** A gesture that moves its own shape while Space is held (a marquee's box), by document pixels, instead of the view panning. */
export type SpaceMoves = { spaceMove?: (dx: number, dy: number) => void }

/**
 * Photo owns its canvas tools, so enforce host tool policy here even when no rail is mounted. It
 * also adds Photoshop's held keys, in the page (the shared canvas has none of them): Space pans
 * (with Cmd it zooms in, Cmd and Alt out) before a press and during any drag, where a marquee's box
 * moves instead; Cmd turns a press into the Move tool's (plan 2026-11-24, stage 2); and the wheel
 * zooms with Alt as with Cmd, a mouse notch by at most 1.2 times.
 */
export function PhotoCanvas({ activeTool, tool, onDenied, held, temporaryMove, cursor, viewport, ...props }: CanvasProps) {
  const ui = useUi()
  const { host } = useKitEditor()
  const allowed = !host.readOnly && !ui.state(`tool:${activeTool}`).denied
  const allowedNow = useRef(allowed)
  allowedNow.current = allowed
  const temporaryNow = useRef(temporaryMove)
  temporaryNow.current = temporaryMove
  const keys = useLive(held)
  const guarded = useMemo(() => {
    let cancelGesture: (() => void) | null = null
    let context: Context | null = null
    const cancel = () => {
      cancelGesture?.()
      context?.overlay.clear()
    }
    /** The press's own tool: the Hand or Zoom while Space is held, the Move tool with Cmd, else the active tool. */
    const startOf = (pointer: KitPointer, nextContext: Context): Gesture | void => {
      const now = held.get()
      if (now.space) return (pointer.meta ? ZOOM_TOOL : HAND_TOOL).begin(pointer, nextContext)
      const move = temporaryNow.current
      // Cmd is the Move tool's for this press: its own Auto-Select, not Cmd's toggle of it.
      if (move && pointer.meta) return move.begin({ ...pointer, meta: false }, nextContext)
      return tool.begin(pointer, nextContext)
    }
    const canvasTool: KitTool = {
      ...tool,
      begin(pointer, nextContext) {
        context = nextContext
        viewport.current = nextContext.viewport
        held.set({ ...held.get(), meta: pointer.meta, alt: pointer.alt, shift: pointer.shift })
        if (!allowedNow.current) return HAND_TOOL.begin(pointer, nextContext)
        const gesture = startOf(pointer, nextContext) as (Gesture & SpaceMoves) | void
        if (!gesture) return
        let finished = false
        let last = { x: pointer.clientX, y: pointer.clientY }
        const stop = () => {
          if (finished) return
          finished = true
          cancelGesture = null
          gesture.cancel?.(context ?? nextContext)
          context?.overlay.clear()
        }
        cancelGesture = stop
        return {
          move(next, current) {
            context = current
            if (!allowedNow.current) stop()
            if (finished) return
            const dx = next.clientX - last.x
            const dy = next.clientY - last.y
            last = { x: next.clientX, y: next.clientY }
            // Space held mid-drag: the view pans (a marquee's box moves), the gesture waits.
            if (held.get().space) {
              if (gesture.spaceMove) {
                const zoom = current.viewport.view.zoom || 1
                gesture.spaceMove(dx / zoom, dy / zoom)
                gesture.move?.(next, current)
              } else current.viewport.panBy(dx, dy)
              return
            }
            gesture.move?.(next, current)
          },
          end(next, current) {
            context = current
            if (!allowedNow.current) stop()
            if (finished) return
            finished = true
            cancelGesture = null
            gesture.end?.(next, current)
          },
          cancel(current) {
            context = current
            stop()
          },
        }
      },
      hover(pointer, current) {
        context = current
        viewport.current = current.viewport
        if (allowedNow.current && !held.get().space) tool.hover?.(pointer, current)
      },
      track(pointer, current) {
        viewport.current = current.viewport
        tool.track?.(pointer, current)
      },
      doubleClick(pointer, current) {
        if (allowedNow.current && !held.get().space) tool.doubleClick?.(pointer, current)
      },
    }
    return { tool: canvasTool, cancel }
  }, [tool, held, viewport])
  useLayoutEffect(() => {
    if (!allowed) guarded.cancel()
  }, [allowed, guarded])
  useLayoutEffect(() => {
    // Notify once per denial, independently of tool objects rebuilt by document updates.
    if (!allowed) onDenied()
  }, [allowed, onDenied])
  useLayoutEffect(() => guarded.cancel, [guarded])

  // The held keys, from the window (the shared canvas keeps Space to itself otherwise: it is
  // Photo's here, so Cmd+Space can zoom and a drag can pan). Space typed into a field or pressed
  // on a button stays theirs.
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      const down = event.type === 'keydown'
      let space = held.get().space
      if (event.code === 'Space' && !event.isComposing) {
        if (down && takesSpace(event.target)) return
        space = down
        event.preventDefault()
        event.stopPropagation()
      }
      held.set(modifiersOf(event, space))
    }
    const reset = () => held.set({ space: false, meta: false, alt: false, shift: false, capsLock: false })
    window.addEventListener('keydown', key, true)
    window.addEventListener('keyup', key, true)
    window.addEventListener('blur', reset)
    return () => {
      window.removeEventListener('keydown', key, true)
      window.removeEventListener('keyup', key, true)
      window.removeEventListener('blur', reset)
    }
  }, [held])

  // The wheel over the canvas: Cmd, Ctrl (and a trackpad's pinch) or Alt zooms about the pointer,
  // a mouse notch by at most 1.2 times; other wheels pan as the shared canvas does.
  useEffect(() => {
    const wheel = (event: WheelEvent) => {
      const target = event.target
      if (!(target instanceof Element) || target.getAttribute('data-testid') !== 'kit-canvas-surface') return
      if (!(event.ctrlKey || event.metaKey || event.altKey)) return
      const view = viewport.current
      if (!view) return
      event.preventDefault()
      event.stopPropagation()
      view.zoomTo(view.view.zoom * wheelZoomFactor(event.deltaY || (event.altKey ? event.deltaX : 0), event.deltaMode), event.clientX, event.clientY)
    }
    window.addEventListener('wheel', wheel, { capture: true, passive: false })
    return () => window.removeEventListener('wheel', wheel, { capture: true })
  }, [viewport])

  const shown = !allowed
    ? HAND_TOOL.cursor
    : keys.space
      ? keys.meta
        ? keys.alt
          ? 'zoom-out'
          : 'zoom-in'
        : 'grab'
      : keys.meta && temporaryMove
        ? temporaryMove.cursor
        : (cursor ?? tool.cursor)
  return <KitDocumentCanvas {...props} tool={{ ...guarded.tool, cursor: shown }} />
}
