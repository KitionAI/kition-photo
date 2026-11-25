import { useEffect, useState } from 'react'
import { useKitStore, type KitEngine, type KitTool } from '@kition/sdk/editor'
import { HIT_RADIUS } from '../tools/canvas/common'
import { DEFAULT_VIEW, validView, type ViewSettings } from './view'
import { NO_GUIDES, guideAt, guidePosition, type Guides } from './snap'
import type { GuideDraft } from './draw'

/** View's rulers, extras and snapping, kept in the kit store (they last across sessions, every Photo tab follows them). */
export function useViewSettings() {
  return useKitStore<ViewSettings>('kit.photo', 'view', { initial: DEFAULT_VIEW, validate: validView })
}

/** The document's guides as the engine has them now (the binding's `guides`), read again after every change. */
export function useDocGuides(doc: KitEngine | null, revision: number): Guides {
  const [guides, setGuides] = useState<Guides>(NO_GUIDES)
  useEffect(() => {
    if (!doc) {
      setGuides(NO_GUIDES)
      return undefined
    }
    let current = true
    doc.invoke<string>('guides').then(
      (answer) => {
        if (!current) return
        const value = JSON.parse(answer) as Partial<Guides>
        const next = { horizontal: Array.isArray(value.horizontal) ? value.horizontal : [], vertical: Array.isArray(value.vertical) ? value.vertical : [] }
        setGuides((before) => (JSON.stringify(before) === JSON.stringify(next) ? before : next))
      },
      () => undefined,
    )
    return () => {
      current = false
    }
  }, [doc, revision])
  return guides
}

/** What a guide drag needs from the editor. */
export type GuideControl = {
  guides: () => Guides
  /** Guides can be grabbed now: they show and are not locked. */
  grabbable: () => boolean
  /** The canvas's size in its CSS pixels: a guide let go outside it (over a ruler) is deleted. */
  size: () => { width: number; height: number } | null
  draft: (draft: GuideDraft | null) => void
  move: (orientation: 'horizontal' | 'vertical', index: number, position: number) => void
  remove: (orientation: 'horizontal' | 'vertical', index: number) => void
  setCursor: (cursor: string | null) => void
}

/**
 * `tool` with guides to grab (the Move tool, and the Move tool Cmd lends): a press on a guide drags
 * it (Shift to the ruler's ticks), let go over the canvas it moves there, let go over a ruler (or
 * off the canvas) it is deleted; the cursor shows the guide's direction over it.
 */
export function withGuides(tool: KitTool, control: GuideControl): KitTool {
  let pointing = false
  const hitOf = (pointer: { x: number; y: number }, zoom: number) => (control.grabbable() ? guideAt(control.guides(), pointer, HIT_RADIUS / (zoom || 1)) : null)
  return {
    ...tool,
    begin: (pointer, context) => {
      const zoom = context.viewport.view.zoom || 1
      const hit = hitOf(pointer, zoom)
      if (!hit) return tool.begin(pointer, context)
      const { orientation, index } = hit
      const from = control.guides()[orientation][index]
      let last = { position: from, leaving: false }
      const place = (at: { x: number; y: number; canvasX: number; canvasY: number; shift: boolean }) => {
        const size = control.size()
        const leaving = at.canvasX < 0 || at.canvasY < 0 || (size != null && (at.canvasX > size.width || at.canvasY > size.height))
        last = { position: guidePosition(orientation === 'vertical' ? at.x : at.y, zoom, at.shift), leaving }
        control.draft({ orientation, position: last.position, index, leaving })
      }
      return {
        move: place,
        end: (at) => {
          place(at)
          control.draft(null)
          if (last.leaving) control.remove(orientation, index)
          else if (last.position !== from) control.move(orientation, index, last.position)
        },
        cancel: () => control.draft(null),
      }
    },
    hover: (pointer, context) => {
      const hit = hitOf(pointer, context.viewport.view.zoom)
      if (hit) {
        pointing = true
        control.setCursor(hit.orientation === 'vertical' ? 'col-resize' : 'row-resize')
        return
      }
      if (pointing) {
        pointing = false
        control.setCursor(null)
      }
      tool.hover?.(pointer, context)
    },
  }
}
