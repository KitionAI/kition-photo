import { Choice, CurveEditor, Histogram, NumberField, useKitEditor, type CurvePoint } from '@kition/sdk/editor'
import { HandleTrack } from './HandleTrack'
import { gammaForMidtone, midtoneInput, RGB_CHANNELS, setLevel, type Levels, type RgbChannel } from './levels'

/**
 * Levels' and Curves' editors (Photoshop's dialogs and Properties sections): the channel, its
 * histogram, and the handles or the curve. Controlled: the caller keeps the values (a dialog's
 * draft, an adjustment layer's settings) and gets each change with `final` (a drag's last value,
 * a typed value, a key) so Properties makes one history step per gesture.
 */

/** The image's histogram per channel, as the engine's `histogram` extra answers it (256 bins); absent while it is read. */
export type Histograms = Partial<Record<RgbChannel, ArrayLike<number>>>

/** The colour a channel's histogram and curve are drawn in; the composite in the text colour. */
const CHANNEL_COLOR: Readonly<Record<RgbChannel, string | undefined>> = { rgb: undefined, red: '#e5484d', green: '#30a46c', blue: '#3e63dd' }

function ChannelChoice({ channel, onChannel, testId }: { channel: RgbChannel; onChannel: (channel: RgbChannel) => void; testId: string }) {
  const { t } = useKitEditor()
  const names: Readonly<Record<RgbChannel, string>> = { rgb: t('dialog.composite'), red: t('dialog.red'), green: t('dialog.green'), blue: t('dialog.blue') }
  return <Choice label={t('dialog.channel')} value={channel} choices={RGB_CHANNELS.map((item) => [item, names[item]] as const)} onChange={onChannel} testId={`${testId}-channel`} />
}

export function LevelsEditor({ levels, channel, onChannel, histograms, onChange, testId }: { levels: Readonly<Record<RgbChannel, Levels>>; channel: RgbChannel; onChannel: (channel: RgbChannel) => void; histograms: Histograms | null; onChange: (next: Record<RgbChannel, Levels>, final: boolean) => void; testId: string }) {
  const { t } = useKitEditor()
  const current = levels[channel]
  const change = (next: Levels, final: boolean) => onChange({ ...levels, [channel]: next }, final)
  const set = (key: keyof Levels) => (value: number, final: boolean) => change(setLevel(current, key, value), final)
  const bins = histograms?.[channel]
  // Short labels in the boxes (Photoshop's fields under the histogram are bare numbers); the full name for screen readers.
  const field = (key: keyof Levels, short: string, name: string, min: number, max: number, step: number) => <NumberField key={key} label={short} name={name} value={current[key]} min={min} max={max} step={step} onChange={set(key)} width={104} testId={`${testId}-${key}`} />
  return (
    <div className="photo-tone-editor" data-testid={`${testId}-levels`}>
      <ChannelChoice channel={channel} onChannel={onChannel} testId={testId} />
      <div className="photo-dialog-caption">{t('dialog.inputLevels')}</div>
      <div className="photo-histogram-box" style={{ color: CHANNEL_COLOR[channel] }}>
        {bins ? <Histogram bins={bins} height={96} scale="rank" testId={`${testId}-histogram`} /> : <div className="photo-histogram-empty" style={{ height: 96 }} />}
      </div>
      <HandleTrack
        handles={[
          { value: current.inBlack, label: t('dialog.shadowInput'), shade: '#000000', testId: `${testId}-handle-inBlack` },
          { value: midtoneInput(current), label: t('dialog.midtoneInput'), shade: '#808080', testId: `${testId}-handle-gamma` },
          { value: current.inWhite, label: t('dialog.highlightInput'), shade: '#ffffff', testId: `${testId}-handle-inWhite` },
        ]}
        onChange={(index, value, final) => {
          if (index === 0) set('inBlack')(value, final)
          else if (index === 2) set('inWhite')(value, final)
          else change(setLevel(current, 'gamma', gammaForMidtone(current, value)), final)
        }}
        testId={`${testId}-input-track`}
      />
      <div className="photo-tone-fields">
        {field('inBlack', t('dialog.black'), t('dialog.shadowInput'), 0, 253, 1)}
        {field('gamma', t('dialog.gamma'), t('dialog.midtoneInput'), 0.01, 9.99, 0.01)}
        {field('inWhite', t('dialog.white'), t('dialog.highlightInput'), 2, 255, 1)}
      </div>
      <div className="photo-dialog-caption">{t('dialog.outputLevels')}</div>
      <HandleTrack
        handles={[
          { value: current.outBlack, label: t('dialog.shadowOutput'), shade: '#000000', testId: `${testId}-handle-outBlack` },
          { value: current.outWhite, label: t('dialog.highlightOutput'), shade: '#ffffff', testId: `${testId}-handle-outWhite` },
        ]}
        gradient="linear-gradient(to right, #000, #fff)"
        onChange={(index, value, final) => set(index === 0 ? 'outBlack' : 'outWhite')(value, final)}
        testId={`${testId}-output-track`}
      />
      <div className="photo-tone-fields">
        {field('outBlack', t('dialog.black'), t('dialog.shadowOutput'), 0, 255, 1)}
        {field('outWhite', t('dialog.white'), t('dialog.highlightOutput'), 0, 255, 1)}
      </div>
    </div>
  )
}

export function CurvesEditor({ curves, channel, onChannel, histograms, onChange, testId }: { curves: Readonly<Record<RgbChannel, readonly CurvePoint[]>>; channel: RgbChannel; onChannel: (channel: RgbChannel) => void; histograms: Histograms | null; onChange: (next: Record<RgbChannel, readonly CurvePoint[]>, final: boolean) => void; testId: string }) {
  const { t } = useKitEditor()
  const bins = histograms?.[channel]
  return (
    <div className="photo-tone-editor" data-testid={`${testId}-curves`}>
      <ChannelChoice channel={channel} onChannel={onChannel} testId={testId} />
      <div className="photo-curve-box">
        {bins ? (
          <div className="photo-curve-histogram" style={{ color: CHANNEL_COLOR[channel] }} aria-hidden="true">
            <Histogram bins={bins} height={260} scale="rank" />
          </div>
        ) : null}
        <CurveEditor points={curves[channel]} kind="rgb" color={CHANNEL_COLOR[channel]} label={t('dialog.curve')} onChange={(points, phase) => onChange({ ...curves, [channel]: points }, phase === 'end')} testId={`${testId}-curve`} />
      </div>
      <div className="photo-dialog-hint">{t('dialog.curveHint')}</div>
    </div>
  )
}
