import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from 'react'
import { GradientEditorDialog } from './GradientEditorDialog'
import type { GradientDraft } from './gradient'

/**
 * Photoshop's Gradient Editor as the editor's one way to edit a gradient (plan 2026-11-24, stage
 * 5): the Gradient tool's swatch, a gradient overlay's and a gradient fill layer's gradient open it
 * through `useGradientEditor`. One is open at a time, over the dialog that asked for it.
 */
export type GradientRequest = {
  title: string
  value: GradientDraft
  /** Whether the target keeps midpoints (a gradient fill layer); elsewhere the editor hides them. */
  midpoints: boolean
  /** Each gradient as the person edits it; the starting one again on Cancel. */
  onLive?: (draft: GradientDraft) => void
}

export type EditGradient = (request: GradientRequest) => Promise<GradientDraft | null>

type Open = GradientRequest & { serial: number; resolve: (draft: GradientDraft | null) => void }
type Host = { edit: EditGradient; open: Open | null; close: (draft: GradientDraft | null) => void; colors: { foreground: string; background: string } }

const HostContext = createContext<Host | null>(null)

/** The gradient editor of the editor around, or null outside one. */
export function useGradientEditor(): EditGradient | null {
  return useContext(HostContext)?.edit ?? null
}

export function GradientEditorProvider({ colors, children }: { colors: { foreground: string; background: string }; children: ReactNode }) {
  const [open, setOpen] = useState<Open | null>(null)
  const openNow = useRef<Open | null>(null)
  const serial = useRef(0)
  const edit = useCallback<EditGradient>(
    (request) =>
      new Promise((resolve) => {
        openNow.current?.resolve(null)
        serial.current += 1
        const next = { ...request, serial: serial.current, resolve }
        openNow.current = next
        setOpen(next)
      }),
    [],
  )
  const close = useCallback((draft: GradientDraft | null) => {
    const current = openNow.current
    openNow.current = null
    setOpen(null)
    current?.resolve(draft)
  }, [])
  const host = useMemo<Host>(() => ({ edit, open, close, colors: { foreground: colors.foreground, background: colors.background } }), [edit, open, close, colors.foreground, colors.background])
  return <HostContext.Provider value={host}>{children}</HostContext.Provider>
}

/** Draws the open gradient editor where the editor's dialogs are drawn (before the Color Picker, which opens over it). */
export function GradientEditorOutlet() {
  const host = useContext(HostContext)
  const open = host?.open
  if (!host || !open) return null
  return <GradientEditorDialog key={open.serial} title={open.title} value={open.value} midpoints={open.midpoints} onLive={open.onLive} foreground={host.colors.foreground} background={host.colors.background} onDone={host.close} />
}
