import { useCallback, useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from 'react'

/** Where a dragged row would land: on a row (above, below, or into a group), or on a footer button (`data-layer-drop`). */
export type LayerDrop = { layer: number; target: number; position: 'above' | 'below' | 'into' } | { layer: number; command: string }

/** The row being dragged and where the pointer is, for the ghost that follows it. */
export type LayerGhost = { layer: number; x: number; y: number }

/** How far the pointer moves before a press becomes a drag (CSS pixels). */
const DRAG_START = 4
/** How close to the list's top or bottom edge the pointer scrolls it, and how fast at the edge (pixels a frame). */
const SCROLL_EDGE = 24
const SCROLL_SPEED = 12

/** Where a row dragged from `layer` would land under a point. */
function dropAt(x: number, y: number, layer: number, groups: ReadonlySet<number>): LayerDrop | null {
  const element = document.elementFromPoint(x, y)
  const button = element?.closest<HTMLElement>('[data-layer-drop]')
  if (button?.dataset.layerDrop) return { layer, command: button.dataset.layerDrop }
  const row = element?.closest<HTMLElement>('[data-layer-row]')
  const target = Number(row?.dataset.layerRow)
  if (!row || !Number.isFinite(target) || target === layer) return null
  const box = row.getBoundingClientRect()
  const at = (y - box.top) / Math.max(1, box.height)
  return { layer, target, position: groups.has(target) && at >= 0.3 && at < 0.7 ? 'into' : at < 0.5 ? 'above' : 'below' }
}

/**
 * Upstream's row drag (`layer_drag_and_drop`, `footer_drop`): a press on a row that moves a few
 * pixels drags it; over a row it lands above, below, or (a group's middle) into it; over a footer
 * button it runs that button's command for the row (the whole selection when the row is in it).
 * Near the list's edges the list scrolls; a ghost of the row follows the pointer. The click that
 * ends a drag is swallowed (`swallowClick`).
 */
export function useLayerDrag({ list, groups, selected, run, enabled, document }: { list: RefObject<HTMLElement | null>; groups: ReadonlySet<number>; selected: readonly number[]; run: (command: string, params: Record<string, unknown>) => void; enabled: boolean; document: unknown }) {
  const [drop, setDrop] = useState<LayerDrop | null>(null)
  const [ghost, setGhost] = useState<LayerGhost | null>(null)
  const drag = useRef<{ layer: number; x: number; y: number; active: boolean; scroll: number; frame: number } | null>(null)
  const dragged = useRef(false)
  const latest = useRef({ groups, selected, run })
  latest.current = { groups, selected, run }
  const cancel = useCallback(() => {
    const current = drag.current
    if (!current) return
    cancelAnimationFrame(current.frame)
    dragged.current ||= current.active
    drag.current = null
    setDrop(null)
    setGhost(null)
  }, [])
  useLayoutEffect(() => {
    if (!enabled) cancel()
  }, [enabled, cancel])
  // A restored list or replacement document cannot revive the old pointer gesture.
  useLayoutEffect(() => cancel, [document, cancel])

  useEffect(() => {
    const update = (current: NonNullable<typeof drag.current>) => {
      setDrop(dropAt(current.x, current.y, current.layer, latest.current.groups))
      setGhost({ layer: current.layer, x: current.x, y: current.y })
    }
    // While the pointer rests near an edge, the list keeps scrolling and the drop follows the rows under it.
    const scroll = () => {
      const current = drag.current
      if (!current || !current.scroll || !list.current) return
      list.current.scrollTop += current.scroll
      update(current)
      current.frame = requestAnimationFrame(scroll)
    }
    const move = (event: PointerEvent) => {
      const current = drag.current
      if (!current) return
      if (!current.active && Math.hypot(event.clientX - current.x, event.clientY - current.y) < DRAG_START) return
      current.active = true
      current.x = event.clientX
      current.y = event.clientY
      const box = list.current?.getBoundingClientRect()
      const before = current.scroll
      current.scroll = !box ? 0 : event.clientY < box.top + SCROLL_EDGE ? -SCROLL_SPEED : event.clientY > box.bottom - SCROLL_EDGE ? SCROLL_SPEED : 0
      if (current.scroll && !before) current.frame = requestAnimationFrame(scroll)
      update(current)
    }
    const up = (event: PointerEvent) => {
      const current = drag.current
      drag.current = null
      if (!current) return
      cancelAnimationFrame(current.frame)
      if (!current.active) return
      dragged.current = true
      setDrop(null)
      setGhost(null)
      const target = event.type === 'pointerup' ? dropAt(event.clientX, event.clientY, current.layer, latest.current.groups) : null
      if (!target) return
      if ('command' in target) latest.current.run(target.command, latest.current.selected.includes(current.layer) ? {} : { layer: current.layer })
      else latest.current.run('layer.moveTo', { layer: target.layer, target: target.target, position: target.position })
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    window.addEventListener('pointercancel', up)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
      window.removeEventListener('pointercancel', up)
      if (drag.current) cancelAnimationFrame(drag.current.frame)
    }
  }, [list])

  return {
    drop,
    ghost,
    /** A press on a row: the start of a drag, if it moves. */
    press: (event: ReactPointerEvent<HTMLElement>, layer: number) => {
      dragged.current = false
      if (!enabled || event.button !== 0) return
      drag.current = { layer, x: event.clientX, y: event.clientY, active: false, scroll: 0, frame: 0 }
    },
    /** Whether the click now arriving ends a drag (and is not a click). */
    swallowClick: () => {
      const was = dragged.current
      dragged.current = false
      return was
    },
  }
}
