# lottie-renderer — `rlottie-sys`

A **patched fork** of [Samsung/rlottie](https://github.com/Samsung/rlottie)
vendored at `vendor/rlottie/`, compiled to a static library by the
`rlottie-sys` Rust crate (via the `cc` crate — no CMake). Build-only: the
crate exports **no Rust bindings**; consumers hand-write their `extern "C"`
signatures against rlottie's C API.

Extracted from `pancake-vn/pancake-work-client` @ `14b764345e71`
(`packages/pancake_work_native_render/rust/`), where it is consumed by the
`render_core` crate as a Cargo git dependency pinned by `rev` in `Cargo.lock`.
Extraction rationale + full analysis:
`docs/plans/2026-07-06-rlottie-sys-extraction-plan.md` in that repo.

## Consuming

```toml
[dependencies]
rlottie-sys = { git = "https://github.com/pancake-vn/lottie-renderer.git", rev = "<sha>" }
```

```rust
// The crate must be referenced or rustc drops the unused rlib — and the
// static lib with it — from the final link.
use rlottie_sys as _;
```

This repo is private: put `git-fetch-with-cli = true` under `[net]` in a
`.cargo/config.toml` so cargo shells out to `git` and reuses your normal
GitHub auth (ssh `insteadOf` rewrite or https credential helper). CI mints a
GitHub App installation token and rewrites `https://github.com/` to inject it
(same pattern as pancake-work-changelog).

## Why no link glue here (READ before touching build.rs)

`cargo:rustc-link-arg` does **not** propagate from a dependency's build
script — only `rustc-link-lib`/`rustc-link-search` do. This crate is an rlib
with no link step, so a `rustc-link-arg` emitted here silently vanishes; on
Android that would drop the NDK sysroot's `libc++_static.a`/`libc++abi.a`
absolute-path links and fail the final link with undefined C++ symbols.

**All platform link glue lives in the consumer's `build.rs`** (`render_core`):
the C++ runtime choice per platform, `jnigraphics`, `shlwapi`, and the Android
sysroot absolute-path links. This crate's `build.rs` only compiles.

## Local patches (ledger)

The vendored tree is upstream **plus** the patches below — every divergence
is marked `LOCAL PATCH(pancake-work)` in the source and MUST be listed here
and in `vendor/rlottie/VENDOR.md` (the authoritative ledger). Both patches
fix constructs the Noto animated emoji set uses heavily; before them 59 of
the 881 bundled emoji had runs of blank frames.

1. **Animated time remap on a stretched layer** —
   `src/lottie/lottiemodel.h`, `Layer::timeRemap()`.
   Upstream divided the time-remapped frame by the layer's time stretch
   (`sr`) a second time; AE bakes the stretch into the remap keyframes and
   lottie-web divides by `sr` only when the layer has no `tm`. A stretched
   remapped precomp overshot its source timeline and rendered blank
   (e.g. `1f601` beaming face, blank frames 124–163).

2. **Matte source outside its ip/op window** —
   `src/lottie/lottieitem.cpp`, `CompLayer::renderHelper()` +
   `CompLayer::preprocessStage()`.
   Upstream dropped the matted layer whenever the matte-source layer was
   outside its own in/out window. An INVERTED (`AlphaInv`/`LumaInv`) matte
   over an absent source selects everything, so the matted layer now renders
   unmatted in that case (AE/lottie-web parity; e.g. `1f31a` new moon face,
   blank frames 0–91). Straight `Alpha`/`Luma` mattes keep the upstream skip.

The patches are locked in by `rlottie_conformance_tests` in the consumer
(`render_core`'s `src/lib.rs`, fixtures in `rust/testdata/`) — a refresh here
that silently drops a patch fails the consumer's `cargo test`.

## Refreshing from upstream

Upstream is effectively unmaintained, so treat this fork as permanent. If you
do pull upstream changes: re-apply every patch in the ledger, keep the
`LOCAL PATCH(pancake-work)` markers, update `VENDOR.md` + this README, then
run the consumer's `cargo test` (conformance tests) before bumping the `rev`
in pancake-work-client. When adding NEW patches, check
[Telegram's rlottie fork](https://github.com/TelegramMessenger/rlottie)
first — they fixed many similar cases.

## Licensing

MIT top-level with per-component licenses (Skia BSD, FreeType FTL, pixman
MIT, stb MIT/public-domain, rapidjson MIT, MPL for `vinterpolator.cpp`) —
all texts aggregated in `LICENSE`, per-directory mapping in
`vendor/rlottie/COPYING`. FreeType's FTL requires attribution in distributed
docs. The consuming app's binary bundles rlottie, so the consumer keeps its
own copy of the aggregated license texts for distribution; auditors review
this repo at the pinned `rev`.
