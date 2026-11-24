import { createRoot } from 'react-dom/client'
import { connectHost, createKitStrings, loadPhrases, withPhrases } from '@kition/sdk/editor'
import '@kition/sdk/editor/styles.css'
import './app.css'
import './commandIcons'
import { App } from './App'
import { CATALOGS, PHRASES } from './strings'
import manifest from '../../kit.json'
import { PHOTO_UI } from './photoUi'

const root = createRoot(document.getElementById('root')!)

// Opened outside a host, the page says where it belongs instead of waiting forever.
const alone = window.setTimeout(() => {
  const t = createKitStrings(CATALOGS, navigator.language)
  root.render(<div className="kui-alone">{t('noHost')}</div>)
}, 3000)

// Upstream's translations of its English labels and command titles (Photo's own for labels upstream lacks).
const UPSTREAM = import.meta.glob('./i18n/upstream/*.tsv', { query: '?raw', import: 'default' })

// `replica`: in a host that keeps replicas, a PSD keeps its KDM replica (identity, collaboration)
// across saves; elsewhere the editor opens and saves files as before. `view-state`: the host keeps
// each person's view (the selection, collapsed groups, …) apart from the file; without it the view
// is not kept between sessions.
void connectHost({ id: 'kit.photo', version: manifest.version, capabilities: ['ai', 'replica', 'view-state'], brand: manifest.ui.brand, display: manifest.ui.display, ui: PHOTO_UI }).then(async (host) => {
  window.clearTimeout(alone)
  const phrase = withPhrases(await loadPhrases(UPSTREAM, host.init.locale), PHRASES, host.init.locale)
  // No StrictMode: its doubled effects would open the document twice, in two engine workers.
  root.render(<App host={host} phrase={phrase} />)
})
