import { useSyncExternalStore } from 'react'
import { IconButton, TesseraIcons, type Strings } from '@kition/sdk/editor'
import { explainError } from './errors'

/**
 * The engine's errors as the person reads them (plan 2026-11-24, stage 6): a notice with the
 * editor's own sentence (`errors.ts`), in the editor's language; an error the editor does not know
 * shows a general sentence and, folded under Details, what the engine said. A known error closes
 * by itself after a while; an unknown one stays until closed, so its detail can be read.
 */

export type Notice = { id: number; title: string; message: string; detail: string | null }

let notices: readonly Notice[] = []
let serial = 0
const listeners = new Set<() => void>()
const emit = () => {
  for (const listener of [...listeners]) listener()
}

/** How long a notice for a known error stays. */
const KNOWN_MS = 8000
/** At most this many notices show; older ones give way. */
const MOST = 3

export function dismissNotice(id: number) {
  notices = notices.filter((notice) => notice.id !== id)
  emit()
}

/** Shows `message` (and `detail`, folded) under `title`. */
export function pushNotice(title: string, message: string, detail: string | null = null): number {
  serial += 1
  const id = serial
  notices = [...notices, { id, title, message, detail }].slice(-MOST)
  emit()
  if (!detail) setTimeout(() => dismissNotice(id), KNOWN_MS)
  return id
}

/** Reports `error` under `title` in the editor's words (`t`): the `report` the editor state takes. */
export function reportError(t: Strings, title: string, error: unknown): number {
  const explained = explainError(error)
  return pushNotice(title || t('error.title'), t(explained.key), explained.detail)
}

const subscribe = (listener: () => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function ErrorNotices({ t }: { t: Strings }) {
  const shown = useSyncExternalStore(subscribe, () => notices)
  if (!shown.length) return null
  return (
    <div className="photo-notices" role="region" aria-label={t('error.title')}>
      {shown.map((notice) => (
        <div key={notice.id} className="photo-notice" role="alert" data-testid="photo-error">
          <div className="photo-notice-head">
            <strong className="photo-notice-title">{notice.title}</strong>
            <IconButton icon={TesseraIcons.XIcon} label={t('error.dismiss')} onClick={() => dismissNotice(notice.id)} data-testid="photo-error-close" />
          </div>
          <p className="photo-notice-message" data-testid="photo-error-message">
            {notice.message}
          </p>
          {notice.detail ? (
            <details className="photo-notice-detail" data-testid="photo-error-detail">
              <summary>{t('error.details')}</summary>
              <code>{notice.detail}</code>
            </details>
          ) : null}
        </div>
      ))}
    </div>
  )
}
