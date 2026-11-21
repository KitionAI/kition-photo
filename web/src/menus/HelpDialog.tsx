import { Button, Dialog, useKitEditor, useUi } from '@kition/sdk/editor'
import { PHOTO_TOOLS } from '../tools/catalog'
import { PHOTO_MENUS } from './catalog'
import { isHiddenOnWeb } from './hidden'

export type HelpTopic = 'about' | 'shortcuts'

/** Photo's tools by key, in the rail's order: `[key, upstream's tool labels]`, for the tools `usable` lets a key choose. */
function toolKeys(usable: (id: string) => boolean) {
  const byKey = new Map<string, string[]>()
  for (const tool of PHOTO_TOOLS) if (tool.key && usable(tool.id)) byKey.set(tool.key, [...(byKey.get(tool.key) ?? []), tool.label])
  return [...byKey]
}

/** Help's dialogs: About (name and version) and Keyboard Shortcuts (Photo's tool keys, and the shortcuts of the menu rows it runs). */
export function HelpDialog({ topic, version, onClose }: { topic: HelpTopic; version: string; onClose: () => void }) {
  const { t, phrase, byId, testId } = useKitEditor()
  const ui = useUi()
  const close = (
    <Button variant="primary" onClick={onClose} data-testid={`${testId}-help-close`}>
      {t('close')}
    </Button>
  )
  if (topic === 'about') {
    return (
      <Dialog title={phrase('About Kition Photo')} actions={close} onClose={onClose}>
        <div data-testid={`${testId}-about`}>
          <p>
            <strong>Kition Photo</strong>
          </p>
          <p>{t('help.version', { version })}</p>
        </div>
      </Dialog>
    )
  }
  // Rows Photo runs (the engine's and Kition's own), once each: a row the engine lacks does nothing.
  const seen = new Set<string>()
  const rows = PHOTO_MENUS.filter((row) => (row.command.startsWith('kit:') || byId.has(row.command)) && !isHiddenOnWeb(row))
    .map((row) => ({ row, shortcut: row.shortcut ?? byId.get(row.command)?.shortcut ?? null }))
    .filter((item) => item.shortcut && !seen.has(item.row.command) && seen.add(item.row.command))
  return (
    <Dialog title={phrase('Keyboard Shortcuts')} actions={close} onClose={onClose}>
      <div className="photo-shortcuts" data-testid={`${testId}-shortcuts`}>
        <h3>{t('help.tools')}</h3>
        <dl>
          {toolKeys((id) => !ui.state(`tool:${id}`).denied).map(([key, labels]) => (
            <div key={key}>
              <dt>{labels.map((label) => phrase(label)).join(', ')}</dt>
              <dd>
                <kbd>{key}</kbd>
              </dd>
            </div>
          ))}
        </dl>
        <h3>{t('help.menus')}</h3>
        <dl>
          {rows.map(({ row, shortcut }) => (
            <div key={`${row.path.join('/')}/${row.command}`}>
              <dt>{[...row.path, row.label].map(phrase).join(' › ')}</dt>
              <dd>
                <kbd>{shortcut}</kbd>
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </Dialog>
  )
}
