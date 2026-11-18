import { ColorPicker, KitElement, parseHex, rgbToHex, useKitEditor, type RGB } from '@kition/kit-editor'
import type { PhotoColorStore } from '../colors'
import { useRemembered } from './PropsParts'

/**
 * Upstream's Color panel (`panels.rs` `color_field`, its Photoshop theme): the foreground and
 * background chips, a click choosing which colour the field edits (framed), and Kit UI's colour
 * picker (saturation and brightness field, hue strip, hex, and R, G, B) on that colour. Colours
 * live in the engine (`usePhotoColors`) and are no history step, so every move of the field sets
 * the colour at once. Upstream's double-click opens its Color Picker dialog, which comes in wave
 * 2; the field here already edits the same colour.
 */
export function ColorPanel({ colors }: { colors: PhotoColorStore }) {
  const { phrase } = useKitEditor()
  // Which colour the field edits outlives the tab, as upstream's `color_panel.background`.
  const [background, setBackground] = useRemembered('color-background', false)
  const target = background ? 'background' : 'foreground'
  const value = parseHex(colors[target]) ?? ([0, 0, 0] as RGB)
  return (
    <div className="photo-color-panel" data-testid="photo-color-panel">
      <KitElement id="control:color.chips">
        <div className="photo-color-chips" role="group" aria-label={phrase('Colors')}>
          <button
            type="button"
            className="photo-color-chip photo-color-chip-background"
            style={{ background: colors.background }}
            aria-pressed={background}
            title={phrase('Set background color')}
            aria-label={phrase('Set background color')}
            onClick={() => setBackground(true)}
            data-testid="photo-color-chip-background"
          />
          <button
            type="button"
            className="photo-color-chip photo-color-chip-foreground"
            style={{ background: colors.foreground }}
            aria-pressed={!background}
            title={phrase('Set foreground color')}
            aria-label={phrase('Set foreground color')}
            onClick={() => setBackground(false)}
            data-testid="photo-color-chip-foreground"
          />
        </div>
      </KitElement>
      <ColorPicker
        value={value}
        uiId="control:color.picker"
        testId="photo-color-picker"
        onChange={(rgb) => colors.set({ [target]: rgbToHex(rgb) })}
      />
    </div>
  )
}
