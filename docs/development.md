# Development guide

## Repository layout

| Path | What |
| --- | --- |
| `crates/` | The engine (Rust), grouped as `crates/{support,render,formats,engine,kdm}/<name>` |
| `bindings/` | The engine's two outlets: `wasm` (the editor's engine) and `headless` (the kit executable `kition-photo` for agents) |
| `web/` | The kit's editor (React, on kition-sdk's `@kition/sdk/editor`) |
| `kit.json` | The kit manifest (KHP) |
| `agent/` | The agent guide and checks (KHP) |
| `conformance/` | KHP conformance fixtures and their expected previews |
| `features.yaml`, `cases/` | The feature catalogue and its gate cases ([quality.md](quality.md)) |
| `README.md`, `CHANGELOG.md`, `AGENTS.md` | Overview, release notes, rules for agents and contributors |
| `assets/readme/` | The README's pictures ([Take the README pictures](#take-the-readme-pictures)) |
| `docs/` | This guide, [architecture](architecture.md), [gaps](gaps.md), [roadmap](roadmap.md), [quality](quality.md), [attribution](attribution.md), [writing format](writing-format.md); feature notes in `features/`, target-app parity in `parity/`, the scenario suite in `testing/` |
| `assets/` | Fonts and the spelling dictionary bundled with the engine |
| `examples/` | An example WebAssembly filter plug-in (`plugins/invert-rs`, not a workspace member) |
| `tools/` | Repository tooling: `xtask` (`cargo xtask …`) and `scripts/` |
| `LICENSE`, `NOTICE` | Apache-2.0 and third-party notices |
| `Cargo.toml`, `Cargo.lock` | The Cargo workspace |

Feature notes: [plug-ins](features/plugins.md), [Raw Develop histogram](features/raw-develop-histogram.md),
[Proximity Match](features/proximity-match-performance.md), [Radial Blur](features/radial-blur-performance.md),
[selection distance transforms](features/selection-distance-performance.md).

## Prerequisites

- Rust stable (1.95+). Add the web target with `rustup target add wasm32-unknown-unknown`.
- macOS, Windows or Linux.
- Node and pnpm, for `web/` (the kit's editor).

## Build and test

```sh
cargo test --workspace                                     # everything
cargo xtask ci                                             # fmt + clippy + tests + layers + wasm
cargo xtask stats                                          # tests and lines per crate
cargo build --release -p kition-photo                         # the headless kit executable
cd web && pnpm install && pnpm engine && pnpm test:flow     # the kit's editor (Vite + wasm engine)
```

Image code is slow at `opt-level 0`, so the workspace profile builds dependencies at `opt-level 2`.

By default `pnpm install` in `web/` installs the published `@kition/sdk` and `@kition/tessera`
packages from npm, at the versions the committed `web/pnpm-lock.yaml` pins, so the editor needs no
other checkout. To work against local checkouts instead, run `KITION_LINK=1 pnpm install` in
`web/`: `web/.pnpmfile.cjs` then links `@kition/sdk` from the kition-sdk checkout
(`packages/sdk`) and `@kition/tessera` from the kition-tessera checkout (`packages/tessera`) next
to this repository (or the ones `KITION_SDK` and `KITION_TESSERA` name), each one whose package is
there. Both packages' `exports` point at their built `dist/`, so run `pnpm build` in those two
package folders first. The linked install rewrites `web/pnpm-lock.yaml`; do not commit that
lockfile.

For independent editing scenarios, failure reports and CI setup, see
[scenario testing](testing/scenarios.md). The test suite runs against Kition Photo alone.

## Take the README pictures

The README's editor pictures (`assets/readme/hero.png`, `see-1.png` … `see-4.png`) are taken by
[`web/scripts/readme-shots.mjs`](../web/scripts/readme-shots.mjs) on kition-sdk's shared runner
(`tools/readme-shots/shots.mjs`). It starts the development host, opens the showcase sample
`photo/autumn-sale-poster` from a kition-showcase checkout next to this repository (or the one
`KITION_SHOWCASE` names), and takes each picture at 1440 × 900, device scale 2, in the Classic
display mode, light theme and English. It needs the built engine in `web/public/` and a
kition-sdk checkout next to this repository (or `KITION_SDK`); it is a tool, not a test.

```sh
cd web
node scripts/readme-shots.mjs              # every picture, or name some: hero see-2
pngquant --quality 70-90 --force --ext .png ../assets/readme/hero.png ../assets/readme/see-*.png
```

Look at every picture before committing it, and keep each one under 400 KB. `made-1.png` …
`made-4.png` are the `thumb.png` of four showcase samples (`neon-noodle-bar-sign`,
`cinematic-color-grade`, `isometric-city-block`, `magazine-feature-spread`), copied as they are.

## Publish

Kition ships the kit's package, `kition-photo-<version>.tgz` (the `photo-ui/` folder), a release
asset of this repository tagged `package-v<version>`. It is built on GitHub, never on a developer
machine: the Package workflow (`.github/workflows/package.yml`, which calls Kition's reusable
`kit-package.yml`) builds the commit it is started on, with `gh workflow run package.yml -R
KitionAI/kition-photo`, and publishes the Release with the package's lock entry,
`kition-photo-<version>.json`. Kition pins it with `node scripts/pin-kits.mjs photo` (see
`docs/kits.md` there). A version is published once: bump `version` in `kit.json` to publish a
change.

## Fonts

Font assets live in [KitionAI/kition-fonts](https://github.com/KitionAI/kition-fonts), never in
this repo: don't commit font files here (Inter and JetBrains Mono in `assets/fonts/` are the only
exceptions). `KITION_FONTS_DIR` is an **optional build input**, never a Cargo dependency:

```sh
git clone https://github.com/KitionAI/kition-fonts ../kition-fonts
KITION_FONTS_DIR="$PWD/../kition-fonts" cargo test --workspace
```

- Use an absolute path (`$PWD/...`): `build.rs` runs in `crates/support/text`, so a relative
  `KITION_FONTS_DIR` would resolve from there.
- `crates/support/text/build.rs` reads `$KITION_FONTS_DIR/fonts/manifest.txt` and embeds the fonts.
  Unset, the embedded set is empty and the build is unchanged. A bad path is a build warning, or
  an error with `KITION_FONTS_REQUIRED=1`.
- **Web:** the wasm32 build never embeds these fonts, even with `KITION_FONTS_DIR` set (size
  budget). It loads fonts served next to it, on demand, in the same manifest format
  (`crates/support/text/src/served.rs`).
- Tests that need the fonts skip with a message when the directory is unset.

## Native format

`crates/formats/format` defines the lossless native bundle. It is either a ZIP of STORE entries or a
directory with the same layout:

- `manifest.json`: the versioned document tree, with migrations in `format/src/migrate.rs`.
- `tiles/<blake3>.zst` and `blobs/<blake3>.zst`: zstd-compressed objects, content-addressed by the
  BLAKE3 hash of their uncompressed bytes.
- `thumb.png` and `composite/preview.png`: previews.

Keep one writer per open document: re-saving then only compresses and writes tiles that changed.
Directory bundles verify objects on first encounter in a folder; later saves reuse them while
their file size and modification time are unchanged, and re-verify changed objects, repair
missing or damaged objects, and garbage-collect unreferenced ones.

`kition-photo-io` routes the format through this crate in `import`/`export`, detecting it by magic
or by extension.

## Colour management

`crates/support/cms` is our own pure-Rust ICC engine (v2/v4 parsing, matrix/TRC and LUT profiles, all four
intents, black point compensation). It ships CC0 built-in profiles (see `crates/support/cms/README.md`).

- Documents carry an optional embedded ICC profile (`Document::icc_profile`); `edit.assignProfile`
  and `edit.convertToProfile` change it. Mode changes (`image.mode.*`) convert through cms.
- Convert colours with `kition_photo_cms::transform::cached(src, dst, opts)`: transforms are cached
  process-wide and integer buffers use precomputed tables or a device link.

## Testing strategy

- **Unit and property tests** in every crate. proptest is used for tile COW, regions and codecs.
- **Format crates:**
  - synthetic generators (`psd::testgen`)
  - byte-exact round trips
  - malformed-input sweeps (truncate at every offset)
  - fuzz targets (`crates/*/fuzz`)
- **Real-file corpora** in `corpus/` (gitignored, fetched at pinned commits, sha256-verified):
  opt-in locally through the `corpus` cargo feature. See [Test corpora](#test-corpora).
- **Composite oracle:** a PSD's embedded merged image is compared with our compositor's output.

## Test corpora

Real files are our best oracles, but they are large binaries, so they never go into this
repository. They live in `corpus/` (gitignored) and are fetched at **pinned commits** and checked
against committed **sha256 manifests**. All pins are in one place:
[`tools/xtask/src/corpus_pins.rs`](../tools/xtask/src/corpus_pins.rs).

Every corpus below is downloaded from a public source, except `corpus/photoshop/`: the
Photoshop-authored reference files are maintained privately by the maintainers and are not
distributed. Their tests run in the maintainers' environment; without `corpus/photoshop/` they are
skipped with a message, and everything else runs as usual.

| Directory | What | Source | Manifest |
|---|---|---|---|
| `corpus/photoshop/` | 258 PSDs authored with Photoshop: editable filters, layer-style effect shapes, the text engine, adjustments in every mode and depth | the maintainers' private corpus (not distributed) | `tools/xtask/photoshop-corpus.sha256` |
| `corpus/psd/` | 170 small psd-tools and ag-psd files, the mix most PSD tests use | psd-tools and ag-psd upstreams (MIT) | `tools/xtask/psd-corpus.sha256` |
| `corpus/psd-tools/` | the complete psd-tools test set (309 files) | psd-tools upstream (MIT) | `tools/xtask/psd-tools-corpus.sha256` |
| `corpus/heif/` | 9 small HEIC/HEIF files (checkerboards, RGB strips, a grid-tiled photo with EXIF/XMP, each with Apple's decode as `.ref.png`; a 10-bit RGBA file with its source PNG), for the `heif` feature | heic-rs (MIT OR Apache-2.0) and pillow-heif (BSD-3-Clause) upstreams | `tools/xtask/heif-corpus.sha256` |
| `corpus/exr/` | the 5 deep OpenEXR test images (scanline deep data with half colour and u32 ID channels; 2.3 MB), checked against the ID manifests of their upstream sidecars | OpenEXR upstream at v3.5.2 (BSD-3-Clause) | `tools/xtask/exr-corpus.sha256` |
| `corpus/affinity/` | 21 public Affinity documents and the 18 Affinity 3 samples, with exported PNG references | vector-art and affinity-samples (CC0), AFDesignLoad, Jac21/Branding and AssetStoreTemplate (MIT) upstreams | `tools/xtask/affinity-corpus.sha256` |
| `corpus/pngsuite/` | PngSuite | schaik.com release archive (public domain) | (fixed archive) |

```sh
cargo xtask corpus                 # where each corpus lives, its pin, present or missing
cargo xtask corpus --all           # fetch every public corpus missing or stale (cold: about 15 s; verified copies are left alone); corpus/photoshop is skipped
cargo xtask test-corpus            # fetch, then cargo test --release --features corpus (+ heif on codecs, io) on psd, codecs, io, engine
cargo xtask test-corpus -p io      # narrow to one crate (repeat -p for more)
cargo xtask test-corpus --changed  # only if psd, io, codecs, compose, text, format or affinity changed vs origin/main
cargo xtask test-corpus -- --nocapture   # pass arguments to the test binaries (per-file tables)
tools/scripts/fetch-corpus.sh      # the same as cargo xtask corpus --all
```

**Opt-in locally, strict when opted in.**

- The corpus tests sit behind the `corpus` cargo feature of `kition-photo-psd`, `kition-photo-codecs`,
  `kition-photo-io` and `kition-photo-engine`, so plain `cargo test` neither compiles nor needs them.
  The HEIF ones also need the `heif` feature of `kition-photo-codecs`/`kition-photo-io` (test-corpus
  turns it on).
- With the feature on, a missing public corpus is a failure ("run `cargo xtask corpus --all`"),
  never a silent skip, and every floor is enforced. The one exception is the private
  `corpus/photoshop/`: its tests print that it is missing and skip.
- If you touch psd, io, codecs, compose, text or format, run `cargo xtask test-corpus` before you
  finish.

**The Photoshop corpus (maintainers only).** The maintainers keep it in a private repository,
`kition-photo-corpus`; outside contributors do not have it and do not need it.

- **Authoring clone:** a normal git clone, placed next to this checkout
  (`../kition-photo-corpus`). Its generators write there.
- **Consuming copy:** `corpus/photoshop/` here, a plain directory checked against the pinned
  manifest. `cargo xtask corpus --photoshop --local` (or `--all --local`, `test-corpus --local`)
  copies `photoshop/` from `../kition-photo-corpus` (or `KITION_PHOTO_CORPUS_REPO=<path>`); it is
  never downloaded, and `--photoshop` without `--local` stops with an error saying so.
- **Not a submodule or a subtree:** a subtree would put the binaries back into this repository's
  history, and submodules cause init and detached-HEAD friction for every contributor. A pin plus
  a manifest gives the same reproducibility.
- The third-party sets (psd-tools, ag-psd, heic-rs, pillow-heif, OpenEXR, the Affinity sets and
  PngSuite) are public: they are downloaded from their upstreams (the GitHub tarball of the pinned
  commit, or PngSuite's release archive), with no credentials, and verified.

## Rendering fidelity (PSD oracle)

`cargo xtask test-corpus -p io -- --nocapture` compares our composite of every corpus PSD with
Photoshop's own merged image (PASS ≤ 2/255) and checks that export → re-import renders the same,
per source with its own floors: `corpus/psd`, the psd-tools set (also a truncation/corruption
sweep over every file that must never panic) and, where the maintainers' private corpus is
present, the Photoshop set `corpus/photoshop` (per-feature-group totals: `smart-filters`, `effects`, `text`, `adjustments/<mode><bits>`). Smart
objects and type layers composite Photoshop's cached pixels there; `cargo xtask test-corpus -p
engine -- --nocapture` (`crates/engine/engine/tests/photoshop_oracles.rs`) re-renders them with our
editable-filter stack and text engine and is the failure map for both. A panic is reported as `CRASH`
and fails the run. Files without a real merged image (Maximize Compatibility off) are judged
against their embedded thumbnail instead (`PASS (thumbnail)`, a strict low-resolution check); a
file only SKIPs when it has no oracle at all. Raise the floors in `crates/formats/io/tests/corpus.rs` when
they improve; never lower them. Synthetic reproductions of corpus findings live in
`crates/formats/io/tests/corpus_regressions.rs` (corpus files are never committed). To dig into one file:

```sh
cargo run --release -p kition-photo-io --example oracle_diff -- corpus/psd/<file>.psd 0 png /tmp/diff.png
```

writes ours | Photoshop | a diff heatmap side by side; `col`, `row`, `worst [n]`, `grid x0 y0 x1
y1 [ch]`, `layerpx x y` and `DUMP_FX=1` print samples, the worst pixels, value grids, per-layer
pixels and raw effect descriptors. Findings so far: fill-layer gradients are framed by the layer's
mask bounds; Photoshop's gradient Smoothness is a Catmull-Rom blend and "Perceptual" interpolation
is Oklab (baked into dense stops on import, `crates/formats/io/src/gradient_bake.rs`); a shape layer's
vector stroke is drawn above its clipped layers; linked effect patterns tile from the layer's
`fxrp` reference point; stroke distances follow a 5 × 5 chamfer metric (1, √2, √5) seeded at
sub-pixel edge offsets; interior effects keep the layer's alpha; outside strokes blend onto the
backdrop with their own modes, an upper stroke covering lower ones.
