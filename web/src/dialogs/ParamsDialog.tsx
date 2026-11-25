import { useMemo, useState } from 'react'
import { useKitEditor, type KitCommand } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { DialogFrame } from './DialogFrame'
import { CheckRow, ChoiceRow, ColorRow, NumberRow, SliderRow, TextRow } from './controls'
import { dialogTitle, optionLabel, paramLabel } from './labels'
import { initialValues, paramFields, paramsOf, type ParamField, type ParamValues } from './params'
import { previews } from './routes'

/** The values each command's dialog was last OK'd with, for the session (Photoshop's dialogs remember theirs). */
const remembered = new Map<string, ParamValues>()

/**
 * Choices a command takes from the document rather than its schema: Load Selection's channel (the
 * saved selections, then the active layer's transparency and mask).
 */
function documentFields(command: string, info: PhotoInfo, words: { t: (key: string, values?: Record<string, string | number>) => string }): { field: ParamField; labels: Record<string, string> }[] {
  if (command !== 'select.loadSelection') return []
  const active = info.layers.find((layer) => layer.id === info.activeLayer)
  const alpha = info.channels?.alpha ?? []
  const options = [...alpha.map((channel) => channel.name), 'transparency', ...(active?.hasMask ? ['mask'] : [])]
  const labels: Record<string, string> = { transparency: words.t('dialog.layerTransparency', { name: active?.name ?? '' }), mask: words.t('dialog.layerMask', { name: active?.name ?? '' }) }
  return [{ field: { key: 'channel', kind: 'choice', options, initial: options[0] }, labels }]
}

/**
 * The generic parameter dialog (plan 2026-11-24, stage 1): a control per parameter a person sets,
 * labelled in the editor's language, with a live preview for filters and adjustments. It opens
 * with the values last OK'd (the engine's defaults the first time); Alt+Cancel resets them.
 */
export function ParamsDialog({ command, info, values: given, onClose }: { command: KitCommand; info: PhotoInfo; values?: Record<string, unknown>; onClose: () => void }) {
  const words = useKitEditor()
  const extra = useMemo(() => documentFields(command.id, info, words), [command.id, info, words])
  const fields = useMemo(() => [...extra.map((item) => item.field), ...paramFields(command)], [command, extra])
  const optionLabels = useMemo(() => Object.assign({}, ...extra.map((item) => item.labels)) as Record<string, string>, [extra])
  const defaults = useMemo(() => initialValues(fields), [fields])
  const [values, setValues] = useState<ParamValues>(() => initialValues(fields, { ...remembered.get(command.id), ...given }))
  const set = (key: string, value: ParamValues[string]) => setValues((current) => ({ ...current, [key]: value }))
  const params = paramsOf(fields, values)
  const label = (key: string) => paramLabel(words, key)
  const testId = (key: string) => `photo-dialog-param-${key}`
  return (
    <DialogFrame
      command={command.id}
      title={dialogTitle(words, command.title)}
      params={params}
      previewing={previews(command.id)}
      onReset={() => setValues(defaults)}
      onClose={(committed) => {
        if (committed) remembered.set(command.id, values)
        onClose()
      }}
    >
      <div className="photo-dialog-fields">
        {fields.map((field) => {
          const value = values[field.key]
          switch (field.kind) {
            case 'slider':
              return <SliderRow key={field.key} label={label(field.key)} value={typeof value === 'number' ? value : field.initial} min={field.min} max={field.max} softMax={field.softMax} decimals={field.decimals} fallback={field.initial} onChange={(next) => set(field.key, next)} testId={testId(field.key)} />
            case 'number':
              return <NumberRow key={field.key} label={label(field.key)} value={typeof value === 'number' ? value : null} min={field.min} max={field.max} decimals={field.decimals} onChange={(next) => set(field.key, next)} testId={testId(field.key)} />
            case 'choice':
              return (
                <ChoiceRow
                  key={field.key}
                  label={label(field.key)}
                  value={typeof value === 'string' ? value : field.initial}
                  choices={field.options.map((option) => [option, optionLabels[option] ?? optionLabel(words, option)] as const)}
                  onChange={(next) => set(field.key, next)}
                  testId={testId(field.key)}
                />
              )
            case 'check':
              return <CheckRow key={field.key} label={label(field.key)} checked={value === true} onChange={(next) => set(field.key, next)} testId={testId(field.key)} />
            case 'color':
              return <ColorRow key={field.key} label={label(field.key)} value={typeof value === 'string' ? value : field.initial} onChange={(next) => set(field.key, next)} testId={testId(field.key)} />
            case 'text':
              return <TextRow key={field.key} label={label(field.key)} value={typeof value === 'string' ? value : ''} placeholder={field.key === 'name' ? words.t('dialog.layerNameHint') : undefined} onChange={(next) => set(field.key, next)} testId={testId(field.key)} />
          }
        })}
      </div>
    </DialogFrame>
  )
}
