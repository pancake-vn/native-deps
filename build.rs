//! Compiles the vendored rlottie C++ (plus the NEON shim where needed) into
//! a static library. COMPILE ONLY — deliberately no platform link glue here.
//!
//! Why: `cargo:rustc-link-arg` does NOT propagate from a dependency's build
//! script to the crate that links the final binary (unlike `rustc-link-lib` /
//! `rustc-link-search`, which do). This crate builds as an rlib with no link
//! step of its own, so any `rustc-link-arg` it emitted would silently vanish
//! — on Android that means the NDK sysroot's libc++_static.a / libc++abi.a
//! absolute-path links disappear and the final link fails with undefined C++
//! symbols. The consumer (`render_core` in pancake-work-client) therefore
//! owns the C++ runtime choice, `jnigraphics`, `shlwapi`, and the Android
//! sysroot links in ITS build.rs. If you add link requirements here, they
//! must be `rustc-link-lib`/`rustc-link-search` only.

use std::env;
use std::fs;
use std::path::PathBuf;

// rlottie's cmake/config.h.in configured for LOTTIE_MODULE=OFF,
// LOTTIE_THREAD=ON (default), LOTTIE_CACHE=ON (default) — the exact
// configuration the cmake-based spike built and proved on-device.
const RLOTTIE_CONFIG_H: &str = r#"/* #undef LOTTIE_MODULE */

#ifdef LOTTIE_MODULE
#define LOTTIE_IMAGE_MODULE_SUPPORT
#endif

#define LOTTIE_IMAGE_MODULE_PLUGIN "librlottie-image-loader.so"

#define LOTTIE_THREAD

#ifdef LOTTIE_THREAD
#define LOTTIE_THREAD_SUPPORT
#endif

#define LOTTIE_CACHE

#ifdef LOTTIE_CACHE
#define LOTTIE_CACHE_SUPPORT
#endif
"#;

// Source list mirrors rlottie's src/**/CMakeLists.txt with LOTTIE_MODULE=OFF
// (stb_image.cpp folded into the lib, no image-loader plugin). The arm32-only
// pixman-arm-neon-asm.S is intentionally absent: rlottie's CMake only
// assembles it when ARCH == "arm", and no target we build is 32-bit ARM.
const RLOTTIE_SOURCES: &[&str] = &[
    // src/vector
    "src/vector/vrect.cpp",
    "src/vector/vdasher.cpp",
    "src/vector/vbrush.cpp",
    "src/vector/vbitmap.cpp",
    "src/vector/vpainter.cpp",
    "src/vector/vdrawhelper_common.cpp",
    "src/vector/vdrawhelper.cpp",
    "src/vector/vdrawhelper_sse2.cpp",
    "src/vector/vdrawhelper_neon.cpp",
    "src/vector/vrle.cpp",
    "src/vector/vpath.cpp",
    "src/vector/vpathmesure.cpp",
    "src/vector/vmatrix.cpp",
    "src/vector/velapsedtimer.cpp",
    "src/vector/vdebug.cpp",
    "src/vector/vinterpolator.cpp",
    "src/vector/vbezier.cpp",
    "src/vector/vraster.cpp",
    "src/vector/vdrawable.cpp",
    "src/vector/vimageloader.cpp",
    "src/vector/varenaalloc.cpp",
    // src/vector/freetype
    "src/vector/freetype/v_ft_math.cpp",
    "src/vector/freetype/v_ft_raster.cpp",
    "src/vector/freetype/v_ft_stroker.cpp",
    // src/vector/stb (LOTTIE_MODULE=OFF -> compiled into the lib)
    "src/vector/stb/stb_image.cpp",
    // src/lottie
    "src/lottie/lottieitem.cpp",
    "src/lottie/lottieitem_capi.cpp",
    "src/lottie/lottieloader.cpp",
    "src/lottie/lottiemodel.cpp",
    "src/lottie/lottieproxymodel.cpp",
    "src/lottie/lottieparser.cpp",
    "src/lottie/lottieanimation.cpp",
    "src/lottie/lottiekeypath.cpp",
    // src/lottie/zip
    "src/lottie/zip/zip.cpp",
    // src/binding/c
    "src/binding/c/lottieanimation_capi.cpp",
];

fn main() {
    // Vendored rlottie (Samsung/rlottie, MIT — see vendor/rlottie/COPYING and
    // vendor/rlottie/licenses/). Upstream plus the local patches listed in
    // vendor/rlottie/VENDOR.md "Local patches"; never format or lint.
    let rlottie_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/rlottie");

    let target = env::var("TARGET").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // configure_file(cmake/config.h.in config.h) replacement.
    let config_dir = out_dir.join("rlottie_config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.h"), RLOTTIE_CONFIG_H).unwrap();

    let mut rl = cc::Build::new();
    rl.cpp(true)
        .std("c++14")
        .include(rlottie_dir.join("inc"))
        .include(rlottie_dir.join("src/vector"))
        .include(rlottie_dir.join("src/vector/freetype"))
        .include(rlottie_dir.join("src/vector/pixman"))
        .include(rlottie_dir.join("src/vector/stb"))
        .include(rlottie_dir.join("src/lottie"))
        .include(rlottie_dir.join("src/lottie/zip"))
        .include(rlottie_dir.join("src/binding/c"))
        .include(&config_dir)
        // Always build rlottie optimized, matching the cmake Release build the
        // prototypes proved: debug rlottie is visibly slow, and nobody debugs
        // vendor C++ through a cargo debug profile.
        .opt_level(3)
        .define("NDEBUG", None)
        .debug(false)
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-unwind-tables")
        .flag_if_supported("-fno-asynchronous-unwind-tables")
        .flag_if_supported("-fno-rtti")
        .flag_if_supported("-fvisibility=hidden")
        .warnings(false)
        // The consumer links the C++ runtime (see the header comment); keep
        // cc from emitting its own (possibly different) choice — a
        // `rustc-link-lib` from here WOULD propagate and could conflict.
        .cpp_link_stdlib(None::<&str>);
    if target.contains("apple-darwin") {
        // Match Rust's minimum so the objects link without version warnings.
        rl.flag("-mmacosx-version-min=11.0");
    }
    if target.contains("windows") {
        // On _WIN32 rlottiecommon.h expands RLOTTIE_API to
        // __declspec(dllimport) unless RLOTTIE_BUILD is set — a hard MSVC
        // error on the TUs that DEFINE those functions. Elsewhere the macro
        // stays empty (visibility handled by -fvisibility=hidden above).
        rl.define("RLOTTIE_BUILD", None);
        // Deliberately NOT defining __SSE2__ (MSVC never does): rlottie's
        // SSE2 fast path uses gcc-style vector casts —
        // `(__m128i)_mm_shuffle_ps(...)` in vdrawhelper_sse2.cpp — which are
        // hard C2440 errors under cl.exe (__m128/__m128i are distinct union
        // types there; upstream only ever compiles that file where -msse2
        // probes succeed, i.e. never MSVC). Windows takes the portable C
        // path, same as Android x86. Re-enabling needs a documented vendor
        // patch to _mm_castsi128_ps/_mm_castps_si128 first.
    }
    for src in RLOTTIE_SOURCES {
        rl.file(rlottie_dir.join(src));
    }
    // compile() also emits rustc-link-lib=static=rlottie + the OUT_DIR
    // link-search; both propagate through the rlib to the final link.
    rl.compile("rlottie");

    // Apple arm64 clang and NDK armv7 clang define the legacy __ARM_NEON__
    // macro, so rlottie's vdrawhelper_neon.cpp references two aarch32 pixman
    // asm routines it never assembles under these toolchains (confirmed:
    // undefined symbols on iOS/macOS arm64; the prototype hit the same on
    // 32-bit Android). Provide them as portable C. Android arm64 is NOT
    // affected (NDK aarch64 clang doesn't define the macro).
    if target.starts_with("aarch64-apple") || target.starts_with("armv7-linux-android") {
        let mut shim = cc::Build::new();
        shim.file("csrc/rlottie_neon_shim.c").opt_level(3).debug(false);
        if target.contains("apple-darwin") {
            shim.flag("-mmacosx-version-min=11.0");
        }
        shim.compile("rlottie_neon_shim");
    }

    // DEP_RLOTTIE_INCLUDE for a consumer that ever compiles C against the
    // rlottie C API (none does today — render_core hand-writes externs).
    println!("cargo:include={}", rlottie_dir.join("inc").display());

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc/rlottie_neon_shim.c");
    println!("cargo:rerun-if-changed={}", rlottie_dir.join("src").display());
}
