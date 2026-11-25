import { useEffect, useRef, useState } from 'react'
import { useEngineQuery, useKitEditor } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { useLive, type DocPoint, type Live } from './live'
import { byteOf, cmykOf, formatLength, type RulerUnits } from './readouts'

/**
 * Upstream's Info panel (`panels.rs` `info_panel`): the colour under the pointer in R, G, B and
 * C, M, Y, K (upstream's naive device CMYK, in percent), the pointer's position and the
 * selection's size in the ruler unit (Preferences › Units & Rulers), and the document's size in
 * pixels. The colour is the composite's pixel (`file.sampleColor` at the pixel's centre),
 * asked at most once per animation frame and never while an answer is pending, and kept per pixel
 * and image revision as upstream keeps `info_sample`; the last answer shows until the next one
 * comes. The pointer is reported however it moves over the canvas (a gesture, Space or the Hand
 * tool panning: Kit UI's `track`).
 */
export function InfoPanel({ info, pointer }: { info: PhotoInfo; pointer: Live<DocPoint> }) {
  const { phrase, query, session } = useKitEditor()
  const units = useEngineQuery<RulerUnits>('prefs.get', { path: 'unitsAndRulers' })
  const point = useLive(pointer)
  const revision = session.info?.revision ?? 0
  const doc = session.doc
  const documentNow = useRef(doc)
  documentNow.current = doc
  const inside = point && point.x >= 0 && point.y >= 0 && point.x < info.width && point.y < info.height ? { x: Math.floor(point.x), y: Math.floor(point.y) } : null
  const key = inside ? `${inside.x},${inside.y}@${revision}` : null
  const [sample, setSample] = useState<{ doc: typeof doc; key: string; rgba: number[] } | null>(null)
  const pending = useRef<typeof doc>(null)
  const wanted = useRef(key)
  wanted.current = key

  useEffect(() => {
    if (!doc || !key || !inside || (sample?.doc === doc && sample.key === key)) return undefined
    const frame = window.requestAnimationFrame(() => {
      // One request at a time: the answer asks again if the pointer moved on meanwhile.
      if (pending.current === doc) return
      pending.current = doc
      const asked = key
      void query<number[]>('file.sampleColor', { x: inside.x + 0.5, y: inside.y + 0.5 }).then((rgba) => {
        if (pending.current === doc) pending.current = null
        if (documentNow.current !== doc) return
        if (rgba) setSample({ doc, key: asked, rgba })
        // A newer pixel waits for this answer: the state change (or a dummy) wakes the effect again.
        else if (wanted.current !== asked) setSample((current) => (current ? { ...current } : current))
      })
    })
    return () => window.cancelAnimationFrame(frame)
    // `inside` is `key`'s pixel.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc, key, query, sample])

  // While the pixel under the pointer is asked for, the last one answered stays (no dash between pixels).
  const rgba = inside && sample?.doc === doc ? sample.rgba : null
  const dash = '—'
  const cmyk = rgba ? cmykOf(rgba) : null
  const x = (px: number) => formatLength(px, units, info.resolution, info.width)
  const y = (px: number) => formatLength(px, units, info.resolution, info.height)
  const selection = info.selectionBounds
  const row = (label: string, value: string, testId: string) => (
    <div key={label}>
      <dt>{label}</dt>
      <dd data-testid={testId}>{value}</dd>
    </div>
  )
  return (
    <div className="photo-info" data-testid="photo-info">
      <div className="photo-info-columns">
        <dl className="photo-readout">
          {(['R:', 'G:', 'B:'] as const).map((label, index) => row(label, rgba ? String(byteOf(rgba[index])) : dash, `photo-info-${label[0].toLowerCase()}`))}
        </dl>
        <dl className="photo-readout">
          {(['C:', 'M:', 'Y:', 'K:'] as const).map((label, index) => row(label, cmyk ? `${cmyk[index]}%` : dash, `photo-info-${['cyan', 'magenta', 'yellow', 'black'][index]}`))}
        </dl>
      </div>
      <hr className="photo-info-rule" />
      <div className="photo-info-columns">
        <dl className="photo-readout">
          {row('X:', inside ? x(inside.x) : dash, 'photo-info-x')}
          {row('Y:', inside ? y(inside.y) : dash, 'photo-info-y')}
        </dl>
        <dl className="photo-readout">
          {row('W:', selection ? x(selection[2]) : dash, 'photo-info-w')}
          {row('H:', selection ? y(selection[3]) : dash, 'photo-info-h')}
        </dl>
      </div>
      <hr className="photo-info-rule" />
      <p className="photo-info-doc" data-testid="photo-info-doc">
        {phrase('Doc: {w} × {h} px').replace('{w}', String(info.width)).replace('{h}', String(info.height))}
      </p>
    </div>
  )
}
