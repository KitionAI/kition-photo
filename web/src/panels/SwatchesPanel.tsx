import { useState, type MouseEvent } from 'react'
import { TesseraIcons } from '@kition/sdk/editor'
import { IconButton, KitElement, PanelFooter, useKitEditor } from '@kition/sdk/editor'
import type { PhotoColorStore } from '../colors'

const Plus = TesseraIcons.PlusIcon
const Trash2 = TesseraIcons.TrashIcon

/**
 * Upstream's Swatches panel (`panels.rs` `swatches`) on the engine's swatch library
 * (`swatch.list`, Photoshop's groups), shared by every Photo tab and kept between documents
 * (`usePhotoColors`): a click sets the foreground, a right-click the background (`swatch.use`),
 * as upstream's; the footer adds the foreground as a new swatch (`swatch.add`) and deletes the
 * swatch last clicked (`swatch.delete`). None of this is a history step.
 */
export function SwatchesPanel({ colors }: { colors: PhotoColorStore }) {
  const { phrase, t } = useKitEditor()
  const [selected, setSelected] = useState<number | null>(null)
  const list = colors.swatches

  const pick = (index: number, event: MouseEvent, background: boolean) => {
    event.preventDefault()
    setSelected(index)
    void colors.pickSwatch(index, background ? 'background' : 'foreground')
  }
  const remove = async () => {
    if (selected == null) return
    setSelected(null)
    await colors.deleteSwatch(selected)
  }

  let index = 0
  return (
    <div className="photo-swatches" data-testid="photo-swatches">
      <KitElement id="control:swatch.list">
        <div className="photo-swatches-groups">
          {(list?.groups ?? []).map((group) => (
            <section key={group.name} className="photo-swatches-group" aria-label={group.name}>
              {list && list.groups.length > 1 ? <h3 className="photo-swatches-name">{group.name}</h3> : null}
              <div className="photo-swatches-grid">
                {group.swatches.map((swatch) => {
                  const at = index++
                  return (
                    <button
                      key={at}
                      type="button"
                      className="photo-swatches-cell"
                      style={{ background: swatch.hex }}
                      title={swatch.name}
                      aria-label={swatch.name}
                      aria-pressed={selected === at}
                      onClick={(event) => pick(at, event, false)}
                      onContextMenu={(event) => pick(at, event, true)}
                      data-testid={`photo-swatch-${at}`}
                      data-color={swatch.hex}
                    />
                  )
                })}
              </div>
            </section>
          ))}
        </div>
      </KitElement>
      <p className="photo-swatches-hint">{phrase('Click sets foreground · right-click sets background')}</p>
      <PanelFooter
        end={
          <>
            <KitElement id="control:swatch.new">
              <IconButton icon={Plus} label={phrase('Create new swatch')} onClick={() => void colors.addSwatch(colors.foreground)} data-testid="photo-swatches-new" />
            </KitElement>
            <KitElement id="control:swatch.delete">
              <IconButton icon={Trash2} label={t('swatch.delete')} disabled={selected == null} onClick={() => void remove()} data-testid="photo-swatches-delete" />
            </KitElement>
          </>
        }
      />
    </div>
  )
}
