import { useEffect, useMemo, useRef, useState } from 'react'
import { type IconComponent } from '@kition/kit-editor'
import { TesseraIcons } from '@kition/kit-editor'
import { Button, Dialog, IconButton, KitElement, PanelFooter, Thumbnail, deviceScale, useKitEditor, useUi } from '@kition/kit-editor'
import type { PhotoInfo } from '../photo'
import { useThumbnails, type ThumbnailSource } from './thumbnails'

const Brush = TesseraIcons.BrushIcon
const Camera = TesseraIcons.CameraIcon
const Image = TesseraIcons.ImageIcon
const Layers = TesseraIcons.LayersIcon
const PaintBucket = TesseraIcons.PaintBucketIcon
const SlidersHorizontal = TesseraIcons.SlidersHorizontalIcon
const SquareDashed = TesseraIcons.SquareDashedIcon
const Trash2 = TesseraIcons.TrashIcon

/**
 * Upstream's History panel (`panels.rs` `history`): the snapshot row (the document as opened, its
 * bottom layer's thumbnail and the document's name), then every state, those Redo steps through
 * dimmed after the current one. Clicking a state undoes or redoes to it (`history.goTo`, one
 * command). The footer deletes the current state, once confirmed (`history.deleteState`); a new document from a state would open a second document
 * in the engine, which the web editor cannot show (hidden, `menus/hidden.ts`), and the engine has
 * no snapshots yet.
 */

/** The snapshot row's thumbnail side (CSS pixels), upstream's 28. */
const SNAPSHOT = 28

/** Upstream's icon for a state, from its name. */
export function stateIcon(name: string): IconComponent {
  if (name.includes('Brush') || name.includes('Eraser')) return Brush
  if (name.includes('Layer')) return Layers
  if (['Select', 'Marquee', 'Deselect', 'Lasso'].some((word) => name.includes(word))) return SquareDashed
  if (name.includes('Fill')) return PaintBucket
  if (name === 'Open') return Image
  return SlidersHorizontal
}

export function HistoryPanel({ info }: { info: PhotoInfo }) {
  const { run, enabled, phrase, t, session } = useKitEditor()
  const ui = useUi()
  const list = useRef<HTMLUListElement>(null)
  const [confirming, setConfirming] = useState(false)
  // A host's `disable` of the list, or a history with only its opening state, stops the rows.
  const listDisabled = ui.state('control:history.list').disabled || !enabled('history.goTo')
  const current = info.historyIndex
  const states = [...info.history.map((name) => ({ name, redo: false })), ...info.historyRedo.map((name) => ({ name, redo: true }))]
  // The document's first (bottom) layer, as upstream's snapshot row shows it.
  const bottom = [...info.layers].reverse().find((layer) => layer.depth === 0)
  const sources = useMemo<ThumbnailSource[]>(() => (bottom?.id != null ? [{ kind: 'layer', layer: bottom.id }] : []), [bottom?.id])
  const thumbnail = useThumbnails(session.doc, session.change.revision, sources, SNAPSHOT * deviceScale())

  // The current state stays in view as the history grows or steps.
  useEffect(() => {
    list.current?.querySelector<HTMLElement>('[aria-current="step"]')?.scrollIntoView?.({ block: 'nearest' })
  }, [current, states.length])

  /** Undoes or redoes to state `index` in one command (one repaint, one change for the host). */
  const goTo = (index: number) => {
    if (!listDisabled && index !== current) void run('history.goTo', { state: index })
  }
  const deleteState = () => {
    setConfirming(false)
    void run('history.deleteState', {})
  }

  return (
    <div className="photo-history" data-testid="photo-history">
      <div className="photo-history-snapshot" data-testid="photo-history-snapshot">
        <Thumbnail image={bottom?.id != null ? thumbnail({ kind: 'layer', layer: bottom.id }) : null} size={SNAPSHOT} />
        <span className="photo-history-name">{info.name}</span>
      </div>
      <KitElement id="control:history.list">
        <ul className="photo-history-rows" ref={list} aria-label={phrase('History')} data-testid="photo-history-rows">
          {states.map((state, index) => {
            const Icon = stateIcon(state.name)
            return (
              <li key={`${index}:${state.name}`}>
                <button
                  type="button"
                  className="photo-history-row"
                  aria-current={index === current ? 'step' : undefined}
                  data-redo={state.redo}
                  disabled={listDisabled && index !== current}
                  onClick={() => goTo(index)}
                  data-testid={`photo-history-state-${index}`}
                >
                  <Icon size={13} aria-hidden="true" />
                  <span>{phrase(state.name)}</span>
                </button>
              </li>
            )
          })}
        </ul>
      </KitElement>
      <PanelFooter
        end={
          <>
            {/* Create new document from current state is left out: `history.newDocument` opens a second document in the engine (`hiddenReason`). */}
            <KitElement id="control:history.snapshot">
              {/* The engine keeps no snapshots yet (`history_cmds.rs`): the button says so. */}
              <IconButton icon={Camera} label={`${phrase('Create new snapshot')} (${t('panels.later')})`} disabled data-testid="photo-history-snapshot-new" />
            </KitElement>
            <KitElement id="control:history.delete">
              <IconButton icon={Trash2} label={phrase('Delete current state')} disabled={!enabled('history.deleteState')} onClick={() => setConfirming(true)} data-testid="photo-history-delete" />
              {/* Photoshop asks before it deletes a state (and every state after it). */}
              {confirming ? (
                <Dialog
                  title={phrase('Delete current state')}
                  onClose={() => setConfirming(false)}
                  actions={
                    <>
                      <Button onClick={() => setConfirming(false)} data-testid="photo-history-delete-cancel">
                        {t('cancel')}
                      </Button>
                      <Button variant="primary" onClick={deleteState} data-testid="photo-history-delete-confirm">
                        {phrase('Delete')}
                      </Button>
                    </>
                  }
                >
                  <p data-testid="photo-history-delete-question">{t('history.deleteConfirm', { name: phrase(info.history[current] ?? '') })}</p>
                </Dialog>
              ) : null}
            </KitElement>
          </>
        }
      />
    </div>
  )
}
