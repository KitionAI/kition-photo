import { useMemo, useRef, useState } from 'react'
import { PreviewDialog, useEngineQuery, useKitEditor, type KitCommand } from '@kition/sdk/editor'
import type { PhotoLayer } from '../photo'
import { DIALOG_BUTTONS, useDialogPlace } from './DialogFrame'
import { CheckRow, ChoiceRow, ColorRow, FieldRow, SliderRow } from './controls'
import { optionLabel, paramLabel } from './labels'
import { BLEND_MODES } from './params'
import { useStepsPreview, type PreviewStep } from './useCommandPreview'
import { EFFECT_FIELDS, EFFECT_LABELS, blendingChange, effectGradient, isEffectKind, replaceEffects, setEffectGradient, setEffectParams, styleFromLayer, styleRows, toggleEffect, type EffectKind, type FxField, type StyleAnswer, type StyleState } from './layerStyle'
import { useGradientEditor } from '../gradient/GradientEditorHost'
import { bakeMidpoints, cssGradient, fromEngine } from '../gradient/gradient'

/** The dialog's test id: rows `-row-<kind>` (`-row-<kind>-<n>` for a second of a kind) and `-row-blending`, their check boxes `-check-…`, fields `-field-<key>`. */
export const LAYER_STYLE = 'photo-layer-style'

/** What the dialog opens with: the layer's style, each kind's defaults, and the row to select. */
export type LayerStyleStart = { answer: StyleAnswer; defaults: Partial<Record<EffectKind, Record<string, unknown>>>; open: EffectKind | 'blending' }

type Selected = { kind: 'blending' } | { kind: EffectKind; index: number }

/**
 * Photoshop's Layer Style dialog (plan 2026-11-24, stage 5; the reference's `layer_style.rs`):
 * Blending Options and every effect on the left, each with its check box, the selected one's
 * fields on the right, the layer previewing the whole style while it is open. OK leaves one
 * history step (`layer.layerStyle.replace`, then `blendingOptions` when they changed, merged),
 * Cancel restores the layer. Opened by Layer › Layer Style's rows, the Layers panel's fx menu and
 * a double click on a layer row.
 */
export function LayerStyleDialog({ command, layer, start, onClose }: { command: KitCommand; layer: PhotoLayer; start: LayerStyleStart; onClose: () => void }) {
  const words = useKitEditor()
  const { t, phrase } = words
  // The layer as the dialog found it (its preview changes the layer the editor shows meanwhile).
  const [initial] = useState(() => styleFromLayer(start.answer, layer))
  const opened = useMemo((): { state: StyleState; selected: Selected } => {
    if (start.open === 'blending') return { state: initial, selected: { kind: 'blending' } }
    const kind = start.open
    const index = initial.entries.findIndex((entry) => entry.kind === kind)
    // A menu row's effect is switched on (and added when the layer has none of it), as Photoshop's.
    const toggled = toggleEffect(initial, kind, index < 0 ? null : index, true, start.defaults[kind] ?? {})
    return { state: toggled.state, selected: { kind, index: toggled.index } }
  }, [initial, start])
  const [state, setState] = useState<StyleState>(() => opened.state)
  const [selected, setSelected] = useState<Selected>(opened.selected)
  const stateNow = useRef(state)
  stateNow.current = state
  const editGradient = useGradientEditor()
  const patterns = useEngineQuery<{ id: string; name: string }[]>('pattern.list') ?? []
  const layerId = layer.id
  const params = useMemo(() => ({ effects: replaceEffects(state), blending: blendingChange(state, initial) }), [state, initial])
  const unchanged = useMemo(() => JSON.stringify({ effects: replaceEffects(initial), blending: null }) === JSON.stringify(params), [initial, params])
  const steps = (values: Record<string, unknown>): PreviewStep[] => {
    const blending = values.blending as Record<string, unknown> | null
    return [{ command: 'layer.layerStyle.replace', params: { layer: layerId, effects: values.effects } }, ...(blending ? [{ command: 'layer.layerStyle.blendingOptions', params: { layer: layerId, ...blending } }] : [])]
  }
  const run = useStepsPreview(`${command.id}-${layerId}`, steps, params, true)
  const place = useDialogPlace()

  const rows = styleRows(state)
  const label = (kind: EffectKind) => phrase(EFFECT_LABELS[kind])
  const rowId = (kind: EffectKind, index: number | null) => {
    const ofKind = rows.filter((row) => row.kind === kind)
    const position = ofKind.findIndex((row) => row.index === index)
    return position > 0 ? `${kind}-${position + 1}` : kind
  }
  const choose = (kind: EffectKind, index: number | null) => {
    if (index == null) {
      const added = toggleEffect(stateNow.current, kind, null, true, start.defaults[kind] ?? {})
      setState(added.state)
      setSelected({ kind, index: added.index })
    } else setSelected({ kind, index })
  }
  const check = (kind: EffectKind, index: number | null, on: boolean) => {
    const toggled = toggleEffect(stateNow.current, kind, index, on, start.defaults[kind] ?? {})
    setState(toggled.state)
    if (on) setSelected({ kind, index: toggled.index })
  }
  const entry = selected.kind === 'blending' ? null : state.entries[selected.index]
  const set = (patch: Record<string, unknown>) => {
    if (selected.kind === 'blending') return
    setState(setEffectParams(stateNow.current, selected.index, patch))
  }
  const fieldId = (key: string) => `${LAYER_STYLE}-field-${key}`
  const fieldOf = (field: FxField) => {
    if (!entry) return null
    const value = entry.params[field.key]
    const name = paramLabel(words, field.key)
    switch (field.kind) {
      case 'slider': {
        const number = typeof value === 'number' ? value : Number(value ?? 0)
        return <SliderRow key={field.key} label={field.unit ? `${name} (${field.unit})` : name} value={Number.isFinite(number) ? number : 0} min={field.min} max={field.max} softMax={field.softMax} onChange={(next) => set({ [field.key]: next })} testId={fieldId(field.key)} />
      }
      case 'choice': {
        const shown = typeof value === 'string' && field.options.includes(value) ? value : (field.options.find((option) => option.toLowerCase() === String(value ?? '').toLowerCase()) ?? field.options[0])
        return <ChoiceRow key={field.key} label={name} value={shown} choices={field.options.map((option) => [option, optionLabel(words, option)] as const)} onChange={(next) => set({ [field.key]: next })} testId={fieldId(field.key)} />
      }
      case 'check':
        return <CheckRow key={field.key} label={name} checked={value === true} onChange={(next) => set({ [field.key]: next })} testId={fieldId(field.key)} />
      case 'color':
        return <ColorRow key={field.key} label={name} value={typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value) ? value : '#000000'} onChange={(next) => set({ [field.key]: next })} testId={fieldId(field.key)} />
      case 'pattern': {
        const options: (readonly [string, string])[] = [...(field.optional ? [['', t('layerStyle.noTexture')] as const] : []), ...patterns.map((pattern) => [pattern.name, phrase(pattern.name)] as const)]
        if (!options.length) return <p key={field.key} className="photo-dialog-hint">{t('dialog.noPatterns')}</p>
        const shown = typeof value === 'string' && options.some(([option]) => option === value) ? value : options[0][0]
        return <ChoiceRow key={field.key} label={name} value={shown} choices={options} onChange={(next) => set({ [field.key]: next })} testId={fieldId(field.key)} />
      }
      case 'gradient': {
        const gradient = effectGradient(entry)
        const draft = fromEngine({ stops: gradient.stops.map((stop) => [stop.at, stop.color]), transparency: gradient.opacity.map((stop) => [stop.at, stop.opacity]) })
        const index = selected.kind === 'blending' ? -1 : selected.index
        const apply = (next: typeof draft) => {
          const baked = bakeMidpoints(next, '#000000', '#ffffff')
          setState(
            setEffectGradient(
              stateNow.current,
              index,
              baked.stops.map(([at, color]) => ({ at, color })),
              baked.transparency.map(([at, opacity]) => ({ at, opacity })),
            ),
          )
        }
        return (
          <FieldRow key={field.key} label={phrase('Gradient')}>
            <button
              type="button"
              className="photo-gradient-chip"
              style={{ backgroundImage: `${cssGradient(draft, '#000000', '#ffffff')}, var(--photo-checker)` }}
              title={t('gradient.edit')}
              aria-label={t('gradient.edit')}
              disabled={!editGradient}
              onClick={() => {
                if (!editGradient) return
                const before = stateNow.current
                void editGradient({ title: t('gradient.title'), value: draft, midpoints: false, onLive: apply }).then((chosen) => {
                  if (chosen) apply(chosen)
                  else setState(before)
                })
              }}
              data-testid={fieldId('gradient')}
            />
          </FieldRow>
        )
      }
    }
  }

  return (
    <PreviewDialog
      title={phrase('Layer Style')}
      position={place.position}
      onMove={place.onMove}
      preview={run.preview}
      onOk={async () => {
        if (unchanged) run.cancel()
        else await run.ok()
        onClose()
      }}
      onCancel={() => {
        run.cancel()
        onClose()
      }}
      ids={DIALOG_BUTTONS}
      testId={LAYER_STYLE}
    >
      <div className="photo-layer-style" data-layer={layerId}>
        <ul className="photo-layer-style-list" role="listbox" aria-label={phrase('Styles')}>
          <li>
            <button type="button" className="photo-layer-style-row" role="option" aria-selected={selected.kind === 'blending'} onClick={() => setSelected({ kind: 'blending' })} data-testid={`${LAYER_STYLE}-row-blending`}>
              {phrase('Blending Options')}
            </button>
          </li>
          {rows.map((row) => {
            const id = rowId(row.kind, row.index)
            const isSelected = selected.kind === row.kind && row.index != null && selected.index === row.index
            return (
              <li key={`${row.kind}-${row.index ?? 'new'}`} className="photo-layer-style-item">
                <input type="checkbox" checked={row.enabled} aria-label={label(row.kind)} onChange={(event) => check(row.kind, row.index, event.target.checked)} data-testid={`${LAYER_STYLE}-check-${id}`} />
                <button type="button" className="photo-layer-style-row" role="option" aria-selected={isSelected} onClick={() => choose(row.kind, row.index)} data-on={row.enabled} data-testid={`${LAYER_STYLE}-row-${id}`}>
                  {label(row.kind)}
                </button>
              </li>
            )
          })}
        </ul>
        <div className="photo-layer-style-fields photo-dialog-fields" data-effect={selected.kind}>
          <div className="photo-dialog-caption">{selected.kind === 'blending' ? phrase('Blending Options') : label(selected.kind)}</div>
          {selected.kind === 'blending' ? (
            <>
              <ChoiceRow label={paramLabel(words, 'blend')} value={state.blending.blend} choices={BLEND_MODES.map((mode) => [mode, optionLabel(words, mode)] as const)} onChange={(blend) => setState({ ...stateNow.current, blending: { ...stateNow.current.blending, blend } })} testId={fieldId('blend')} />
              <SliderRow label={`${phrase('Opacity')} (%)`} value={state.blending.opacity} min={0} max={100} onChange={(opacity) => setState({ ...stateNow.current, blending: { ...stateNow.current.blending, opacity } })} testId={fieldId('opacity')} />
              <SliderRow label={`${phrase('Fill Opacity')} (%)`} value={state.blending.fill} min={0} max={100} onChange={(fill) => setState({ ...stateNow.current, blending: { ...stateNow.current.blending, fill } })} testId={fieldId('fill')} />
            </>
          ) : entry ? (
            isEffectKind(entry.kind) ? EFFECT_FIELDS[entry.kind].map(fieldOf) : null
          ) : null}
        </div>
      </div>
    </PreviewDialog>
  )
}
