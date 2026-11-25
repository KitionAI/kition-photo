import { useState } from 'react'
import { useKitEditor, type KitCommand } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { DialogFrame } from './DialogFrame'
import { CheckRow, ChoiceRow, ColorRow, NumberRow } from './controls'
import { dialogTitle } from './labels'
import { anchorArrow, ANCHORS, fromPixels, IMAGE_UNITS, keptProportion, memorySize, roundIn, side, toPixels, unitDecimals, type Anchor, type LengthUnit } from './sizes'

/**
 * Image Size and Canvas Size (Photoshop's): the new size in a unit of the person's choosing,
 * Image Size keeping the proportions and resampling (or, without resampling, changing only the
 * resolution), Canvas Size growing or cropping around an anchor. Neither previews: OK runs once.
 */

type Props = { command: KitCommand; info: PhotoInfo; onClose: () => void }

const RESAMPLE = ['bicubic', 'bilinear', 'nearest', 'lanczos', 'preserveDetails'] as const
type Resample = (typeof RESAMPLE)[number]

/** Channels in memory for a colour mode, for the size line. */
const channelsOf = (mode: string) => (/gray/i.test(mode) ? 1 : /cmyk/i.test(mode) ? 4 : 3)

export function ImageSizeDialog({ command, info, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [unit, setUnit] = useState<LengthUnit>('pixels')
  // The new size in pixels (fractional while proportions are kept) and resolution.
  const [width, setWidth] = useState(info.width)
  const [height, setHeight] = useState(info.height)
  const [resolution, setResolution] = useState(info.resolution || 72)
  const [constrain, setConstrain] = useState(true)
  const [resample, setResample] = useState(true)
  const [method, setMethod] = useState<Resample>('bicubic')
  // Without resampling the pixels stay; a physical length changes the resolution instead.
  const shownUnit: LengthUnit = !resample && (unit === 'pixels' || unit === 'percent') ? 'inches' : unit
  const shown = (pixels: number, base: number) => roundIn(fromPixels(pixels, shownUnit, base, resolution), shownUnit)
  const setSide = (which: 'width' | 'height', value: number | null) => {
    if (value === null || !(value > 0)) return
    const base = which === 'width' ? info.width : info.height
    if (!resample) {
      // The physical length now spans the same pixels: the resolution follows.
      const inches = toPixels(value, shownUnit, base, 1)
      if (inches > 0) setResolution(Math.round((base / inches) * 1000) / 1000)
      return
    }
    const pixels = toPixels(value, shownUnit, base, resolution)
    if (which === 'width') {
      setWidth(pixels)
      if (constrain) setHeight(keptProportion('width', pixels, info.width, info.height))
    } else {
      setHeight(pixels)
      if (constrain) setWidth(keptProportion('height', pixels, info.width, info.height))
    }
  }
  const setRes = (value: number | null) => {
    if (value === null || !(value > 0)) return
    if (resample && shownUnit !== 'pixels' && shownUnit !== 'percent') {
      // A physical size stays put while the resolution changes: the pixels follow.
      setWidth((current) => (current / resolution) * value)
      setHeight((current) => (current / resolution) * value)
    }
    setResolution(value)
  }
  const pixelsWide = resample ? side(width) : info.width
  const pixelsHigh = resample ? side(height) : info.height
  const valid = pixelsWide !== null && pixelsHigh !== null && resolution >= 1 && resolution <= 10000
  const params = valid ? { width: pixelsWide, height: pixelsHigh, resolution, resample: resample ? method : 'none' } : {}
  const channels = channelsOf(info.mode)
  const unitLabel = (value: LengthUnit) => t(`dialog.unit.${value}`)
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={params} previewing={false} okDisabled={!valid} unchanged={pixelsWide === info.width && pixelsHigh === info.height && resolution === info.resolution} onClose={() => onClose()}>
      <div className="photo-dialog-fields">
        <div className="photo-dialog-caption" data-testid="photo-dialog-size-line">
          {valid && (pixelsWide !== info.width || pixelsHigh !== info.height)
            ? t('dialog.imageSizeWas', { size: memorySize(pixelsWide, pixelsHigh, channels, info.depth), was: memorySize(info.width, info.height, channels, info.depth) })
            : t('dialog.imageSizeNow', { size: memorySize(info.width, info.height, channels, info.depth), width: info.width, height: info.height })}
        </div>
        <ChoiceRow label={t('dialog.units')} value={shownUnit} choices={IMAGE_UNITS.filter((item) => resample || (item !== 'pixels' && item !== 'percent')).map((item) => [item, unitLabel(item)] as const)} onChange={setUnit} testId="photo-dialog-unit" />
        <NumberRow label={t('dialog.width')} value={shown(resample ? width : info.width, info.width)} min={0} max={1_000_000} decimals={unitDecimals(shownUnit)} unit={unitLabel(shownUnit)} onChange={(value) => setSide('width', value)} testId="photo-dialog-width" />
        <NumberRow label={t('dialog.height')} value={shown(resample ? height : info.height, info.height)} min={0} max={1_000_000} decimals={unitDecimals(shownUnit)} unit={unitLabel(shownUnit)} onChange={(value) => setSide('height', value)} testId="photo-dialog-height" />
        <NumberRow label={t('dialog.resolution')} value={Math.round(resolution * 1000) / 1000} min={1} max={10000} decimals={3} unit={t('dialog.pixelsPerInch')} onChange={setRes} testId="photo-dialog-resolution" />
        <CheckRow
          label={t('dialog.constrain')}
          checked={constrain || !resample}
          disabled={!resample}
          onChange={(on) => {
            setConstrain(on)
            if (on) setHeight(keptProportion('width', width, info.width, info.height))
          }}
          testId="photo-dialog-constrain"
        />
        <CheckRow
          label={t('dialog.resample')}
          checked={resample}
          onChange={(on) => {
            setResample(on)
            if (!on) {
              setWidth(info.width)
              setHeight(info.height)
            }
          }}
          testId="photo-dialog-resample"
        />
        <ChoiceRow label={t('dialog.resampleMethod')} value={method} disabled={!resample} choices={RESAMPLE.map((item) => [item, t(`dialog.resample.${item}`)] as const)} onChange={setMethod} testId="photo-dialog-method" />
        {valid ? null : <div className="photo-dialog-warning">{t('dialog.badSize')}</div>}
      </div>
    </DialogFrame>
  )
}

const EXTENSIONS = ['background', 'foreground', 'white', 'black', 'transparent', 'color'] as const
type Extension = (typeof EXTENSIONS)[number]

export function CanvasSizeDialog({ command, info, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [unit, setUnit] = useState<LengthUnit>('pixels')
  const [relative, setRelative] = useState(false)
  // The size as typed, in pixels (absolute) or as the change (relative).
  const [width, setWidth] = useState<number>(info.width)
  const [height, setHeight] = useState<number>(info.height)
  const [anchor, setAnchor] = useState<Anchor>('center')
  const [extension, setExtension] = useState<Extension>('background')
  const [color, setColor] = useState('#808080')
  const resolution = info.resolution || 72
  const shown = (pixels: number, base: number) => roundIn(fromPixels(pixels, unit, base, resolution), unit)
  const newWidth = side(relative ? info.width + width : width)
  const newHeight = side(relative ? info.height + height : height)
  const valid = newWidth !== null && newHeight !== null
  const params = valid ? { width: newWidth, height: newHeight, anchor, extensionColor: extension === 'color' ? color : extension } : {}
  const unitLabel = (value: LengthUnit) => t(`dialog.unit.${value}`)
  return (
    <DialogFrame command={command.id} title={dialogTitle(words, command.title)} params={params} previewing={false} okDisabled={!valid} unchanged={newWidth === info.width && newHeight === info.height} onClose={() => onClose()}>
      <div className="photo-dialog-fields">
        <div className="photo-dialog-caption">{t('dialog.currentSize', { width: info.width, height: info.height })}</div>
        <ChoiceRow label={t('dialog.units')} value={unit} choices={IMAGE_UNITS.map((item) => [item, unitLabel(item)] as const)} onChange={setUnit} testId="photo-dialog-unit" />
        <NumberRow label={t('dialog.width')} value={shown(width, info.width)} min={relative ? -1_000_000 : 0} max={1_000_000} decimals={unitDecimals(unit)} unit={unitLabel(unit)} onChange={(value) => value !== null && setWidth(toPixels(value, unit, info.width, resolution))} testId="photo-dialog-width" />
        <NumberRow label={t('dialog.height')} value={shown(height, info.height)} min={relative ? -1_000_000 : 0} max={1_000_000} decimals={unitDecimals(unit)} unit={unitLabel(unit)} onChange={(value) => value !== null && setHeight(toPixels(value, unit, info.height, resolution))} testId="photo-dialog-height" />
        <CheckRow
          label={t('dialog.relative')}
          checked={relative}
          onChange={(on) => {
            // Relative starts from no change; absolute from the size it would make.
            setWidth(on ? (newWidth ?? info.width) - info.width : (newWidth ?? info.width))
            setHeight(on ? (newHeight ?? info.height) - info.height : (newHeight ?? info.height))
            setRelative(on)
          }}
          testId="photo-dialog-relative"
        />
        <div className="photo-dialog-row">
          <span className="photo-dialog-label">{t('dialog.anchor')}</span>
          <div className="photo-anchor-grid" role="radiogroup" aria-label={t('dialog.anchor')}>
            {ANCHORS.map((cell) => (
              <button key={cell} type="button" role="radio" aria-checked={cell === anchor} aria-label={cell} className="photo-anchor-cell" data-anchor={cell === anchor || undefined} onClick={() => setAnchor(cell)} data-testid={`photo-dialog-anchor-${cell}`}>
                {anchorArrow(anchor, cell)}
              </button>
            ))}
          </div>
        </div>
        <ChoiceRow label={t('dialog.extension')} value={extension} choices={EXTENSIONS.map((item) => [item, t(`dialog.extension.${item}`)] as const)} onChange={setExtension} testId="photo-dialog-extension" />
        {extension === 'color' ? <ColorRow label={t('dialog.color')} value={color} onChange={setColor} testId="photo-dialog-extension-color" /> : null}
        <div className="photo-dialog-caption" data-testid="photo-dialog-new-size">
          {valid ? t('dialog.newSize', { width: newWidth, height: newHeight }) : t('dialog.badSize')}
        </div>
      </div>
    </DialogFrame>
  )
}
