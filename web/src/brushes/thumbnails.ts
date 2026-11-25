import { KitEngine } from '@kition/sdk/editor'
import { engineManifest } from '../engine'
import { THUMB_HEIGHT, THUMB_WIDTH, thumbnailSize, thumbnailStroke, type BrushPreset } from './brushes'

/**
 * Each brush preset's stroke, as the engine paints it (plan 2026-11-24, stage 5): a scratch
 * document of the engine, made once for the page, gets one row per preset, painted with that
 * preset (`paint.stroke` with `preset`), and is read back once; each row becomes a picture. The
 * document the person edits is never touched. A preset the engine cannot paint has no picture.
 */
let painted: Promise<Map<string, string>> | null = null
let paintedFor = ''

/** The presets' stroke pictures (data URLs, by preset name). */
export function presetThumbnails(presets: readonly BrushPreset[]): Promise<Map<string, string>> {
  const key = presets.map((preset) => preset.name).join('\n')
  if (painted && paintedFor === key) return painted
  paintedFor = key
  painted = paint(presets).catch(() => new Map<string, string>())
  return painted
}

async function paint(presets: readonly BrushPreset[]): Promise<Map<string, string>> {
  const out = new Map<string, string>()
  if (!presets.length || typeof document === 'undefined') return out
  const height = presets.length * THUMB_HEIGHT
  const engine = await KitEngine.create(engineManifest(), 'brushes', THUMB_WIDTH, height)
  try {
    await engine.execute('edit.fill', { contents: 'white' })
    const drawn: string[] = []
    for (const [row, preset] of presets.entries()) {
      try {
        await engine.execute('paint.stroke', { preset: preset.name, size: thumbnailSize(preset), color: '#000000', opacity: 1, flow: 1, points: thumbnailStroke(row), seed: 7 })
        drawn.push(preset.name)
      } catch {
        drawn.push('')
      }
    }
    const pixels = await engine.render(0, 0, 1, THUMB_WIDTH, height)
    const canvas = document.createElement('canvas')
    canvas.width = THUMB_WIDTH
    canvas.height = THUMB_HEIGHT
    const context = canvas.getContext('2d')
    if (!context) return out
    drawn.forEach((name, row) => {
      if (!name) return
      const start = row * THUMB_WIDTH * THUMB_HEIGHT * 4
      const slice = pixels.slice(start, start + THUMB_WIDTH * THUMB_HEIGHT * 4)
      if (slice.length !== THUMB_WIDTH * THUMB_HEIGHT * 4) return
      context.putImageData(new ImageData(new Uint8ClampedArray(slice), THUMB_WIDTH, THUMB_HEIGHT), 0, 0)
      out.set(name, canvas.toDataURL('image/png'))
    })
    return out
  } finally {
    engine.dispose()
  }
}
