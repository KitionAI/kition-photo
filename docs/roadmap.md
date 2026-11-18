# Roadmap

> **Last updated:** 2026-11-17 · **Source:** `features.yaml` (updated 2026-11-17) and the engine's command list

Open work on Kition Photo, in order. Each item names where its detail lives.

1. **Prove the P1 features end to end:** `photo.channel.edit`, `photo.color.management`, `photo.color.swatches`, `photo.edit.define`, `photo.edit.fill-stroke`, `photo.edit.history`, `photo.edit.transform`, `photo.file.automate`, `photo.file.documents`, `photo.file.export`, `photo.file.place`, `photo.file.print`, `photo.filter.blur`, `photo.filter.distort`, `photo.filter.editable`, `photo.filter.gallery`, `photo.filter.noise-sharpen`, `photo.filter.raw-develop`, `photo.filter.stylize`, `photo.image.adjustments`, `photo.image.analysis`, `photo.image.calculations`, `photo.image.size`, `photo.image.variables`, `photo.layer.adjustment`, `photo.layer.arrange`, `photo.layer.artboards`, `photo.layer.comps`, `photo.layer.manage`, `photo.layer.masks`, `photo.layer.object-layers`, `photo.layer.rasterize`, `photo.layer.styles`, `photo.layer.video`, `photo.paint.gradients`, `photo.paint.tools`, `photo.path.shapes`, `photo.select.make`, `photo.type.features`, `photo.type.styles`, `photo.type.text`, `photo.view.guides`: each needs a case that shows it working in the editor and surviving save and reopen before it is `has`. Detail in [gaps.md](gaps.md).
2. **Then the P2 features:** `photo.app.preferences`, `photo.assets.presets`, `photo.notes.annotate`.
3. **Keep the catalogue complete:** every new engine command joins a feature in `features.yaml` with a case.
