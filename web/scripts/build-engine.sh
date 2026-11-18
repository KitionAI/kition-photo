#!/bin/sh
# Builds the Photo engine (bindings/wasm) to WebAssembly into public/engine, where the
# editor loads it from its own package. Needs wasm-pack.
set -eu
web="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$web/.." && pwd)"
if toolchain="$(rustc --print sysroot 2>/dev/null)" && [ -d "$toolchain/lib" ]; then
  export DYLD_FALLBACK_LIBRARY_PATH="$toolchain/lib${DYLD_FALLBACK_LIBRARY_PATH:+:$DYLD_FALLBACK_LIBRARY_PATH}"
fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target}"
cd "$repo"
wasm-pack build bindings/wasm --target web --release --no-typescript --out-dir "$web/public/engine"
rm -f "$web/public/engine/.gitignore" "$web/public/engine/package.json"
# Chinese text needs a CJK face the web build does not embed: the editor gives the engine this one
# (from the kition-fonts checkout next to the repository, or KITION_FONTS_DIR) before it opens.
fonts="${KITION_FONTS_DIR:-$repo/../kition-fonts}/fonts/droid-sans-fallback"
rm -rf "$web/public/fonts"
if [ -f "$fonts/DroidSansFallbackFull.ttf" ]; then
  mkdir -p "$web/public/fonts"
  cp "$fonts/DroidSansFallbackFull.ttf" "$web/public/fonts/"
  cp "$fonts/NOTICE" "$web/public/fonts/DroidSansFallback-NOTICE.txt"
else
  echo "warning: no kition-fonts checkout at $fonts; Chinese type will show as boxes" >&2
fi
