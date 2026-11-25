import { useState } from 'react'
import { useEngineQuery, useKitEditor, type KitCommand } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { DialogFrame } from './DialogFrame'
import { CheckRow, ChoiceRow, ColorRow, SliderRow } from './controls'
import { dialogTitle, optionLabel } from './labels'
import { BLEND_MODES } from './params'

/**
 * Fill and Stroke (Photoshop's Edit › Fill… and Stroke…): what to fill with or the line's width,
 * colour and place, then the blending (mode, opacity, preserve transparency). Like Photoshop's
 * they do not preview; OK runs once. Each opens with the choices last OK'd in this session.
 */

type Props = { command: KitCommand; info: PhotoInfo; colors: { foreground: string; background: string }; onClose: () => void }

const CONTENTS = ['foreground', 'background', 'color', 'smart', 'pattern', 'history', 'black', 'gray', 'white'] as const
type Contents = (typeof CONTENTS)[number]

type FillState = { contents: Contents; color: string | null; pattern: string | null; mode: string; opacity: number; preserveTransparency: boolean; colorAdaptation: boolean }
type StrokeState = { width: number; color: string | null; location: 'inside' | 'center' | 'outside'; mode: string; opacity: number; preserveTransparency: boolean }

let keptFill: FillState | null = null
let keptStroke: StrokeState | null = null

/** Fill's parameters: the contents and what it needs (a colour, a pattern), and the blending. */
export function fillParams(state: FillState, foreground: string): Record<string, unknown> {
  const params: Record<string, unknown> = { contents: state.contents, mode: state.mode, opacity: state.opacity, preserveTransparency: state.preserveTransparency }
  if (state.contents === 'color') params.color = state.color ?? foreground
  if (state.contents === 'pattern' && state.pattern) params.pattern = state.pattern
  if (state.contents === 'smart') params.colorAdaptation = state.colorAdaptation
  return params
}

function Blending({ mode, opacity, preserveTransparency, onChange }: { mode: string; opacity: number; preserveTransparency: boolean; onChange: (patch: Partial<{ mode: string; opacity: number; preserveTransparency: boolean }>) => void }) {
  const words = useKitEditor()
  const { t } = words
  return (
    <>
      <div className="photo-dialog-caption">{t('dialog.blending')}</div>
      <ChoiceRow label={t('dialog.mode')} value={mode} choices={BLEND_MODES.map((item) => [item, optionLabel(words, item)] as const)} onChange={(next) => onChange({ mode: next })} testId="photo-dialog-mode" />
      <SliderRow label={t('dialog.opacity')} value={opacity} min={0} max={100} fallback={100} onChange={(next) => onChange({ opacity: next })} testId="photo-dialog-opacity" />
      <CheckRow label={t('dialog.preserveTransparency')} checked={preserveTransparency} onChange={(next) => onChange({ preserveTransparency: next })} testId="photo-dialog-preserve-transparency" />
    </>
  )
}

export function FillDialog({ command, colors, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [state, setState] = useState<FillState>(() => keptFill ?? { contents: 'foreground', color: null, pattern: null, mode: 'normal', opacity: 100, preserveTransparency: false, colorAdaptation: true })
  const patch = (next: Partial<FillState>) => setState((current) => ({ ...current, ...next }))
  const patterns = useEngineQuery<{ id: string; name: string }[]>('pattern.list') ?? []
  const pattern = state.pattern ?? patterns[0]?.id ?? null
  const shown = { ...state, pattern }
  return (
    <DialogFrame
      command={command.id}
      title={dialogTitle(words, command.title)}
      params={fillParams(shown, colors.foreground)}
      previewing={false}
      okDisabled={state.contents === 'pattern' && !pattern}
      onClose={(committed) => {
        if (committed) keptFill = shown
        onClose()
      }}
    >
      <div className="photo-dialog-fields">
        <ChoiceRow label={t('dialog.contents')} value={state.contents} choices={CONTENTS.map((item) => [item, t(`dialog.fill.${item}`)] as const)} onChange={(contents) => patch({ contents })} testId="photo-dialog-contents" />
        {state.contents === 'color' ? <ColorRow label={t('dialog.color')} value={state.color ?? colors.foreground} onChange={(color) => patch({ color })} testId="photo-dialog-color" /> : null}
        {state.contents === 'pattern' ? (
          patterns.length ? (
            <ChoiceRow label={t('dialog.pattern')} value={pattern ?? ''} choices={patterns.map((item) => [item.id, words.phrase(item.name)] as const)} onChange={(next) => patch({ pattern: next })} testId="photo-dialog-pattern" />
          ) : (
            <div className="photo-dialog-hint">{t('dialog.noPatterns')}</div>
          )
        ) : null}
        {state.contents === 'smart' ? <CheckRow label={t('dialog.colorAdaptation')} checked={state.colorAdaptation} onChange={(colorAdaptation) => patch({ colorAdaptation })} testId="photo-dialog-color-adaptation" /> : null}
        <Blending mode={state.mode} opacity={state.opacity} preserveTransparency={state.preserveTransparency} onChange={patch} />
      </div>
    </DialogFrame>
  )
}

export function StrokeDialog({ command, info, colors, onClose }: Props) {
  const words = useKitEditor()
  const { t } = words
  const [state, setState] = useState<StrokeState>(() => keptStroke ?? { width: 1, color: null, location: 'center', mode: 'normal', opacity: 100, preserveTransparency: false })
  const patch = (next: Partial<StrokeState>) => setState((current) => ({ ...current, ...next }))
  const color = state.color ?? colors.foreground
  return (
    <DialogFrame
      command={command.id}
      title={dialogTitle(words, command.title)}
      params={{ width: state.width, color, location: state.location, mode: state.mode, opacity: state.opacity, preserveTransparency: state.preserveTransparency }}
      previewing={false}
      okDisabled={!info.hasSelection}
      onClose={(committed) => {
        if (committed) keptStroke = { ...state, color: state.color }
        onClose()
      }}
    >
      <div className="photo-dialog-fields">
        {info.hasSelection ? null : <div className="photo-dialog-warning">{t('dialog.needsSelection')}</div>}
        <div className="photo-dialog-caption">{t('dialog.stroke')}</div>
        <SliderRow label={t('dialog.width')} value={state.width} min={1} max={250} softMax={64} fallback={1} onChange={(width) => patch({ width })} testId="photo-dialog-width" />
        <ColorRow label={t('dialog.color')} value={color} onChange={(next) => patch({ color: next })} testId="photo-dialog-color" />
        <ChoiceRow label={t('dialog.location')} value={state.location} choices={(['inside', 'center', 'outside'] as const).map((item) => [item, t(`dialog.location.${item}`)] as const)} onChange={(location) => patch({ location })} testId="photo-dialog-location" />
        <Blending mode={state.mode} opacity={state.opacity} preserveTransparency={state.preserveTransparency} onChange={patch} />
      </div>
    </DialogFrame>
  )
}
