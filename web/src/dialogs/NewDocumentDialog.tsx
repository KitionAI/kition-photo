import { useState } from 'react'
import { KitEngine, PreviewDialog, toast, useKitEditor } from '@kition/sdk/editor'
import { engineManifest } from '../engine'
import { ChoiceRow, NumberRow, TextRow } from './controls'
import { DIALOG_BUTTONS, DIALOG_TEST_ID, useDialogPlace } from './DialogFrame'
import { DOCUMENT_UNITS, fromPixels, roundIn, side, toPixels, unitDecimals, type LengthUnit } from './sizes'

/**
 * File › New… (Photoshop's New Document): a name, a preset or a size in a unit, the resolution and
 * the background. The editor shows the one document its host opened, so OK makes the new file
 * beside it through the host (`host/write`, as an export) and says where; the person opens it from
 * the workspace.
 */

const PRESETS = {
  default: { width: 1920, height: 1080, resolution: 72, unit: 'pixels' },
  square: { width: 1080, height: 1080, resolution: 72, unit: 'pixels' },
  uhd: { width: 3840, height: 2160, resolution: 72, unit: 'pixels' },
  web: { width: 1366, height: 768, resolution: 72, unit: 'pixels' },
  a4: { width: 2480, height: 3508, resolution: 300, unit: 'millimeters' },
  letter: { width: 2550, height: 3300, resolution: 300, unit: 'inches' },
} as const satisfies Record<string, { width: number; height: number; resolution: number; unit: LengthUnit }>
type Preset = keyof typeof PRESETS | 'custom'

const BACKGROUNDS = ['white', 'black', 'background', 'transparent'] as const
type Background = (typeof BACKGROUNDS)[number]

/** A file name without characters a file system refuses, never empty. */
export function documentFileName(name: string, fallback: string) {
  const clean = name.replace(/[\\/:*?"<>|]+/g, ' ').trim() || fallback
  return /\.(psd|psb)$/i.test(clean) ? clean : `${clean}.psd`
}

export function NewDocumentDialog({ colors, onClose }: { colors: { foreground: string; background: string }; onClose: () => void }) {
  const { t, host } = useKitEditor()
  const [name, setName] = useState(() => t('dialog.untitled'))
  const [preset, setPreset] = useState<Preset>('default')
  const [unit, setUnit] = useState<LengthUnit>('pixels')
  const [width, setWidth] = useState<number>(PRESETS.default.width)
  const [height, setHeight] = useState<number>(PRESETS.default.height)
  const [resolution, setResolution] = useState<number>(PRESETS.default.resolution)
  const [background, setBackground] = useState<Background>('white')
  const wide = side(width)
  const high = side(height)
  const valid = wide !== null && high !== null && resolution >= 1 && resolution <= 10000
  const choose = (next: Preset) => {
    setPreset(next)
    if (next === 'custom') return
    const values = PRESETS[next]
    setWidth(values.width)
    setHeight(values.height)
    setResolution(values.resolution)
    setUnit(values.unit)
  }
  const shown = (pixels: number) => roundIn(fromPixels(pixels, unit, pixels, resolution), unit)
  const setSide = (set: (pixels: number) => void) => (value: number | null) => {
    if (value === null) return
    setPreset('custom')
    set(toPixels(value, unit, 0, resolution))
  }
  const create = async () => {
    if (wide === null || high === null) return
    const file = documentFileName(name, t('dialog.untitled'))
    const engine = await KitEngine.create(engineManifest(), file, wide, high)
    try {
      if (resolution !== 72) await engine.execute('image.imageSize', { resolution, resample: 'none' })
      if (background === 'black') await engine.execute('edit.fill', { contents: 'black' })
      else if (background === 'background') await engine.execute('edit.fill', { contents: 'color', color: colors.background })
      else if (background === 'transparent') {
        await engine.execute('layer.new.layerFromBackground', {})
        await engine.execute('select.all', {})
        await engine.execute('edit.clear', {})
        await engine.execute('select.deselect', {})
      }
      const path = await host.exportFile(file, await engine.save())
      toast('success', t('dialog.created', { path }))
    } catch (error) {
      toast('error', t('dialog.createFailed'), error instanceof Error ? error.message : String(error))
      throw error
    } finally {
      engine.dispose()
    }
  }
  const unitLabel = (value: LengthUnit) => t(`dialog.unit.${value}`)
  const place = useDialogPlace()
  return (
    <PreviewDialog position={place.position} onMove={place.onMove} title={t('dialog.newDocument')} okLabel={t('dialog.create')} okDisabled={!valid} onOk={async () => { await create(); onClose() }} onCancel={onClose} ids={DIALOG_BUTTONS} testId={DIALOG_TEST_ID}>
      <div className="photo-dialog photo-dialog-fields" data-command="kit:file.newDocument">
        <TextRow label={t('dialog.name')} value={name} onChange={setName} testId="photo-dialog-name" />
        <ChoiceRow label={t('dialog.preset')} value={preset} choices={[...(Object.keys(PRESETS) as (keyof typeof PRESETS)[]).map((key) => [key, t(`dialog.preset.${key}`)] as const), ['custom', t('dialog.preset.custom')] as const]} onChange={choose} testId="photo-dialog-preset" />
        <ChoiceRow label={t('dialog.units')} value={unit} choices={DOCUMENT_UNITS.map((item) => [item, unitLabel(item)] as const)} onChange={setUnit} testId="photo-dialog-unit" />
        <NumberRow label={t('dialog.width')} value={shown(width)} min={0} max={1_000_000} decimals={unitDecimals(unit)} unit={unitLabel(unit)} onChange={setSide(setWidth)} testId="photo-dialog-width" />
        <NumberRow label={t('dialog.height')} value={shown(height)} min={0} max={1_000_000} decimals={unitDecimals(unit)} unit={unitLabel(unit)} onChange={setSide(setHeight)} testId="photo-dialog-height" />
        <NumberRow
          label={t('dialog.resolution')}
          value={resolution}
          min={1}
          max={10000}
          decimals={0}
          unit={t('dialog.pixelsPerInch')}
          onChange={(value) => {
            if (value === null || !(value > 0)) return
            setPreset('custom')
            // A physical size keeps its length: its pixels follow the resolution.
            if (unit !== 'pixels') {
              setWidth((current) => (current / resolution) * value)
              setHeight((current) => (current / resolution) * value)
            }
            setResolution(value)
          }}
          testId="photo-dialog-resolution"
        />
        <ChoiceRow label={t('dialog.background')} value={background} choices={BACKGROUNDS.map((item) => [item, t(`dialog.background.${item}`)] as const)} onChange={setBackground} testId="photo-dialog-background" />
        {valid ? null : <div className="photo-dialog-warning">{t('dialog.badSize')}</div>}
      </div>
    </PreviewDialog>
  )
}
