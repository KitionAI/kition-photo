import { rgbToHex, useEngineQuery, useKitEditor } from '@kition/sdk/editor'
import { typeLayerOf, type TypeInfo, type TypeLayer } from './typeStyle'
import { fontList } from '../fonts/catalog'
import { useServedFonts } from '../fonts/served'

/**
 * A type layer's style as the editor shows it (upstream's `type_tool::styles_at`), shared by the
 * Type options bar, the Properties panel, and the Character and Paragraph panels: the first run's
 * character style (sizes as the layer's transform shows them) and the first paragraph's, the
 * orientation, and the anti-aliasing (see `typeStyle`).
 */

export { shownSize, type TypeLayer } from './typeStyle'
export type Fonts = { families: string[] }
export type Faces = { faces: { style: string }[] }

/** The style of type layer `id` (asked again after every change), or null for none; `at`: the character whose style shows (see `typeLayerOf`). */
export function useTypeLayer(id: number | undefined, at?: number): TypeLayer | null {
  const info = useEngineQuery<TypeInfo>('text.info', { layer: id ?? -1 }, id)
  return id == null ? null : typeLayerOf(id, info, rgbToHex, at)
}

/**
 * The families the engine has and the editor serves (plan 2026-11-24, stage 9: a served family loads
 * when chosen), by name, and the styles of `family`, each list keeping a name it lacks (so a choice
 * still shows it): a family nothing has is marked as substituted, one loading says so.
 */
export function useFontChoices(family: string, style: string) {
  const { t } = useKitEditor()
  const families = useEngineQuery<Fonts>('text.fonts')?.families ?? []
  const { catalog, loading } = useServedFonts()
  const styles = useEngineQuery<Faces>('text.fonts', { family })?.faces.map((face) => face.style) ?? []
  const label = (name: string, state: string) => (state === 'missing' ? t('font.substituted', { family: name }) : state === 'loading' ? t('font.loadingName', { family: name }) : name)
  return {
    loading: family && loading.has(family) ? family : ([...loading][0] ?? null),
    fonts: fontList(families, catalog, loading, family).map(({ family: name, state }) => [name, label(name, state)] as const),
    styles: (styles.includes(style) ? styles : [style, ...styles]).map((name) => [name, name] as const),
  }
}

/** A new family's style: the current one when the family has it, else its first (upstream's `apply` on a font change). */
export async function styleForFamily(query: <R>(id: string, params?: Record<string, unknown>) => Promise<R | null>, family: string, style: string) {
  const result = await query<Faces>('text.fonts', { family })
  // A disposed/replaced engine answers null through query(). It is not a family with no
  // styles: choosing a fallback then could apply an old request to the replacement document.
  if (!result) return null
  const faces = result.faces.map((face) => face.style)
  return faces.includes(style) ? style : (faces[0] ?? 'Regular')
}
