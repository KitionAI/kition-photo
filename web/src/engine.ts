import type { KitModuleManifest } from '@kition/kit-editor'

/** The engine, from this package's own folder. */
export function engineManifest(): KitModuleManifest {
  return {
    kitId: 'kit.photo',
    base: new URL('./engine/', document.baseURI).href,
    module: 'kition_photo_kit',
    documentClass: 'PhotoDocument',
    // The web build embeds no CJK face (build-engine.sh copies this one from kition-fonts).
    fonts: [new URL('./fonts/DroidSansFallbackFull.ttf', document.baseURI).href],
  }
}
