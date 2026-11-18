import { rgbToHex, useEngineQuery } from '@kition/kit-editor'
import { typeLayerOf, type TypeInfo, type TypeLayer } from './typeStyle'

/**
 * A type layer's style as the editor shows it (upstream's `type_tool::styles_at`), shared by the
 * Type options bar, the Properties panel, and the Character and Paragraph panels: the first run's
 * character style (sizes as the layer's transform shows them) and the first paragraph's, the
 * orientation, and the anti-aliasing (see `typeStyle`).
 */

export { shownSize, type TypeLayer } from './typeStyle'
export type Fonts = { families: string[] }
export type Faces = { faces: { style: string }[] }

/** The style of type layer `id` (asked again after every change), or null for none. */
export function useTypeLayer(id: number | undefined): TypeLayer | null {
  const info = useEngineQuery<TypeInfo>('type.info', { layer: id ?? -1 }, id)
  return id == null ? null : typeLayerOf(id, info, rgbToHex)
}

/** The families and the styles of `family` the engine has, each list keeping a name it lacks (so a choice still shows it). */
export function useFontChoices(family: string, style: string) {
  const families = useEngineQuery<Fonts>('type.fonts')?.families ?? []
  const styles = useEngineQuery<Faces>('type.fonts', { family })?.faces.map((face) => face.style) ?? []
  return {
    fonts: (families.includes(family) ? families : [family, ...families]).map((name) => [name, name] as const),
    styles: (styles.includes(style) ? styles : [style, ...styles]).map((name) => [name, name] as const),
  }
}

/** A new family's style: the current one when the family has it, else its first (upstream's `apply` on a font change). */
export async function styleForFamily(query: <R>(id: string, params?: Record<string, unknown>) => Promise<R | null>, family: string, style: string) {
  const result = await query<Faces>('type.fonts', { family })
  // A disposed/replaced engine answers null through query(). It is not a family with no
  // styles: choosing a fallback then could apply an old request to the replacement document.
  if (!result) return null
  const faces = result.faces.map((face) => face.style)
  return faces.includes(style) ? style : (faces[0] ?? 'Regular')
}
