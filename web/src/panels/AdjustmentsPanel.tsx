import { type IconComponent } from '@kition/sdk/editor'
import { TesseraIcons } from '@kition/sdk/editor'
import { IconButton, KitElement, useKitEditor, type KitUiItem } from '@kition/sdk/editor'

const Blend = TesseraIcons.BlendIcon
const CircleDot = TesseraIcons.CircleDotIcon
const Contrast = TesseraIcons.ContrastIcon
const Droplet = TesseraIcons.DropletIcon
const Gauge = TesseraIcons.GaugeIcon
const Grid3x3 = TesseraIcons.Grid3x3Icon
const Layers = TesseraIcons.LayersIcon
const Palette = TesseraIcons.PaletteIcon
const PenTool = TesseraIcons.PenToolIcon
const Scan = TesseraIcons.ScanIcon
const SlidersHorizontal = TesseraIcons.SlidersHorizontalIcon
const Sparkles = TesseraIcons.SparklesIcon
const Square = TesseraIcons.SquareIcon
const SquaresSubtract = TesseraIcons.SquaresSubtractIcon
const Sun = TesseraIcons.SunIcon
const SwatchBook = TesseraIcons.SwatchBookIcon

/**
 * Upstream's Adjustments panel (`panels.rs` `adjustments_grid`): a grid of one-click adjustment
 * layers, each `layer.newAdjustmentLayer.<kind>` with the engine's defaults (the engine masks the
 * new layer with the selection, and the mask target's session aims it), then the Properties tab
 * shows the new layer's settings, as upstream switches to it.
 */

/** Upstream's grid, in its order: the kind, its icon, and its label (the tooltip). */
export const ADJUSTMENTS: readonly (readonly [string, IconComponent, string])[] = [
  ['brightnessContrast', Sun, 'Brightness/Contrast'],
  ['levels', Gauge, 'Levels'],
  ['curves', PenTool, 'Curves'],
  ['exposure', Scan, 'Exposure'],
  ['vibrance', Sparkles, 'Vibrance'],
  ['hueSaturation', Droplet, 'Hue/Saturation'],
  ['colorBalance', Blend, 'Color Balance'],
  ['blackWhite', Contrast, 'Black & White'],
  ['photoFilter', CircleDot, 'Photo Filter'],
  ['channelMixer', SlidersHorizontal, 'Channel Mixer'],
  ['invert', SquaresSubtract, 'Invert'],
  ['posterize', Layers, 'Posterize'],
  ['threshold', Square, 'Threshold'],
  ['gradientMap', Palette, 'Gradient Map'],
  ['selectiveColor', SwatchBook, 'Selective Color'],
  ['colorLookup', Grid3x3, 'Color Lookup'],
]

/** The grid's buttons as Kit UI elements: a host hides or disables each kind by its id. */
export const ADJUSTMENT_CONTROLS: KitUiItem[] = ADJUSTMENTS.map(([kind, , label]) => ({ id: `control:adjustments.${kind}`, label, parent: 'panel:adjustments' }))

export function AdjustmentsPanel({ onAdded }: { onAdded: () => void }) {
  const { run, enabled, phrase } = useKitEditor()
  const add = async (kind: string) => {
    if ((await run(`layer.newAdjustmentLayer.${kind}`, {})) != null) onAdded()
  }
  return (
    <div className="photo-adjustments" data-testid="photo-adjustments">
      <p className="kui-muted photo-adjustments-hint">{phrase('Add an adjustment')}</p>
      <div className="photo-adjustments-grid" role="group" aria-label={phrase('Add an adjustment')}>
        {ADJUSTMENTS.map(([kind, icon, label]) => (
          <KitElement key={kind} id={`control:adjustments.${kind}`}>
            <IconButton icon={icon} label={phrase(label)} disabled={!enabled(`layer.newAdjustmentLayer.${kind}`)} onClick={() => void add(kind)} data-testid={`photo-adjustments-${kind}`} />
          </KitElement>
        ))}
      </div>
    </div>
  )
}
