//! Build-only `-sys` crate: compiles the vendored, patched Samsung/rlottie
//! C++ (provenance + local-patch ledger in `vendor/rlottie/VENDOR.md`) into
//! a static library that links into whatever depends on this crate.
//!
//! No Rust bindings live here — the consumer (`render_core` in
//! pancake-work-client) declares its own `extern "C"` signatures against
//! rlottie's C API and owns ALL platform link glue. That split is load-
//! bearing, not stylistic: `cargo:rustc-link-arg` does not propagate from a
//! dependency's build script (see README.md "Why no link glue here").
//!
//! Consumers must reference this crate (`use rlottie_sys as _;`) or rustc
//! drops the unused rlib — and the static lib with it — from the final link.
