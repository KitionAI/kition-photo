# Changelog

Changes to Kition Photo, newest first. A version is the kit package version in `kit.json`;
each package is published with a `package-v<version>` tag.

## Unreleased

- A `.kphoto` stores every tile once: tiles (`tiles/<sha256>`, their encoded bytes) and blobs
  (`blobs/<sha256>.zst`) are named by the SHA-256 of their bytes, as the replica names its blobs,
  so the manifest and `kdm.bin` share them (no second copy under `blobs/kdm/`). The file holds no
  thumbnail or preview, and nobody's view: the selection, the quick mask, expanded groups, open
  notes, the timeline's playhead and the last applied layer comp, the document handle and the title
  are left out, so two people who open a file and save it unedited write the same bytes. The
  editor keeps each person's view with the host (capability `view-state`) and puts it back when
  the image opens.
- A `.kphoto` holds the image's content, not its history (kition-sdk data model 7.3, rule M16b):
  `kdm.bin` is the replica's content snapshot (of the history, only the seal; no deleted layers,
  no overwritten values, no authors) and the replica blobs are only those its state references.
  Hosts seal the document before saving (`prepareSave`), and a save of edits nobody sealed is
  refused; the undo history stays in the session and the host's store. A file opens on a stored replica by its changes or, when saved elsewhere, by
  its state. kition-sdk 82a3224.
- The replica's schema is frozen at version 1: notes, layer comps, saved paths, patterns, layer
  styles and video frames are object lists, a type layer's text is a text with its style runs as
  marks, the guides a map. A `.kphoto` saved by an earlier build opens from its manifest as a new
  document.

## 0.1.3

- The same kit as 0.1.2, built on kition-sdk 6672663 (the replica's undo passes over an undo group whose changes came to nothing): the 0.1.2 package was built on kition-sdk 6b4d471.

## 0.1.2

- A host tells a picture file's document id (`document.identity`) and makes a copied picture a
  new document (`document.fork`), Kit Process Protocol section 5.6 (kition-sdk 6672663). A
  `.kphoto` embeds its replica, so two copies of the same bundle were one document until now; a
  fork keeps the layers, their ids and pixels, and carries a new id and none of the source's
  history. A PSD or PSB embeds no replica, so it carries no id and forks as the same bytes.
- The headless program speaks Kit Process Protocol version 1 (`headless.protocol` is
  `kition-kit/1`), as hosts on kition-sdk from bb1b07b require.

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
