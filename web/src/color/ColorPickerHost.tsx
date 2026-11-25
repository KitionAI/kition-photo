import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from 'react'
import { ColorPickerDialog } from './ColorPickerDialog'

/**
 * Photoshop's Color Picker as the editor's one way to choose a colour (plan 2026-11-24, stage 5):
 * the tool rail's chips, Fill, Stroke, the type colour, shape fills and strokes, layer-style
 * colours and gradient stops all open it through `useColorPicker`. One picker is open at a time,
 * over whatever dialog asked for it; while it is open a press on the canvas samples the picture
 * there (`sample`).
 */
export type PickRequest = {
  /** The dialog's title (the colour being chosen: "Foreground Color", "Stroke Color"). */
  title: string
  /** The colour it starts from (`#rrggbb`), shown as "current". */
  value: string
  /** Each colour as the person moves through them (the foreground shows them at once); the start colour again on Cancel. */
  onLive?: (hex: string) => void
}

/** Opens the picker: the chosen colour on OK, null on Cancel. */
export type PickColor = (request: PickRequest) => Promise<string | null>

/** The picture's colour under a page point, or null when the point is not over the canvas. */
export type SampleAt = (clientX: number, clientY: number) => Promise<string | null> | null

type Open = PickRequest & { serial: number; resolve: (hex: string | null) => void }
type Host = { pick: PickColor; open: Open | null; close: (hex: string | null) => void; sample: SampleAt }

const HostContext = createContext<Host | null>(null)

/** The picker of the editor around (null outside one, where a colour well falls back to the browser's). */
export function useColorPicker(): PickColor | null {
  return useContext(HostContext)?.pick ?? null
}

/** Holds the open picker; `ColorPickerOutlet`, inside the editor's frame, draws it. */
export function ColorPickerProvider({ sample, children }: { sample: SampleAt; children: ReactNode }) {
  const [open, setOpen] = useState<Open | null>(null)
  const openNow = useRef<Open | null>(null)
  const serial = useRef(0)
  const sampleNow = useRef(sample)
  sampleNow.current = sample
  const pick = useCallback<PickColor>(
    (request) =>
      new Promise((resolve) => {
        // A second request replaces the first, which answers Cancel.
        openNow.current?.resolve(null)
        serial.current += 1
        const next = { ...request, serial: serial.current, resolve }
        openNow.current = next
        setOpen(next)
      }),
    [],
  )
  const close = useCallback((hex: string | null) => {
    const current = openNow.current
    openNow.current = null
    setOpen(null)
    current?.resolve(hex)
  }, [])
  const stableSample = useCallback<SampleAt>((x, y) => sampleNow.current(x, y), [])
  const host = useMemo<Host>(() => ({ pick, open, close, sample: stableSample }), [pick, open, close, stableSample])
  return <HostContext.Provider value={host}>{children}</HostContext.Provider>
}

/** Draws the open picker where the editor's dialogs are drawn. */
export function ColorPickerOutlet() {
  const host = useContext(HostContext)
  const open = host?.open
  if (!host || !open) return null
  return <ColorPickerDialog key={open.serial} title={open.title} value={open.value} onLive={open.onLive} sample={host.sample} onDone={host.close} />
}
