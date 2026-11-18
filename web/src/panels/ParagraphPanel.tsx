import { NoLayerHint } from './CharacterPanel'
import { ParagraphSection } from './TypeSections'
import { useTypeStyle, type TypeTarget } from './useTypeStyle'

/**
 * Window › Paragraph (upstream's `character_panel` with its paragraph tab): the Paragraph section
 * on the active type layer. Without one, upstream's hint shows above it, and only the Type tool's
 * alignment (left, center, right) can change, for new type.
 */
export function ParagraphPanel({ target }: { target: TypeTarget }) {
  const style = useTypeStyle(target)
  return (
    <div className="photo-type-panel" data-testid="photo-paragraph" data-target={style.layer ? 'layer' : 'tool'}>
      <NoLayerHint shown={!style.layer} />
      <ParagraphSection style={style} testId="photo-paragraph" uiId={null} />
    </div>
  )
}
