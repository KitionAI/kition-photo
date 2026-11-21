import type { RefObject } from 'react'
import { BarSeparator, Hint, KitElement, NumberField, useKitEditor, type KitCanvasHandle } from '@kition/sdk/editor'
import type { PhotoInfo } from './photo'
import type { PhotoToolId } from './tools/catalog'
import { TOOL_HINTS } from './tools/hints'
import { fill } from './options/fields'

/** Upstream's mode names in the status bar (`canvas::mode_label`), by the engine's mode. */
const MODE_LABELS: Readonly<Record<string, string>> = { Rgb: 'RGB', Grayscale: 'Gray', Cmyk: 'CMYK', Lab: 'Lab', Indexed: 'Indexed', Bitmap: 'Bitmap', Duotone: 'Duotone', Multichannel: 'Multichannel' }

/** Upstream's zoom field range, in percent. */
const ZOOM_MIN = 1
const ZOOM_MAX = 3200

/**
 * The status bar's own part (Kit UI `control:status`), as upstream's `status_bar`: the zoom field
 * (`control:status.zoom`, typed or stepped with the arrows, moving the canvas), the colour mode and
 * depth, the document's size, its layer count, and the active tool's hint. The shell adds the
 * unsaved and agent marks after it, and the render timing at the end.
 */
export function PhotoStatus({ info, zoom, canvas, tool }: { info: PhotoInfo; zoom: number | null; canvas: RefObject<KitCanvasHandle | null>; tool: PhotoToolId }) {
  const { phrase } = useKitEditor()
  const hint = TOOL_HINTS[tool]
  // Upstream's plural phrase: "one|other" in English, one form in Chinese.
  const forms = phrase('{n} layer|{n} layers').split('|')
  const layers = fill(forms[forms.length > 1 && info.layers.length !== 1 ? 1 : 0], { n: String(info.layers.length) })
  return (
    <KitElement id="control:status">
      <span className="photo-status" data-testid="photo-status">
        {zoom != null ? (
          <KitElement id="control:status.zoom">
            <span className="photo-status-zoom">
              <NumberField label="" name={phrase('Zoom')} value={Math.round(zoom * 10000) / 100} min={ZOOM_MIN} max={ZOOM_MAX} unit="%" width={72} onChange={(percent) => canvas.current?.zoomTo(percent / 100)} testId="photo-status-zoom" />
            </span>
          </KitElement>
        ) : null}
        <BarSeparator />
        <span>{fill(phrase('{mode} Color · {bits} bit'), { mode: phrase(MODE_LABELS[info.mode] ?? info.mode), bits: String(info.depth) })}</span>
        <BarSeparator />
        <span className="tabular-nums">{`${info.width} × ${info.height} px`}</span>
        <BarSeparator />
        <span>{layers}</span>
        {hint ? (
          <>
            <BarSeparator />
            <Hint testId="photo-status-hint">{fill(phrase(hint.text), { ...hint.keys })}</Hint>
          </>
        ) : null}
      </span>
    </KitElement>
  )
}
