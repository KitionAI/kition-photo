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
kition_fonts="${KITION_FONTS_DIR:-$repo/../kition-fonts}"
fonts="$kition_fonts/fonts/droid-sans-fallback"
rm -rf "$web/public/fonts"
if [ -f "$fonts/DroidSansFallbackFull.ttf" ]; then
  mkdir -p "$web/public/fonts"
  cp "$fonts/DroidSansFallbackFull.ttf" "$web/public/fonts/"
  cp "$fonts/NOTICE" "$web/public/fonts/DroidSansFallback-NOTICE.txt"
else
  echo "warning: no kition-fonts checkout at $fonts; Chinese type will show as boxes" >&2
fi
# The Type tool's other families (plan 2026-11-24, stage 9), loaded when a person picks one or a
# document uses it: each kition-fonts folder is copied whole (the font files with their licence
# beside them), and their lines of kition-fonts' manifest become public/fonts/manifest.txt, which
# the editor reads (web/src/fonts/). Latin sans, serif, mono and display, Chinese Kai and a Mincho
# (Song-style) face, Japanese, Korean and Arabic; NOTICE names them all.
served="arimo carlito bricolage-grotesque lora tinos caladea cousine ma-shan-zheng shippori-mincho biz-udmincho biz-udpgothic nanum-gothic noto-sans-arabic"
if [ -f "$kition_fonts/fonts/manifest.txt" ]; then
  mkdir -p "$web/public/fonts"
  {
    echo "# The Type tool's served fonts, from kition-fonts' manifest (written by scripts/build-engine.sh)."
    for family in $served; do
      if [ -d "$kition_fonts/fonts/$family" ]; then
        cp -R "$kition_fonts/fonts/$family" "$web/public/fonts/"
        grep -F " | fonts/$family/" "$kition_fonts/fonts/manifest.txt" || true
      else
        echo "warning: kition-fonts has no fonts/$family; the Type tool's list leaves it out" >&2
      fi
    done
  } > "$web/public/fonts/manifest.txt"
fi
