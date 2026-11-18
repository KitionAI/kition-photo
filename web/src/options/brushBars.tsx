import { TesseraIcons } from '@kition/kit-editor'
import { useKitEditor } from '@kition/kit-editor'
import { controlOf } from './controls'
import { BlendMode, Check, Choice, FillSampling, Hint, Icons, Num, Opt, Sep, Toggle, fill, useBar } from './fields'

const CircleDot = TesseraIcons.CircleDotIcon
const Pipette = TesseraIcons.PipetteIcon
const Square = TesseraIcons.SquareIcon

/** Upstream's brush chip: the tip at its hardness, with its size under it. */
function TipPreview({ size, hardness }: { size: number; hardness: number | null }) {
  const edge = Math.max(0, Math.min(100, hardness ?? 100))
  const id = `photo-tip-${edge}`
  return (
    <span className="photo-brush-tip" aria-hidden="true">
      <svg width="22" height="22" viewBox="0 0 22 22">
        <defs>
          <radialGradient id={id}>
            <stop offset={`${edge * 0.98}%`} stopColor="currentColor" stopOpacity="1" />
            <stop offset="100%" stopColor="currentColor" stopOpacity="0" />
          </radialGradient>
        </defs>
        <circle cx="11" cy="11" r="9" fill={`url(#${id})`} />
      </svg>
      <span>{Math.round(size)}</span>
    </span>
  )
}

/** The brush tip (the engine's one brush): its preview, Size, and (unless the tool is always hard) Hardness. */
export function BrushTip({ hardness = true }: { hardness?: boolean }) {
  const { phrase } = useKitEditor()
  const { o } = useBar()
  return (
    <>
      <TipPreview size={o.size} hardness={hardness ? o.hardness : null} />
      <Num k="size" label={phrase('Size')} unit="px" />
      {hardness ? <Num k="hardness" label={phrase('Hardness')} unit="%" /> : null}
      <Sep />
    </>
  )
}

/** Brush and Eraser (upstream's Photoshop bar, with the non-Photoshop bar's Size and Hardness). */
export function BrushBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <BrushTip />
      <BlendMode k="mode" />
      <Num k="opacity" label={phrase('Opacity')} unit="%" />
      <Toggle k="pressureOpacity" icon={CircleDot} label={phrase('Pressure for Opacity')} />
      <Num k="flow" label={phrase('Flow')} unit="%" />
      <Num k="smoothing" label={phrase('Smoothing')} unit="%" />
      <Sep />
      <Toggle k="pressureSize" icon={CircleDot} label={phrase('Pressure for Size')} />
    </>
  )
}

/** The Pencil: no hardness or flow (it is always hard), and Auto Erase. */
export function PencilBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <BrushTip hardness={false} />
      <BlendMode k="mode" />
      <Num k="opacity" label={phrase('Opacity')} unit="%" />
      <Num k="smoothing" label={phrase('Smoothing')} unit="%" />
      <Sep />
      <Check k="autoErase" label={phrase('Auto Erase')} />
    </>
  )
}

export function MixerBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <BrushTip />
      <Num k="wet" label={phrase('Wet')} unit="%" />
      <Num k="load" label={phrase('Load')} unit="%" />
      <Num k="mix" label={phrase('Mix')} unit="%" />
      <Num k="mixerFlow" label={phrase('Flow')} unit="%" />
      <Sep />
      <Check k="mixerSampleAll" label={phrase('Sample All Layers')} />
    </>
  )
}

export function MagicEraserBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <FillSampling />
      <Sep />
      <Num k="eraserOpacity" label={phrase('Opacity')} unit="%" />
    </>
  )
}

export function BackgroundEraserBar() {
  const { phrase, t } = useKitEditor()
  return (
    <>
      <BrushTip />
      <Icons
        k="bgSampling"
        label={t('options.sampling')}
        choices={[
          { value: 'continuous', icon: Pipette, label: phrase('Sampling: Continuous') },
          { value: 'once', icon: CircleDot, label: phrase('Sampling: Once') },
          { value: 'backgroundSwatch', icon: Square, label: phrase('Sampling: Background Swatch') },
        ]}
      />
      <Sep />
      <Choice
        k="bgLimits"
        label={phrase('Limits:')}
        choices={[
          ['discontiguous', phrase('Discontiguous')],
          ['contiguous', phrase('Contiguous')],
          ['findEdges', phrase('Find Edges')],
        ]}
      />
      <Num k="bgTolerance" label={phrase('Tolerance')} unit="%" />
      <Check k="protectForeground" label={phrase('Protect Foreground Color')} />
    </>
  )
}

/** Choices drawn as check boxes of which one is on (upstream's Spot Healing type and Patch mode). */
function Radios({ k, label, choices }: { k: 'spotType' | 'patchMode'; label: string; choices: readonly (readonly [string, string])[] }) {
  const { o, set } = useBar()
  const field = controlOf(k)
  return (
    <Opt field={field} option={k}>
      <span className="photo-options-label">{label}</span>
      {choices.map(([choice, text]) => (
        <label key={choice} className="kui-checkbox">
          <input type="checkbox" checked={o[k] === choice} onChange={() => set({ [k]: choice })} data-testid={`photo-option-${field}-${choice}`} />
          {text}
        </label>
      ))}
    </Opt>
  )
}

/** The retouching tools (upstream's `retouch_ui::options_bar`), after the brush tip. */
export function RetouchBar() {
  const { phrase } = useKitEditor()
  const { tool, cloneSourceSet } = useBar()
  const sampleAll = <Check k="sampleAllLayers" label={phrase('Sample All Layers')} />
  switch (tool) {
    case 'SpotHealing':
      return (
        <>
          <BrushTip />
          <Radios
            k="spotType"
            label={phrase('Type:')}
            choices={[
              ['smart', phrase('Smart Fill')],
              ['createTexture', phrase('Create Texture')],
              ['proximityMatch', phrase('Proximity Match')],
            ]}
          />
          <Sep />
          {sampleAll}
        </>
      )
    case 'Patch':
      return (
        <>
          <Radios
            k="patchMode"
            label={phrase('Patch:')}
            choices={[
              ['source', phrase('Source')],
              ['destination', phrase('Destination')],
            ]}
          />
          <Sep />
          <Hint>{phrase('Lasso around an area, then drag the selection')}</Hint>
        </>
      )
    case 'SmartMove':
      return (
        <>
          <Choice
            k="camMode"
            label={phrase('Mode')}
            choices={[
              ['move', phrase('Move')],
              ['extend', phrase('Extend')],
            ]}
          />
          <Num k="structure" label={phrase('Structure:')} />
          <Num k="colorAdapt" label={phrase('Color:')} />
          {sampleAll}
          <Sep />
          <Hint>{phrase('Lasso around an area, then drag the selection')}</Hint>
        </>
      )
    case 'Healing':
    case 'CloneStamp':
      return (
        <>
          <BrushTip />
          <Check k="aligned" label={phrase('Aligned')} />
          <Choice
            k="cloneSample"
            label={phrase('Sample:')}
            choices={[
              ['current', phrase('Current Layer')],
              ['currentAndBelow', phrase('Current & Below')],
              ['all', phrase('All Layers')],
            ]}
          />
          {cloneSourceSet ? null : (
            <>
              <Sep />
              <Hint testId="photo-clone-hint">{fill(phrase('{key}-click to set the source'), { key: 'Alt' })}</Hint>
            </>
          )}
        </>
      )
    case 'Dodge':
    case 'Burn':
      return (
        <>
          <BrushTip />
          <Choice
            k="range"
            label={phrase('Range:')}
            choices={[
              ['shadows', phrase('Shadows')],
              ['midtones', phrase('Midtones')],
              ['highlights', phrase('Highlights')],
            ]}
          />
          <Num k="exposure" label={phrase('Exposure:')} unit="%" />
          <Check k="protectTones" label={phrase('Protect Tones')} />
        </>
      )
    case 'Sponge':
      return (
        <>
          <BrushTip />
          <Choice
            k="spongeMode"
            label={phrase('Mode')}
            choices={[
              ['desaturate', phrase('Desaturate')],
              ['saturate', phrase('Saturate')],
            ]}
          />
          <Check k="vibrance" label={phrase('Vibrance')} />
        </>
      )
    case 'Blur':
    case 'Sharpen':
    case 'Smudge':
      return (
        <>
          <BrushTip />
          <Num k="strength" label={phrase('Strength:')} unit="%" />
          {sampleAll}
          {tool === 'Sharpen' ? <Check k="protectDetail" label={phrase('Protect Detail')} /> : null}
          {tool === 'Smudge' ? <Check k="fingerPainting" label={phrase('Finger Painting')} /> : null}
        </>
      )
    case 'HistoryBrush':
      return (
        <>
          <BrushTip />
          <Hint>{phrase("Paints from the document's opening state")}</Hint>
        </>
      )
    default:
      return null
  }
}
