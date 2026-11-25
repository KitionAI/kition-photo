import { useState, type ReactNode } from 'react'
import { PreviewDialog } from '@kition/sdk/editor'
import { useCommandPreview } from './useCommandPreview'

/** The dialogs' OK, Cancel and Preview: the shared Kit UI ids `photoUi.ts` declares. */
export const DIALOG_BUTTONS = { ok: 'control:dialog.ok', cancel: 'control:dialog.cancel', preview: 'control:dialog.preview' }

/** Where the person last moved a dialog (by its title), for the session: the next opens there too, off the picture. */
let lastPlace: { x: number; y: number } | null = null

/** A dialog's place: where the person last moved one, kept as they move this one. */
export function useDialogPlace() {
  const [place, setPlace] = useState(lastPlace)
  return {
    position: place,
    onMove: (at: { x: number; y: number }) => {
      lastPlace = at
      setPlace(at)
    },
  }
}

/** The test id every command dialog has (its buttons: `-ok`, `-cancel`, `-preview`). */
export const DIALOG_TEST_ID = 'photo-dialog'

/**
 * A command's dialog: `PreviewDialog` running `command` with `params` (previewing it on the canvas
 * when `previewing`), OK leaving one history step and Cancel undoing the preview, then `onClose`.
 * Alt turns Cancel into Reset when the dialog has `onReset`. OK with `unchanged` values closes
 * without running. Its title bar moves it, and the next dialog opens where it was left.
 */
export function DialogFrame({
  command,
  title,
  params,
  previewing,
  onClose,
  onReset,
  okLabel,
  okDisabled,
  unchanged = false,
  children,
}: {
  command: string
  title: string
  params: Record<string, unknown>
  previewing: boolean
  onClose: (committed: boolean) => void
  onReset?: () => void
  okLabel?: string
  okDisabled?: boolean
  /** The values change nothing (a size the image already has): OK closes without a step. */
  unchanged?: boolean
  children: ReactNode
}) {
  const run = useCommandPreview(command, params, previewing)
  const place = useDialogPlace()
  return (
    <PreviewDialog
      position={place.position}
      onMove={place.onMove}
      title={title}
      preview={run.preview}
      okLabel={okLabel}
      okDisabled={okDisabled}
      onOk={async () => {
        if (unchanged) run.cancel()
        else await run.ok()
        onClose(!unchanged)
      }}
      onCancel={() => {
        run.cancel()
        onClose(false)
      }}
      onReset={onReset}
      ids={DIALOG_BUTTONS}
      testId={DIALOG_TEST_ID}
    >
      <div className="photo-dialog" data-command={command}>
        {children}
      </div>
    </PreviewDialog>
  )
}
