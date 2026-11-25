import { useState, type ReactNode } from 'react'
import { ColorWell } from '../color/ColorWell'
import { TesseraIcons } from '@kition/sdk/editor'
import { KitElement, useReadOnly } from '@kition/sdk/editor'

const ChevronDown = TesseraIcons.ChevronDownIcon
const ChevronRight = TesseraIcons.ChevronRightIcon

/**
 * The Properties panel's shared parts (upstream `props_layout.rs`): collapsible sections, rows,
 * and colour swatches. A section is a Kit UI element (`control:properties.<section>`), so a host
 * hides or disables a whole section by its id.
 */

/** Panel choices that outlive the panel (upstream keeps them in egui's memory for the session): open sections, linked fields. */
const remembered = new Map<string, unknown>()

/** A value the panel keeps for the session, across documents and remounts. */
export function useRemembered<T>(key: string, initial: T) {
  const [value, setValue] = useState<T>(() => (remembered.has(key) ? (remembered.get(key) as T) : initial))
  const set = (next: T) => {
    remembered.set(key, next)
    setValue(next)
  }
  return [value, set] as const
}

/**
 * A collapsible section: a chevron and title row, then its fields while open (upstream
 * `section`, whose open state is shared by every place that draws a section of the same `id`, as
 * the Properties panel's and the Character panel's Character sections are). It is the Kit UI
 * element `uiId` (`control:properties.<id>` by default; none with null, inside a panel that is
 * the element), and its test ids start with `testId` (`photo-props-<id>` by default).
 */
export function Section({ id, title, uiId, testId = `photo-props-${id}`, children }: { id: string; title: string; uiId?: string | null; testId?: string; children: ReactNode }) {
  const [open, setOpen] = useRemembered(`section:${id}`, true)
  const section = (
    <section className="photo-props-section" data-testid={testId}>
      <button type="button" className="photo-props-section-head" aria-expanded={open} onClick={() => setOpen(!open)} data-testid={`${testId}-head`}>
        {open ? <ChevronDown size={12} aria-hidden="true" /> : <ChevronRight size={12} aria-hidden="true" />}
        <span>{title}</span>
      </button>
      {open ? (
        <div className="photo-props-section-body">
          <EditingControls>{children}</EditingControls>
        </div>
      ) : null}
    </section>
  )
  return uiId === null ? section : <KitElement id={uiId ?? `control:properties.${id}`}>{section}</KitElement>
}

/** A row of fields. */
export function Row({ children }: { children: ReactNode }) {
  return <div className="photo-props-row">{children}</div>
}

/**
 * A colour swatch with the browser's picker: `onChange` gets each colour of a picker session with
 * `final: false`, and the session's end (the picker closing) once with `final: true`, so the
 * caller keeps a session as one history step.
 */
/**
 * A panel's editing controls, all disabled for a viewer's read-only document (the engine refuses
 * their edits anyway); `display: contents`, so it adds no box. Sections hold theirs in one.
 */
export function EditingControls({ children }: { children: ReactNode }) {
  return (
    <fieldset className="photo-editing-controls" disabled={useReadOnly()}>
      {children}
    </fieldset>
  )
}

export function ColorSwatch({ label, value, disabled, onChange, testId }: { label: string; value: string; disabled?: boolean; onChange: (value: string, final: boolean) => void; testId?: string }) {
  // The chip opens the Color Picker (plan 2026-11-24, stage 5): its OK is one final change.
  return <ColorWell value={value} label={label} disabled={disabled} onChange={onChange} testId={testId} />
}
