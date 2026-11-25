import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { KitEngine, RasterRect } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { disabledWithMaskTarget, maskTargetApplies, withMaskTarget, type MaskTargetCommands } from './maskTarget'

type Session = { doc: KitEngine | null; state: PhotoInfo | null; run: (command: string, params?: Record<string, unknown>, damage?: RasterRect | null) => Promise<unknown> }

/** The engine's `maskTargetCommands` answer, or none when it cannot give one (an older engine). */
async function aimed(doc: KitEngine): Promise<MaskTargetCommands> {
  try {
    const answer = JSON.parse(await doc.invoke<string>('maskTargetCommands')) as Partial<MaskTargetCommands>
    return { follows: answer.follows ?? [], disabled: answer.disabled ?? [] }
  } catch {
    // An older, live engine can lack this extra. A disposed engine must not turn a
    // pending command into an untargeted edit of whichever document opened next.
    await doc.disabledCommands()
    return { follows: [], disabled: [] }
  }
}

/**
 * The mask target, and the session the editor runs through: every command it runs (canvas tools,
 * menus, keys, dialogs) is aimed at the active layer's mask while the mask is the target, as
 * upstream's central `run` does, and its enabled checks ask the engine with that aim (a targeted
 * mask enables Invert on an adjustment layer). Viewing a mask targets it too. The agent's runs
 * (`ai/run`) are not aimed: they name their own target. The checks are asked again after every change, and every way the target
 * changes (a thumbnail or channel click) runs a command first.
 */
export function useMaskTarget<S extends Session>(session: S) {
  const [maskTarget, setMaskTarget] = useState(false)
  const { doc } = session
  const documentNow = useRef(doc)
  documentNow.current = doc
  useLayoutEffect(() => { setMaskTarget(false) }, [doc])
  // Viewing a layer mask (Alt-click, the Channels mask eye, or anything else that sets the view) targets it, as upstream's `sync_mask_targets`.
  const view = session.state?.channels?.layerMaskView
  const viewKey = view ? `${view.layer}:${view.mode}` : null
  useEffect(() => {
    if (viewKey) setMaskTarget(true)
  }, [doc, viewKey])
  const applies = maskTargetApplies(maskTarget, session.state)
  const appliesNow = useRef(applies)
  appliesNow.current = applies
  // The commands that follow the target are the engine's, fixed for its build: asked once per document.
  const follows = useMemo(() => (doc ? aimed(doc).then((answer) => new Set(answer.follows), () => null) : Promise.resolve(null)), [doc])
  const aimedDoc = useMemo(
    () =>
      doc
        ? new Proxy(doc, {
            get(target, property) {
              if (property === 'disabledCommands') {
                return async () => {
                  const disabled = await target.disabledCommands()
                  return appliesNow.current ? disabledWithMaskTarget(disabled, await aimed(target)) : disabled
                }
              }
              const value: unknown = Reflect.get(target, property, target)
              return typeof value === 'function' ? value.bind(target) : value
            },
          })
        : null,
    [doc],
  )
  const followsNow = useRef(follows)
  followsNow.current = follows
  // Stable, as the session's own `run` is: callers keep it across documents.
  const baseRun = session.run
  /** `params` aimed as `run` aims them; null when the document changed meanwhile. */
  const aim = useCallback(async (command: string, params: Record<string, unknown>) => {
    const open = documentNow.current
    const targeted = appliesNow.current
    const commands = await followsNow.current
    if (!commands || documentNow.current !== open) return null
    return withMaskTarget(command, params, commands, targeted)
  }, [])
  const run = useCallback(
    async (command: string, params: Record<string, unknown> = {}, damage: RasterRect | null = null) => {
      const aimed = await aim(command, params)
      return aimed ? baseRun(command, aimed, damage) : null
    },
    [aim, baseRun],
  )
  const aimedSession = { ...session, doc: aimedDoc, run } as S
  // `aim` is for what reaches the engine beside `run`: a live stroke's begin (plan 2026-11-24, stage 3).
  return { session: aimedSession, maskTarget: applies, setMaskTarget, aim }
}
