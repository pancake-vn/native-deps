#!/usr/bin/env bash
# Harvest a decode-only libvpx source subset + per-target build configs into
# this crate (Chromium/AOSP-style vendoring — design doc in pancake-work-client:
# docs/plans/2026-07-10-webm-sticker-support-design.md §7.2 option C).
#
# Usage:  ./vendor.sh [--target <libvpx-configure-target>]
#         default target: arm64-darwin20-gcc  (config dir: arm64-darwin)
#
# Runs libvpx's own configure+make ONCE, offline, on a dev machine; the build
# hosts (CI, Windows box, app builds) only ever compile the checked-in
# sources with cc-rs — no perl/make/bash/nasm needed at app build time.
#
# Re-run per target family to add configs/<target>/ (Phase 5 of the
# implementation plan). The upstream/ source subset is the UNION of every
# harvested target's source list; build.rs picks per-target files from
# configs/<target>/sources.txt.

set -euo pipefail

LIBVPX_TAG="v1.16.0"
TARGET="${2:-arm64-darwin20-gcc}"
if [[ "${1:-}" == "--target" ]]; then TARGET="$2"; fi

# Map configure-target -> config dir name used by build.rs.
case "$TARGET" in
  # arm64-darwin-gcc (no version suffix) is libvpx's iOS-device target;
  # arm64-darwinNN-gcc are macOS.
  arm64-darwin-gcc) CONFIG_DIR="arm64-ios" ;;
  arm64-darwin*) CONFIG_DIR="arm64-darwin" ;;
  arm64-android*) CONFIG_DIR="arm64-android" ;;
  arm64-linux*) CONFIG_DIR="arm64-linux" ;;
  arm64-win64*) CONFIG_DIR="arm64-win64" ;;
  armv7-android*) CONFIG_DIR="armv7-android" ;;
  x86_64-android*) CONFIG_DIR="x86_64-android" ;;
  x86_64-linux*) CONFIG_DIR="x86_64-linux" ;;
  x86_64-darwin*) CONFIG_DIR="x86_64-darwin" ;;
  x86_64-win64*) CONFIG_DIR="x86_64-win64" ;;
  x86-android*) CONFIG_DIR="x86-android" ;;
  generic-gnu) CONFIG_DIR="generic" ;;
  *) echo "unmapped target $TARGET — add a case above"; exit 1 ;;
esac

CRATE_DIR="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Reuse an existing checkout via LIBVPX_SRC=/path/to/libvpx (must be at
# $LIBVPX_TAG); otherwise clone fresh.
if [[ -n "${LIBVPX_SRC:-}" ]]; then
  echo "== using existing libvpx checkout: $LIBVPX_SRC"
  ln -s "$LIBVPX_SRC" "$WORK/libvpx"
else
  echo "== cloning libvpx $LIBVPX_TAG"
  git clone --quiet --depth 1 --branch "$LIBVPX_TAG" \
    https://chromium.googlesource.com/webm/libvpx "$WORK/libvpx"
fi

echo "== configure --target=$TARGET (decode-only)"
mkdir -p "$WORK/build"
cd "$WORK/build"
# x86/x86_64: all SIMD disabled -> pure C, no nasm/yasm on any build host
# (decision: design doc §7.2 — ~35x perf headroom, x86 targets are desktops;
# per-arch SIMD configs can be added later if a real machine misses budget).
# x86-android (i686) rides along here too: it's cargokit's 4th Android ABI
# for local debug builds, same no-SIMD rationale, no dedicated decision needed.
EXTRA_FLAGS=""
if [[ "$TARGET" == x86_64-* || "$TARGET" == x86-* ]]; then
  EXTRA_FLAGS="--disable-mmx --disable-sse --disable-sse2 --disable-sse3 \
    --disable-ssse3 --disable-sse4_1 --disable-avx --disable-avx2 \
    --disable-avx512 --disable-runtime-cpu-detect"
fi

"$WORK/libvpx/configure" --target="$TARGET" \
  --disable-vp8 --disable-vp9-encoder --enable-vp9-decoder \
  --disable-examples --disable-tools --disable-docs --disable-unit-tests \
  --disable-webm-io --disable-libyuv --disable-install-bins \
  --disable-install-docs $EXTRA_FLAGS >/dev/null

echo "== make (harvesting the compiled source list from the object tree)"
make -j8 libvpx.a >/dev/null 2>&1 || make libvpx.a

# Every compiled object mirrors its source path: vp9/decoder/foo.c.o etc.
SRC_LIST="$(find . -name '*.o' | sed -e 's|^\./||' -e 's|\.o$||' | sort)"

echo "== copying sources into upstream/ + configs into configs/$CONFIG_DIR/"
mkdir -p "$CRATE_DIR/upstream" "$CRATE_DIR/configs/$CONFIG_DIR"

{
  echo "# libvpx $LIBVPX_TAG, configure --target=$TARGET (decode-only; see vendor.sh)"
  echo "$SRC_LIST"
} > "$CRATE_DIR/configs/$CONFIG_DIR/sources.txt"

for src in $SRC_LIST; do
  # vpx_config.c is per-target generated: it lives ONLY in configs/<target>/
  # (copied below); build.rs compiles it from there, never from upstream/.
  [[ "$src" == "vpx_config.c" ]] && continue
  mkdir -p "$CRATE_DIR/upstream/$(dirname "$src")"
  if [[ "$src" == *.asm.S ]]; then
    # 32-bit ARM's NEON routines are hand-written ARM-syntax .asm, converted
    # to GNU-syntax .S by libvpx's ads2gas.pl at build time. Vendor the
    # already-converted .S (it's plain assembler, no perl needed to consume
    # it) from the build tree — the .asm source was never compiled directly.
    cp "$WORK/build/$src" "$CRATE_DIR/upstream/$src"
  else
    cp "$WORK/libvpx/$src" "$CRATE_DIR/upstream/$src"
  fi
done

# Headers: the compile-time closure is hard to enumerate; copy every header
# under the directories the sources came from, plus the public API dir.
cd "$WORK/libvpx"
for dir in vpx vpx_dsp vpx_mem vpx_ports vpx_scale vpx_util vp9 common; do
  [[ -d "$dir" ]] || continue
  find "$dir" -name '*.h' | while read -r h; do
    mkdir -p "$CRATE_DIR/upstream/$(dirname "$h")"
    cp "$h" "$CRATE_DIR/upstream/$h"
  done
done

# Per-target generated configs. vpx_config.asm is only produced for 32-bit
# ARM targets (the .asm.S sources `.include` it for build-time constants).
for f in vpx_config.h vpx_config.c vpx_config.asm vpx_version.h vpx_dsp_rtcd.h vpx_scale_rtcd.h vp9_rtcd.h; do
  [[ -f "$WORK/build/$f" ]] && cp "$WORK/build/$f" "$CRATE_DIR/configs/$CONFIG_DIR/$f"
done

echo "== done: $(echo "$SRC_LIST" | wc -l | tr -d ' ') sources, configs/$CONFIG_DIR/"
