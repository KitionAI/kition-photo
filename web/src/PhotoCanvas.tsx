import { useLayoutEffect, useMemo, useRef, type ComponentProps } from 'react'
import { HAND_TOOL, KitDocumentCanvas, useKitEditor, useUi, type KitTool } from '@kition/sdk/editor'
import type { PhotoToolId } from './tools/catalog'

type CanvasProps = ComponentProps<typeof KitDocumentCanvas> & { activeTool: PhotoToolId; tool: KitTool; onDenied: () => void }
type Context = Parameters<KitTool['begin']>[1]

/** Photo owns its canvas tools, so enforce host tool policy here even when no rail is mounted. */
export function PhotoCanvas({ activeTool, tool, onDenied, ...props }: CanvasProps) {
  const ui = useUi()
  const { host } = useKitEditor()
  const allowed = !host.readOnly && !ui.state(`tool:${activeTool}`).denied
  const allowedNow = useRef(allowed)
  allowedNow.current = allowed
  const guarded = useMemo(() => {
    let cancelGesture: (() => void) | null = null
    let context: Context | null = null
    const cancel = () => {
      cancelGesture?.()
      context?.overlay.clear()
    }
    const canvasTool: KitTool = {
      ...tool,
      begin(pointer, nextContext) {
        context = nextContext
        if (!allowedNow.current) return HAND_TOOL.begin(pointer, nextContext)
        const gesture = tool.begin(pointer, nextContext)
        if (!gesture) return
        let finished = false
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
            if (!finished) gesture.move?.(next, current)
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
        if (allowedNow.current) tool.hover?.(pointer, current)
      },
      doubleClick(pointer, current) {
        if (allowedNow.current) tool.doubleClick?.(pointer, current)
      },
    }
    return { tool: canvasTool, cancel }
  }, [tool])
  useLayoutEffect(() => {
    if (!allowed) guarded.cancel()
  }, [allowed, guarded])
  useLayoutEffect(() => {
    // Notify once per denial, independently of tool objects rebuilt by document updates.
    if (!allowed) onDenied()
  }, [allowed, onDenied])
  useLayoutEffect(() => guarded.cancel, [guarded])
  return <KitDocumentCanvas {...props} tool={allowed ? guarded.tool : { ...guarded.tool, cursor: HAND_TOOL.cursor }} />
}
