# Changelog

Changes to Kition Photo, newest first. A version is the kit package version in `kit.json`;
each package is published with a `package-v<version>` tag.

## 0.1.1

- `kit.json` declares the engine's editor state as `engine.editorState`, the same list as the
  engine's `EDITOR_STATE`, so a host that does not run the engine reads `inspect()` without it.
- The package is built and published on GitHub with Kition's `kit-package.yml`.
- The README gives the kit to an agent with `kition`, no build: `npm install -g @kition/cli` and
  `kition agent add`; it links KHP at `kitharness/kitharness`.
- Tests only: the Photoshop reference corpus is private to the maintainers, and the browser
  scenarios are steadier on CI.

## 0.1.0

First public release of the source, under the Apache License 2.0. It contains:

- The Rust engine: layered documents with groups, blend modes, layer masks, clipping masks,
  adjustment and fill layers, layer styles, type (CJK included) and object layers; painting,
  selections, paths and shapes, transforms and filters; ICC colour management; and over 850
  commands, each claimed by a feature in `features.yaml`.
- Files: PSD and PSB opened and edited with their layers kept, the native `.kphoto`, and export
  to PNG, JPEG, TIFF and WebP.
- The kit engine (`kition-photo-kit`), native and WebAssembly: the document is a KDM replica, and
  a `.kphoto` keeps the replica's identity and undo history across saves. A check catalogue
  reports empty, hidden or off-canvas layers, empty type, missing object-layer sources and
  substituted fonts.
- The headless kit (`kition-photo`) for agents over the Kit Protocol: it inspects the layer stack,
  runs the editor's own commands on a layer by name or id, checks and exports the image, and works
  in drafts. A write names the revision it read and is refused when someone else changed what it
  rests on. It reads and writes only inside its workspace.
- The kit's editor (`web/`): the menu bar, options bar, toolbox, canvas tools, and the Layers,
  Properties, Adjustments, Channels, Paths, Character, Paragraph, Color, Swatches, History,
  Navigator, Info and Histogram panels, in kit-editor's frame.
- NOTICE and docs/attribution.md list the upstream work, the translations, the fonts and every
  asset, and Loro (MIT), bundled by the editor and linked by the engine.
