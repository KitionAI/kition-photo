// The editor in each display mode, through the development host: node tests/modes.mjs [chromium|webkit]
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { runModeSmoke } from '@kition/kit-dev-host/mode-smoke'

await runModeSmoke({
  kit: 'kit.photo',
  config: new URL('../vite.config.ts', import.meta.url).pathname,
  browser: process.argv[2] === 'webkit' ? webkit : chromium,
  createServer,
  // The rail's tools are not in the row (D3): the rail always shows them.
  line: { tab: 'home', primary: ['photo-cmd-layer.new.layer', 'photo-home-transform', 'photo-home-adjustments', 'photo-home-filters', 'photo-home-select', 'photo-cmd-edit.fill', 'photo-cmd-image.imageSize'] },
})
