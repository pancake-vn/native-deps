# vpx-sys

Vendored [libvpx](https://github.com/webmproject/libvpx) **v1.16.0**,
decode-only subset, compiled to a static library with cc-rs, plus bindgen FFI
bindings for the VP9 decoder API. Consumed by `render_core`
(`pancake-work-client/packages/pancake_work_native_render/rust`) to decode
Telegram-style webm (VP9) video stickers — including the alpha plane carried
as a second VP9 bitstream in `BlockAdditions`.

> **Repo note:** this repository previously hosted the `rlottie-sys` crate.
> That crate was retired when `render_core`'s Lottie backend moved to ThorVG
> (vendored from crates.io); the repo was wiped and repurposed for `vpx-sys`.
> The old code is in git history before the repurpose commit.

## Design

Chromium/AOSP-style vendoring (design doc:
`pancake-work-client/docs/plans/2026-07-10-webm-sticker-support-design.md`,
§7.2 option C):

- `upstream/` — the union of every harvested target's libvpx source list.
  Never edited by hand; produced by `vendor.sh`.
- `configs/<target>/` — per-target generated headers (`vpx_config.h`,
  `*_rtcd.h`) and the exact source list (`sources.txt`) `build.rs` compiles
  for that target.
- `build.rs` — picks the config dir from `CARGO_CFG_TARGET_{ARCH,OS}`,
  compiles the listed sources with cc-rs, and bindgens `ffi.h` (the decoder
  API surface `render_core` uses).
- `src/lib.rs` — raw bindings only. Safe wrappers live in the consumer
  (`render_core`'s `renderer/webm/vp9.rs`), not here.

Build hosts (CI, the Windows box, app builds via cargokit) only compile
checked-in sources: **cc + libclang are the only requirements** — no perl,
make, configure, or nasm at app build time.

## Updating libvpx / adding a target

`vendor.sh` runs libvpx's own `configure + make` once, offline, on a dev
machine, then harvests the source subset and per-target config:

```bash
./vendor.sh                                  # default: arm64-darwin20-gcc
./vendor.sh --target x86_64-win64-vs17       # add/refresh a target config
```

Bump `LIBVPX_TAG` in `vendor.sh` to update libvpx, re-run for every dir in
`configs/`, and update `LICENSE`/`PATENTS` if upstream changed them.

## Consuming

Private repo — cargo must fetch via the git CLI so your normal GitHub auth
applies (`net.git-fetch-with-cli = true` in the consumer's `.cargo/config.toml`):

```toml
vpx-sys = { git = "https://github.com/pancake-vn/lottie-renderer.git", rev = "<sha>" }
```

Pin a `rev` and bump it together with `Cargo.lock`.

## License

The vendored libvpx sources are BSD-3-Clause (`LICENSE`) with the WebM
Project's additional IP grant (`PATENTS`), both copied verbatim from libvpx
v1.16.0. The crate's own glue (build.rs, vendor.sh) is covered by the same
license.
