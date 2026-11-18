import { useEffect, useRef, useState } from 'react'
import { TesseraIcons } from '@kition/kit-editor'
import { Choice, Histogram, KitElement, useKitEditor } from '@kition/kit-editor'
import type { PhotoInfo } from '../photo'
import { useRemembered } from './PropsParts'
import { cacheLevel, histogramStats, retryDelay } from './readouts'


const TriangleAlert = TesseraIcons.TriangleAlertIcon

/** The `histogram` extra's channels, with upstream's labels (Photoshop's Histogram panel adds Luminosity). */
const CHANNELS = [
  ['rgb', 'RGB'],
  ['red', 'Red'],
  ['green', 'Green'],
  ['blue', 'Blue'],
  ['luminosity', 'Luminosity'],
] as const

type Channel = (typeof CHANNELS)[number][0]

/** Upstream recomputes the histogram at most every 250 ms while the image keeps changing. */
const REFRESH_MS = 250

/**
 * Upstream's Histogram panel (`tone::histogram_panel`): the whole image's histogram of the chosen
 * channel (the engine's `histogram` extra, sampled from a reduced render), drawn as upstream's
 * `draw_histogram` draws it (the raw bins, scaled to the 251st-smallest bin), with its statistics
 * (mean, standard deviation, median, pixels, cache level). While the image changes it is computed
 * at most every 250 ms, one request at a time; a new channel is asked for at once; and only while
 * the tab shows (the dock draws only a group's front tab). While it lags the image, Photoshop's
 * warning triangle shows.
 */
export function HistogramPanel({ info }: { info: PhotoInfo }) {
  const { phrase, session } = useKitEditor()
  const [channel, setChannel] = useRemembered<Channel>('histogram-channel', 'rgb')
  const revision = session.info?.revision ?? 0
  const [shown, setShown] = useState<{ doc: typeof session.doc; key: string; bins: Uint32Array } | null>(null)
  const key = `${channel}:${revision}`
  const doc = session.doc
  const documentNow = useRef(doc)
  documentNow.current = doc
  const current = shown?.doc === doc ? shown : null
  const inFlight = useRef<typeof doc>(null)
  const failures = useRef<{ doc: typeof doc; key: string; count: number }>({ doc: null, key: '', count: 0 })
  const lastAt = useRef(Number.NEGATIVE_INFINITY)
  const [retry, setRetry] = useState(0)
  useEffect(() => {
    if (!doc || current?.key === key) return undefined
    // A failing ask backs off, and gives up until the next change after its last retry.
    const failed = failures.current.doc === doc && failures.current.key === key ? failures.current.count : 0
    const backoff = retryDelay(failed)
    if (backoff === null) return undefined
    // A channel the panel has not shown yet comes at once; a changed image waits its turn.
    const sameChannel = current?.key.startsWith(`${channel}:`) ?? false
    const timer = window.setTimeout(
      () => {
        if (inFlight.current === doc) return
        inFlight.current = doc
        lastAt.current = performance.now()
        const asked = key
        void doc.invoke<Uint32Array>('histogram', channel).then(
          (bins) => {
            if (inFlight.current === doc) inFlight.current = null
            if (documentNow.current !== doc) return
            setShown({ doc, key: asked, bins })
          },
          () => {
            if (inFlight.current === doc) inFlight.current = null
            if (documentNow.current !== doc) return
            failures.current = { doc, key: asked, count: (failures.current.doc === doc && failures.current.key === asked ? failures.current.count : 0) + 1 }
            setRetry((count) => count + 1)
          },
        )
      },
      Math.max(backoff, sameChannel ? lastAt.current + REFRESH_MS - performance.now() : 0),
    )
    return () => window.clearTimeout(timer)
    // `shown` answers re-run this for the latest key; `retry` after a failure.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channel, doc, key, retry, current?.key])

  const stale = current?.key !== key
  const stats = current ? histogramStats(current.bins) : null
  const rows: [string, string][] = stats
    ? [
        ['Mean:', stats.mean.toFixed(2)],
        ['Std Dev:', stats.stdDev.toFixed(2)],
        ['Median:', String(stats.median)],
        ['Pixels:', String(info.width * info.height)],
        ['Cache Level:', String(cacheLevel(info.width, info.height))],
      ]
    : []
  return (
    <div className="photo-histogram" data-testid="photo-histogram" data-channel={current?.key.split(':')[0]} data-stale={stale}>
      <KitElement id="control:histogram.channel">
        <Choice label={phrase('Channel:')} value={channel} choices={CHANNELS.map(([value, label]) => [value, phrase(label)] as const)} onChange={setChannel} testId="photo-histogram-channel" />
      </KitElement>
      <div className="photo-histogram-graph" data-channel={channel}>
        {current ? <Histogram bins={current.bins} scale="rank" height={100} testId="photo-histogram-bins" /> : <div className="photo-histogram-empty" style={{ height: 100 }} />}
        {stale ? <TriangleAlert className="photo-histogram-stale" size={12} aria-hidden="true" data-testid="photo-histogram-stale" /> : null}
      </div>
      {rows.length ? (
        <dl className="photo-readout photo-histogram-stats" data-testid="photo-histogram-stats">
          {rows.map(([label, value]) => (
            <div key={label}>
              <dt>{phrase(label)}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      ) : null}
    </div>
  )
}
