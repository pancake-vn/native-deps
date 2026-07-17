//! Raw FFI bindings to the vendored libvpx VP9 decoder (see `vendor.sh` for
//! how `upstream/` + `configs/` are produced). Safe wrappers live in
//! `render_core`'s `renderer/webm/vp9.rs`, not here.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
