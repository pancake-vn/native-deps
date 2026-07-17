//! Compile the vendored libvpx decode-only subset (harvested by `vendor.sh`)
//! and bindgen the decoder API. Per-target file lists + generated configs
//! live in `configs/<target>/`; adding a platform = re-running vendor.sh for
//! it (Phase 5 of the implementation plan), never touching build hosts.

use std::env;
use std::path::PathBuf;

fn config_dir_for_target() -> &'static str {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    match (arch.as_str(), os.as_str()) {
        ("aarch64", "macos") => "arm64-darwin",
        ("aarch64", "ios") => "arm64-ios",
        ("aarch64", "android") => "arm64-android",
        ("aarch64", "linux") => "arm64-linux",
        ("aarch64", "windows") => "arm64-win64",
        ("x86_64", "macos") => "x86_64-darwin",
        ("x86_64", "linux") => "x86_64-linux",
        ("x86_64", "windows") => "x86_64-win64",
        (a, o) => panic!(
            "vpx-sys: no vendored libvpx config for {a}-{o}. \
             Run vendor.sh --target <libvpx target> and add the mapping here."
        ),
    }
}

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let upstream = crate_dir.join("upstream");
    let config = crate_dir.join("configs").join(config_dir_for_target());
    println!("cargo:rerun-if-changed=upstream");
    println!("cargo:rerun-if-changed=configs");
    println!("cargo:rerun-if-changed=ffi.h");

    let sources: Vec<String> = std::fs::read_to_string(config.join("sources.txt"))
        .expect("configs/<target>/sources.txt — produced by vendor.sh")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();

    // libvpx's own make rules compile the dotprod/i8mm NEON-extension files
    // with per-object `-march` flags (they're gated behind runtime CPU
    // detection, so the baseline stays plain armv8). Mirror that here with
    // one cc pass per flag set — cc-rs has no per-file flags.
    let flavor = |src: &str| {
        if src.ends_with("_neon_i8mm.c") {
            2
        } else if src.ends_with("_neon_dotprod.c") {
            1
        } else {
            0
        }
    };
    let arch_flags = [
        "",
        "-march=armv8.2-a+dotprod",
        "-march=armv8.2-a+dotprod+i8mm",
    ];
    let lib_names = ["vpx", "vpx_neon_dotprod", "vpx_neon_i8mm"];
    for f in 0..3 {
        let files: Vec<_> = sources.iter().filter(|s| flavor(s) == f).collect();
        if files.is_empty() {
            continue;
        }
        let mut build = cc::Build::new();
        build.include(&config).include(&upstream);
        if !arch_flags[f].is_empty() {
            build.flag(arch_flags[f]);
        }
        for src in files {
            // vpx_config.c is per-target generated — compile the config
            // dir's copy, not an upstream one.
            if src == "vpx_config.c" {
                build.file(config.join(src));
            } else {
                build.file(upstream.join(src));
            }
        }
        build.warnings(false).compile(lib_names[f]);
    }

    let bindings = bindgen::Builder::default()
        .header(crate_dir.join("ffi.h").to_string_lossy())
        .clang_arg(format!("-I{}", upstream.display()))
        .clang_arg(format!("-I{}", config.display()))
        .allowlist_function("vpx_.*")
        .allowlist_type("vpx_.*")
        .allowlist_var("VPX_.*|vpx_.*")
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        })
        .generate()
        .expect(
            "bindgen over vpx decoder headers (build hosts need libclang, \
                 same as thorvg-sys)",
        );
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out.join("bindings.rs")).unwrap();
}
