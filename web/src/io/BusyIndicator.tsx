import { useEffect, useState, useSyncExternalStore } from 'react'
import { Button, type Strings } from '@kition/sdk/editor'
import type { Busy } from './busy'

/**
 * The long command's progress (plan 2026-11-24, stage 6): after half a second, a bar at the foot of
 * the canvas with what runs, how long it has run, and Cancel. The engine gives no fraction done, so
 * the bar moves without a percentage. It blocks nothing: the editor's panels, menus and view keep
 * working while the engine finishes.
 */
export function BusyIndicator({ busy, t, onCancel }: { busy: Busy; t: Strings; onCancel: () => void }) {
  const job = useSyncExternalStore(
    (listener) => busy.subscribe(listener),
    () => busy.current,
  )
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!job) return
    const timer = window.setInterval(() => setNow(Date.now()), 250)
    return () => window.clearInterval(timer)
  }, [job])
  if (!job) return null
  const seconds = Math.max(0, Math.floor((now - job.started) / 1000))
  return (
    <div className="photo-busy" role="status" aria-live="polite" data-testid="photo-busy" data-label={job.label}>
      <div className="photo-busy-text">
        <span className="photo-busy-label">{t('busy.working', { label: job.label })}</span>
        <span className="photo-busy-time tabular-nums">{seconds} s</span>
      </div>
      <div className="photo-busy-bar" role="progressbar" aria-label={job.label} aria-busy="true">
        <span />
      </div>
      {job.cancellable ? (
        <Button size="sm" onClick={onCancel} data-testid="photo-busy-cancel">
          {t('busy.cancel')}
        </Button>
      ) : null}
    </div>
  )
}
