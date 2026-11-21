import { useKitEditor } from '@kition/sdk/editor'
import { CharacterSection } from './TypeSections'
import { useTypeStyle, type TypeTarget } from './useTypeStyle'

/**
 * Window › Character (upstream's `character_panel`): the Character section on the active type
 * layer. Without one, upstream's hint shows above it, and only the Type tool's own options (font,
 * style, size) and the foreground colour can change, for new type.
 */
export function CharacterPanel({ target }: { target: TypeTarget }) {
  const style = useTypeStyle(target)
  return (
    <div className="photo-type-panel" data-testid="photo-character" data-target={style.layer ? 'layer' : 'tool'}>
      <NoLayerHint shown={!style.layer} />
      <CharacterSection style={style} testId="photo-character" uiId={null} />
    </div>
  )
}

/** Upstream's hint while no type layer is active. */
export function NoLayerHint({ shown }: { shown: boolean }) {
  const { phrase } = useKitEditor()
  return shown ? <p className="photo-type-hint">{phrase('Select a type layer to edit its character and paragraph settings.')}</p> : null
}
