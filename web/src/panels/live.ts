import { useSyncExternalStore } from 'react'
import type { KitTool } from '@kition/kit-editor'

/**
 * Values that change with every pointer move or scroll (where the pointer is over the document,
 * how the canvas views it): kept outside React state, so they redraw only the panels that read
 * them (Info, Navigator), never the whole editor.
 */
export type Live<T> = { get: () => T; set: (value: T) => void; subscribe: (listener: () => void) => () => void }

export function createLive<T>(initial: T): Live<T> {
  let value = initial
  const listeners = new Set<() => void>()
  return {
    get: () => value,
    set(next) {
      if (Object.is(next, value)) return
      value = next
      for (const listener of listeners) listener()
    },
    subscribe(listener) {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}

/** A live value, redrawing its reader when it changes. */
export function useLive<T>(live: Live<T>): T {
  return useSyncExternalStore(live.subscribe, live.get, live.get)
}

/** The pointer over the document, in document pixels (upstream's `hover_doc`), or null off the canvas. */
export type DocPoint = { x: number; y: number } | null

/**
 * `tool`, reporting the pointer in document pixels on every move over the canvas, whatever it is
 * doing (hovering, a gesture, the view panned with Space or the Hand tool: Kit UI's `track`), and
 * null when it leaves the canvas (`leave`): the Info panel's readout.
 */
export function trackPointer(tool: KitTool, report: (point: DocPoint) => void): KitTool {
  return {
    ...tool,
    track(pointer, context) {
      report({ x: pointer.x, y: pointer.y })
      tool.track?.(pointer, context)
    },
    leave(context) {
      report(null)
      tool.leave?.(context)
    },
  }
}
