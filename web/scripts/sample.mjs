// Makes public/sample/Sample.psd with the engine itself: a gradient sky, a sun, a title, two layers.
import fs from 'node:fs/promises'
import { fileURLToPath, pathToFileURL } from 'node:url'

const web = fileURLToPath(new URL('..', import.meta.url))
const engine = await import(pathToFileURL(`${web}public/engine/kition_photo_kit.js`).href)
await engine.default({ module_or_path: await fs.readFile(`${web}public/engine/kition_photo_kit_bg.wasm`) })
const doc = engine.PhotoDocument.create('Sample.psd', 1200, 800)
const run = (command, params = {}) => doc.execute(command, JSON.stringify(params))
run('paint.gradient', { from: [0, 0], to: [0, 800], colors: ['#1d2b64', '#f8cdda'] })
run('layer.new.layer')
run('shape.add', { kind: 'ellipse', rect: [820, 160, 200, 200], fill: '#ffd166' })
run('text.add', { x: 80, y: 680, text: 'Northern Lights', size: 96, color: '#ffffff' })
await fs.writeFile(`${web}public/sample/Sample.psd`, doc.save())
console.log('wrote public/sample/Sample.psd')
