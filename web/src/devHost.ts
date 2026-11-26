/** The development host (dev.html): opens `public/sample/Sample.psd`, or a file chosen in the page. */
import { startDevHost } from '@kition/sdk/dev-host'

// `host/pick` answers with workspace images (Place) and look files (Color Lookup's Load).
startDevHost({ sample: './sample/Sample.psd', viewState: true, pickable: /\.(png|jpe?g|gif|svg|webp|cube|3dl|look|csp)$/i })

type DevHost = { files: Map<string, { bytes: ArrayBuffer; version: number }>; open: (path: string, bytes: ArrayBuffer) => Promise<void> }

/**
 * File › New… (plan 2026-11-24, stage 8): the editor writes the new document beside the open one
 * through the host, as an export, and in Kition the person opens it from the workspace. This page
 * has no workspace to open it from, so a new PSD the editor writes opens here at once, and a
 * person can start from a blank canvas standalone. Files the page opens itself (the file chooser,
 * `devHost.open`) are opening already and are left alone.
 */
const host = (window as unknown as { devHost?: DevHost }).devHost
if (host) {
  const files = host.files
  const set = files.set.bind(files)
  files.set = (path, entry) => {
    const created = !files.has(path)
    set(path, entry)
    if (created && /\.(psd|psb)$/i.test(path)) {
      setTimeout(() => {
        const status = document.getElementById('status')?.textContent ?? ''
        if (status.includes(`opening ${path}`) || status.includes(`${path} open in`)) return
        void host.open(path, entry.bytes)
      }, 0)
    }
    return files
  }
}
