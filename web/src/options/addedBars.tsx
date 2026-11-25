import { TesseraIcons, useKitEditor } from '@kition/sdk/editor'
import { BrushTip } from './brushBars'
import { Check, Choice, Hint, Icons, Num, Sep } from './fields'

const CircleDot = TesseraIcons.CircleDotIcon
const Pipette = TesseraIcons.PipetteIcon
const Square = TesseraIcons.SquareIcon

/**
 * The bars of the tools Kition Photo adds to upstream's rail (plan 2026-11-24, stage 7). They draw
 * only options upstream's tools already keep (the brush tip, the Background Eraser's sampling,
 * limits and tolerance, Anti-alias, Aligned), shared as upstream shares them.
 */

/** Red Eye: a click on a red pupil darkens it (the engine's pupil size and darkening, 50 % each). */
export function RedEyeBar() {
  const { t } = useKitEditor()
  return <Hint>{t('redEye.hint')}</Hint>
}

/** Color Replacement: the brush tip, sampling, limits, tolerance and anti-aliasing (the foreground colour replaces). */
export function ColorReplacementBar() {
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
      <Check k="antiAlias" label={phrase('Anti-alias')} />
    </>
  )
}

/** Pattern Stamp: the brush tip and Aligned; the pattern is the Patterns panel's current one. */
export function PatternStampBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <BrushTip />
      <Check k="aligned" label={phrase('Aligned')} />
    </>
  )
}

/** Convert Point: what its presses do. */
export function ConvertPointBar() {
  const { t } = useKitEditor()
  return <Hint>{t('convertPoint.hint')}</Hint>
}
