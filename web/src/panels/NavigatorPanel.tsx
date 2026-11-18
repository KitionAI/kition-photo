import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent, type RefObject } from 'react'
import { Button, KitElement, deviceScale, useKitEditor, type KitCanvasHandle, type KitCanvasView } from '@kition/kit-editor'
import type { PhotoInfo } from '../photo'
import { useLive, type Live } from './live'
import { ZOOM_SLIDER, centredOn, retryDelay, navigatorImage, navigatorViewBox, sliderOfZoom, zoomOfSlider } from './readouts'

/** The frame's height (upstream's `box_h`). */
const FRAME_HEIGHT = 150
/** How long the image rests before the thumbnail is drawn again. */
const REFRESH_MS = 250
/** The thumbnail's longest side in device pixels: enough for any dock width. */
const MAX_SIDE = 640

/**
 * Upstream's Navigator panel (`panels.rs` `navigator`): the whole image, small (the engine's
 * `navigatorImage`, rendered from a reduced copy, at most every 250 ms while the image changes,
 * one request at a time, and only while the tab shows: the dock draws only a group's front tab),
 * with the canvas's visible area outlined; pressing or dragging on it centres the canvas there.
 * Below, Zoom with Fit, 100 %, and 200 %, and the zoom slider over log2 of the zoom (1 % to
 * 3200 %).
 */
export function NavigatorPanel({ info, view, canvas }: { info: PhotoInfo; view: Live<KitCanvasView | null>; canvas: RefObject<KitCanvasHandle | null> }) {
  const { phrase, session } = useKitEditor()
  const current = useLive(view)
  const frameRef = useRef<HTMLDivElement>(null)
  const thumb = useRef<HTMLCanvasElement>(null)
  const [frameWidth, setFrameWidth] = useState(0)
  const [drawn, setDrawn] = useState<{ doc: typeof session.doc; key: string } | null>(null)
  // When the thumbnail was last asked for: a changing image redraws it at most every REFRESH_MS.
  const lastAt = useRef(Number.NEGATIVE_INFINITY)
  const frame = { width: frameWidth, height: FRAME_HEIGHT }
  const image = navigatorImage(frame, info)
  const scale = image.width / Math.max(1, info.width)
  const scaleDevice = deviceScale()
  // The image's longer side in device pixels (the engine draws no more than the image's own).
  const side = Math.max(1, Math.round(Math.min(MAX_SIDE, Math.max(image.width, image.height) * scaleDevice)))
  const revision = session.info?.revision ?? 0
  const key = `${revision}:${side}`
  const doc = session.doc
  const documentNow = useRef(doc)
  documentNow.current = doc
  const ready = drawn?.doc === doc && drawn?.key === key

  useLayoutEffect(() => {
    const element = frameRef.current
    if (!element) return undefined
    setFrameWidth(element.clientWidth)
    if (typeof ResizeObserver === 'undefined') return undefined
    const observer = new ResizeObserver(() => setFrameWidth(element.clientWidth))
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  // At most one request in flight, and one every REFRESH_MS while the image changes; the latest
  // key is asked for once the one in flight answers.
  const inFlight = useRef<typeof doc>(null)
  const failures = useRef<{ doc: typeof doc; key: string; count: number }>({ doc: null, key: '', count: 0 })
  const [retry, setRetry] = useState(0)
  useEffect(() => {
    if (!doc || frameWidth <= 0 || ready) return undefined
    // A failing ask backs off, and gives up until the next change after its last retry.
    const failed = failures.current.doc === doc && failures.current.key === key ? failures.current.count : 0
    const backoff = retryDelay(failed)
    if (backoff === null) return undefined
    const timer = window.setTimeout(
      () => {
        if (inFlight.current === doc) return
        inFlight.current = doc
        lastAt.current = performance.now()
        const asked = key
        void doc.invoke<Uint8Array>('navigatorImage', side).then(
          (bytes) => {
            if (inFlight.current === doc) inFlight.current = null
            if (documentNow.current !== doc) return
            const element = thumb.current
            const context = element?.getContext('2d')
            const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
            const [width, height] = bytes.byteLength >= 8 ? [view.getUint32(0, true), view.getUint32(4, true)] : [0, 0]
            if (element && context && width > 0 && bytes.byteLength >= 8 + width * height * 4) {
              element.width = width
              element.height = height
              context.putImageData(new ImageData(new Uint8ClampedArray(bytes.buffer as ArrayBuffer, bytes.byteOffset + 8, width * height * 4), width, height), 0, 0)
            }
            setDrawn({ doc, key: asked })
            setRetry((count) => count + 1)
          },
          () => {
            if (inFlight.current === doc) inFlight.current = null
            if (documentNow.current !== doc) return
            failures.current = { doc, key: asked, count: (failures.current.doc === doc && failures.current.key === asked ? failures.current.count : 0) + 1 }
            setRetry((count) => count + 1)
          },
        )
      },
      Math.max(backoff, lastAt.current + REFRESH_MS - performance.now()),
    )
    return () => window.clearTimeout(timer)
    // `retry` asks again once a request in flight answers; the key names the revision and the size.
  }, [doc, ready, frameWidth, key, retry, side])

  const box = current && current.width > 0 ? navigatorViewBox(current, current, image, scale, frame) : null
  const centre = (event: PointerEvent<HTMLDivElement>) => {
    const handle = canvas.current
    const now = view.get()
    const element = frameRef.current
    if (!handle || !now || !element) return
    const rect = element.getBoundingClientRect()
    const point = { x: (event.clientX - rect.left - image.x) / scale, y: (event.clientY - rect.top - image.y) / scale }
    const at = centredOn(point, now, now, info)
    handle.panTo(at.x, at.y)
  }
  const dragging = useRef(false)
  const zoom = current?.zoom ?? 1
  return (
    <div className="photo-navigator" data-testid="photo-navigator">
      <KitElement id="control:navigator.preview">
        <div
          ref={frameRef}
          className="photo-navigator-frame"
          style={{ height: FRAME_HEIGHT }}
          role="presentation"
          onPointerDown={(event) => {
            event.currentTarget.setPointerCapture?.(event.pointerId)
            dragging.current = true
            centre(event)
          }}
          onPointerMove={(event) => dragging.current && centre(event)}
          onPointerUp={() => {
            dragging.current = false
          }}
          onPointerCancel={() => {
            dragging.current = false
          }}
          data-testid="photo-navigator-frame"
          data-drawn={ready}
        >
          <canvas ref={thumb} className="photo-navigator-image" style={{ left: image.x, top: image.y, width: image.width, height: image.height }} data-testid="photo-navigator-image" />
          {box ? <span className="photo-navigator-view" style={{ left: box.x, top: box.y, width: box.width, height: box.height }} data-testid="photo-navigator-view" /> : null}
        </div>
      </KitElement>
      <KitElement id="control:navigator.zoom">
        <div className="photo-navigator-zoom">
          <span className="photo-navigator-label">{phrase('Zoom')}</span>
          <span className="photo-navigator-presets">
            <Button onClick={() => canvas.current?.fit()} data-testid="photo-navigator-fit">
              {phrase('Fit')}
            </Button>
            {[1, 2].map((preset) => (
              <Button key={preset} aria-pressed={Math.abs(zoom - preset) < 1e-3} onClick={() => canvas.current?.zoomTo(preset)} data-testid={`photo-navigator-zoom-${preset * 100}`}>
                {`${preset * 100}%`}
              </Button>
            ))}
          </span>
        </div>
        <input
          type="range"
          className="photo-navigator-slider"
          min={ZOOM_SLIDER.min}
          max={ZOOM_SLIDER.max}
          step={0.01}
          value={sliderOfZoom(zoom)}
          aria-label={phrase('Zoom')}
          aria-valuetext={`${Math.round(zoom * 100)}%`}
          onChange={(event) => canvas.current?.zoomTo(zoomOfSlider(Number(event.target.value)))}
          data-testid="photo-navigator-slider"
        />
      </KitElement>
    </div>
  )
}
