import { TesseraIcons } from '@kition/kit-editor'
import { Button, Dialog, useKitEditor } from '@kition/kit-editor'

const TriangleAlert = TesseraIcons.TriangleAlertIcon

/** The name the prompt's title shows, as Help › About does. */
const APP_NAME = 'Kition Photo'

/** The "Rasterize?" question (upstream's `rasterize_prompt::body`): Photoshop's text for the layer's kind, with a warning mark. */
export function RasterizePrompt({ message, onConfirm, onCancel }: { message: string; onConfirm: () => void; onCancel: () => void }) {
  const { t, phrase, testId } = useKitEditor()
  return (
    <Dialog
      title={APP_NAME}
      onClose={onCancel}
      actions={
        <>
          <Button onClick={onCancel} data-testid={`${testId}-rasterize-cancel`}>
            {t('cancel')}
          </Button>
          <Button variant="primary" onClick={onConfirm} data-testid={`${testId}-rasterize-confirm`}>
            {phrase('OK')}
          </Button>
        </>
      }
    >
      <div className="photo-rasterize" data-testid={`${testId}-rasterize`}>
        <TriangleAlert className="photo-rasterize-mark" size={30} aria-hidden="true" />
        <p data-testid={`${testId}-rasterize-question`}>{phrase(message)}</p>
      </div>
    </Dialog>
  )
}
