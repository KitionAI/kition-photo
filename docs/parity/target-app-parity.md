# Target-app parity

> **Last updated:** 2026-11-17 · **Source:** `features.yaml` (updated 2026-11-17) and the engine's command list

Each feature in `features.yaml` records its status against what the feature promises and, where it
has one, the target application it is compared with (Photoshop 2026).
The difference column is the feature's recorded gap.

| Feature | Priority | Status | Target | Difference |
| --- | --- | --- | --- | --- |
| `photo.agent.act` An agent changes layer properties | P0 | has | — | — |
| `photo.agent.context` An agent's call says what it acts on: editor-state hooks, on, layer / region / colors; observed reads supplemented by implementation checks | P0 | has | — | — |
| `photo.agent.ids` One id space: layers by the ids inspect lists in queries, batches, receipts and conflicts | P0 | has | — | — |
| `photo.agent.observe` An agent reads the layer stack and acts on what it finds | P0 | has | — | — |
| `photo.agent.premises` Guard premises for properties, pixel/filter inputs, the layer a command acts on, selected layers, the canvas, mode and stored members, stored presets, channels, linked moves, names, stack indices and points | P0 | has | — | — |
| `photo.agent.recover` An agent fixes what the check reports | P0 | has | — | — |
| `photo.agent.saved` In memory = saved: what the kit holds after an agent's command is what its saved file opens as | P0 | has | — | — |
| `photo.layer.adjustment-basics` Levels and Hue/Saturation adjustment layers survive saving to PSD and reopening | P0 | has | Photoshop 2026 | — |
| `photo.layer.basics` Layer opacity, fill, blend mode, visibility, name, label colour, duplicates and groups survive saving to PSD and reopening | P0 | has | Photoshop 2026 | PSD stores opacity and fill in one byte each; a reopened file numbers its layers afresh. |
| `photo.layer.mask-basics` A layer mask from a selection and a clipping mask survive saving to PSD and reopening | P0 | has | Photoshop 2026 | — |
| `photo.layer.new-layer` A new layer survives saving to PSD and reopening | P0 | has | Photoshop 2026 | — |
| `photo.layer.object-layer-basics` An object layer and its contents survive saving to PSD and reopening | P0 | has | Photoshop 2026 | A reopened placed layer is linked to the uuid of the file the PSD carries for it, embedded or not; Photoshop shows it as embedded. |
| `photo.layer.solid-fill` A solid colour fill layer survives saving to PSD and reopening | P0 | has | Photoshop 2026 | — |
| `photo.layer.style-basics` Drop Shadow and Stroke layer styles survive saving to PSD and reopening | P0 | has | Photoshop 2026 | — |
| `photo.paint.stroke` A brush stroke survives saving to PSD and reopening | P0 | has | Photoshop 2026 | — |
| `photo.type.point-text` Point text with CJK and symbols survives saving to PSD and reopening | P0 | has | Photoshop 2026 | EngineData stores numbers with five decimals; a run without a PostScript name gets the one its family and weight give. |
| `photo.agent.draft` An agent works in a draft over a conversation | P1 | has | — | — |
| `photo.agent.preview` A preview by explicit view (file.render) and measurements beside it (image.measure) | P1 | has | — | — |
| `photo.channel.edit` Channels, spot channels and channel targets | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.color.management` Colour settings, profiles, image modes and proofing | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.color.swatches` Swatches and patterns | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.document.text` The image's words as plain text for search (document.text): visible type layers top to bottom, a part per layer | P1 | has | — | — |
| `photo.edit.define` Define brush presets, custom shapes and patterns | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.edit.fill-stroke` Fill, stroke, fade, Smart Fill and Smart Scale | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.edit.history` Undo, redo, history states, purge, cut, copy, paste and clear | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.edit.transform` Free Transform, warps, Pin Warp and Perspective Warp, auto-align and auto-blend | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.editor.display-modes` The editor in each display mode: Classic gives way before stepping down, Single line does not repeat the rail's tools, and Escape drops a drag or a crop first in Focus | P1 | has | — | — |
| `photo.file.automate` Automate, scripts, actions and background jobs | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.file.documents` Create, open as, close, revert and save a copy of documents | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.file.export` Export layers, artboards, comps, paths and images; Quick Export and image assets | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.file.place` Place embedded and linked files; import notes, video frames and data sets | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.file.print` Print, print one copy and file info | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.blur` Blur filters and the Blur Gallery | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.distort` Distort filters, Liquify, Vanishing Point, Adaptive Wide Angle and Lens Correction | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.editable` Editable filters and Last Filter | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.gallery` Filter Gallery artistic, brush stroke, sketch and texture filters | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.noise-sharpen` Noise and sharpen filters | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.raw-develop` Raw Develop filter | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.filter.stylize` Stylize, pixelate, render, video and other filters | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.image.adjustments` Image adjustments and auto tone, contrast and colour | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.image.analysis` Measurement, count and the measurement log | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.image.calculations` Apply Image, Calculations and Trap | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.image.size` Image and canvas size, crop, trim, rotate, duplicate and reveal all | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.image.variables` Variables and data sets | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.adjustment` Adjustment and fill layers | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.arrange` Align, distribute and arrange layers | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.artboards` Artboards and frames | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.comps` Layer comps | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.manage` Group, duplicate, merge, link, lock, select, rename and delete layers | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.masks` Layer masks, vector masks and clipping masks | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.object-layers` Object layers and stack modes | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.rasterize` Rasterize, matting and combine shapes | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.styles` Layer styles and blending options | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.layer.video` Video layers and the timeline | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.paint.gradients` Gradient fills and stops | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.paint.tools` Painting, retouching and clone tools, brushes and tool colours | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.path.shapes` Paths, shapes and clipping paths | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.select.make` Make, modify, save and load selections | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.type.features` OpenType features, anti-aliasing and orientation | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.type.styles` Character and paragraph styles | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.type.text` Create and edit point and paragraph type; warp, convert, fonts, spelling and find and replace | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.view.guides` Guides, guide layouts and slices | P1 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.app.preferences` Preferences, keyboard shortcuts, menus, toolbar, preset migration and plug-ins | P2 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.assets.presets` Brush, gradient, pattern, style, shape, tool and document presets | P2 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |
| `photo.notes.annotate` Notes | P2 | partial | Photoshop 2026 | The engine has these commands; no case yet shows the area working end to end in the editor and surviving save and reopen. |

| Priority | has | partial | missing |
| --- | ---: | ---: | ---: |
| P0 | 16 | 0 | 0 |
| P1 | 4 | 42 | 0 |
| P2 | 0 | 3 | 0 |
