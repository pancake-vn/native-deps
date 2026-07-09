// rlottie references two 32-bit-ARM pixman NEON asm routines from
// vdrawhelper_neon.cpp. On Apple arm64 clang still defines __ARM_NEON__, so that
// file compiles and needs these symbols, but rlottie only assembles the (aarch32)
// .S for ARCH==arm -> they're undefined on iOS arm64. Provide portable C versions
// (rlottie is geometry-bound, so no meaningful perf loss). Semantics match
// rlottie's own scalar color fills (premultiplied ARGB).
#include <stdint.h>

static inline uint32_t byte_mul(uint32_t x, uint32_t a) {
    uint32_t t = (x & 0x00ff00ffu) * a;
    t = (t + ((t >> 8) & 0x00ff00ffu) + 0x00800080u) >> 8;
    t &= 0x00ff00ffu;
    x = ((x >> 8) & 0x00ff00ffu) * a;
    x = (x + ((x >> 8) & 0x00ff00ffu) + 0x00800080u);
    x &= 0xff00ff00u;
    return x | t;
}

// SRC: fill w*h pixels with the solid (premultiplied) color `src`.
void pixman_composite_src_n_8888_asm_neon(int32_t w, int32_t h, uint32_t *dst,
                                          int32_t dst_stride, uint32_t src) {
    for (int32_t y = 0; y < h; ++y) {
        uint32_t *row = dst + (int64_t)y * dst_stride;
        for (int32_t x = 0; x < w; ++x) row[x] = src;
    }
}

// OVER: composite solid premultiplied color `src` onto dst: dst = src + dst*(1-a).
void pixman_composite_over_n_8888_asm_neon(int32_t w, int32_t h, uint32_t *dst,
                                           int32_t dst_stride, uint32_t src) {
    uint32_t ia = 255u - (src >> 24);
    for (int32_t y = 0; y < h; ++y) {
        uint32_t *row = dst + (int64_t)y * dst_stride;
        for (int32_t x = 0; x < w; ++x) row[x] = src + byte_mul(row[x], ia);
    }
}
