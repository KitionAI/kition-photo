import { createContext, useContext } from 'react'
import { createLive, useLive, type Live } from '../panels/live'
import type { TypeTarget } from './session'

/**
 * Type being edited on the canvas, as the rest of the editor reaches it (plan 2026-11-24,
 * stage 4): the type style controls (the options bar, Properties, Character, Paragraph) act on
 * the session's selected range through `target`, and the Layers panel's double click on a type
 * thumbnail starts editing with `editLayer`.
 */
export type TypeEditing = {
  target: Live<TypeTarget | null>
  /** Edits type layer `layer` with all its text selected (the Type tool chosen). */
  editLayer: (layer: number) => void
}

export const TypeEditingContext = createContext<TypeEditing | null>(null)

const NONE = createLive<TypeTarget | null>(null)

/** The type being edited now (null: none). */
export function useTypeEditingTarget(): TypeTarget | null {
  return useLive(useContext(TypeEditingContext)?.target ?? NONE)
}

export function useTypeEditing(): TypeEditing | null {
  return useContext(TypeEditingContext)
}
