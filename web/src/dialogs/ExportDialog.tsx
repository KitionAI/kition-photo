import { useEffect, useMemo, useRef, useState } from 'react'
import { PreviewDialog, useKitEditor } from '@kition/sdk/editor'
import { CheckRow, ChoiceRow, ColorRow, NumberRow, SliderRow } from './controls'
import { DIALOG_BUTTONS, DIALOG_TEST_ID, useDialogPlace } from './DialogFrame'
import { useDebounced } from './useCommandPreview'
import {
  EXPORT_FORMATS,
  FORMAT_LABELS,
  FORMAT_MIME,
  defaultSettings,
  exportParams,
  fileSize,
  hasQuality,
  keepsTransparency,
  outputSize,
  validSize,
  withScale,
  withSide,
  type ExportFormat,
  type ExportSettings,
  type ImageSize,
} from '../io/exportOptions'
import { explainError } from '../io/errors'

/**
 * File › Export › Export As… (plan 2026-11-24, stage 6): the format (PNG, JPEG, WebP, TIFF), its
 * quality, the size (a scale, or width and height kept in proportion), and transparency or the
 * matte under it, with the exported image itself as the preview and its exact file size. Export
 * hands the same bytes to the host, which writes the file beside the document.
 */

/** What an export answers: the file and its size in pixels. */
export type Exported = { bytes: Uint8Array; width: number; height: number }

/** The last settings, for the session: the next Export As opens with them (its size follows the image). */
let remembered: Omit<ExportSettings, 'width' | 'height' | 'sizing' | 'scale'> & { scale: number } | null = null

const settingsFor = (size: ImageSize): ExportSettings => {
  const start = defaultSettings(size)
  return remembered ? withScale({ ...start, ...remembered }, remembered.scale, size) : start
}

const PREVIEW_MS = 250

export function ExportDialog({ size, exportImage, write, onClose }: { size: ImageSize; exportImage: (params: Record<string, unknown>) => Promise<Exported>; write: (format: ExportFormat, bytes: Uint8Array) => Promise<void>; onClose: () => void }) {
  const { t, host } = useKitEditor()
  const [settings, setSettings] = useState(() => settingsFor(size))
  const set = (patch: Partial<ExportSettings>) => setSettings((now) => ({ ...now, ...patch }))
  const valid = validSize(settings, size)
  const params = useMemo(() => exportParams(settings, size), [settings, size])
  const key = JSON.stringify(params)
  const settled = useDebounced(params, PREVIEW_MS)
  const settledKey = JSON.stringify(settled)
  // The last export the preview shows: its bytes go to the host when the settings are still these.
  const [preview, setPreview] = useState<{ key: string; url: string; exported: Exported } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [updating, setUpdating] = useState(false)
  const serial = useRef(0)
  const exportNow = useRef(exportImage)
  exportNow.current = exportImage
  useEffect(() => {
    if (!valid) return
    serial.current += 1
    const mine = serial.current
    setUpdating(true)
    exportNow
      .current(settled)
      .then((exported) => {
        if (mine !== serial.current) return
        const url = URL.createObjectURL(new Blob([exported.bytes.slice().buffer], { type: FORMAT_MIME[settled.format as ExportFormat] ?? 'image/png' }))
        setPreview((before) => {
          if (before) URL.revokeObjectURL(before.url)
          return { key: settledKey, url, exported }
        })
        setError(null)
      })
      .catch((reason: unknown) => {
        if (mine === serial.current) setError(t(explainError(reason).key))
      })
      .finally(() => {
        if (mine === serial.current) setUpdating(false)
      })
    // `settledKey` names `settled`'s values; `t` only words an error.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settledKey, valid])
  useEffect(() => () => {
    serial.current += 1
    setPreview((before) => {
      if (before) URL.revokeObjectURL(before.url)
      return null
    })
  }, [])
  const out = outputSize(settings, size)
  const place = useDialogPlace()
  const format = settings.format
  const allowed = host.permissions.export
  return (
    <PreviewDialog
      position={place.position}
      onMove={place.onMove}
      title={t('export.title')}
      okLabel={t('export.export')}
      okDisabled={!valid || !allowed}
      onOk={async () => {
        const exported = preview && preview.key === key ? preview.exported : await exportImage(params)
        await write(format, exported.bytes)
        remembered = { ...settings, scale: settings.sizing === 'scale' ? settings.scale : (100 * out.width) / size.width }
        onClose()
      }}
      onCancel={onClose}
      ids={DIALOG_BUTTONS}
      testId={DIALOG_TEST_ID}
    >
      <div className="photo-dialog photo-export" data-command="kit:file.exportAs">
        <div className="photo-dialog-fields photo-export-fields">
          <ChoiceRow label={t('export.format')} value={format} choices={EXPORT_FORMATS.map((item) => [item, FORMAT_LABELS[item]] as const)} onChange={(value) => set({ format: value })} testId="photo-export-format" />
          {format === 'webp' ? <CheckRow label={t('export.lossless')} checked={settings.lossless} onChange={(lossless) => set({ lossless })} testId="photo-export-lossless" /> : null}
          {hasQuality(settings) ? <SliderRow label={t('export.quality')} value={settings.quality} min={1} max={100} onChange={(quality) => set({ quality })} testId="photo-export-quality" /> : null}
          <ChoiceRow
            label={t('export.sizing')}
            value={settings.sizing}
            choices={[
              ['scale', t('export.sizing.scale')],
              ['pixels', t('export.sizing.pixels')],
            ]}
            onChange={(sizing) => set({ sizing })}
            testId="photo-export-sizing"
          />
          {settings.sizing === 'scale' ? (
            <NumberRow label={t('export.scale')} value={settings.scale} min={1} max={1000} decimals={0} unit={t('export.percent')} onChange={(value) => value !== null && setSettings((now) => withScale(now, value, size))} testId="photo-export-scale" />
          ) : (
            <>
              <NumberRow label={t('export.width')} value={settings.width} min={1} max={30000} decimals={0} unit={t('export.pixels')} onChange={(value) => value !== null && setSettings((now) => withSide(now, 'width', value, size))} testId="photo-export-width" />
              <NumberRow label={t('export.height')} value={settings.height} min={1} max={30000} decimals={0} unit={t('export.pixels')} onChange={(value) => value !== null && setSettings((now) => withSide(now, 'height', value, size))} testId="photo-export-height" />
              <CheckRow label={t('export.linked')} checked={settings.linked} onChange={(linked) => setSettings((now) => (linked ? withSide({ ...now, linked }, 'width', now.width, size) : { ...now, linked }))} testId="photo-export-linked" />
            </>
          )}
          {keepsTransparency(format) ? <CheckRow label={t('export.transparency')} checked={settings.transparency} onChange={(transparency) => set({ transparency })} testId="photo-export-transparency" /> : null}
          {!keepsTransparency(format) || !settings.transparency ? <ColorRow label={t('export.matte')} value={settings.matte} onChange={(matte) => set({ matte })} testId="photo-export-matte" /> : null}
          {valid ? null : <div className="photo-dialog-warning">{t('export.badSize')}</div>}
          {allowed ? null : <div className="photo-dialog-warning">{t('export.notAllowed')}</div>}
        </div>
        <figure className="photo-export-preview" aria-label={t('export.preview')} aria-busy={updating}>
          <div className="photo-export-picture">{preview ? <img src={preview.url} alt={t('export.preview')} data-testid="photo-export-preview" data-width={preview.exported.width} data-height={preview.exported.height} /> : null}</div>
          <figcaption className="tabular-nums" data-testid="photo-export-size" data-bytes={preview && preview.key === key ? preview.exported.bytes.length : ''}>
            <span>{t('export.outputSize', { width: out.width, height: out.height })}</span>
            <span>{updating || !preview || preview.key !== key ? t('export.updating') : t('export.fileSize', { size: fileSize(preview.exported.bytes.length, document.documentElement.lang || 'en-US') })}</span>
          </figcaption>
          {error ? <div className="photo-dialog-warning">{error}</div> : null}
        </figure>
      </div>
    </PreviewDialog>
  )
}
