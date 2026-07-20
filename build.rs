//! Compile the vendored libvpx decode-only subset (harvested by `vendor.sh`)
//! and bindgen the decoder API. Per-target file lists + generated configs
//! live in `configs/<target>/`; adding a platform = re-running vendor.sh for
//! it (Phase 5 of the implementation plan), never touching build hosts.

use std::env;
use std::path::{Path, PathBuf};

fn config_dir_for_target() -> &'static str {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    match (arch.as_str(), os.as_str()) {
        ("aarch64", "macos") => "arm64-darwin",
        ("aarch64", "ios") => "arm64-ios",
        ("aarch64", "android") => "arm64-android",
        ("aarch64", "linux") => "arm64-linux",
        ("aarch64", "windows") => "arm64-win64",
        ("arm", "android") => "armv7-android",
        ("x86_64", "android") => "x86_64-android",
        ("x86", "android") => "x86-android",
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

    let mut builder = bindgen::Builder::default()
        .header(crate_dir.join("ffi.h").to_string_lossy())
        .clang_arg(format!("-I{}", upstream.display()))
        .clang_arg(format!("-I{}", config.display()))
        .allowlist_function("vpx_.*")
        .allowlist_type("vpx_.*")
        .allowlist_var("VPX_.*|vpx_.*")
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        });

    // Rust's iOS-simulator target ("aarch64-apple-ios-sim") uses a "-sim"
    // shorthand that only rustc's own codegen backend understands — it's
    // not valid LLVM triple syntax. libclang (which bindgen drives
    // directly, not through rustc) rejects it: "version 'sim' in target
    // triple ... is invalid". Spell out the triple + sysroot libclang
    // actually accepts instead. Only "aarch64" ships a simulator config
    // (see config_dir_for_target), so no arch branch is needed here.
    if env::var("TARGET").unwrap().ends_with("-ios-sim") {
        let min_ios = env::var("IPHONEOS_DEPLOYMENT_TARGET").unwrap_or_else(|_| "15.0".into());
        let sdk_path = std::process::Command::new("xcrun")
            .args(["--sdk", "iphonesimulator", "--show-sdk-path"])
            .output()
            .expect("xcrun --sdk iphonesimulator --show-sdk-path");
        let sdk_path = String::from_utf8(sdk_path.stdout).unwrap();
        builder = builder
            .clang_arg(format!("--target=arm64-apple-ios{min_ios}-simulator"))
            .clang_arg(format!("-isysroot{}", sdk_path.trim()));
    }

    // Android needs the same treatment for a different reason. bindgen drives
    // libclang in-process, so unlike the cc-rs passes above it never sees
    // CC_*/CFLAGS_* — left alone it parses ffi.h against the *host* sysroot.
    // On a macOS build host that fails outright (clang's own inttypes.h
    // #include_next's a libc one that isn't there), and anywhere it did
    // resolve it would be worse: host headers mint bindings whose layout
    // disagrees with the bionic-compiled libvpx.a produced above. Point it at
    // the NDK sysroot instead, derived from the compiler cargokit handed us so
    // that the NDK root, its version and the host tag are never hardcoded —
    // they differ across dev machines and CI runners.
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "android" {
        let target = env::var("TARGET").unwrap();
        // cc-rs-style vars are looked up hyphenated first, then underscored.
        let lookup = |prefix: &str| {
            env::var(format!("{prefix}_{target}"))
                .or_else(|_| env::var(format!("{prefix}_{}", target.replace('-', "_"))))
        };
        let cc = lookup("CC").expect(
            "CC_<target> — set by cargokit (android_environment.dart) for Android builds",
        );
        // <ndk>/toolchains/llvm/prebuilt/<host>/bin/clang
        //   -> <ndk>/toolchains/llvm/prebuilt/<host>/sysroot
        let sysroot = Path::new(&cc)
            .parent()
            .and_then(Path::parent)
            .expect("CC_<target> should point at <toolchain>/bin/clang")
            .join("sysroot");
        // CFLAGS_<target> is "--target=<triple><api>". Reuse it verbatim so the
        // API level and triple spelling match what libvpx.a was compiled with,
        // rather than re-deriving them and risking a mismatch.
        let target_arg = lookup("CFLAGS").unwrap_or_else(|_| format!("--target={target}"));
        builder = builder
            .clang_args(target_arg.split_whitespace())
            .clang_arg(format!("--sysroot={}", sysroot.display()));
    }

    let bindings = builder.generate().expect(
        "bindgen over vpx decoder headers (build hosts need libclang, \
             same as thorvg-sys)",
    );
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out.join("bindings.rs")).unwrap();
}
