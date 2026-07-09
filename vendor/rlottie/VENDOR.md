# Vendored rlottie

Source: https://github.com/Samsung/rlottie (the tree the 2026-07 prototype
vendored and proved on macOS/iOS/Android — see
`docs/plans/2026-07-01-rlottie-ffi-android-prototype-design.md` in
pancake-work-client, where this tree lived at
`packages/pancake_work_native_render/rust/vendor/rlottie` until it was
extracted here — pancake-work-client @ `14b764345e71`).

- Upstream tree + the local patches listed below. Never format, lint, or
  hand-edit these files beyond those patches; every divergence from upstream
  MUST be listed in "Local patches" and marked `LOCAL PATCH(pancake-work)` in
  the source.
- Only `inc/`, `src/`, `licenses/`, `COPYING`, `AUTHORS`, `README.md` are
  vendored; upstream build files (CMake/meson), examples and tests are not —
  the crate's `build.rs` compiles the sources directly with the `cc` crate.
- Licensing: MIT top-level with per-component licenses in `licenses/`
  (FreeType's FTL requires attribution in distributed docs). Flagged for
  legal review.

## Local patches

Both patches fix constructs the Noto animated emoji set uses heavily; before
them 59 of the 881 bundled emoji had runs of blank frames (blank picker loops
and blank frozen reaction chips). Locked in by `rlottie_conformance_tests` in
the CONSUMING crate — `render_core` in pancake-work-client
(`packages/pancake_work_native_render/rust/src/lib.rs`, fixtures in
`rust/testdata/`) — so an upstream refresh here that silently drops a patch
fails the consumer's `cargo test` — 2026-07-09.

1. **Animated time remap on a stretched layer** —
   `src/lottie/lottiemodel.h`, `Layer::timeRemap()`.
   Upstream divided the time-remapped frame by `mTimeStreatch` (`sr`) a
   second time; AE bakes the stretch into the remap keyframe placement and
   lottie-web divides by `sr` only when the layer has no `tm`
   (`CompElement.prepareFrame`). A stretched (`sr != 1`) remapped precomp
   overshot its source timeline and rendered blank (e.g. `1f601` beaming
   face, blank frames 124–163).

2. **Matte source outside its ip/op window** —
   `src/lottie/lottieitem.cpp`, `CompLayer::renderHelper()` +
   `CompLayer::preprocessStage()`.
   Upstream dropped the matted layer whenever the matte-source layer was
   outside its own in/out window. An INVERTED (`AlphaInv`/`LumaInv`) matte
   over an absent source selects everything, so the matted layer now renders
   unmatted in that case (AE/lottie-web parity; e.g. `1f31a` new moon face,
   blank frames 0–91). Straight `Alpha`/`Luma` mattes keep the upstream skip:
   an empty matte selects nothing.
