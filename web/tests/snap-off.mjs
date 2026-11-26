// Snapping off for scripted gestures (plan 2026-11-24, stage 9). View › Snap To › Layers and
// Document Bounds and Smart Guides are on by default, as Photoshop's; a test whose drags were
// written against the editor without them turns them off before the editor loads, in the kit
// store where View › Snap To and View › Show keep them (`kit:kit.photo:view`, `src/guides/view.ts`).
// A test that changes them itself keeps its own choice: the value is written only when none is stored.

/** The View settings a scripted gesture test starts from: snapping to guides and the grid only, no Smart Guides. */
export const SNAP_OFF_VIEW = { smartGuides: false, snapTo: { guides: true, grid: true, layers: false, documentBounds: false } }

/** Turns snapping to layers and the document's bounds and Smart Guides off for every page `target` (a page or a context) loads. */
export async function snapOff(target) {
  await target.addInitScript((view) => {
    try {
      if (localStorage.getItem('kit:kit.photo:view') == null) localStorage.setItem('kit:kit.photo:view', JSON.stringify(view))
    } catch {
      // A page without storage (about:blank) has nothing to set.
    }
  }, SNAP_OFF_VIEW)
}
