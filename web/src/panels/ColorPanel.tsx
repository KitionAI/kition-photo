import { useRef } from 'react'
import { ColorPicker, GradientSlider, KitElement, parseHex, rgbToHex, useKitEditor, type RGB } from '@kition/sdk/editor'
import type { PhotoColorStore } from '../colors'
import { fieldsOfHsb, rgbToHsb } from '../color/colorMath'
import { useColorPicker } from '../color/ColorPickerHost'
import { useRemembered } from './PropsParts'

/** The hue strip's colours, left to right (0° to 360°). */
const HUES = ['#ff0000', '#ffff00', '#00ff00', '#00ffff', '#0000ff', '#ff00ff', '#ff0000']

/**
 * Upstream's Color panel (`panels.rs` `color_field`, its Photoshop theme): the foreground and
 * background chips, a click choosing which colour the field edits (framed), and Kit UI's colour
 * picker (saturation and brightness field, hue strip, hex, and R, G, B) on that colour. Colours
 * live in the engine (`usePhotoColors`) and are no history step, so every move of the field sets
 * the colour at once. Under it, Hue, Saturation and Brightness sliders (Photoshop's HSB Sliders)
 * edit the same colour; a double-click on a chip opens the Color Picker dialog for it (plan
 * 2026-11-24, stage 5).
 */
export function ColorPanel({ colors }: { colors: PhotoColorStore }) {
  const { phrase, t } = useKitEditor()
  const pick = useColorPicker()
  // Which colour the field edits outlives the tab, as upstream's `color_panel.background`.
  const [background, setBackground] = useRemembered('color-background', false)
  const target = background ? 'background' : 'foreground'
  const value = parseHex(colors[target]) ?? ([0, 0, 0] as RGB)
  // A grey has no hue: the sliders keep the last one, so raising Saturation comes back to it.
  const hue = useRef(0)
  const hsb = rgbToHsb(value, hue.current)
  if (hsb.s > 0 && hsb.b > 0) hue.current = hsb.h
  const setHsb = (patch: Partial<typeof hsb>) => {
    const next = { ...hsb, ...patch }
    if (patch.h !== undefined) hue.current = patch.h
    colors.set({ [target]: fieldsOfHsb(next).hex })
  }
  const openPicker = (which: 'foreground' | 'background') => {
    if (!pick) return
    const start = colors[which]
    void pick({ title: t(`color.title.${which}`), value: start, onLive: (hex) => colors.set({ [which]: hex }) }).then((chosen) => {
      if (chosen) colors.set({ [which]: chosen })
    })
  }
  const bright = fieldsOfHsb({ h: hsb.h, s: hsb.s, b: 100 }).hex
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
            onDoubleClick={() => openPicker('background')}
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
            onDoubleClick={() => openPicker('foreground')}
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
      <div className="photo-color-sliders" data-testid="photo-color-hsb">
        <GradientSlider label={t('color.hue')} value={Math.round(hsb.h)} min={0} max={360} stops={HUES} onChange={(h) => setHsb({ h })} testId="photo-color-hsb-h" />
        <GradientSlider label={t('color.saturation')} value={Math.round(hsb.s)} min={0} max={100} stops={[fieldsOfHsb({ h: hsb.h, s: 0, b: hsb.b }).hex, fieldsOfHsb({ h: hsb.h, s: 100, b: hsb.b }).hex]} onChange={(s) => setHsb({ s })} testId="photo-color-hsb-s" />
        <GradientSlider label={t('color.brightness')} value={Math.round(hsb.b)} min={0} max={100} stops={['#000000', bright]} onChange={(b) => setHsb({ b })} testId="photo-color-hsb-b" />
      </div>
    </div>
  )
}
