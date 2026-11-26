import { useEffect, useRef, useState } from 'react'
import { PreviewDialog, useKitEditor } from '@kition/sdk/editor'
import { SHIFT_SWITCH, type PhotoPrefs } from '../prefs'
import { CheckRow, ChoiceRow } from './controls'
import { DIALOG_BUTTONS, DIALOG_TEST_ID, useDialogPlace } from './DialogFrame'

/**
 * Edit › Preferences › General… (Cmd+K): the painting cursor, zooming with the scroll wheel, Use Shift
 * Key for Tool Switch, and the Move tool's Show Transform Controls. OK keeps them (no history step: they are the person's, not
 * the document's); Cancel changes nothing.
 */
export function PreferencesDialog({ prefs, onSave, onClose }: { prefs: PhotoPrefs; onSave: (next: PhotoPrefs) => void; onClose: () => void }) {
  const { t, query } = useKitEditor()
  const [draft, setDraft] = useState(prefs)
  // Use Shift Key for Tool Switch shows what the engine's tool keys do now (an agent may have set it), read once on open.
  const queryOnOpen = useRef(query)
  useEffect(() => {
    let live = true
    void queryOnOpen.current<boolean>('prefs.get', { path: SHIFT_SWITCH }).then((value) => {
      if (live && typeof value === 'boolean') setDraft((now) => ({ ...now, shiftToolSwitch: value }))
    })
    return () => {
      live = false
    }
  }, [])
  const place = useDialogPlace()
  return (
    <PreviewDialog
      position={place.position}
      onMove={place.onMove}
      title={t('prefs.title')}
      onOk={() => {
        onSave(draft)
        onClose()
      }}
      onCancel={onClose}
      ids={DIALOG_BUTTONS}
      testId={DIALOG_TEST_ID}
    >
      <div className="photo-dialog photo-dialog-fields" data-command="edit.preferences.general">
        <ChoiceRow
          label={t('prefs.brushCursor')}
          value={draft.brushCursor}
          choices={[
            ['outline', t('prefs.brushCursor.outline')],
            ['precise', t('prefs.brushCursor.precise')],
          ]}
          onChange={(brushCursor) => setDraft({ ...draft, brushCursor })}
          testId="photo-dialog-pref-brushCursor"
        />
        <CheckRow label={t('prefs.zoomWithWheel')} checked={draft.zoomWithWheel} onChange={(zoomWithWheel) => setDraft({ ...draft, zoomWithWheel })} testId="photo-dialog-pref-zoomWithWheel" />
        <CheckRow label={t('prefs.shiftToolSwitch')} checked={draft.shiftToolSwitch} onChange={(shiftToolSwitch) => setDraft({ ...draft, shiftToolSwitch })} testId="photo-dialog-pref-shiftToolSwitch" />
        <CheckRow label={t('prefs.showTransform')} checked={draft.showTransform} onChange={(showTransform) => setDraft({ ...draft, showTransform })} testId="photo-dialog-pref-showTransform" />
      </div>
    </PreviewDialog>
  )
}
