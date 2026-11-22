# Kition Photo Architecture

> **Last reviewed:** 2026-11-17 · **Last updated:** 2026-11-17 · **Target:** Photoshop 2026

This document describes the kit as built (engine, `bindings/`, React editor in `web/`); sections 5, 6, 8 and 9 are design sketches.

This plan sets out:
- the Cargo workspace,
- the rules that keep the engine independent of the editor,
- the interface the editor and the headless program use,
- how the PSD, compositing, history and automation parts fit together.

---

## 1. Goals and principles

**Goals**
- A fully open-source layered image editor, roughly matching Photoshop's feature surface over time. It ships as Kition's `kit.photo` kit: the engine, a React editor (`web/`) that a host shows in an iframe, and a headless program (`bindings/headless`) that agents drive.
- **The engine does not depend on the editor.** The editor reaches it only through `bindings/wasm`, and the headless program through `automation`.
- **The core compiles to `wasm32`**: that is how the editor runs it.
- PSD read and write lives in its **own crate that can be published separately.**

**Principles**

1. **Engine-first, headless-first.** Every feature is reachable without the editor: from tests and from the headless program. The editor is one client of the engine.
2. **Everything is a command.** Each user-visible action has a stable command id (such as `filter.blur.gaussianBlur`) with JSON parameters. The editor's menus, the headless program and its automation all dispatch the same commands.
3. **Data describes the UI; toolkits draw it.** Tools produce *overlay primitives* (lines, handles, marching-ants paths) as data. Filter and adjustment dialogs are generated from *parameter schemas*. A new toolkit implements one renderer for each, not about 150 bespoke dialogs.
4. **One algorithm, one implementation.** Every algorithm has one CPU implementation (Rust + rayon, deterministic). There is no GPU path, and we never keep hand-synced copies of an algorithm in several languages.
5. **Immutable snapshots, copy-on-write tiles.** Pixel data is stored in `Arc`-shared 256² tiles. Taking a document snapshot is O(layers), which makes undo, background jobs, autosave and UI reads cheap and lock-free.
6. **Determinism.** The same input gives the same output across thread counts, and between preview and export. Noise is hashed from document coordinates.
7. **Pure Rust by default.** C/C++ dependencies are allowed only behind Cargo features, in I/O-edge crates (for example an optional LibRaw).
8. **Clean-room.** We study other editors' *behaviour* only, and we must **not copy proprietary source** (Rust, WGSL, C++ or JS), even where it ships as readable source. Specs come from public format docs (Adobe PSD spec, ISO/ICC), academic papers (PatchMatch, Poisson blending, ARAP) and observed behaviour. Never paste code from another product.

### 1.1 Avoiding GIMP's hole

The criticism of GIMP is historically true, and much of it still applies in 2026.

**Bit depth.**
- GIMP's original core hard-coded 8-bit-per-channel tiles.
- High bit depth needed a whole new engine, GEGL. GEGL began around 2000, was introduced as optional in 2.6 (2008), and the port only finished in **GIMP 2.10 (2018)**, "the result of six years of work" on that port alone.
- Only then did GIMP get 16/32-bit integer and float processing.

**Non-destructive editing.**
- Photoshop has had adjustment layers since about 1994.
- GIMP got non-destructive *filters* in **3.0 (March 2025)**, about seven years after 2.10 because of the GTK3 port.
- It got non-destructive *layer types* (link layers, vector layers) in **3.2 (March 2026)**.

**CMYK.**
- GIMP 3.x is still "a 100% RGB editor" internally.
- CMYK exists only as export (JPEG/TIFF/PSD) and soft-proofing.

**PSD.**
- Import is still lossy. Photoshop adjustment layers and most layer styles are not carried over faithfully, and text often needs re-typing.
- 3.2 converts legacy drop and inner shadows into GIMP filters, and added PSB export.

**Root cause:** fundamental properties were **baked into the core as assumptions instead of being data.** Pixel depth, colour model, and "a layer is a pixel buffer" were all hard-coded. Every later feature that touched them needed a rewrite of everything built on top.

Sources: [GIMP 2.10 release notes](https://gimp.org/release-notes/gimp-2.10.html), [GEGL on Wikipedia](https://en.wikipedia.org/wiki/GEGL), [GIMP 3.2 release notes](https://www.gimp.org/release-notes/gimp-3.2.html), [LWN on GIMP 3.0 colour](https://lwn.net/Articles/998793/).

**Rules we adopt from day one** (cheap now, and each would force a rewrite later):

| Property | Rule | Where |
|---|---|---|
| **Bit depth** | Every surface carries a runtime `PixelFormat { depth: U8\|U16\|F16\|F32, model, alpha }`. Algorithms are written generically over a `Sample` trait and monomorphized per depth, or run on f32 working buffers and convert at tile edges. No `u8` pixel type appears in any public engine API. CI runs the gating tests at 8, 16 and 32f. | `raster`, `color`, `algo` |
| **Colour model** | `ColorMode` (RGB, Gray, CMYK, Lab, Indexed, Bitmap, plus multichannel/spot) is in `doc` from the start. Channel count is dynamic (1–N plus alpha plus spot). Only RGB/Gray *rendering* ships first, but the storage, PSD round-trip and conversion paths never assume 3 or 4 channels. | `doc`, `raster`, `psd` |
| **Colour management** | Every document has an ICC profile. The compositor takes a working-space descriptor. The display transform is a separate final stage. Blending space (gamma vs linear) is a per-document option. | `color`, `compose` |
| **Non-destructive by construction** | `LayerContent` is an enum that includes Adjustment, Fill, Text, Shape and Smart from v1, with editable filters as a stack on any layer. The compositor evaluates a *graph*, not a list of flattened buffers. Destructive ops are just "apply and bake". | `doc`, `compose` |
| **File formats** | Formats plug in through `trait ImageDecoder/Encoder` plus a registry with capability flags (depths, modes, layers, alpha, metadata, ICC). Adding a format is one crate feature. It must never need a change in core types. Lossless PSD round-trip preserves unknown blocks. | `codecs`, `io`, `psd` |
| **Size** | Tiled, sparse storage behind a `TileStore` trait, with 64-bit coordinates for PSB-scale canvases (300k × 300k). Nothing assumes a document fits in one allocation or one GPU texture. | `raster` |
| **Editor** | The engine never depends on a UI toolkit (§3–§4). GIMP spent years porting GTK2 to GTK3; our editor can be replaced without touching the engine. | layering rule 2 |

The test is simple: **adding 64-bit float, a new colour model, or a new file format should touch a leaf crate, not the foundation.** We check this by adding a (hidden) CMYK and a 32f code path early, even before they are exposed in the UI.

---

## 2. Workspace layout

```text
kition-photo/
├─ Cargo.toml                  # [workspace], shared deps, lints, profiles
├─ kit.json                    # the kit manifest
├─ crates/
│  ├─ support/                 ── foundation ──
│  │  ├─ geom/                 kition-photo-geom      points, rects, affine/perspective, tile coords, bezier
│  │  ├─ cms/                  kition-photo-cms       pure-Rust ICC colour management: profiles, transforms, intents, BPC, soft proof, 3D LUTs
│  │  ├─ color/                kition-photo-color     pixel formats, color spaces, blend-mode math (scalar reference); conversions via cms
│  │  └─ text/                 kition-photo-text      font DB, shaping + layout (parley/skrifa/harfrust), text-layer rasterization
│  ├─ render/
│  │  └─ raster/               kition-photo-raster    tiled COW surfaces, masks, mip pyramids, damage regions, pixel iterators
│  ├─ engine/                  ── document, algorithms, façade ──
│  │  ├─ doc/                  kition-photo-doc       document model: layer tree, masks, effects, channels, paths, guides, metadata (pure data + serde)
│  │  ├─ ops/                  kition-photo-ops       reversible operations on a doc + transactions + history/undo
│  │  ├─ algo/                 kition-photo-algo      adjustments, filters, selection, inpaint/heal, warp/transform, resampling
│  │  ├─ paint/                kition-photo-paint     brush engine: dabs, dynamics, stroke smoothing, brush presets
│  │  ├─ vector/               kition-photo-vector    shapes, paths, strokes → coverage
│  │  ├─ compose/              kition-photo-compose   the CPU compositor
│  │  ├─ plugins/              kition-photo-plugins   sandboxed WebAssembly filter plug-ins (wasmi host, limits, ABI v1, registry)
│  │  ├─ engine/               kition-photo-engine    Session: open docs, command registry + dispatch, jobs, preferences
│  │  └─ automation/           kition-photo-automation  automation session (documents, workspace file I/O, JSON lines) over the command registry
│  ├─ formats/                 ── I/O ──
│  │  ├─ psd/                  kition-photo-psd       PSD/PSB read + write; its OWN format-level model; depends on nothing in this workspace
│  │  ├─ codecs/               kition-photo-codecs    flat raster decode/encode at format-supported depths; AVIF encode only (feature `avif`), HEIF/HEIC decode only (feature `heif`); no JPEG XL
│  │  ├─ heif/                 kition-photo-heif      optional HEIF/HEIC decoder (heic-rs, pure Rust); used only by codecs behind its `heif` feature (standalone)
│  │  ├─ affinity/             kition-photo-affinity  bounded native Affinity reader: archive, object stream, layers, vectors, text, pixels (standalone; read only)
│  │  ├─ raw/                  kition-photo-raw       clean-room raw camera file decode (DNG, CR2, TIFF/EP) + develop pipeline (standalone, like psd)
│  │  ├─ format/               kition-photo-format    native document format (.kphoto bundle): manifest + content-addressed tiles
│  │  └─ io/                   kition-photo-io        import/export orchestration; doc ⇄ PSD mapping
│  └─ kdm/
│     └─ kdm-model/            kdm-model              bridges the document to a kit.photo KDM replica
├─ bindings/
│  ├─ wasm/                    kition-photo-kit       the engine behind Kition's `KitDocument` contract, compiled to WebAssembly for the editor
│  └─ headless/                kition-photo           the headless kit executable that agents drive
├─ web/                        the kit's editor (React, on `@kition/sdk/editor`)
├─ assets/                     bundled fonts and the spelling dictionary
├─ cases/                      gating tests (see `docs/quality.md`)
└─ tools/
   ├─ xtask/                   build tasks: layers, wasm, ci, stats, corpus, test-corpus, version, eval
   └─ scripts/                 corpus fetch wrapper, layered-TIFF fixture writer
```

This is the layout as it is today. Fuzz targets live next to the format crates (`crates/formats/*/fuzz`).

**Crate granularity:** start with the crates above. Split `algo` into `-adjust`, `-filters`, `-select`, `-inpaint` and `-warp` once any module passes about 10k lines, or once compile times hurt. Its internal module boundaries should already follow those lines.

**Naming:** directories are short and packages are prefixed `kition-photo-`. `kition-photo-psd` gets a neutral, publishable name (such as `psd-rw`) if we release it to crates.io.

---

## 3. Dependency layering (enforced)

```text
 L7  bindings/* · xtask             (binaries and tooling: exempt from the rules)
 L6  automation
 L5  kdm-model · engine
 L4  io · plugins
 L3  compose · format
 L2  ops · algo · paint · text · vector
 L1  doc
 L0  geom · cms · color · raster           psd, codecs, heif, raw, affinity (standalone, no workspace deps)
```

**Rules** (checked by `cargo xtask layers` in CI, which parses `cargo metadata`):

1. A crate may depend only on crates in **lower** layers. No cycles and no sideways dependencies, except where listed.
2. **Nothing below L6 may depend on a UI toolkit or windowing crate** (`UI_CRATES` in `tools/xtask/src/layers.rs`). No crate in the workspace does today: the editor is the React app in `web/`.
3. **`kition-photo-psd` depends on no workspace crate.** The doc ⇄ PSD mapping lives in `io`. This keeps the PSD crate publishable and reusable by other projects. The same holds for every standalone crate, with one documented exception: `codecs` → `heif` (both standalone and publishable; `STANDALONE_EXCEPTIONS` in `tools/xtask/src/layers.rs`).
4. **wasm gate:** `cargo xtask wasm` runs `cargo check --target wasm32-unknown-unknown` on every crate of the wasm set.
5. **C dependencies** (optional LibRaw, pdfium) only behind features, only in `codecs`, `raw` or `io`, and never on by default for the web target.

---

## 4. The engine/editor seam (the most important interface)

Every frontend goes through one entry point. Keeping it small and data-oriented is what keeps the editor replaceable.

### 4.1 `Session`: commands in, JSON out

```rust
// kition-photo-engine
impl Session {
    /// Run a command by id with JSON params. Returns a JSON result.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<Value>;
}
```

### 4.2 The command registry: the menu and automation catalogue

Each command is a `CommandSpec` (`crates/engine/engine/src/commands.rs`) with the following:
- `id` (`"filter.blur.gaussianBlur"`), `label`, `menu` (`["Filter","Blur"]`), and a default `shortcut`.
- `params`: a human- and agent-readable description of the parameters.
- `enabled`: whether the command can run in the session's current state, with the reason when it cannot.
- `run`: the implementation.
- `journal`: whether the command is recorded in the session journal (false for queries).

**Consumers:**
- The editor (`web/`) dispatches commands by id from its menus, options bar, tools and panels.
- `automation` serves the registry to agents through the headless kit.

### 4.3 The editor

- `web/` is a React app on `@kition/sdk/editor`. It loads the engine as the WebAssembly module `kition_photo_kit` (class `PhotoDocument`, built from `bindings/wasm`; see `web/src/engine.ts` and `kit.json`).
- Menus are in `web/src/menus/`, the options bar in `web/src/options/`, the canvas tools in `web/src/tools/canvas/`, and the panels in `web/src/panels/`.
- The editor holds only view state. All document edits go through the engine's commands, by id.

**Litmus test for the seam:** the headless program must be able to do *everything* the editor does.

---

## 5. Document model (`kition-photo-doc`)

```rust
pub struct Document {
    pub id: DocId, pub size: Size, pub resolution_dpi: f32,
    pub mode: ColorMode /* Rgb | Gray | Cmyk | Lab | Bitmap | Indexed */, pub depth: Depth /* U8 | U16 | F32 */,
    pub profile: IccProfileRef,
    pub root: LayerGroup,            // tree
    pub channels: Vec<AlphaChannel>, // saved selections/spot
    pub paths: Vec<NamedPath>, pub guides: Guides, pub slices: .., pub metadata: Metadata /* XMP, EXIF */,
    pub selection: Option<Mask>,     // current selection (a raster mask + optional vector outline)
}

pub struct Layer { pub id: LayerId, pub name: String, pub visible: bool, pub locks: Locks,
                   pub blend: BlendMode, pub opacity: f32, pub fill_opacity: f32, pub clipped: bool,
                   pub mask: Option<RasterMask>, pub vector_mask: Option<VectorMask>,
                   pub effects: Effects, pub blend_if: BlendIf, pub content: LayerContent }

pub enum LayerContent {
    Raster(TiledSurface),
    Group(LayerGroup /* children, pass-through or isolated */),
    Adjustment(Adjustment /* Curves, Levels, HueSat, … as params */),
    Fill(Fill /* solid | gradient | pattern */),
    Text(TextLayer /* runs, paragraph styles, warp; cached raster */),
    Shape(ShapeLayer /* vector paths + fill/stroke */),
    Smart(ObjectLayer /* embedded or linked source + transform + editable filters */),
}
```

- **Pixels.** `raster::TiledSurface<F>` is a sparse map from `TileCoord` to `Arc<Tile<F>>`, with 256×256 tiles. Missing tiles are transparent, or take a fill value. Mutation calls `Arc::make_mut`, so it is copy-on-write.
- **Mip pyramids** are derived lazily per layer, for zoomed-out views.
- **Big documents.** The tile store is behind a `TileStore` trait: in-memory now, disk-spilling ("scratch disk") later, with LRU eviction under a global memory budget.
- **The first milestone supports RGB and Gray, at 8/16/32f.** CMYK and Lab are modelled in the types from day one (so PSD round-trips don't lose them) but are rendered by converting to RGB until later phases.

---

## 6. Operations and history (`kition-photo-ops`)

- **Every mutation is an `Op`**, such as `SetLayerProps`, `PaintTiles{layer, tiles_before, tiles_after}`, `AddLayer`, `MoveLayer` or `ApplyFilter`.
- **A command produces a `Transaction` of ops** with a label ("Gaussian Blur").
- **History stores transactions.** Because tiles are `Arc`-shared, "before" states cost only the tiles that actually changed. Undo means swapping `Arc`s, so no pixel copies are made.
- **Each state remembers its targeted layers** (the active layer and the layer selection, as they were when the state was created: when the document was opened, or right after the step that made it). Selecting layers is not a step and doesn't change any state's target, but undo and redo target the restored state's layers again, as Photoshop does.
- **Brush strokes** accumulate into one transaction per stroke. Tiles are published incrementally with damage regions, so the viewport updates live.
- **Snapshots for readers.** After each committed op the engine publishes a new `Arc<DocSnapshot>` through `arc-swap`. The UI, background jobs (filters, AI inference, export) and autosave read snapshots without locks.
- **Background jobs** compute from snapshot N and commit as a transaction. If the doc changed meanwhile, the job rebases, meaning it re-targets the same layer id if it still exists, or reports a conflict.
- **History features:** a history panel, a memory cap with oldest-first eviction, and named snapshots. The history brush comes later, reading tiles from a past state.

---

## 7. Rendering

### 7.1 Planner (`compose`)

`plan(&DocSnapshot, region) -> Vec<DrawOp>` flattens the layer tree into a linear instruction stream:
- `PushGroup{isolated, blend, opacity}` / `PopGroup`
- `BeginClipBase` / `Clipped`
- `DrawTiles{layer, blend, opacity, mask}`
- `Adjust{program}`
- `Effect{kind, params}`
- `Fill{...}`

The compositor consumes this plan. This is the only place that encodes Photoshop layer semantics: pass-through groups, clipping groups, fill vs opacity, blend-if and knockout.

### 7.2 Backend

- **CPU (`compose`).** The only compositor, done per tile with rayon. The editor (through `bindings/wasm`), the headless program, export and tests all use it. There is no GPU backend.

### 7.3 Color

- Blending happens in document space by default, which is Photoshop-compatible. A per-document "linear light blending" option is also available.
- **Display transform** (`crates/engine/engine/src/display_color.rs`): document profile → monitor profile (relative colorimetric + BPC), cached per (document profile, mode, monitor). When the document profile matches the monitor (sRGB on sRGB) there is no transform. CMYK documents are read through their embedded CMYK profile (`kition_photo_color::convert::with_cmyk_space`, entered by the compositors and composite exports).
- **Monitor profile:** the `edit.colorSettings` Monitor Profile is `auto`, a built-in RGB profile or an `.icc` path. `auto` uses a display's profile when the embedding program supplies one, and sRGB otherwise; neither `bindings/` nor `web/` supplies one. `ColorState::monitor_status_for` says what is applied per display: `auto`, `manual`, or `fallback` to sRGB with the reason (missing, unreadable, non-RGB or unusable as a destination); `edit.colorSettings` returns it (`monitorStatus`, `displays`).
- **HDR/EDR output** (an `rgba16float` surface with an extended-range colorspace) is a later-phase feature. The interfaces already carry `f32` pixels.

### 7.4 Oracle

**PSD files contain Photoshop's own merged composite.** Running our compositor on a PSD's layers and diffing against its embedded composite gives a free, large conformance test suite. `crates/formats/io/tests/corpus.rs` runs this over the PSD corpora. The corpora are fetched at pinned commits into `corpus/` (never committed) by `cargo xtask corpus --all`; our own Photoshop-authored oracles are the maintainers' private corpus, not distributed (see `docs/development.md` › Test corpora).

---

## 8. `kition-photo-psd`: standalone PSD/PSB crate

**Scope:** faithful, format-level read and write of PSD (v1) and PSB (v2, 64-bit lengths). It has no dependency on our document model.

```text
psd/src/
  header.rs         color mode, depth, size
  color_mode_data.rs
  resources/        image resources (1005 resolution, 1036 thumbnail, 1039 ICC, 1060 XMP, 1058 EXIF, slices, guides, …)
  layers/           layer records, channel image data (raw | RLE/PackBits | ZIP | ZIP+prediction), masks, blending ranges
  tagged/           additional layer info blocks, one module per key:
                    luni lsct lyid lfx2/lrFX (effects) TySh (text) SoLd/PlLd/lnk2 (object layers) vmsk/vsms/vogk (vector)
                    adjustment keys (curv levl hue2 blnc selc mixr grdm phfl brit thrs post nvrt expA vibA clrL blwh)
                    SoCo GdFl PtFl (fills), Patt, Txt2, …
  descriptor.rs     ActionDescriptor / OSType parser+writer (used by many tagged blocks)
  engine_data.rs    text "EngineData" (PostScript-like) parser+writer
  image_data.rs     merged composite
  write.rs          serializer (+ RLE/ZIP encoders)
```

- **Lossless round-trip.** Unknown or unsupported tagged blocks are kept as raw bytes and re-emitted unchanged. That way files keep features we don't model yet, like 3D or video.
- **Lazy decode.** The parser indexes channel data and decodes it only on request. This makes opening a 2 GB PSB fast and memory-bounded.
- **The merged composite on write is supplied by the caller** as an image buffer. `io` renders it with `compose`, so `psd` never needs a compositor.
- **Mapping lives in `io::psd`.** It converts `psd::File` into `doc::Document` and back, with fidelity levels:
  1. Flattened image.
  2. Raster layers, masks, blend/opacity/fill, visibility, names.
  3. Groups (including pass-through), clipping, blend-if, adjustment and fill layers.
  4. Layer effects (lfx2).
  5. Text layers: shaped with `text`; the original descriptors are kept for round-trip.
  6. Object layers: embedded and linked, live both ways (`io::object_layer_map`). Import reads the
     placed-layer data (`SoLd`: file id, transform quad, warp), the editable filter stack
     (`filterFX`: modelled filters become their Kition Photo command and params; any other filter
     stays verbatim and is listed as not editable) and the filter mask (global `FEid`). Export
     writes `PlLd` + `SoLd`, embeds the source in `lnk2` (a `.kphoto` source becomes a PSB of the
     nested document, written by our own PSD writer) and a filter cache with the unfiltered
     pixels and the mask in `FEid`; unedited imported object layers stay byte-identical.
  7. Vector masks and shape layers.
- **Testing:**
  - Round-trip byte-stability for unmodified files.
  - Open → save → open equality.
  - Composite-oracle diffs (§7.4).
  - `cargo-fuzz` on the parser.
  - A corpus from MIT/BSD-licensed test sets (ag-psd, psd-tools), fetched by `xtask`, not committed.
  - Our own Photoshop-authored oracle PSDs (editable filters, effect shapes, type, adjustments in every mode and depth): the maintainers' private corpus, not distributed, pinned the same way and copied from their clone; its tests are skipped without it.
- **Existing crates:** `psd` (read-only, limited) and the new `ag-psd` Rust port. We evaluate them in spike week 1, then either depend on one, fork it, or write our own. Given how central PSD is, writing our own behind a stable API is likely.

---

## 9. Native file format (`kition-photo-format`)

- **Format:** a `.kphoto` file is a zip (store mode) or directory bundle.
  - `manifest.json`: a versioned document tree with serde, including `format_version` and migrations.
  - `tiles/<blake3>.zst`: content-addressed tiles, zstd-compressed.
  - `thumb.png` and `composite/` mips for quick look and previews.
- **Saving is incremental.** Tiles are content-addressed, so a save writes only new tiles plus the manifest, and garbage-collects unreferenced ones.
- **Autosave and crash recovery** reuse the same writer into a recovery directory, reading from a snapshot on a background thread.
- **PSD is the interchange format,** and `.kphoto` is the lossless native one.

---

## 10. Algorithms (`algo`, `paint`, `text`, `vector`)

| Area | Contents (first pass) |
|---|---|
| Adjustments | levels, curves, brightness/contrast, exposure, vibrance, hue/sat, color balance, B&W, photo filter, channel mixer, gradient map, selective color, invert/posterize/threshold, 3D LUT |
| Filters | gaussian/box/motion/radial/surface/lens blur, unsharp/smart sharpen, high pass, noise add/reduce/median/dust, distort family (twirl, wave, polar, spherize, displace), stylize, render (clouds, lens flare), pixelate, color to alpha |
| Selection | marquee/lasso geometry → mask, magic wand (flood fill tolerance), color range, quick select (graph cut / superpixels), feather, grow/shrink, edge refine |
| Inpaint/heal | spot heal, healing brush (Poisson), smart fill (PatchMatch + multiscale EM) |
| Warp | free transform (affine/perspective/warp mesh), liquify, pin warp (ARAP), lens correction, resampling (Lanczos/bicubic) |
| Paint (`paint`) | dab generation (hardness, roundness, angle, spacing, scatter), dynamics from pressure/tilt/velocity, smoothing (pulled string / lazy mouse), wet edges, airbrush, eraser, clone/heal sources, blend modes per stroke, `.abr` import |
| Text (`text`) | font DB trait (system fonts native, bundled fonts on web), parley layout, per-run styles, paragraph options, warp; renders to a raster cache |
| Vector (`vector`) | shapes, bezier paths, pen tool geometry, strokes (dash, align, caps), boolean ops → coverage via vello_cpu |

**Rules for every algorithm:**
- **Signature:** a pure function `fn(input: &SurfaceView, params: &P, region: Rect, cancel: &CancelToken) -> Surface`.
- **Params:** `P: Pod + JsonSchema + Serialize + Deserialize`.
- **Registration:** registered with the command registry by the engine, not by the algo crate itself, so `algo` stays UI- and engine-agnostic.
- **Tiles:** tile-aware, requesting an input halo of `radius` pixels.
- **Testing:** unit tests in the crate, plus benchmark examples (`crates/engine/algo/examples/`).

---

## 11. Engine runtime model

- **In the editor** the engine is the WebAssembly module built from `bindings/wasm`; the editor calls it through `@kition/sdk/editor`.
- **In the headless program** (`bindings/headless`) the engine runs in-process behind the `automation` session.
- `Session::execute` runs a command synchronously, including job-capable commands; `Session::start` runs those in the background (`crates/engine/engine/src/jobs.rs`).
- **Cancellation:** `Session::cancel_job` sets a job's flag; the algorithms check it per tile, row band or iteration, and the result is discarded.

---

## 12. Automation and extensibility

- **`automation`:** the automation session (`Headless`) over an in-process engine `Session`.
  The headless kit (`bindings/headless`) serves it to agents (Kition reaches it through its kits
  MCP). Requests use dotted names
  (`doc.open`, `doc.save`, `doc.inspect`, `doc.render`, `engine.execute`, `batch`, …).
  - `AuthorizedWorkspace`, which holds independent read and write directory capabilities. Remote
    paths are validated relative names; engine commands that still require ambient filesystem
    access fail closed at the automation boundary.
- **Actions:** recorded as `Action` steps (`[[id, params], …]`, `crates/engine/engine/src/actions_cmds.rs`), replayable in batch (File → Automate → Batch).
- **Scripting (later):** embed a scripting language over the same registry. Options are Rhai, or Lua via mlua (C). JS via QuickJS is possible if we want Photoshop-script familiarity.
- **Plugins:** sandboxed WebAssembly filter plug-ins (`kition-photo-plugins`, L4) run by `wasmi`, a pure-Rust interpreter, with fuel, memory, stack and wall-time limits and no host imports. They are driven by the `plugin.*` commands and listed under Filter › Plug-ins; the ABI is in [`plugins.md`](features/plugins.md). Native Photoshop `.8BF`/CEP/UXP hosting is out of scope (it needs unsafe FFI and can't run on the web). Panel plug-ins are later.

---

## 13. Testing and quality

| Layer | What |
|---|---|
| Unit | per crate; algorithms against small hand-computed fixtures |
| Gating tests | `cases/` (see [quality.md](quality.md)) |
| PSD | round-trip, composite oracle, fuzzing |
| Engine | command-level tests: open fixture → dispatch commands → assert snapshot/render; action replays |
| Editor | Playwright scenarios in `web/tests/` (see [testing/scenarios.md](testing/scenarios.md)) |
| Perf | criterion benches (`crates/engine/compose/benches/`) and benchmark examples (`crates/engine/algo/examples/`) |
| Portability | `cargo xtask wasm` checks the wasm32 build; the CI workflow (`.github/workflows/ci.yml`) runs every test on Linux |

**Performance targets for v1:**
- Stroke-to-pixel within one frame at 120 Hz.
- Pan and zoom at display refresh on a 100 MP / 30-layer document.
- Open a 100 MP JPEG in under 1.5 s.
- 20k×20k 16-bit documents on a 16 GB machine.

---

## 14. Cargo workspace

The workspace manifest is `Cargo.toml`:
- members are `crates/*/*`, `bindings/*` and `xtask`;
- the license is Apache-2.0, the edition 2024 and the minimum Rust version 1.95;
- `unsafe_code = "forbid"`;
- dependencies build at `opt-level = 2` in the dev profile (image code is unusable at `opt-level` 0);
- the release profile uses thin LTO, and the `wasm-release` profile optimizes for size except in the pixel-processing crates.

---

## 15. Roadmap

Open work is listed in [roadmap.md](roadmap.md), from the feature catalogue (`features.yaml`).

---

## 16. Open decisions

1. **Editor.** Decided: the React editor in `web/`, on `@kition/sdk/editor`.
2. **License.** Decided: Apache-2.0.
3. **Project name.** Decided: `kition-photo`.
4. **PSD crate.** Decided: our own, `kition-photo-psd` (section 8).
5. **Default working depth and blending.** 8-bit + gamma-space blending matches Photoshop. 16-bit/float + linear is the modern default. The proposal is to follow Photoshop by default and make the other a per-document option.
6. **Scope of Photoshop plugin compatibility.** Decided: no native `.8BF` hosting; a sandboxed WebAssembly plug-in API instead (see [`plugins.md`](features/plugins.md)).
