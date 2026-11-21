import { TesseraIcons } from '@kition/sdk/editor'
import { useKitEditor } from '@kition/sdk/editor'
import { BrushTip } from './brushBars'
import { Act, Check, Choice, FillSampling, Hint, Icon, Icons, Num, Sep, fill, useBar } from './fields'

const ArrowLeftRight = TesseraIcons.ArrowLeftRightIcon
const Minus = TesseraIcons.MinusIcon
const Plus = TesseraIcons.PlusIcon
const Square = TesseraIcons.SquareIcon
const SquaresIntersect = TesseraIcons.SquaresIntersectIcon

/** New, add, subtract, and intersect (upstream's `selection_mode_buttons`, one mode for every selection tool); Shift and Alt still win over them. */
function SelectionModes() {
  const { phrase, t } = useKitEditor()
  return (
    <Icons
      k="selectionMode"
      label={t('options.selectionMode')}
      choices={[
        { value: 'replace', icon: Square, label: phrase('New selection') },
        { value: 'add', icon: Plus, label: fill(phrase('Add to selection  ({key})'), { key: 'Shift' }) },
        { value: 'subtract', icon: Minus, label: fill(phrase('Subtract from selection  ({key})'), { key: 'Alt' }) },
        { value: 'intersect', icon: SquaresIntersect, label: fill(phrase('Intersect with selection  ({key})'), { key: 'Shift+Alt' }) },
      ]}
    />
  )
}

/** Select › Refine Selection is a later wave's workspace: drawn, as upstream, but not yet pressable. */
function RefineSelection() {
  const { phrase, t } = useKitEditor()
  return <Act field="refineSelection" label={phrase('Refine Selection…')} title={t('options.later')} disabled onClick={() => undefined} />
}

function SelectSubject() {
  const { phrase, run, enabled } = useKitEditor()
  return <Act field="selectSubject" label={phrase('Select Subject')} disabled={!enabled('select.subject')} onClick={() => void run('select.subject', {})} />
}

export function MarqueeBar() {
  const { phrase } = useKitEditor()
  const { tool, o, set } = useBar()
  const fixed = o.marqueeStyle !== 'normal'
  const size = o.marqueeStyle === 'fixedSize'
  // A fixed size takes whole pixels, a fixed ratio fractions down to upstream's 0.001.
  const limits = size ? { min: 1, max: 300000, step: 1, unit: 'px' } : { min: 0.001, max: 999, step: 0.001 }
  // A new style starts its numbers over, as upstream: 64 × 64 px for Fixed Size, else 1 : 1.
  const restyle = (marqueeStyle: string) => {
    const side = marqueeStyle === 'fixedSize' ? 64 : 1
    set({ marqueeStyle, marqueeWidth: side, marqueeHeight: side })
  }
  return (
    <>
      <SelectionModes />
      <Sep />
      <Num k="feather" label={phrase('Feather')} unit="px" />
      {/* Photoshop greys Anti-alias for the Rectangular Marquee: its edges are always hard. */}
      <Check k="antiAlias" label={phrase('Anti-alias')} disabled={tool !== 'EllipseMarquee'} />
      <Sep />
      <Choice
        k="marqueeStyle"
        label={phrase('Style')}
        choices={[
          ['normal', phrase('Normal')],
          ['fixedRatio', phrase('Fixed Ratio')],
          ['fixedSize', phrase('Fixed Size')],
        ]}
        onChange={restyle}
      />
      <Num k="marqueeWidth" label={phrase('Width')} {...limits} disabled={!fixed} />
      <Icon field="swapSize" icon={ArrowLeftRight} label={phrase('Swaps height and width')} disabled={!fixed} onClick={() => set({ marqueeWidth: o.marqueeHeight, marqueeHeight: o.marqueeWidth })} />
      <Num k="marqueeHeight" label={phrase('Height')} {...limits} disabled={!fixed} />
      <Sep />
      <RefineSelection />
    </>
  )
}

export function LassoBar() {
  const { phrase } = useKitEditor()
  const { tool } = useBar()
  return (
    <>
      <SelectionModes />
      <Sep />
      <Num k="feather" label={phrase('Feather')} unit="px" />
      <Check k="antiAlias" label={phrase('Anti-alias')} />
      <Sep />
      {tool === 'MagneticLasso' ? (
        <>
          <Num k="magneticWidth" label={phrase('Width')} unit="px" />
          <Num k="contrast" label={phrase('Contrast')} unit="%" />
          <Num k="frequency" label={phrase('Frequency')} />
          <Sep />
        </>
      ) : null}
      <RefineSelection />
      {tool === 'MagneticLasso' ? null : <Hint>{fill(phrase('Drag or click polygon points · hold Alt during lasso for straight segments · {add} add · {sub} before drawing subtracts'), { add: 'Shift', sub: 'Alt' })}</Hint>}
    </>
  )
}

export function MagicWandBar() {
  return (
    <>
      <SelectionModes />
      <Sep />
      <FillSampling />
      <Sep />
      <SelectSubject />
      <RefineSelection />
    </>
  )
}

export function QuickSelectionBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <BrushTip hardness={false} />
      <Check k="sampleAllLayers" label={phrase('Sample All Layers')} />
      <Check k="enhanceEdge" label={phrase('Enhance Edge')} />
      <Hint>{fill(phrase('{key} to subtract'), { key: 'Alt' })}</Hint>
      <Sep />
      <SelectSubject />
    </>
  )
}

export function ObjectSelectionBar() {
  const { phrase } = useKitEditor()
  return (
    <>
      <Check k="sampleAllLayers" label={phrase('Sample All Layers')} />
      <Hint>{phrase('Drag a rectangle around the object')}</Hint>
      <Sep />
      <SelectSubject />
    </>
  )
}
