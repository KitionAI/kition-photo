import { createContext, Fragment, useContext, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'
import { useKitEditor, useUi, type KitCommand } from '@kition/sdk/editor'
import type { PhotoInfo } from '../photo'
import { BrightnessContrastDialog, ColorBalanceDialog, CurvesDialog, HueSaturationDialog, LevelsDialog } from './AdjustmentDialogs'
import { FillDialog, StrokeDialog } from './FillStrokeDialogs'
import { NewDocumentDialog } from './NewDocumentDialog'
import { PreferencesDialog } from './PreferencesDialog'
import type { PhotoPrefs } from '../prefs'
import { paramFields } from './params'
import { ParamsDialog } from './ParamsDialog'
import { dialogFor, type DialogKind } from './routes'
import { CanvasSizeDialog, ImageSizeDialog } from './SizeDialogs'
import { RGB_CHANNELS } from './levels'
import type { Histograms } from './ToneEditors'
import { LayerStyleDialog, type LayerStyleStart } from './LayerStyleDialog'
import { EFFECT_KINDS, isEffectKind, type StyleAnswer } from './layerStyle'

/**
 * The dialog host (plan 2026-11-24, stage 1): every way to run a command as a person does (a menu
 * row, a ribbon button, Home's, a shortcut, a panel's "…" row) goes through `invoke`, which opens
 * the command's dialog when it has one (`routes.ts`) instead of running it with the engine's
 * defaults. One dialog is open at a time.
 */
export type Dialogs = {
  /** Runs `command` as a click does: opens its dialog when it has one, else runs it. */
  invoke: (command: string) => void
  /** Opens `command`'s dialog (starting from `values`); false when it has none, true when it has one (open, or refused as disabled or denied). */
  open: (command: string, values?: Record<string, unknown>) => boolean
  /** Whether `command` opens a dialog. */
  asks: (command: string) => boolean
  /** File › New…'s dialog. */
  newDocument: () => void
}

const DialogsContext = createContext<Dialogs | null>(null)
export const DialogsProvider = DialogsContext.Provider

/** The editor's dialog host; outside one (a test's tree), commands run at once. */
export function useDialogs(): Dialogs {
  const own = useContext(DialogsContext)
  const { run } = useKitEditor()
  return own ?? { invoke: (command) => void run(command), open: () => false, asks: () => false, newDocument: () => undefined }
}

/** The dialog of `command`, or null: its purpose-built one, or the generic one when it has parameters to ask. */
export function routeOf(command: KitCommand | undefined): DialogKind | null {
  return command ? dialogFor({ id: command.id, title: command.title, asks: paramFields(command).length > 0 }) : null
}

type Open = { serial: number; command: KitCommand | null; kind: DialogKind | 'newDocument'; values?: Record<string, unknown>; histograms: Histograms | null; style?: LayerStyleStart }

/**
 * Holds the open dialog and answers `api` (which App passes to the menus, built outside the
 * editor's provider). Levels and Curves read the image's histograms before they open, so the
 * histogram shows the image before the preview changes it.
 */
export function PhotoDialogs({ api, info, colors, prefs, onPlace }: { api: RefObject<Dialogs | null>; info: PhotoInfo | null; colors: { foreground: string; background: string }; prefs: { value: PhotoPrefs; save: (next: PhotoPrefs) => void }; onPlace?: () => void }) {
  const editor = useKitEditor()
  const ui = useUi()
  const [open, setOpen] = useState<Open | null>(null)
  const serial = useRef(0)
  const busy = useRef(false)
  const latest = useRef({ editor, ui, open, onPlace })
  latest.current = { editor, ui, open, onPlace }

  useLayoutEffect(() => {
    const show = (next: Omit<Open, 'serial'>) => {
      serial.current += 1
      setOpen({ ...next, serial: serial.current })
    }
    const openDialog: Dialogs['open'] = (id, values) => {
      const { editor: now, ui: uiNow, open: current } = latest.current
      const command = now.byId.get(id)
      const kind = routeOf(command)
      if (!command || !kind) return false
      if (current || busy.current || !now.enabled(id) || uiNow.state(`command:${id}`, command.menu).denied) return true
      if (kind === 'place') {
        // Place Embedded asks the host for the file, not for parameters (plan 2026-11-24, stage 6).
        latest.current.onPlace?.()
        return true
      }
      if (kind === 'levels' || kind === 'curves') {
        // The histograms of the image as it is, read before the dialog's preview runs.
        const doc = now.session.doc
        busy.current = true
        void Promise.all(RGB_CHANNELS.map((channel) => (doc ? doc.invoke<Uint32Array>('histogram', channel).catch(() => null) : Promise.resolve(null))))
          .then((bins) => {
            const histograms: Histograms = {}
            RGB_CHANNELS.forEach((channel, index) => {
              if (bins[index]) histograms[channel] = bins[index]
            })
            show({ command, kind, values, histograms })
          })
          .finally(() => {
            busy.current = false
          })
        return true
      }
      if (kind === 'layerStyle') {
        // The layer's style and each effect's defaults, read before the dialog's preview runs.
        busy.current = true
        const kindOf = id.slice('layer.layerStyle.'.length)
        void Promise.all([now.query<StyleAnswer>('layer.layerStyle.get', {}), ...EFFECT_KINDS.map((effect) => now.query<{ params?: Record<string, unknown> }>('layer.layerStyle.defaultFor', { kind: effect }))])
          .then(([answer, ...defaults]) => {
            if (!answer) return
            const style: LayerStyleStart = {
              answer,
              defaults: Object.fromEntries(EFFECT_KINDS.map((effect, index) => [effect, defaults[index]?.params ?? {}])),
              open: isEffectKind(kindOf) ? kindOf : 'blending',
            }
            show({ command, kind, values, histograms: null, style })
          })
          .finally(() => {
            busy.current = false
          })
        return true
      }
      show({ command, kind, values, histograms: null })
      return true
    }
    api.current = {
      open: openDialog,
      invoke: (id) => {
        if (!openDialog(id)) void latest.current.editor.run(id)
      },
      asks: (id) => routeOf(latest.current.editor.byId.get(id)) !== null,
      newDocument: () => {
        if (!latest.current.open) show({ command: null, kind: 'newDocument', histograms: null })
      },
    }
  })

  if (!open || !info) return null
  const close = () => setOpen(null)
  const { command, kind } = open
  if (kind === 'newDocument') return <NewDocumentDialog key={open.serial} colors={colors} onClose={close} />
  if (!command) return null
  const props = { command, onClose: close }
  const dialog = ((): ReactNode => {
    switch (kind) {
      case 'levels':
        return <LevelsDialog {...props} histograms={open.histograms} />
      case 'curves':
        return <CurvesDialog {...props} histograms={open.histograms} />
      case 'hueSaturation':
        return <HueSaturationDialog {...props} />
      case 'brightnessContrast':
        return <BrightnessContrastDialog {...props} />
      case 'colorBalance':
        return <ColorBalanceDialog {...props} />
      case 'imageSize':
        return <ImageSizeDialog {...props} info={info} />
      case 'canvasSize':
        return <CanvasSizeDialog {...props} info={info} />
      case 'fill':
        return <FillDialog {...props} info={info} colors={colors} />
      case 'stroke':
        return <StrokeDialog {...props} info={info} colors={colors} />
      case 'layerStyle': {
        const layer = info.layers.find((item) => item.id === info.activeLayer)
        return layer && open.style ? <LayerStyleDialog {...props} layer={layer} start={open.style} /> : null
      }
      case 'preferences':
        return <PreferencesDialog prefs={prefs.value} onSave={prefs.save} onClose={close} />
      case 'place':
        return null
      case 'params':
        return <ParamsDialog {...props} info={info} values={open.values} />
    }
  })()
  // A new dialog (its serial) starts from its own values, never the last one's.
  return <Fragment key={open.serial}>{dialog}</Fragment>
}
