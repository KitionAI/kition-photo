import type { Preview } from '../../dialogs/preview'

/**
 * A gesture's result on the canvas while it is under way (plan 2026-11-24, stage 2): the Move
 * tool's layer, Free Transform's box, a nudge, Quick Selection's stroke. Each `push` runs the
 * command with the gesture's values so far, taking the previous run's step away first (stage 1's
 * preview: `history.deleteState`), so the history holds one step for the whole gesture. Pushes
 * that arrive while a run is under way wait, and only the latest runs: a fast pointer never
 * queues up runs. `finish` leaves exactly one step with the final values (keeping the last run
 * when it already has them); `cancel` leaves none.
 */
export type LivePreview = {
  push: (params: Record<string, unknown>) => void
  finish: (params: Record<string, unknown>) => Promise<void>
  cancel: () => Promise<void>
  /** Whether a run is on the canvas (its step in the history). */
  readonly shown: boolean
}

export function livePreview(preview: Preview, failed: (error: unknown) => void = () => undefined): LivePreview {
  let running: Promise<void> | null = null
  let next: Record<string, unknown> | null = null
  let closed = false
  const pump = () => {
    if (running || !next || closed) return
    const params = next
    next = null
    running = preview
      .show(params)
      .catch(failed)
      .finally(() => {
        running = null
        pump()
      })
  }
  const settle = async () => {
    next = null
    while (running) await running
  }
  return {
    get shown() {
      return preview.shown
    },
    push(params) {
      next = params
      pump()
    },
    async finish(params) {
      closed = true
      await settle()
      await preview.commit(params).catch(failed)
    },
    async cancel() {
      closed = true
      await settle()
      await preview.drop()
    },
  }
}
